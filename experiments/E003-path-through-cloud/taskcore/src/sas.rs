//! Fast Downward's SAS file from the task, written streaming: task.py's `write_sas` and `_expand`, rule for rule,
//! so that with no reduction the file is byte for byte the one Python writes (the format check).
//!
//! The file names its variables in its head (only the keys the operators, their conditions and the goal use, and
//! the pick variables), and the operator count before the operators, so the writer goes over the actions twice:
//! pass 1 expands every action into its variants and keeps only what the head needs (which keys are used, the
//! pick variables, the counts, the metric); pass 2 expands them again and writes each operator as it comes. Memory
//! is one action's variants at a time; nothing of the file is kept.
//!
//! The rules (task.py, module docstring and `_expand`): a precondition set that holds for every value is left out;
//! an action with an empty one is dropped; a condition set that holds for every value is left out. An action's
//! variants are the product of its precondition sets (keys in key order) and of its guessed values; past
//! `limit` (task.PRODUCT_LIMIT) the guesses go through pick variables instead. In each variant a condition on a
//! precondition key is decided by the chosen value; a condition set becomes one conditional effect per value; a
//! write of the value the key already has (by its own condition, or by the precondition) is left out; duplicate
//! effects are written once; a variant with no effect left is dropped; two effects on one key that can both
//! happen with different values raise. Then the pick operators (one per pick variable and value, cost 0) and the
//! goal operators (a goal of a set of values: the extra variable `goal`, one operator per choice, cost 0).
//!
//! Two encodings. `Task` is task.py's, byte for byte. `Model` (the default) changes two things so that the file's
//! optimal cost is the task's (the sum of the plan's actions' costs), whatever the reductions did:
//!
//! * the metric is 1 whenever the file has a pick or goal operator. With metric 0 Fast Downward gives every
//!   operator cost 1 (root_task.cc:469), pick and goal operators too, so a plan's cost would count its picks; the
//!   reductions and the fold change which actions pass PRODUCT_LIMIT, so they would change that count;
//! * a goal of a set of values is not a latch: every effect that writes a goal-set fact a value outside its set
//!   also sets `goal` to 0, under the same conditions. In task.py's encoding nothing sets `goal` back, so a plan
//!   can meet the set, fire the goal operator and leave the set. Here `goal` is 1 at the end only if the goal
//!   operator fired after the last write that left a set, so every set holds at the end.
//!
//! The operators, their order and their numbers are the same in both encodings (a reset is one more effect of an
//! operator that already writes), so a plan reads back the same way.

use crate::model::*;
use std::io::Write;

pub const PRE_LIMIT: u128 = 4096;

#[derive(Default, Debug, Clone)]
pub struct SasCounts {
    pub actions: u64,
    pub operators: u64,
    pub action_ops: u64,
    pub pick_ops: u64,
    pub goal_ops: u64,
    pub effects: u64,
    pub cond_effects: u64,
    pub variables: u64,
    pub values: u64,
    pub pick_vars: u64,
    pub variants: u64,
    pub max_variants: u64,
    pub dead_actions: u64,
    pub empty_variants: u64,
    pub pick_actions: u64,
    pub noop_effects: u64,
    pub goal_resets: u64,
    pub bytes: u64,
}

pub struct Variant<'b> {
    pub chosen: &'b [(u32, u32)],
    pub guessed: &'b [(u32, u32)],
    pub effs: &'b [u32],
    pub ranges: &'b [(usize, usize)],
    pub cost: u32,
    // per guessed write, its live conditions (task.py's `Op.guessed` when): a range of `conds`, each a (fact,
    // offset, length) of `arena`; read only by `write_ops`
    pub guess_conds: &'b [(usize, usize)],
    pub conds: &'b [(u32, usize, usize)],
    pub arena: &'b [u32],
}

#[derive(Clone, Copy)]
struct XEff {
    ws: usize,
    we: usize,
    key: u32,
    guess: bool,
    val: u32,
    gstart: usize,
    glen: usize,
}

/// An action's guesses written through pick variables: (key, its values, its conditions as (fact, values)).
pub type Picked = Vec<(u32, Vec<u32>, Vec<(u32, Vec<u32>)>)>;

pub struct Expander<'t> {
    pub t: &'t Task,
    pub nf: u32,
    pub rank: Vec<u32>,
    pub limit: u128,
    fixed: Vec<u32>,
    pub picks: Vec<bool>,
    pub c: SasCounts,
    pub check: bool,
    pub model: bool,         // the model encoding (module docstring); false: task.py's
    goal_set: Vec<Vec<u32>>, // per fact its goal set when the goal is a set of values, else empty
}

fn advance(idx: &mut [usize], len: impl Fn(usize) -> usize) -> bool {
    for j in (0..idx.len()).rev() {
        idx[j] += 1;
        if idx[j] < len(j) {
            return true;
        }
        idx[j] = 0;
    }
    false
}

fn cond_last(e: &[u32], key: u32) -> Option<u32> {
    let nc = e[2] as usize;
    for j in (0..nc).rev() {
        if e[3 + 2 * j] == key {
            return Some(e[4 + 2 * j]);
        }
    }
    None
}

/// task.py `_check`: two effects on one key that can both happen must write the same value.
fn compatible(a: &[u32], b: &[u32]) -> bool {
    let na = a[2] as usize;
    for j in 0..na {
        let k = a[3 + 2 * j];
        // dict semantics: the last entry of a key wins; check each key once, at its last entry
        if (j + 1..na).any(|l| a[3 + 2 * l] == k) {
            continue;
        }
        if let Some(vb) = cond_last(b, k) {
            if vb != a[4 + 2 * j] {
                return false;
            }
        }
    }
    true
}

impl<'t> Expander<'t> {
    pub fn new(t: &'t Task, limit: u128) -> Self {
        let nf = t.facts.len() as u32;
        let mut order: Vec<u32> = (0..nf).collect();
        order.sort_by(|&a, &b| t.key(a).cmp(t.key(b)));
        let mut rank = vec![0u32; nf as usize];
        for (r, &f) in order.iter().enumerate() {
            rank[f as usize] = r as u32;
        }
        let mut goal_set = vec![Vec::new(); nf as usize];
        for (f, vals) in &t.goal {
            if vals.len() > 1 {
                goal_set[*f as usize] = vals.clone();
            }
        }
        Expander {
            t,
            nf,
            rank,
            limit,
            fixed: vec![NONE; nf as usize],
            picks: vec![false; nf as usize],
            c: SasCounts::default(),
            check: true,
            model: false,
            goal_set,
        }
    }

    /// The variants of action `ai`, each given to `cb` (module docstring). Returns the action's picked guesses.
    pub fn expand(&mut self, ai: usize, cb: &mut dyn FnMut(&Variant) -> Result<(), String>) -> Result<Picked, String> {
        let t = self.t;
        let nf = self.nf;
        let v = t.acts.view(ai);
        let cost = t.acts.meta[ai].cost;
        let size = |f: u32| t.facts.size(f);
        let mut arena: Vec<u32> = Vec::new();
        let mut pre: Vec<(u32, usize, usize)> = Vec::new();
        for (f, s) in v.pre() {
            if s.is_empty() {
                self.c.dead_actions += 1;
                return Ok(Vec::new());
            }
            if s.len() as u32 == size(f) {
                continue;
            }
            pre.push((f, arena.len(), s.len()));
            arena.extend_from_slice(s);
        }
        let mut wl: Vec<(u32, usize, usize)> = Vec::new();
        let mut xs: Vec<XEff> = Vec::new();
        for e in v.effs() {
            if e.kind == K_BAD {
                let val = if e.vals[0] == NONE { "a step past the domain".to_string() } else { t.strs.get(e.vals[0]).to_string() };
                return Err(format!("{}: {} := {} is not in its domain", t.name(ai), t.key(e.fact), val));
            }
            let ws = wl.len();
            for (cf, cs) in e.when() {
                if cs.len() as u32 == size(cf) {
                    continue;
                }
                wl.push((cf, arena.len(), cs.len()));
                arena.extend_from_slice(cs);
            }
            let we = wl.len();
            if e.kind == K_SET {
                xs.push(XEff { ws, we, key: e.fact, guess: false, val: e.vals[0], gstart: 0, glen: 0 });
            } else {
                xs.push(XEff { ws, we, key: e.fact, guess: true, val: NONE, gstart: arena.len(), glen: e.vals.len() });
                arena.extend_from_slice(e.vals);
            }
        }
        let mut pre_product: u128 = 1;
        for p in &pre {
            pre_product = pre_product.saturating_mul(p.2 as u128);
        }
        if pre_product > PRE_LIMIT {
            return Err(format!("{}: {} variants from precondition sets alone (PRE_LIMIT {})", t.name(ai), pre_product, PRE_LIMIT));
        }
        let mut guess_product: u128 = 1;
        for x in &xs {
            if x.guess {
                guess_product = guess_product.saturating_mul(x.glen as u128);
            }
        }
        let mut picked: Picked = Vec::new();
        if pre_product.saturating_mul(guess_product) > self.limit {
            self.c.pick_actions += 1;
            let mut ys = Vec::with_capacity(xs.len());
            for x in xs {
                if !x.guess {
                    ys.push(x);
                    continue;
                }
                self.picks[x.key as usize] = true;
                let g: Vec<u32> = arena[x.gstart..x.gstart + x.glen].to_vec();
                let when: Vec<(u32, Vec<u32>)> =
                    wl[x.ws..x.we].iter().map(|&(cf, off, len)| (cf, arena[off..off + len].to_vec())).collect();
                picked.push((x.key, g.clone(), when));
                for u in 0..size(x.key) {
                    let ws = wl.len();
                    for j in x.ws..x.we {
                        let e = wl[j];
                        wl.push(e);
                    }
                    wl.push((nf + x.key, arena.len(), 1));
                    arena.push(u);
                    let val = if g.contains(&u) { u } else { g[0] };
                    ys.push(XEff { ws, we: wl.len(), key: x.key, guess: false, val, gstart: 0, glen: 0 });
                }
            }
            xs = ys;
        }
        pre.sort_by_key(|p| self.rank[p.0 as usize]);
        let np = pre.len();
        let mut pidx = vec![0usize; np];
        let mut chosen: Vec<(u32, u32)> = Vec::with_capacity(np);
        let mut live: Vec<(usize, usize, usize)> = Vec::new();
        let mut lw: Vec<(u32, usize, usize)> = Vec::new();
        let mut guesses: Vec<usize> = Vec::new();
        let mut guessed: Vec<(u32, u32)> = Vec::new();
        let mut gconds: Vec<(usize, usize)> = Vec::new();
        let mut effbuf: Vec<u32> = Vec::new();
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        let mut kept: Vec<(usize, usize)> = Vec::new();
        let mut cidx: Vec<usize> = Vec::new();
        let mut nvariants = 0u64;
        let mut result = Ok(());
        'pre: loop {
            chosen.clear();
            for j in 0..np {
                let (f, off, _) = pre[j];
                let val = arena[off + pidx[j]];
                self.fixed[f as usize] = val;
                chosen.push((f, val));
            }
            live.clear();
            lw.clear();
            'eff: for (xi, x) in xs.iter().enumerate() {
                let ls = lw.len();
                for j in x.ws..x.we {
                    let (cf, off, len) = wl[j];
                    if cf < nf && self.fixed[cf as usize] != NONE {
                        if !contains(&arena[off..off + len], self.fixed[cf as usize]) {
                            lw.truncate(ls);
                            continue 'eff;
                        }
                    } else {
                        lw.push((cf, off, len));
                    }
                }
                live.push((xi, ls, lw.len()));
            }
            guesses.clear();
            for (li, &(xi, _, _)) in live.iter().enumerate() {
                if xs[xi].guess {
                    guesses.push(li);
                }
            }
            let mut gidx = vec![0usize; guesses.len()];
            loop {
                nvariants += 1;
                effbuf.clear();
                ranges.clear();
                guessed.clear();
                gconds.clear();
                let mut gi = 0;
                for (li, &(xi, ls, le)) in live.iter().enumerate() {
                    let x = &xs[xi];
                    let value = if x.guess {
                        while guesses[gi] != li {
                            gi += 1;
                        }
                        let val = arena[x.gstart + gidx[gi]];
                        guessed.push((x.key, val));
                        gconds.push((ls, le));
                        val
                    } else {
                        x.val
                    };
                    let nc = le - ls;
                    if (ls..le).any(|j| lw[j].2 == 0) {
                        continue;
                    }
                    cidx.clear();
                    cidx.resize(nc, 0);
                    loop {
                        let mut own = if x.key < nf { self.fixed[x.key as usize] } else { NONE };
                        for j in (0..nc).rev() {
                            if lw[ls + j].0 == x.key {
                                own = arena[lw[ls + j].1 + cidx[j]];
                                break;
                            }
                        }
                        if own == value {
                            self.c.noop_effects += 1;
                        } else {
                            let st = effbuf.len();
                            effbuf.push(x.key);
                            effbuf.push(value);
                            effbuf.push(nc as u32);
                            for j in 0..nc {
                                effbuf.push(lw[ls + j].0);
                                effbuf.push(arena[lw[ls + j].1 + cidx[j]]);
                            }
                            ranges.push((st, effbuf.len()));
                        }
                        if !advance(&mut cidx, |j| lw[ls + j].2) {
                            break;
                        }
                    }
                }
                // the model encoding: a write that leaves a goal set resets `goal` (module docstring)
                if self.model {
                    for r in 0..ranges.len() {
                        let (st0, en0) = ranges[r];
                        let key = effbuf[st0];
                        if key < nf && !self.goal_set[key as usize].is_empty() && !contains(&self.goal_set[key as usize], effbuf[st0 + 1]) {
                            let st = effbuf.len();
                            effbuf.push(2 * nf);
                            effbuf.push(0);
                            effbuf.extend_from_within(st0 + 2..en0);
                            ranges.push((st, effbuf.len()));
                        }
                    }
                }
                // duplicates written once (dict.fromkeys)
                kept.clear();
                if ranges.len() <= 48 {
                    for &r in &ranges {
                        let s = &effbuf[r.0..r.1];
                        if !kept.iter().any(|k| &effbuf[k.0..k.1] == s) {
                            kept.push(r);
                        }
                    }
                } else {
                    let mut seen: FxMap<&[u32], ()> = FxMap::default();
                    seen.reserve(ranges.len());
                    for &r in &ranges {
                        if seen.insert(&effbuf[r.0..r.1], ()).is_none() {
                            kept.push(r);
                        }
                    }
                }
                if kept.is_empty() {
                    self.c.empty_variants += 1;
                } else {
                    if self.check {
                        if let Err(e) = check_effects(&effbuf, &kept) {
                            result = Err(format!("{}: {}", t.name(ai), e));
                            break 'pre;
                        }
                    }
                    let var = Variant {
                        chosen: &chosen,
                        guessed: &guessed,
                        effs: &effbuf,
                        ranges: &kept,
                        cost,
                        guess_conds: &gconds,
                        conds: &lw,
                        arena: &arena,
                    };
                    if let Err(e) = cb(&var) {
                        result = Err(e);
                        break 'pre;
                    }
                }
                let lens: Vec<usize> = guesses.iter().map(|&li| xs[live[li].0].glen).collect();
                if !advance(&mut gidx, |j| lens[j]) {
                    break;
                }
            }
            if !advance(&mut pidx, |j| pre[j].2) {
                break;
            }
        }
        for p in &pre {
            self.fixed[p.0 as usize] = NONE;
        }
        result?;
        self.c.variants += nvariants;
        self.c.max_variants = self.c.max_variants.max(nvariants);
        Ok(picked)
    }

    pub fn var_key(&self, var: u32) -> String {
        if var < self.nf {
            self.t.key(var).to_string()
        } else if var < 2 * self.nf {
            format!("pick:{}", self.t.key(var - self.nf))
        } else {
            "goal".to_string()
        }
    }

    pub fn var_size(&self, var: u32) -> u32 {
        if var < 2 * self.nf {
            self.t.facts.size(var % self.nf.max(1))
        } else {
            2
        }
    }

    pub fn value_name(&self, var: u32, val: u32) -> String {
        if var < 2 * self.nf {
            let f = if var < self.nf { var } else { var - self.nf };
            self.t.strs.get(self.t.facts.dom(f)[val as usize]).to_string()
        } else {
            val.to_string()
        }
    }
}

fn check_effects(effbuf: &[u32], kept: &[(usize, usize)]) -> Result<(), String> {
    let mut by: Vec<(u32, usize)> = kept.iter().enumerate().map(|(i, r)| (effbuf[r.0], i)).collect();
    by.sort_unstable();
    let mut i = 0;
    while i < by.len() {
        let mut j = i;
        while j < by.len() && by[j].0 == by[i].0 {
            j += 1;
        }
        if j - i > 1 {
            let v0 = effbuf[kept[by[i].1].0 + 1];
            if by[i..j].iter().any(|&(_, k)| effbuf[kept[k].0 + 1] != v0) {
                for a in i..j {
                    for b in a + 1..j {
                        let ea = &effbuf[kept[by[a].1].0..kept[by[a].1].1];
                        let eb = &effbuf[kept[by[b].1].0..kept[by[b].1].1];
                        if ea[1] != eb[1] && compatible(ea, eb) {
                            return Err(format!("two effects on fact {} can both happen", ea[0]));
                        }
                    }
                }
            }
        }
        i = j;
    }
    Ok(())
}

/// The facts out of their domains at the start (`start[f]` NONE) that the file of `t` would name: task.py raises
/// OutOfDomain ("build the task again with that value") exactly for these (`SasTask.state` checks the file's
/// variables: the goal's keys and every key a variant's precondition, effect or condition names). Run on the input
/// task, before the fold and the reductions: they can make a fact the input reads unread (a fold whose paths cover
/// the fact's domain, (b), (d)), and the answer must not depend on them. Only the actions that name such a fact are
/// expanded. Errors of the expansion (task.py's write_sas raises on them too) are returned.
pub fn start_misses(t: &Task, start: &[u32], limit: u128) -> Result<Vec<String>, String> {
    let nf = t.facts.len();
    let none: Vec<bool> = (0..nf).map(|f| start[f] == NONE).collect();
    if !none.iter().any(|&b| b) {
        return Ok(Vec::new());
    }
    let mut used = vec![false; nf];
    for (f, _) in &t.goal {
        used[*f as usize] = true;
    }
    let mut x = Expander::new(t, limit);
    for ai in 0..t.acts.len() {
        let v = t.acts.view(ai);
        let names = v.pre().any(|(f, _)| none[f as usize])
            || v.effs().any(|e| none[e.fact as usize] || e.when().any(|(f, _)| none[f as usize]));
        if !names {
            continue;
        }
        x.expand(ai, &mut |var: &Variant| {
            for &(f, _) in var.chosen {
                used[f as usize] = true;
            }
            for &(s, _) in var.ranges {
                let e = &var.effs[s..];
                for k in std::iter::once(e[0]).chain((0..e[2] as usize).map(|j| e[3 + 2 * j])) {
                    if (k as usize) < nf {
                        used[k as usize] = true;
                    }
                }
            }
            Ok(())
        })?;
    }
    Ok((0..nf).filter(|&f| none[f] && used[f]).map(|f| t.key(f as u32).to_string()).collect())
}

/// What pass 1 learns: the head's variables and the counts.
pub struct Plan {
    pub vars: Vec<u32>,  // variable ids in file order
    pub index: Vec<u32>, // variable id -> position (NONE when unused)
    pub metric: bool,
    pub n_ops: u64,
    pub goal_single: Vec<(u32, u32)>,
    pub goal_sets: Vec<(u32, Vec<u32>)>,
    pub counts: SasCounts,
}

pub fn goal_parts(t: &Task) -> Result<(Vec<(u32, u32)>, Vec<(u32, Vec<u32>)>), String> {
    let mut single = Vec::new();
    let mut sets = Vec::new();
    for (f, vals) in &t.goal {
        if vals.is_empty() {
            return Err(format!("goal {}: no value of its domain", t.key(*f)));
        }
        if vals.len() == 1 {
            single.push((*f, vals[0]));
        } else {
            sets.push((*f, vals.clone()));
        }
    }
    Ok((single, sets))
}

pub fn pass1(x: &mut Expander) -> Result<Plan, String> {
    let nf = x.nf;
    let t = x.t;
    let mut used = vec![false; 2 * nf as usize + 1];
    for (f, _) in &t.goal {
        used[*f as usize] = true;
    }
    let mut n_ops = 0u64;
    let mut effects = 0u64;
    let mut cond = 0u64;
    let mut metric = false;
    let mut resets = 0u64;
    for ai in 0..t.acts.len() {
        x.expand(ai, &mut |v: &Variant| {
            n_ops += 1;
            for &(f, _) in v.chosen {
                used[f as usize] = true;
            }
            for &(s, _) in v.ranges {
                let e = &v.effs[s..];
                used[e[0] as usize] = true;
                if e[0] == 2 * nf {
                    resets += 1;
                }
                let nc = e[2] as usize;
                if nc > 0 {
                    cond += 1;
                }
                for j in 0..nc {
                    used[e[3 + 2 * j] as usize] = true;
                }
            }
            effects += v.ranges.len() as u64;
            if v.cost != 1 && v.cost != 0 {
                metric = true;
            }
            Ok(())
        })?;
    }
    let action_ops = n_ops;
    let mut pick_ops = 0;
    for f in 0..nf {
        if x.picks[f as usize] {
            used[(nf + f) as usize] = true;
            pick_ops += t.facts.size(f) as u64;
        }
    }
    n_ops += pick_ops;
    effects += pick_ops;
    let (goal_single, goal_sets) = goal_parts(t)?;
    let mut goal_ops = 0u64;
    if !goal_sets.is_empty() {
        goal_ops = goal_sets.iter().map(|(_, v)| v.len() as u64).product();
        effects += goal_ops;
    }
    n_ops += goal_ops;
    if x.model && pick_ops + goal_ops > 0 {
        metric = true; // the model encoding (module docstring)
    }
    let mut vars: Vec<u32> = (0..2 * nf).filter(|&v| used[v as usize]).collect();
    let keys: Vec<String> = (0..2 * nf).map(|v| if used[v as usize] { x.var_key(v) } else { String::new() }).collect();
    vars.sort_by(|&a, &b| keys[a as usize].cmp(&keys[b as usize]));
    if !goal_sets.is_empty() {
        vars.push(2 * nf);
    }
    let mut index = vec![NONE; 2 * nf as usize + 1];
    for (i, &v) in vars.iter().enumerate() {
        index[v as usize] = i as u32;
    }
    let mut c = x.c.clone();
    c.actions = t.acts.len() as u64;
    c.operators = n_ops;
    c.action_ops = action_ops;
    c.pick_ops = pick_ops;
    c.goal_ops = goal_ops;
    c.effects = effects;
    c.cond_effects = cond;
    c.variables = vars.len() as u64;
    c.values = vars.iter().map(|&v| x.var_size(v) as u64).sum();
    c.pick_vars = vars.iter().filter(|&&v| v >= nf && v < 2 * nf).count() as u64;
    c.goal_resets = resets;
    Ok(Plan { vars, index, metric, n_ops, goal_single, goal_sets, counts: c })
}

struct Out<W: Write> {
    w: W,
    bytes: u64,
    buf: [u8; 24],
}

impl<W: Write> Out<W> {
    fn s(&mut self, s: &str) -> std::io::Result<()> {
        self.bytes += s.len() as u64;
        self.w.write_all(s.as_bytes())
    }
    fn n(&mut self, mut v: u64) -> std::io::Result<()> {
        let mut i = self.buf.len();
        loop {
            i -= 1;
            self.buf[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        self.bytes += (self.buf.len() - i) as u64;
        self.w.write_all(&self.buf[i..])
    }
    fn nl(&mut self) -> std::io::Result<()> {
        self.bytes += 1;
        self.w.write_all(b"\n")
    }
    fn line_n(&mut self, v: u64) -> std::io::Result<()> {
        self.n(v)?;
        self.nl()
    }
    fn line(&mut self, s: &str) -> std::io::Result<()> {
        self.s(s)?;
        self.nl()
    }
}

fn atom_kind(key: &str) -> String {
    let head = key.split(':').next().unwrap_or("");
    let k: String = head.chars().filter(|c| c.is_alphanumeric()).collect();
    if k.is_empty() { "fact".to_string() } else { k }
}

/// Pass 2: the file. `start` is the start's value per task fact (NONE where the domain lacks it).
pub fn pass2<W: Write>(x: &mut Expander, plan: &Plan, start: &[u32], w: W) -> Result<u64, String> {
    let io = |e: std::io::Error| e.to_string();
    let nf = x.nf;
    let t = x.t;
    // the start first: an error there must not leave half a file
    let mut misses = Vec::new();
    let mut state = Vec::with_capacity(plan.vars.len());
    for &v in &plan.vars {
        if v >= nf {
            state.push(0);
        } else if start[v as usize] == NONE {
            misses.push(t.key(v).to_string());
        } else {
            state.push(start[v as usize]);
        }
    }
    if !misses.is_empty() {
        return Err(format!("{} facts out of their domains: {:?}", misses.len(), &misses[..misses.len().min(10)]));
    }
    let mut o = Out { w, bytes: 0, buf: [0; 24] };
    for s in ["begin_version", "3", "end_version", "begin_metric"] {
        o.line(s).map_err(io)?;
    }
    o.line_n(plan.metric as u64).map_err(io)?;
    o.line("end_metric").map_err(io)?;
    o.line_n(plan.vars.len() as u64).map_err(io)?;
    for &v in &plan.vars {
        let key = x.var_key(v);
        o.line("begin_variable").map_err(io)?;
        o.line(&key).map_err(io)?;
        o.line("-1").map_err(io)?;
        let size = x.var_size(v);
        o.line_n(size as u64).map_err(io)?;
        let kind = atom_kind(&key);
        for val in 0..size {
            o.line(&format!("Atom {}({}={})", kind, key, x.value_name(v, val))).map_err(io)?;
        }
        o.line("end_variable").map_err(io)?;
    }
    o.line("0").map_err(io)?;
    o.line("begin_state").map_err(io)?;
    for s in &state {
        o.line_n(*s as u64).map_err(io)?;
    }
    o.line("end_state").map_err(io)?;
    o.line("begin_goal").map_err(io)?;
    o.line_n(plan.goal_single.len() as u64 + (!plan.goal_sets.is_empty()) as u64).map_err(io)?;
    for &(f, val) in &plan.goal_single {
        o.n(plan.index[f as usize] as u64).map_err(io)?;
        o.s(" ").map_err(io)?;
        o.line_n(val as u64).map_err(io)?;
    }
    if !plan.goal_sets.is_empty() {
        o.n(plan.index[2 * nf as usize] as u64).map_err(io)?;
        o.line(" 1").map_err(io)?;
    }
    o.line("end_goal").map_err(io)?;
    o.line_n(plan.n_ops).map_err(io)?;
    let mut n = 0u64;
    let index = &plan.index;
    let mut ioerr: Option<std::io::Error> = None;
    for ai in 0..t.acts.len() {
        x.expand(ai, &mut |v: &Variant| {
            let r = (|| -> std::io::Result<()> {
                o.line("begin_operator")?;
                o.s("a")?;
                o.line_n(n)?;
                o.line_n(v.chosen.len() as u64)?;
                for &(f, val) in v.chosen {
                    o.n(index[f as usize] as u64)?;
                    o.s(" ")?;
                    o.line_n(val as u64)?;
                }
                o.line_n(v.ranges.len() as u64)?;
                for &(s, _) in v.ranges {
                    let e = &v.effs[s..];
                    let nc = e[2] as usize;
                    o.n(nc as u64)?;
                    for j in 0..nc {
                        o.s(" ")?;
                        o.n(index[e[3 + 2 * j] as usize] as u64)?;
                        o.s(" ")?;
                        o.n(e[4 + 2 * j] as u64)?;
                    }
                    o.s(" ")?;
                    o.n(index[e[0] as usize] as u64)?;
                    o.s(" -1 ")?;
                    o.line_n(e[1] as u64)?;
                }
                o.line_n(v.cost as u64)?;
                o.line("end_operator")
            })();
            n += 1;
            if let Err(e) = r {
                ioerr = Some(e);
                return Err("write".into());
            }
            Ok(())
        })
        .map_err(|e| if let Some(ie) = ioerr.take() { ie.to_string() } else { e })?;
    }
    let mut pick_facts: Vec<u32> = (0..nf).filter(|&f| x.picks[f as usize]).collect();
    pick_facts.sort_by(|&a, &b| t.key(a).cmp(t.key(b)));
    for f in pick_facts {
        for u in 0..t.facts.size(f) {
            o.line("begin_operator").map_err(io)?;
            o.s("a").map_err(io)?;
            o.line_n(n).map_err(io)?;
            o.line("0").map_err(io)?;
            o.line("1").map_err(io)?;
            o.s("0 ").map_err(io)?;
            o.n(index[(nf + f) as usize] as u64).map_err(io)?;
            o.s(" -1 ").map_err(io)?;
            o.line_n(u as u64).map_err(io)?;
            o.line("0").map_err(io)?;
            o.line("end_operator").map_err(io)?;
            n += 1;
        }
    }
    if !plan.goal_sets.is_empty() {
        let mut gidx = vec![0usize; plan.goal_sets.len()];
        let mut order: Vec<usize> = (0..plan.goal_sets.len()).collect();
        order.sort_by(|&a, &b| t.key(plan.goal_sets[a].0).cmp(t.key(plan.goal_sets[b].0)));
        loop {
            o.line("begin_operator").map_err(io)?;
            o.s("a").map_err(io)?;
            o.line_n(n).map_err(io)?;
            o.line_n(plan.goal_sets.len() as u64).map_err(io)?;
            for &j in &order {
                let (f, vals) = &plan.goal_sets[j];
                o.n(index[*f as usize] as u64).map_err(io)?;
                o.s(" ").map_err(io)?;
                o.line_n(vals[gidx[j]] as u64).map_err(io)?;
            }
            o.line("1").map_err(io)?;
            o.s("0 ").map_err(io)?;
            o.n(index[2 * nf as usize] as u64).map_err(io)?;
            o.line(" -1 1").map_err(io)?;
            o.line("0").map_err(io)?;
            o.line("end_operator").map_err(io)?;
            n += 1;
            let lens: Vec<usize> = plan.goal_sets.iter().map(|(_, v)| v.len()).collect();
            if !advance(&mut gidx, |j| lens[j]) {
                break;
            }
        }
    }
    if n != plan.n_ops {
        return Err(format!("pass 2 wrote {} operators, pass 1 counted {}", n, plan.n_ops));
    }
    o.line("0").map_err(io)?;
    o.w.flush().map_err(io)?;
    Ok(o.bytes)
}

/// One step of a plan, read back (plan_fd.parse_plan: goal operators left out, picks folded into the steps).
pub struct Step {
    pub op: u64,
    pub action: usize,
    pub chosen: Vec<(u32, u32)>,
    pub guessed: Vec<(u32, u32)>, // the variant's guesses, then the picked ones as the pick variables stood
    pub picked: Vec<(u32, Vec<u32>, u32)>, // (key, values, the pick variable's value)
}

/// The plan's operators (their numbers, in plan order) read back to actions.
pub fn decode(x: &mut Expander, plan_ops: &[u64]) -> Result<Vec<Step>, String> {
    let t = x.t;
    let nf = x.nf;
    let want: std::collections::HashSet<u64> = plan_ops.iter().copied().collect();
    let mut found: std::collections::HashMap<u64, (usize, Vec<(u32, u32)>, Vec<(u32, u32)>, Picked)> = Default::default();
    let mut n = 0u64;
    for ai in 0..t.acts.len() {
        let mut here: Vec<(u64, Vec<(u32, u32)>, Vec<(u32, u32)>)> = Vec::new();
        let picked = x.expand(ai, &mut |v: &Variant| {
            if want.contains(&n) {
                here.push((n, v.chosen.to_vec(), v.guessed.to_vec()));
            }
            n += 1;
            Ok(())
        })?;
        for (op, c, g) in here {
            found.insert(op, (ai, c, g, picked.clone()));
        }
    }
    // pick operators, then goal operators
    let mut pick_facts: Vec<u32> = (0..nf).filter(|&f| x.picks[f as usize]).collect();
    pick_facts.sort_by(|&a, &b| t.key(a).cmp(t.key(b)));
    let mut pick_ops: std::collections::HashMap<u64, (u32, u32)> = Default::default();
    for f in pick_facts {
        for u in 0..t.facts.size(f) {
            pick_ops.insert(n, (f, u));
            n += 1;
        }
    }
    let mut picks: std::collections::HashMap<u32, u32> = Default::default(); // a pick variable starts at its first value
    let mut steps = Vec::new();
    for &op in plan_ops {
        if let Some(&(f, u)) = pick_ops.get(&op) {
            picks.insert(f, u);
            continue;
        }
        let Some((ai, chosen, guessed, picked)) = found.get(&op) else {
            continue; // a goal operator
        };
        let mut g = guessed.clone();
        let mut p = Vec::new();
        for (key, vals, _) in picked {
            let u = *picks.get(key).unwrap_or(&0);
            g.push((*key, if vals.contains(&u) { u } else { vals[0] }));
            p.push((*key, vals.clone(), u));
        }
        steps.push(Step { op, action: *ai, chosen: chosen.clone(), guessed: g, picked: p });
    }
    Ok(steps)
}

fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// The operator table (`sas --ops`), so that a plan reads back without a second run: JSON lines, in this order.
///
/// * `{"operators": N, "facts": [...]}`: the operator count and the file's task facts (its variables but the pick
///   and goal ones), in file order;
/// * per operator of an action: `{"op": n, "action": i, "chosen": [[key, value]...], "guessed": [[key, value,
///   [[key, [values]]...]]...]}`, the variant's precondition values and guessed values, each guess with its live
///   conditions (task.py's `Op.chosen` and `Op.guessed`);
/// * per action, after its operators: `{"action": i, "name": ..., "members": [...], "picked": [[key, [values],
///   [[key, [values]]...]]...]}`, the input actions it stands for and its guesses written through pick variables
///   with their conditions (task.py's `Op.picked`);
/// * per pick operator `{"op": n, "pick": [key, value]}`, per goal operator `{"op": n, "goal": true}`.
///
/// Values are the domain's value texts. Operators are numbered as in the file (pass 2's order).
pub fn write_ops<W: Write>(x: &mut Expander, plan: &Plan, mut w: W) -> Result<(), String> {
    let io = |e: std::io::Error| e.to_string();
    let t = x.t;
    let nf = x.nf;
    let key = |f: u32| json_str(t.key(f));
    let val = |f: u32, v: u32| json_str(t.strs.get(t.facts.dom(f)[v as usize]));
    let set = |f: u32, vs: &[u32]| format!("[{}, [{}]]", key(f), vs.iter().map(|&v| val(f, v)).collect::<Vec<_>>().join(", "));
    let facts: Vec<String> = plan.vars.iter().filter(|&&v| v < nf).map(|&v| key(v)).collect();
    writeln!(w, "{{\"operators\": {}, \"facts\": [{}]}}", plan.n_ops, facts.join(", ")).map_err(io)?;
    let mut n = 0u64;
    let mut ioerr: Option<std::io::Error> = None;
    for ai in 0..t.acts.len() {
        let picked = x
            .expand(ai, &mut |v: &Variant| {
                let ch: Vec<String> = v.chosen.iter().map(|&(f, u)| format!("[{}, {}]", key(f), val(f, u))).collect();
                let gu: Vec<String> = v
                    .guessed
                    .iter()
                    .zip(v.guess_conds)
                    .map(|(&(f, u), &(ls, le))| {
                        let c: Vec<String> =
                            v.conds[ls..le].iter().map(|&(cf, off, len)| set(cf, &v.arena[off..off + len])).collect();
                        format!("[{}, {}, [{}]]", key(f), val(f, u), c.join(", "))
                    })
                    .collect();
                if let Err(e) = writeln!(w, "{{\"op\": {}, \"action\": {}, \"chosen\": [{}], \"guessed\": [{}]}}", n, ai, ch.join(", "), gu.join(", ")) {
                    ioerr = Some(e);
                    return Err("write".into());
                }
                n += 1;
                Ok(())
            })
            .map_err(|e| if let Some(ie) = ioerr.take() { ie.to_string() } else { e })?;
        let mem: Vec<String> = t.acts.members(ai).iter().map(|m| m.to_string()).collect();
        let pk: Vec<String> = picked
            .iter()
            .map(|(f, vals, when)| {
                let vs: Vec<String> = vals.iter().map(|&u| val(*f, u)).collect();
                let c: Vec<String> = when.iter().map(|(cf, cs)| set(*cf, cs)).collect();
                format!("[{}, [{}], [{}]]", key(*f), vs.join(", "), c.join(", "))
            })
            .collect();
        writeln!(w, "{{\"action\": {}, \"name\": {}, \"members\": [{}], \"picked\": [{}]}}", ai, json_str(&t.name(ai)), mem.join(", "), pk.join(", "))
            .map_err(io)?;
    }
    let mut pick_facts: Vec<u32> = (0..nf).filter(|&f| x.picks[f as usize]).collect();
    pick_facts.sort_by(|&a, &b| t.key(a).cmp(t.key(b)));
    for f in pick_facts {
        for u in 0..t.facts.size(f) {
            writeln!(w, "{{\"op\": {}, \"pick\": [{}, {}]}}", n, key(f), val(f, u)).map_err(io)?;
            n += 1;
        }
    }
    if !plan.goal_sets.is_empty() {
        let ops: u64 = plan.goal_sets.iter().map(|(_, v)| v.len() as u64).product();
        for _ in 0..ops {
            writeln!(w, "{{\"op\": {}, \"goal\": true}}", n).map_err(io)?;
            n += 1;
        }
    }
    if n != plan.n_ops {
        return Err(format!("the operator table has {} operators, pass 1 counted {}", n, plan.n_ops));
    }
    w.flush().map_err(io)
}
