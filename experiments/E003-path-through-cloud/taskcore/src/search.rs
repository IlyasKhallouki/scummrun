//! The Rust search (E011, plan.md "plan", item 3): Fast Downward's `lazy_greedy([hff], preferred=[hff],
//! reopen_closed=false)` with `hff = eval_modify_costs(ff(), cost_type=plusone)`, rule for rule, on the SAS task held
//! in memory (`SasTask`), so that it returns Fast Downward's plan, byte for byte (`plan_text`), with no SAS text, no
//! translator-output parse and no landmark graph in between.
//!
//! What is Fast Downward's here (release 26.6.0, third_party/downward/src/search), and must stay so for the plans to
//! be the same:
//!
//! * **The task** (tasks/root_task.cc): an operator's preconditions are its prevail conditions then the effects'
//!   preconditions other than -1, in the file's order; with metric 0 every operator costs 1.
//! * **Successor order** (task_utils/successor_generator_factory.cc): the operators stable-sorted by their sorted
//!   precondition list (lexicographic over (var, value), a prefix first), so ties keep the operator's number; the
//!   applicable ones are generated in that order (the generator's tree visits its sorted array in order).
//! * **h_FF** (heuristics/relaxation_heuristic.cc, additive_heuristic.cc, ff_heuristic.cc): one unary operator per
//!   effect (the operator's preconditions and the effect's conditions, sorted and unique; cost the adapted cost), then
//!   `simplify` (an exact duplicate keeps the cheapest, then the lowest number; a unary operator with at most 5
//!   preconditions is dropped when a strict subset of them with the same effect is in the map at no more cost); the
//!   exploration on FD's `AdaptiveQueue` (a bucket queue, last in first out within a bucket, that becomes
//!   libstdc++'s binary heap for good once a key reaches 100 and passes the pushes since the last clear); costs clamped
//!   at 10^8; the relaxed plan and the preferred operators from the goals back along `reached_by`. `reached_by` is
//!   not reset between evaluations, as in FD.
//! * **The adapted cost** (operator_cost.cc): plusone is cost + 1, or 1 when every operator costs 1.
//! * **The lazy search** (search_algorithms/lazy_search.cc, open_lists/alternation_open_list.cc,
//!   best_first_open_list.cc, search_progress.cc): an alternation of [h, h preferred only] with boost 1000, each a map
//!   of h to a FIFO; a successor is inserted under its parent's h, into the preferred list too when its operator is
//!   preferred; `remove_min` takes the non-empty list with the lowest priority (the first on a tie) and adds 1 to it;
//!   a state is evaluated when it is taken, skipped when it was taken before; the goal is tested after the
//!   evaluation, then a new lowest h (not the start's) boosts the preferred list by 1000, then the successors.
//! * **The plan file** (plan_manager.cc): `(name)` per step, then `; cost = C (unit cost|general cost)`.

use crate::model::*;
use crate::sas::{self, Expander, Plan};

mod watch;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::Instant;

// --- the SAS task in memory --------------------------------------------------------------------------------------

/// One effect: its conditions (a range of `SasTask::conds`), the variable and the value it writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Eff {
    pub c0: u32,
    pub c1: u32,
    pub var: u32,
    pub post: u32,
}

/// Fast Downward's task as it reads taskcore's file (no mutex groups, no axioms); operator i is named `a<i>`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SasTask {
    pub metric: bool,
    pub sizes: Vec<u32>,
    pub names: Vec<String>,
    pub init: Vec<u32>,
    pub goal: Vec<(u32, u32)>,
    pub pre_off: Vec<u32>,
    pub pre: Vec<(u32, u32)>,
    pub eff_off: Vec<u32>,
    pub effs: Vec<Eff>,
    pub conds: Vec<(u32, u32)>,
    /// FD's cost: the file's, or 1 with metric 0
    pub cost: Vec<u32>,
}

impl SasTask {
    pub fn n_ops(&self) -> usize {
        self.cost.len()
    }
    pub fn pre(&self, o: usize) -> &[(u32, u32)] {
        &self.pre[self.pre_off[o] as usize..self.pre_off[o + 1] as usize]
    }
    pub fn effs(&self, o: usize) -> &[Eff] {
        &self.effs[self.eff_off[o] as usize..self.eff_off[o + 1] as usize]
    }
    pub fn conds(&self, e: &Eff) -> &[(u32, u32)] {
        &self.conds[e.c0 as usize..e.c1 as usize]
    }
    pub fn applicable(&self, o: usize, s: &[u32]) -> bool {
        self.pre(o).iter().all(|&(v, x)| s[v as usize] == x)
    }
    /// The successor (state_registry.cc): every effect whose conditions hold in `s` writes its value.
    pub fn apply_into(&self, o: usize, s: &[u32], out: &mut Vec<u32>) {
        out.clear();
        out.extend_from_slice(s);
        for e in self.effs(o) {
            if self.conds(e).iter().all(|&(v, x)| s[v as usize] == x) {
                out[e.var as usize] = e.post;
            }
        }
    }
    pub fn is_goal(&self, s: &[u32]) -> bool {
        self.goal.iter().all(|&(v, x)| s[v as usize] == x)
    }
    fn push_op(&mut self, pre: &[(u32, u32)], effs: &[EffSpec], cost: u32) {
        self.pre.extend_from_slice(pre);
        self.pre_off.push(self.pre.len() as u32);
        for (c, var, post) in effs {
            let c0 = self.conds.len() as u32;
            self.conds.extend_from_slice(c);
            self.effs.push(Eff { c0, c1: self.conds.len() as u32, var: *var, post: *post });
        }
        self.eff_off.push(self.effs.len() as u32);
        self.cost.push(cost);
    }
}

/// Fast Downward's reading of a translator-output file of the shape taskcore writes (tasks/root_task.cc): the test
/// that `from_task` builds the task the file holds, and the search on a file (`taskcore search`).
pub fn parse_sas(text: &str) -> Result<SasTask, String> {
    let mut lines = text.lines();
    let mut ln = 0usize;
    let mut next = || -> Result<&str, String> {
        ln += 1;
        lines.next().ok_or_else(|| format!("line {}: end of file", ln))
    };
    fn num<T: std::str::FromStr>(s: &str) -> Result<T, String> {
        s.trim().parse::<T>().map_err(|_| format!("not a number: {:?}", s))
    }
    fn magic(got: &str, want: &str) -> Result<(), String> {
        if got == want {
            Ok(())
        } else {
            Err(format!("expected {:?}, got {:?}", want, got))
        }
    }
    magic(next()?, "begin_version")?;
    magic(next()?, "3")?;
    magic(next()?, "end_version")?;
    magic(next()?, "begin_metric")?;
    let metric = match next()? {
        "0" => false,
        "1" => true,
        m => return Err(format!("metric {:?}", m)),
    };
    magic(next()?, "end_metric")?;
    let nv: usize = num(next()?)?;
    let mut t = SasTask { metric, pre_off: vec![0], eff_off: vec![0], ..Default::default() };
    for _ in 0..nv {
        magic(next()?, "begin_variable")?;
        t.names.push(next()?.to_string());
        magic(next()?, "-1")?;
        let size: u32 = num(next()?)?;
        for _ in 0..size {
            next()?;
        }
        magic(next()?, "end_variable")?;
        t.sizes.push(size);
    }
    let nm: usize = num(next()?)?;
    if nm != 0 {
        return Err(format!("{} mutex groups (taskcore writes none)", nm));
    }
    magic(next()?, "begin_state")?;
    for _ in 0..nv {
        t.init.push(num(next()?)?);
    }
    magic(next()?, "end_state")?;
    magic(next()?, "begin_goal")?;
    let pair = |s: &str| -> Result<(u32, u32), String> {
        let mut p = s.split_ascii_whitespace();
        let a = num(p.next().unwrap_or(""))?;
        let b = num(p.next().unwrap_or(""))?;
        Ok((a, b))
    };
    let ng: usize = num(next()?)?;
    for _ in 0..ng {
        t.goal.push(pair(next()?)?);
    }
    magic(next()?, "end_goal")?;
    let no: usize = num(next()?)?;
    for i in 0..no {
        magic(next()?, "begin_operator")?;
        let name = next()?;
        if name != format!("a{}", i) {
            return Err(format!("operator {} is named {:?} (taskcore names it a{})", i, name, i));
        }
        let np: usize = num(next()?)?;
        let mut pre = Vec::with_capacity(np);
        for _ in 0..np {
            pre.push(pair(next()?)?);
        }
        let ne: usize = num(next()?)?;
        let mut effs = Vec::with_capacity(ne);
        let mut epre = Vec::new();
        for _ in 0..ne {
            let v: Vec<i64> = next()?.split_ascii_whitespace().map(num).collect::<Result<_, _>>()?;
            let nc = *v.first().ok_or("an empty effect line")? as usize;
            if v.len() != 4 + 2 * nc {
                return Err(format!("operator {}: an effect line of {} numbers", i, v.len()));
            }
            let c: Vec<(u32, u32)> = (0..nc).map(|k| (v[1 + 2 * k] as u32, v[2 + 2 * k] as u32)).collect();
            let var = v[1 + 2 * nc] as u32;
            if v[2 + 2 * nc] != -1 {
                epre.push((var, v[2 + 2 * nc] as u32));
            }
            effs.push((c, var, v[3 + 2 * nc] as u32));
        }
        pre.extend(epre);
        let cost: u32 = num(next()?)?;
        magic(next()?, "end_operator")?;
        t.push_op(&pre, &effs, if metric { cost } else { 1 });
    }
    let na: usize = num(next()?)?;
    if na != 0 {
        return Err(format!("{} axioms (taskcore writes none)", na));
    }
    Ok(t)
}

/// An operator's effect as `SasTask::push_op` takes it: (conditions, variable, value).
pub type EffSpec = (Vec<(u32, u32)>, u32, u32);

/// What an operator of `from_task` stands for: a variant of a reduced action, a pick operator, a goal operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpOf {
    /// the reduced action and the variant's number among its written variants
    Action(u32, u32),
    /// a pick operator: the task fact and the value index
    Pick(u32, u32),
    Goal,
}

/// The SAS task `sas::pass2` writes for `x`'s task from `start`, built in memory (`pass1`'s plan given), with what
/// each operator stands for. The same task the file holds: `parse_sas(pass2 text) == from_task(..).0` (the test).
/// Pick windows are refused (E009 runs without them).
pub fn from_task(x: &mut Expander, plan: &Plan, start: &[u32]) -> Result<(SasTask, Vec<OpOf>), String> {
    if x.windows {
        return Err("the in-memory task: pick windows are not supported (E009 runs without them)".into());
    }
    let nf = x.nf;
    let t = x.t;
    let index = &plan.index;
    let mut st = SasTask { metric: plan.metric, pre_off: vec![0], eff_off: vec![0], ..Default::default() };
    let mut misses = Vec::new();
    for &v in &plan.vars {
        st.sizes.push(x.var_size(v));
        st.names.push(x.var_key(v));
        if v >= nf {
            st.init.push(0);
        } else if start[v as usize] == NONE {
            misses.push(t.key(v).to_string());
        } else {
            st.init.push(start[v as usize]);
        }
    }
    if !misses.is_empty() {
        return Err(format!("{} facts out of their domains: {:?}", misses.len(), &misses[..misses.len().min(10)]));
    }
    for &(f, val) in &plan.goal_single {
        st.goal.push((index[f as usize], val));
    }
    if !plan.goal_sets.is_empty() {
        st.goal.push((index[2 * nf as usize], 1));
    }
    let metric = plan.metric;
    let fd_cost = |c: u32| if metric { c } else { 1 };
    let mut of = Vec::with_capacity(plan.n_ops as usize);
    let mut effs: Vec<EffSpec> = Vec::new();
    let mut pre: Vec<(u32, u32)> = Vec::new();
    for ai in 0..t.acts.len() {
        let mut k = 0u32;
        x.expand(ai, &mut |v: &sas::Variant| {
            pre.clear();
            pre.extend(v.chosen.iter().map(|&(f, val)| (index[f as usize], val)));
            effs.clear();
            for &(s, _) in v.ranges {
                let e = &v.effs[s..];
                let nc = e[2] as usize;
                let c: Vec<(u32, u32)> = (0..nc).map(|j| (index[e[3 + 2 * j] as usize], e[4 + 2 * j])).collect();
                effs.push((c, index[e[0] as usize], e[1]));
            }
            st.push_op(&pre, &effs, fd_cost(v.cost));
            of.push(OpOf::Action(ai as u32, k));
            k += 1;
            Ok(())
        })?;
    }
    let mut pick_facts: Vec<u32> = (0..nf).filter(|&f| x.picks[f as usize]).collect();
    pick_facts.sort_by(|&a, &b| t.key(a).cmp(t.key(b)));
    for f in pick_facts {
        for u in 0..t.facts.size(f) {
            st.push_op(&[], &[(Vec::new(), index[(nf + f) as usize], u)], fd_cost(0));
            of.push(OpOf::Pick(f, u));
        }
    }
    if !plan.goal_sets.is_empty() {
        let mut gidx = vec![0usize; plan.goal_sets.len()];
        let mut order: Vec<usize> = (0..plan.goal_sets.len()).collect();
        order.sort_by(|&a, &b| t.key(plan.goal_sets[a].0).cmp(t.key(plan.goal_sets[b].0)));
        let lens: Vec<usize> = plan.goal_sets.iter().map(|(_, v)| v.len()).collect();
        loop {
            let p: Vec<(u32, u32)> = order
                .iter()
                .map(|&j| {
                    let (f, vals) = &plan.goal_sets[j];
                    (index[*f as usize], vals[gidx[j]])
                })
                .collect();
            st.push_op(&p, &[(Vec::new(), index[2 * nf as usize], 1)], fd_cost(0));
            of.push(OpOf::Goal);
            let mut more = false;
            for j in (0..gidx.len()).rev() {
                gidx[j] += 1;
                if gidx[j] < lens[j] {
                    more = true;
                    break;
                }
                gidx[j] = 0;
            }
            if !more {
                break;
            }
        }
    }
    if st.n_ops() as u64 != plan.n_ops {
        return Err(format!("the in-memory task has {} operators, pass 1 counted {}", st.n_ops(), plan.n_ops));
    }
    Ok((st, of))
}

/// `sas::pass1` and `from_task` in one expansion of the actions (what a request runs): pass 1's plan (the same
/// variables, index, metric, operator count and counts) and the in-memory task. The operators are kept with the
/// task's fact ids (pick variable `nf + k`, `goal` `2 nf`) while the actions expand, then renumbered to the file's
/// variables once pass 1's head is known. TASKCORE_CHECK_BUILD set: also built the two-pass way and compared.
pub fn build(x: &mut Expander, start: &[u32]) -> Result<(Plan, SasTask, Vec<OpOf>), String> {
    if x.windows {
        return Err("the in-memory task: pick windows are not supported (E009 runs without them)".into());
    }
    let nf = x.nf;
    let t = x.t;
    let mut used = vec![false; 2 * nf as usize + 2];
    for (f, _) in &t.goal {
        used[*f as usize] = true;
    }
    let (mut n_ops, mut effects, mut cond, mut metric, mut resets) = (0u64, 0u64, 0u64, false, 0u64);
    let mut st = SasTask { pre_off: vec![0], eff_off: vec![0], ..Default::default() };
    let mut of = Vec::new();
    let mut costs_raw: Vec<u32> = Vec::new();
    let kb0 = crate::perf::now();
    let th = if x.fast { crate::par::threads() } else { 1 };
    if th > 1 {
        // the actions in chunks on threads, each with its own expander, joined in action order (`BuildPart`)
        let n = t.acts.len();
        let nchunks = (th * 8).min(n.max(1));
        let size = n.div_ceil(nchunks.max(1)).max(1);
        let (limit, model, check, fast) = (x.limit, x.model, x.check, x.fast);
        let init = || {
            let mut xw = Expander::new(t, limit);
            xw.model = model;
            xw.check = check;
            xw.fast = fast;
            xw
        };
        let parts: Vec<BuildPart> = crate::par::map_dyn(n.div_ceil(size), th, init, |xw, k| BuildPart::of(xw, k * size..((k + 1) * size).min(n)));
        for p in parts {
            if let Some(e) = p.err {
                return Err(e);
            }
            let (bp, be, bc) = (st.pre.len() as u32, st.effs.len() as u32, st.conds.len() as u32);
            st.pre.extend_from_slice(&p.pre);
            st.pre_off.extend(p.pre_end.iter().map(|&e| e + bp));
            st.conds.extend_from_slice(&p.conds);
            st.effs.extend(p.effs.iter().map(|e| Eff { c0: e.c0 + bc, c1: e.c1 + bc, var: e.var, post: e.post }));
            st.eff_off.extend(p.eff_end.iter().map(|&e| e + be));
            costs_raw.extend_from_slice(&p.costs);
            of.extend_from_slice(&p.of);
            for &u in &p.used {
                used[u as usize] = true;
            }
            for &f in &p.picks {
                x.picks[f as usize] = true;
            }
            n_ops += p.n_ops;
            effects += p.effects;
            cond += p.cond;
            metric |= p.metric;
            resets += p.resets;
            x.c.dead_actions += p.c.dead_actions;
            x.c.pick_actions += p.c.pick_actions;
            x.c.noop_effects += p.c.noop_effects;
            x.c.empty_variants += p.c.empty_variants;
            x.c.variants += p.c.variants;
            x.c.max_variants = x.c.max_variants.max(p.c.max_variants);
        }
    }
    for ai in 0..if th > 1 { 0 } else { t.acts.len() } {
        let mut k = 0u32;
        x.expand(ai, &mut |v: &sas::Variant| {
            n_ops += 1;
            for &(f, val) in v.chosen {
                used[f as usize] = true;
                st.pre.push((f, val));
            }
            st.pre_off.push(st.pre.len() as u32);
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
                let c0 = st.conds.len() as u32;
                for j in 0..nc {
                    used[e[3 + 2 * j] as usize] = true;
                    st.conds.push((e[3 + 2 * j], e[4 + 2 * j]));
                }
                st.effs.push(Eff { c0, c1: st.conds.len() as u32, var: e[0], post: e[1] });
            }
            st.eff_off.push(st.effs.len() as u32);
            effects += v.ranges.len() as u64;
            if v.cost != 1 && v.cost != 0 {
                metric = true;
            }
            costs_raw.push(v.cost);
            of.push(OpOf::Action(ai as u32, k));
            k += 1;
            Ok(())
        })?;
    }
    if std::env::var_os("TASKCORE_PROFILE").is_some() {
        eprintln!("profile build: the actions' variants {:.0} M instructions, {} operators", crate::perf::since(kb0).0, n_ops);
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
    let (goal_single, goal_sets) = sas::goal_parts(t)?;
    let mut goal_ops = 0u64;
    if !goal_sets.is_empty() {
        goal_ops = goal_sets.iter().map(|(_, v)| v.len() as u64).product();
        effects += goal_ops;
    }
    n_ops += goal_ops;
    if x.model && pick_ops + goal_ops > 0 {
        metric = true;
    }
    x.n_windows = 0;
    let mut vars: Vec<u32> = (0..2 * nf).filter(|&v| used[v as usize]).collect();
    let keys: Vec<String> = (0..2 * nf).map(|v| if used[v as usize] { x.var_key(v) } else { String::new() }).collect();
    vars.sort_by(|&a, &b| keys[a as usize].cmp(&keys[b as usize]));
    if !goal_sets.is_empty() {
        vars.push(2 * nf);
    }
    let mut index = vec![NONE; 2 * nf as usize + 2];
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
    c.window_mode = false;
    c.windows = 0;
    c.window_pick_ops = 0;
    let plan = Plan { vars, index, metric, n_ops, goal_single, goal_sets, counts: c };
    // the head, then the operators renumbered to the file's variables
    let index = &plan.index;
    st.metric = metric;
    let mut misses = Vec::new();
    for &v in &plan.vars {
        st.sizes.push(x.var_size(v));
        st.names.push(x.var_key(v));
        if v >= nf {
            st.init.push(0);
        } else if start[v as usize] == NONE {
            misses.push(t.key(v).to_string());
        } else {
            st.init.push(start[v as usize]);
        }
    }
    if !misses.is_empty() {
        return Err(format!("{} facts out of their domains: {:?}", misses.len(), &misses[..misses.len().min(10)]));
    }
    for &(f, val) in &plan.goal_single {
        st.goal.push((index[f as usize], val));
    }
    if !plan.goal_sets.is_empty() {
        st.goal.push((index[2 * nf as usize], 1));
    }
    st.pre.iter_mut().for_each(|p| p.0 = index[p.0 as usize]);
    st.conds.iter_mut().for_each(|p| p.0 = index[p.0 as usize]);
    st.effs.iter_mut().for_each(|e| e.var = index[e.var as usize]);
    st.cost = costs_raw.iter().map(|&c| if metric { c } else { 1 }).collect();
    let fd_cost = |c: u32| if metric { c } else { 1 };
    let mut pick_facts: Vec<u32> = (0..nf).filter(|&f| x.picks[f as usize]).collect();
    pick_facts.sort_by(|&a, &b| t.key(a).cmp(t.key(b)));
    for f in pick_facts {
        for u in 0..t.facts.size(f) {
            st.push_op(&[], &[(Vec::new(), index[(nf + f) as usize], u)], fd_cost(0));
            of.push(OpOf::Pick(f, u));
        }
    }
    if !plan.goal_sets.is_empty() {
        let mut gidx = vec![0usize; plan.goal_sets.len()];
        let mut order: Vec<usize> = (0..plan.goal_sets.len()).collect();
        order.sort_by(|&a, &b| t.key(plan.goal_sets[a].0).cmp(t.key(plan.goal_sets[b].0)));
        let lens: Vec<usize> = plan.goal_sets.iter().map(|(_, v)| v.len()).collect();
        loop {
            let p: Vec<(u32, u32)> = order
                .iter()
                .map(|&j| {
                    let (f, vals) = &plan.goal_sets[j];
                    (index[*f as usize], vals[gidx[j]])
                })
                .collect();
            st.push_op(&p, &[(Vec::new(), index[2 * nf as usize], 1)], fd_cost(0));
            of.push(OpOf::Goal);
            let mut more = false;
            for j in (0..gidx.len()).rev() {
                gidx[j] += 1;
                if gidx[j] < lens[j] {
                    more = true;
                    break;
                }
                gidx[j] = 0;
            }
            if !more {
                break;
            }
        }
    }
    if st.n_ops() as u64 != plan.n_ops {
        return Err(format!("the in-memory task has {} operators, its head counted {}", st.n_ops(), plan.n_ops));
    }
    if std::env::var_os("TASKCORE_CHECK_BUILD").is_some() {
        let mut x2 = Expander::new(t, x.limit);
        x2.model = x.model;
        let p2 = sas::pass1(&mut x2)?;
        let (st2, of2) = from_task(&mut x2, &p2, start)?;
        let same_plan = p2.vars == plan.vars
            && p2.index == plan.index
            && p2.metric == plan.metric
            && p2.n_ops == plan.n_ops
            && p2.goal_single == plan.goal_single
            && p2.goal_sets == plan.goal_sets
            && format!("{:?}", p2.counts) == format!("{:?}", plan.counts);
        assert!(same_plan, "build: pass 1's plan differs");
        assert!(st2 == st && of2 == of, "build: the in-memory task differs from the two-pass one");
        eprintln!("build: the one-pass task equals pass 1 and from_task ({} operators)", st.n_ops());
    }
    Ok((plan, st, of))
}

/// This thread's CPU seconds (CLOCK_THREAD_CPUTIME_ID), for the bench's per-evaluation numbers.
pub fn thread_cpu_s() -> f64 {
    #[repr(C)]
    struct Timespec {
        tv_sec: i64,
        tv_nsec: i64,
    }
    extern "C" {
        fn clock_gettime(clk: i32, tp: *mut Timespec) -> i32;
    }
    let mut t = Timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: clock_gettime writes one timespec; 3 is CLOCK_THREAD_CPUTIME_ID on Linux
    if unsafe { clock_gettime(3, &mut t) } != 0 {
        return 0.0;
    }
    t.tv_sec as f64 + t.tv_nsec as f64 * 1e-9
}

// --- the adapted costs ----------------------------------------------------------------------------------------------

/// One chunk of `build`'s actions expanded alone (the fast form's threads): what the sequential loop appends for them, with
/// offsets from the chunk's own start, the facts it marks used, the pick facts, and the expander's counts.
struct BuildPart {
    pre: Vec<(u32, u32)>,
    pre_end: Vec<u32>,
    effs: Vec<Eff>,
    eff_end: Vec<u32>,
    conds: Vec<(u32, u32)>,
    costs: Vec<u32>,
    of: Vec<OpOf>,
    used: Vec<u32>,
    picks: Vec<u32>,
    n_ops: u64,
    effects: u64,
    cond: u64,
    metric: bool,
    resets: u64,
    c: sas::SasCounts,
    err: Option<String>,
}

impl BuildPart {
    fn of(x: &mut Expander, range: std::ops::Range<usize>) -> BuildPart {
        let nf = x.nf;
        let mut p = BuildPart {
            pre: Vec::new(),
            pre_end: Vec::new(),
            effs: Vec::new(),
            eff_end: Vec::new(),
            conds: Vec::new(),
            costs: Vec::new(),
            of: Vec::new(),
            used: Vec::new(),
            picks: Vec::new(),
            n_ops: 0,
            effects: 0,
            cond: 0,
            metric: false,
            resets: 0,
            c: sas::SasCounts::default(),
            err: None,
        };
        let mut used = vec![false; 2 * nf as usize + 2];
        for ai in range {
            let mut k = 0u32;
            let r = x.expand(ai, &mut |v: &sas::Variant| {
                p.n_ops += 1;
                for &(f, val) in v.chosen {
                    used[f as usize] = true;
                    p.pre.push((f, val));
                }
                p.pre_end.push(p.pre.len() as u32);
                for &(s, _) in v.ranges {
                    let e = &v.effs[s..];
                    used[e[0] as usize] = true;
                    if e[0] == 2 * nf {
                        p.resets += 1;
                    }
                    let nc = e[2] as usize;
                    if nc > 0 {
                        p.cond += 1;
                    }
                    let c0 = p.conds.len() as u32;
                    for j in 0..nc {
                        used[e[3 + 2 * j] as usize] = true;
                        p.conds.push((e[3 + 2 * j], e[4 + 2 * j]));
                    }
                    p.effs.push(Eff { c0, c1: p.conds.len() as u32, var: e[0], post: e[1] });
                }
                p.eff_end.push(p.effs.len() as u32);
                p.effects += v.ranges.len() as u64;
                if v.cost != 1 && v.cost != 0 {
                    p.metric = true;
                }
                p.costs.push(v.cost);
                p.of.push(OpOf::Action(ai as u32, k));
                k += 1;
                Ok(())
            });
            if let Err(e) = r {
                p.err = Some(e);
                break;
            }
        }
        p.used = (0..used.len() as u32).filter(|&u| used[u as usize]).collect();
        // the chunk's own picks and counts, the expander left as new for the thread's next chunk
        p.picks = (0..nf).filter(|&f| x.picks[f as usize]).collect();
        for &f in &p.picks {
            x.picks[f as usize] = false;
        }
        p.c = std::mem::take(&mut x.c);
        p
    }
}

/// operator_cost.cc PLUSONE on the task: cost + 1, or 1 for every operator when every operator costs 1.
pub fn plusone(t: &SasTask) -> Vec<i32> {
    let unit = t.cost.iter().all(|&c| c == 1);
    t.cost.iter().map(|&c| if unit { 1 } else { c as i32 + 1 }).collect()
}

// --- FD's AdaptiveQueue ---------------------------------------------------------------------------------------------

const MIN_BUCKETS_BEFORE_SWITCH: i32 = 100;

/// algorithms/priority_queues.h `AdaptiveQueue<PropID>`: a bucket queue that turns into a heap (and stays one).
#[derive(Clone, PartialEq, Debug)]
enum AQueue {
    Bucket { buckets: Vec<Vec<u32>>, current: usize, entries: usize, pushes: i64 },
    Heap(Vec<(i32, u32)>),
}

/// libstdc++'s `std::priority_queue` with FD's `compare_func` (`lhs.first > rhs.first`): `comp(a, b)` is "a below b".
#[inline]
fn comp(a: (i32, u32), b: (i32, u32)) -> bool {
    a.0 > b.0
}

/// bits/stl_heap.h `__push_heap`
fn push_hole(c: &mut [(i32, u32)], mut hole: isize, top: isize, value: (i32, u32)) {
    let mut parent = (hole - 1) / 2;
    while hole > top && comp(c[parent as usize], value) {
        c[hole as usize] = c[parent as usize];
        hole = parent;
        parent = (hole - 1) / 2;
    }
    c[hole as usize] = value;
}

/// bits/stl_heap.h `__adjust_heap`
fn adjust(c: &mut [(i32, u32)], mut hole: isize, len: isize, value: (i32, u32)) {
    let top = hole;
    let mut second = hole;
    while second < (len - 1) / 2 {
        second = 2 * (second + 1);
        if comp(c[second as usize], c[(second - 1) as usize]) {
            second -= 1;
        }
        c[hole as usize] = c[second as usize];
        hole = second;
    }
    if (len & 1) == 0 && second == (len - 2) / 2 {
        second = 2 * (second + 1);
        c[hole as usize] = c[(second - 1) as usize];
        hole = second - 1;
    }
    push_hole(c, hole, top, value);
}

impl AQueue {
    fn new() -> AQueue {
        AQueue::Bucket { buckets: Vec::new(), current: 0, entries: 0, pushes: 0 }
    }
    fn clear(&mut self) {
        match self {
            AQueue::Bucket { buckets, current, entries, pushes } => {
                let mut i = *current;
                while *entries != 0 {
                    *entries -= buckets[i].len();
                    buckets[i].clear();
                    i += 1;
                }
                *current = 0;
                *pushes = 0;
            }
            AQueue::Heap(c) => c.clear(),
        }
    }
    fn empty(&self) -> bool {
        match self {
            AQueue::Bucket { entries, .. } => *entries == 0,
            AQueue::Heap(c) => c.is_empty(),
        }
    }
    fn push(&mut self, key: i32, value: u32) {
        // convert_if_necessary
        if let AQueue::Bucket { buckets, current, entries, pushes } = self {
            if key >= MIN_BUCKETS_BEFORE_SWITCH && (key as i64) > *pushes {
                // extract_sorted_entries: by key from the current bucket, each bucket in its order
                let mut out = Vec::with_capacity(*entries);
                let mut k = *current;
                while *entries != 0 {
                    let b = std::mem::take(&mut buckets[k]);
                    *entries -= b.len();
                    out.extend(b.into_iter().map(|v| (k as i32, v)));
                    k += 1;
                }
                *self = AQueue::Heap(out);
            }
        }
        match self {
            AQueue::Bucket { buckets, current, entries, pushes } => {
                *entries += 1;
                *pushes += 1;
                let k = key as usize;
                if k >= buckets.len() {
                    buckets.resize_with(k + 1, Vec::new);
                } else if k < *current {
                    *current = k;
                }
                buckets[k].push(value);
            }
            AQueue::Heap(c) => {
                c.push((key, value));
                let n = c.len() as isize;
                let v = c[(n - 1) as usize];
                push_hole(c, n - 1, 0, v);
            }
        }
    }
    fn pop(&mut self) -> (i32, u32) {
        match self {
            AQueue::Bucket { buckets, current, entries, .. } => {
                *entries -= 1;
                while *current < buckets.len() && buckets[*current].is_empty() {
                    *current += 1;
                }
                let v = buckets[*current].pop().expect("a non-empty bucket");
                (*current as i32, v)
            }
            AQueue::Heap(c) => {
                let top = c[0];
                let n = c.len() as isize;
                if n > 1 {
                    let last = n - 1;
                    let value = c[last as usize];
                    c[last as usize] = c[0];
                    adjust(c, 0, last, value);
                }
                c.pop();
                top
            }
        }
    }
}

// --- h_FF -------------------------------------------------------------------------------------------------------------

const NO_OP: u32 = u32::MAX;
const MAX_COST_VALUE: i32 = 100_000_000;
const MAX_PRECONDITIONS_TO_TEST: usize = 5;

/// How h_FF's exploration is computed (the same pushes either way, so the same values and preferred operators):
/// `Fd`, FD's loop transcribed (the reference; the default), or `Watch`, by watched preconditions (search/watch.rs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HffMode {
    #[default]
    Fd,
    Watch,
}

impl HffMode {
    pub fn parse(s: &str) -> Result<HffMode, String> {
        match s {
            "fd" => Ok(HffMode::Fd),
            "watch" => Ok(HffMode::Watch),
            _ => Err(format!("h_FF mode {:?} (fd, watch)", s)),
        }
    }
}

/// The relaxed task (relaxation_heuristic.cc) and the FF heuristic's working state (ff_heuristic.cc), kept across
/// evaluations as FD keeps it.
pub struct Ff {
    prop_off: Vec<u32>,
    goal_props: Vec<u32>,
    is_goal: Vec<bool>,
    // unary operators after `simplify`
    u_eff: Vec<u32>,
    u_base: Vec<i32>,
    u_pre_off: Vec<u32>,
    u_pre: Vec<u32>,
    u_op: Vec<u32>,
    prec_of_off: Vec<u32>,
    prec_of: Vec<u32>,
    /// the adapted cost per operator (h_FF's sum)
    op_cost: Vec<i32>,
    // working state
    cost: Vec<i32>,
    reached_by: Vec<u32>,
    marked: Vec<bool>,
    u_cost: Vec<i32>,
    u_unsat: Vec<u32>,
    queue: AQueue,
    relaxed_plan: Vec<bool>,
    plan_ops: Vec<u32>,
    stack: Vec<(u32, bool)>,
    /// the last evaluation's preferred operators (a set: `pref_mark`)
    pub preferred: Vec<u32>,
    pref_mark: Vec<bool>,
    pub n_unary_built: usize,
    pub evaluations: u64,
    pub eval_s: f64,
    /// the evaluations' CPU seconds, millions of instructions and of cycles (this thread's)
    pub eval_cpu_s: f64,
    pub eval_minstr: f64,
    pub eval_mcyc: f64,
    /// the exploration's mode, and the watch structure (built at the first evaluation in mode `Watch`)
    pub mode: HffMode,
    watch: Option<Box<watch::Watch>>,
    check: bool,
    prof: bool,
    log: Vec<(i32, u32, u32)>,
    /// when the watches are built again (watch.rs `maybe_rebuild`): (gap, least)
    watch_policy: (u64, u64),
}

/// A small key for `simplify`'s map: the sorted preconditions (at most `MAX_PRECONDITIONS_TO_TEST + ..` are hashed
/// whole: any length) and the effect.
type SKey = (Box<[u32]>, u32);

/// `build_unary_operators` for the operators in `ops` (`Ff::new`): per effect, the operator's preconditions merged with
/// the effect's conditions (sorted, unique), the effect, the base cost and the operator; offsets from 0.
#[allow(clippy::type_complexity)]
fn unary_ops(t: &SasTask, adj: &[i32], prop_off: &[u32], ops: std::ops::Range<usize>) -> (Vec<u32>, Vec<i32>, Vec<u32>, Vec<u32>, Vec<u32>) {
    let prop = |v: u32, x: u32| prop_off[v as usize] + x;
    let mut b_eff = Vec::new();
    let mut b_base = Vec::new();
    let mut b_pre_off = vec![0u32];
    let mut b_pre: Vec<u32> = Vec::new();
    let mut b_op = Vec::new();
    let mut buf: Vec<u32> = Vec::new();
    let mut opre: Vec<u32> = Vec::new();
    let mut cbuf: Vec<u32> = Vec::new();
    for o in ops {
        opre.clear();
        opre.extend(t.pre(o).iter().map(|&(v, x)| prop(v, x)));
        opre.sort_unstable();
        opre.dedup();
        for e in t.effs(o) {
            // sorted and unique: the operator's (sorted once) merged with the effect's conditions (sorted)
            cbuf.clear();
            cbuf.extend(t.conds(e).iter().map(|&(v, x)| prop(v, x)));
            cbuf.sort_unstable();
            buf.clear();
            let (mut i, mut j) = (0, 0);
            while i < opre.len() || j < cbuf.len() {
                let x = if j == cbuf.len() || (i < opre.len() && opre[i] <= cbuf[j]) {
                    i += 1;
                    opre[i - 1]
                } else {
                    j += 1;
                    cbuf[j - 1]
                };
                if buf.last() != Some(&x) {
                    buf.push(x);
                }
            }
            b_pre.extend_from_slice(&buf);
            b_pre_off.push(b_pre.len() as u32);
            b_eff.push(prop(e.var, e.post));
            b_base.push(adj[o]);
            b_op.push(o as u32);
        }
    }
    (b_eff, b_base, b_pre_off, b_pre, b_op)
}

/// `simplify_keep` per effect: a unary operator's keep flag reads only the unary operators with its effect (both rules
/// look up keys with its effect), so the operators are split by effect into `threads` shards, each run alone with the
/// same rules on its operators (in increasing number), and the flags put back.
pub fn simplify_keep_sharded(pre_off: &[u32], pre: &[u32], eff: &[u32], base: &[i32], n_props: usize, threads: usize) -> Vec<bool> {
    let nb = eff.len();
    let k = threads.max(1);
    let mut shards: Vec<Vec<u32>> = vec![Vec::new(); k];
    for u in 0..nb {
        shards[eff[u] as usize % k].push(u as u32);
    }
    let parts: Vec<Vec<bool>> = crate::par::map_dyn(k, k, || (), |_, s| {
        let us = &shards[s];
        // the shard as its own arrays
        let mut p_off = vec![0u32];
        let mut p = Vec::new();
        let mut e = Vec::with_capacity(us.len());
        let mut b = Vec::with_capacity(us.len());
        for &u in us {
            let u = u as usize;
            p.extend_from_slice(&pre[pre_off[u] as usize..pre_off[u + 1] as usize]);
            p_off.push(p.len() as u32);
            e.push(eff[u]);
            b.push(base[u]);
        }
        simplify_keep_ids(&p_off, &p, &e, &b, n_props, us)
    });
    let mut keep = vec![false; nb];
    for (s, part) in parts.into_iter().enumerate() {
        for (j, &u) in shards[s].iter().enumerate() {
            keep[u as usize] = part[j];
        }
    }
    keep
}

/// FD's `simplify` (module docstring) as a keep flag per unary operator: the reference, FD's map with whole keys.
pub fn simplify_keep_reference(pre_off: &[u32], pre: &[u32], eff: &[u32], base: &[i32]) -> Vec<bool> {
    let nb = eff.len();
    let pre_of = |u: usize| &pre[pre_off[u] as usize..pre_off[u + 1] as usize];
    let mut map: HashMap<SKey, (i32, u32), FxBuild> = HashMap::with_capacity_and_hasher(nb, FxBuild::default());
    for u in 0..nb {
        let key: SKey = (pre_of(u).into(), eff[u]);
        let val = (base[u], u as u32);
        map.entry(key).and_modify(|old| if val < *old { *old = val }).or_insert(val);
    }
    let mut keep = vec![true; nb];
    let mut probe: Vec<u32> = Vec::with_capacity(MAX_PRECONDITIONS_TO_TEST);
    for u in 0..nb {
        let p = pre_of(u);
        let own = map.get(&(p.into(), eff[u]) as &SKey).map(|v| v.1);
        if own != Some(u as u32) {
            keep[u] = false;
            continue;
        }
        if p.len() > MAX_PRECONDITIONS_TO_TEST {
            continue;
        }
        for mask in 0..(1u32 << p.len()) - 1 {
            probe.clear();
            probe.extend(p.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, &x)| x));
            if let Some(&(c, _)) = map.get(&(probe.as_slice().into(), eff[u]) as &SKey) {
                if c <= base[u] {
                    keep[u] = false;
                    break;
                }
            }
        }
    }
    keep
}

#[inline]
fn skey_hash(pre: &[u32], eff: u32) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ (pre.len() as u64);
    for &x in pre {
        h = (h.rotate_left(5) ^ x as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    h = (h.rotate_left(5) ^ eff as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    h ^ (h >> 29)
}

/// FD's `simplify` as a keep flag per unary operator, the same answer as `simplify_keep_reference` (the test, and
/// TASKCORE_CHECK_RELAX on any task): an open-addressing table of unary operator numbers keyed by their (preconditions,
/// effect) slices, so no key is allocated; and a strict subset is looked up only when its size reaches the least
/// precondition count among the unary operators with that effect (a smaller key is in no one's map entry).
pub fn simplify_keep(pre_off: &[u32], pre: &[u32], eff: &[u32], base: &[i32], n_props: usize) -> Vec<bool> {
    const EMPTY: u32 = u32::MAX;
    let nb = eff.len();
    let pre_of = |u: usize| &pre[pre_off[u] as usize..pre_off[u + 1] as usize];
    let cap = (2 * nb.max(1)).next_power_of_two();
    let mask = cap - 1;
    let mut table = vec![EMPTY; cap];
    let mut minlen = vec![u32::MAX; n_props];
    // the map: per key its least (cost, number); `slot_of[u]`: where u's key lives (so its own lookup is a read)
    let mut slot_of = vec![0u32; nb];
    for u in 0..nb {
        let p = pre_of(u);
        let e = eff[u];
        minlen[e as usize] = minlen[e as usize].min(p.len() as u32);
        let mut slot = skey_hash(p, e) as usize & mask;
        loop {
            let x = table[slot];
            if x == EMPTY {
                table[slot] = u as u32;
                break;
            }
            let xu = x as usize;
            if eff[xu] == e && pre_of(xu) == p {
                if (base[u], u as u32) < (base[xu], x) {
                    table[slot] = u as u32;
                }
                break;
            }
            slot = (slot + 1) & mask;
        }
        slot_of[u] = slot as u32;
    }
    let find = |p: &[u32], e: u32| -> u32 {
        let mut slot = skey_hash(p, e) as usize & mask;
        loop {
            let x = table[slot];
            if x == EMPTY {
                return EMPTY;
            }
            if eff[x as usize] == e && pre_of(x as usize) == p {
                return x;
            }
            slot = (slot + 1) & mask;
        }
    };
    let t_ins = std::time::Instant::now();
    let mut probes = 0u64;
    let mut keep = vec![true; nb];
    let mut probe = [0u32; MAX_PRECONDITIONS_TO_TEST];
    for u in 0..nb {
        let p = pre_of(u);
        let e = eff[u];
        if table[slot_of[u] as usize] != u as u32 {
            keep[u] = false;
            continue;
        }
        let n = p.len();
        if n > MAX_PRECONDITIONS_TO_TEST || n as u32 <= minlen[e as usize] {
            continue;
        }
        let least = minlen[e as usize];
        for m in 0..(1u32 << n) - 1 {
            if m.count_ones() < least {
                continue;
            }
            let mut k = 0;
            for (i, &x) in p.iter().enumerate() {
                if m & (1 << i) != 0 {
                    probe[k] = x;
                    k += 1;
                }
            }
            probes += 1;
            let d = find(&probe[..k], e);
            if d != EMPTY && base[d as usize] <= base[u] {
                keep[u] = false;
                break;
            }
        }
    }
    if std::env::var_os("TASKCORE_PROFILE").is_some() {
        eprintln!("profile simplify: the dominance pass {:.3} s, {} subset probes", t_ins.elapsed().as_secs_f64(), probes);
    }
    keep
}

/// `simplify_keep` on a shard of unary operators (`simplify_keep_sharded`): an exact duplicate keeps the cheapest, then
/// the lowest number, with each operator's number `ids[j]` (the whole task's), the rest as `simplify_keep`.
fn simplify_keep_ids(pre_off: &[u32], pre: &[u32], eff: &[u32], base: &[i32], n_props: usize, ids: &[u32]) -> Vec<bool> {
    const EMPTY: u32 = u32::MAX;
    let nb = eff.len();
    let pre_of = |u: usize| &pre[pre_off[u] as usize..pre_off[u + 1] as usize];
    let cap = (2 * nb.max(1)).next_power_of_two();
    let mask = cap - 1;
    let mut table = vec![EMPTY; cap];
    let mut minlen = vec![u32::MAX; n_props];
    // the map: per key its least (cost, number); `slot_of[u]`: where u's key lives (so its own lookup is a read)
    let mut slot_of = vec![0u32; nb];
    for u in 0..nb {
        let p = pre_of(u);
        let e = eff[u];
        minlen[e as usize] = minlen[e as usize].min(p.len() as u32);
        let mut slot = skey_hash(p, e) as usize & mask;
        loop {
            let x = table[slot];
            if x == EMPTY {
                table[slot] = u as u32;
                break;
            }
            let xu = x as usize;
            if eff[xu] == e && pre_of(xu) == p {
                if (base[u], ids[u]) < (base[xu], ids[xu]) {
                    table[slot] = u as u32;
                }
                break;
            }
            slot = (slot + 1) & mask;
        }
        slot_of[u] = slot as u32;
    }
    let find = |p: &[u32], e: u32| -> u32 {
        let mut slot = skey_hash(p, e) as usize & mask;
        loop {
            let x = table[slot];
            if x == EMPTY {
                return EMPTY;
            }
            if eff[x as usize] == e && pre_of(x as usize) == p {
                return x;
            }
            slot = (slot + 1) & mask;
        }
    };
    let t_ins = std::time::Instant::now();
    let mut probes = 0u64;
    let mut keep = vec![true; nb];
    let mut probe = [0u32; MAX_PRECONDITIONS_TO_TEST];
    for u in 0..nb {
        let p = pre_of(u);
        let e = eff[u];
        if table[slot_of[u] as usize] != u as u32 {
            keep[u] = false;
            continue;
        }
        let n = p.len();
        if n > MAX_PRECONDITIONS_TO_TEST || n as u32 <= minlen[e as usize] {
            continue;
        }
        let least = minlen[e as usize];
        for m in 0..(1u32 << n) - 1 {
            if m.count_ones() < least {
                continue;
            }
            let mut k = 0;
            for (i, &x) in p.iter().enumerate() {
                if m & (1 << i) != 0 {
                    probe[k] = x;
                    k += 1;
                }
            }
            probes += 1;
            let d = find(&probe[..k], e);
            if d != EMPTY && base[d as usize] <= base[u] {
                keep[u] = false;
                break;
            }
        }
    }
    let _ = (t_ins, probes);
    keep
}

impl Ff {
    /// The relaxed task of `t` with the adapted costs `adj` (one per operator).
    pub fn new(t: &SasTask, adj: &[i32]) -> Ff {
        Ff::new_threads(t, adj, 1)
    }

    /// `new` with the unary operators built in chunks of operators and `simplify` run per effect (`threads` > 1, the fast
    /// form): the same unary operators in the same order (each chunk's, in operator order), and the same keep flags
    /// (`simplify_keep_sharded`).
    pub fn new_threads(t: &SasTask, adj: &[i32], threads: usize) -> Ff {
        let t_new = Instant::now();
        let mut prop_off = Vec::with_capacity(t.sizes.len());
        let mut n = 0u32;
        for &s in &t.sizes {
            prop_off.push(n);
            n += s;
        }
        let n_props = n as usize;
        let prop = |v: u32, x: u32| prop_off[v as usize] + x;
        let mut is_goal = vec![false; n_props];
        let mut goal_props = Vec::with_capacity(t.goal.len());
        for &(v, x) in &t.goal {
            let p = prop(v, x);
            is_goal[p as usize] = true;
            goal_props.push(p);
        }
        // build_unary_operators
        let t_bu = Instant::now();
        let (b_eff, b_base, b_pre_off, b_pre, b_op) = if threads > 1 {
            let parts = crate::par::map_chunks(t.n_ops(), threads * 4, || (), |_, r| unary_ops(t, adj, &prop_off, r));
            let mut all = (Vec::new(), Vec::new(), vec![0u32], Vec::new(), Vec::new());
            for (e, b, off, pre, op) in parts {
                let base = all.3.len() as u32;
                all.0.extend_from_slice(&e);
                all.1.extend_from_slice(&b);
                all.2.extend(off[1..].iter().map(|&x| x + base));
                all.3.extend_from_slice(&pre);
                all.4.extend_from_slice(&op);
            }
            all
        } else {
            unary_ops(t, adj, &prop_off, 0..t.n_ops())
        };
        let nb = b_eff.len();
        let prof = std::env::var_os("TASKCORE_PROFILE").is_some();
        if prof {
            eprintln!("profile relax: build_unary_operators {:.3} s", t_bu.elapsed().as_secs_f64());
        }
        let tb = Instant::now();
        let keep = if std::env::var_os("TASKCORE_CHECK_RELAX").is_some() {
            let a = if threads > 1 { simplify_keep_sharded(&b_pre_off, &b_pre, &b_eff, &b_base, n_props, threads) } else { simplify_keep(&b_pre_off, &b_pre, &b_eff, &b_base, n_props) };
            let b = simplify_keep_reference(&b_pre_off, &b_pre, &b_eff, &b_base);
            assert!(a == b, "simplify: the fast table differs from the reference");
            eprintln!("simplify: the fast table equals the reference ({} of {} kept)", a.iter().filter(|&&k| k).count(), a.len());
            a
        } else if threads > 1 {
            simplify_keep_sharded(&b_pre_off, &b_pre, &b_eff, &b_base, n_props, threads)
        } else {
            simplify_keep(&b_pre_off, &b_pre, &b_eff, &b_base, n_props)
        };
        Ff::finish_new(t, adj, t_new, tb, prof, prop_off, goal_props, is_goal, nb, keep, &b_eff, &b_base, &b_pre_off, &b_pre, &b_op)
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_new(
        t: &SasTask,
        adj: &[i32],
        t_new: Instant,
        tb: Instant,
        prof: bool,
        prop_off: Vec<u32>,
        goal_props: Vec<u32>,
        is_goal: Vec<bool>,
        nb: usize,
        keep: Vec<bool>,
        b_eff: &[u32],
        b_base: &[i32],
        b_pre_off: &[u32],
        b_pre: &[u32],
        b_op: &[u32],
    ) -> Ff {
        let n_props = is_goal.len();
        let pre_of = |u: usize| &b_pre[b_pre_off[u] as usize..b_pre_off[u + 1] as usize];
        if prof {
            eprintln!("profile relax: {} unary built, simplify {:.3} s", nb, tb.elapsed().as_secs_f64());
        }
        let mut u_eff = Vec::new();
        let mut u_base = Vec::new();
        let mut u_pre_off = vec![0u32];
        let mut u_pre = Vec::new();
        let mut u_op = Vec::new();
        for u in 0..nb {
            if keep[u] {
                u_eff.push(b_eff[u]);
                u_base.push(b_base[u]);
                u_pre.extend_from_slice(pre_of(u));
                u_pre_off.push(u_pre.len() as u32);
                u_op.push(b_op[u]);
            }
        }
        Ff::assemble(t, adj, t_new, prop_off, goal_props, is_goal, n_props, nb, u_eff, u_base, u_pre_off, u_pre, u_op)
    }

    #[allow(clippy::too_many_arguments)]
    fn assemble(
        t: &SasTask,
        adj: &[i32],
        t_new: Instant,
        prop_off: Vec<u32>,
        goal_props: Vec<u32>,
        is_goal: Vec<bool>,
        n_props: usize,
        nb: usize,
        u_eff: Vec<u32>,
        u_base: Vec<i32>,
        u_pre_off: Vec<u32>,
        u_pre: Vec<u32>,
        u_op: Vec<u32>,
    ) -> Ff {
        // cross-reference: per proposition the unary operators it is a precondition of, in their order
        let nu = u_eff.len();
        let mut cnt = vec![0u32; n_props + 1];
        for &p in &u_pre {
            cnt[p as usize + 1] += 1;
        }
        for i in 0..n_props {
            cnt[i + 1] += cnt[i];
        }
        let prec_of_off = cnt.clone();
        let mut fill = cnt;
        let mut prec_of = vec![0u32; u_pre.len()];
        for u in 0..nu {
            for &p in &u_pre[u_pre_off[u] as usize..u_pre_off[u + 1] as usize] {
                prec_of[fill[p as usize] as usize] = u as u32;
                fill[p as usize] += 1;
            }
        }
        if std::env::var_os("TASKCORE_PROFILE").is_some() {
            eprintln!("profile relax: {} unary kept, the relaxed task in {:.3} s", nu, t_new.elapsed().as_secs_f64());
        }
        Ff {
            prop_off,
            goal_props,
            is_goal,
            u_eff,
            u_base,
            u_pre_off,
            u_pre,
            u_op,
            prec_of_off,
            prec_of,
            op_cost: adj.to_vec(),
            cost: vec![-1; n_props],
            reached_by: vec![NO_OP; n_props],
            marked: vec![false; n_props],
            u_cost: vec![0; nu],
            u_unsat: vec![0; nu],
            queue: AQueue::new(),
            relaxed_plan: vec![false; t.n_ops()],
            plan_ops: Vec::new(),
            stack: Vec::new(),
            preferred: Vec::new(),
            pref_mark: vec![false; t.n_ops()],
            n_unary_built: nb,
            evaluations: 0,
            eval_s: 0.0,
            eval_cpu_s: 0.0,
            eval_minstr: 0.0,
            eval_mcyc: 0.0,
            mode: HffMode::Fd,
            watch: None,
            check: std::env::var_os("TASKCORE_CHECK_HFF").is_some(),
            prof: std::env::var_os("TASKCORE_PROFILE_HFF").is_some(),
            log: Vec::new(),
            watch_policy: match std::env::var("TASKCORE_HFF_REBUILD") {
                Ok(v) => {
                    let mut it = v.split(':').map(|x| x.parse::<u64>().unwrap_or(0));
                    (it.next().unwrap_or(64), it.next().unwrap_or(20_000))
                }
                Err(_) => (64, 20_000),
            },
        }
    }

    /// The relaxed task of `t`, its exploration in `mode`.
    pub fn with_mode(t: &SasTask, adj: &[i32], mode: HffMode) -> Ff {
        let mut ff = if mode == HffMode::Watch { Ff::new_threads(t, adj, crate::par::threads()) } else { Ff::new(t, adj) };
        ff.mode = mode;
        ff
    }

    /// The watch structure's numbers (builds, the last evaluation's moves and fires), in mode `Watch`.
    pub fn watch_counts(&self) -> Option<(u64, u64, u64)> {
        self.watch.as_ref().map(|w| (w.builds, w.moves, w.fires))
    }

    pub fn n_unary(&self) -> usize {
        self.u_eff.len()
    }

    #[inline]
    fn enqueue(&mut self, p: u32, cost: i32, op: u32) {
        let c = &mut self.cost[p as usize];
        if *c == -1 || *c > cost {
            *c = cost;
            if self.check {
                self.log.push((cost, p, op));
            }
            self.reached_by[p as usize] = op;
            self.queue.push(cost, p);
        }
    }

    /// h_FF of `s` (None: a dead end), with `preferred` set (ff_heuristic.cc `compute_heuristic`).
    pub fn eval(&mut self, s: &[u32]) -> Option<i64> {
        let c0 = thread_cpu_s();
        let k0 = crate::perf::now();
        let r = self.eval_in(s);
        self.eval_cpu_s += thread_cpu_s() - c0;
        let (i, c) = crate::perf::since(k0);
        self.eval_minstr += i;
        self.eval_mcyc += c;
        r
    }

    fn eval_in(&mut self, s: &[u32]) -> Option<i64> {
        let t0 = Instant::now();
        let c_in = if self.prof { thread_cpu_s() } else { 0.0 };
        let k_in = if self.prof { crate::perf::now() } else { (0, 0) };
        self.evaluations += 1;
        for p in &self.preferred {
            self.pref_mark[*p as usize] = false;
        }
        self.preferred.clear();
        match self.mode {
            HffMode::Fd => self.explore_fd(s),
            HffMode::Watch => {
                let mut w = match self.watch.take() {
                    Some(w) => w,
                    None => {
                        // the first evaluation is FD's; the watches are built from its costs (watch.rs)
                        self.explore_fd(s);
                        let tb = thread_cpu_s();
                        let rank: Vec<u32> = self.cost.iter().map(|&c| if c < 0 { u32::MAX } else { c as u32 }).collect();
                        let w = Box::new(watch::Watch::new(self, &rank, self.watch_policy));
                        if self.prof {
                            eprintln!("profile hff eval 1 (FD's): {:.3} ms, watches built in {:.3} ms", (tb - c_in) * 1e3, (thread_cpu_s() - tb) * 1e3);
                        }
                        self.watch = Some(w);
                        return self.after_exploration(t0);
                    }
                };
                if self.check {
                    // both from the queue as the evaluation found it (a bucket queue that turns into a heap during FD's
                    // run must start as a bucket queue again)
                    let q0 = self.queue.clone();
                    self.log.clear();
                    self.explore_fd(s);
                    let fdlog = std::mem::take(&mut self.log);
                    let (cost, by, queue) = (self.cost.clone(), self.reached_by.clone(), self.queue.clone());
                    self.queue = q0;
                    self.explore_watch(&mut w, s);
                    if fdlog != self.log {
                        let i = (0..fdlog.len().min(self.log.len())).find(|&i| fdlog[i] != self.log[i]).unwrap_or(fdlog.len().min(self.log.len()));
                        panic!(
                            "h_FF by watches: event {} of {} (FD) / {} differs at evaluation {}: FD {:?}, watches {:?} (cost, proposition, operator; cost -1: a pop)",
                            i,
                            fdlog.len(),
                            self.log.len(),
                            self.evaluations,
                            fdlog.get(i),
                            self.log.get(i)
                        );
                    }
                    if cost != self.cost {
                        let d: Vec<(usize, i32, i32)> = (0..cost.len()).filter(|&i| cost[i] != self.cost[i]).map(|i| (i, cost[i], self.cost[i])).take(10).collect();
                        panic!("h_FF by watches: the costs differ from FD's exploration at evaluation {}: (prop, FD, watch) {:?}; goals {:?}", self.evaluations, d, self.goal_props);
                    }
                    assert!(by == self.reached_by, "h_FF by watches: reached_by differs from FD's exploration");
                    assert!(queue == self.queue, "h_FF by watches: the queue differs from FD's exploration");
                } else {
                    self.explore_watch(&mut w, s);
                }
                let (b0, tb) = (w.builds, thread_cpu_s());
                w.maybe_rebuild(self);
                if self.prof {
                    eprintln!(
                        "profile hff eval {}: {:.3} ms, {:.2} M instructions, looks {}, moves {}, fires {}, segments {} live {}{}",
                        self.evaluations,
                        (tb - c_in) * 1e3,
                        crate::perf::since(k_in).0,
                        w.looks,
                        w.moves,
                        w.fires,
                        w.segs.0,
                        w.segs.1,
                        if w.builds > b0 { format!(", rebuilt in {:.3} ms", (thread_cpu_s() - tb) * 1e3) } else { String::new() }
                    );
                }
                self.watch = Some(w);
            }
        }
        self.after_exploration(t0)
    }

    fn after_exploration(&mut self, t0: Instant) -> Option<i64> {
        if self.goal_props.iter().any(|&g| self.cost[g as usize] == -1) {
            self.eval_s += t0.elapsed().as_secs_f64();
            return None;
        }
        self.relaxed_plan_h(t0)
    }

    /// FD's `setup_exploration_queue`, `setup_exploration_queue_state` and `relaxed_exploration`, transcribed.
    fn explore_fd(&mut self, s: &[u32]) {
        // setup_exploration_queue
        self.queue.clear();
        self.cost.iter_mut().for_each(|c| *c = -1);
        self.marked.iter_mut().for_each(|m| *m = false);
        for u in 0..self.u_eff.len() {
            let np = self.u_pre_off[u + 1] - self.u_pre_off[u];
            self.u_unsat[u] = np;
            self.u_cost[u] = self.u_base[u];
            if np == 0 {
                self.enqueue(self.u_eff[u], self.u_base[u], u as u32);
            }
        }
        // setup_exploration_queue_state
        for (v, &x) in s.iter().enumerate() {
            self.enqueue(self.prop_off[v] + x, 0, NO_OP);
        }
        // relaxed_exploration
        let mut unsolved = self.goal_props.len();
        while !self.queue.empty() {
            let (d, p) = self.queue.pop();
            let pc = self.cost[p as usize];
            if pc < d {
                continue;
            }
            if self.check {
                self.log.push((-1, p, d as u32));
            }
            if self.is_goal[p as usize] {
                unsolved -= 1;
                if unsolved == 0 {
                    break;
                }
            }
            let (a, b) = (self.prec_of_off[p as usize] as usize, self.prec_of_off[p as usize + 1] as usize);
            for i in a..b {
                let u = self.prec_of[i] as usize;
                let nc = self.u_cost[u] + pc;
                self.u_cost[u] = if nc > MAX_COST_VALUE { MAX_COST_VALUE } else { nc };
                self.u_unsat[u] -= 1;
                if self.u_unsat[u] == 0 {
                    let (e, c) = (self.u_eff[u], self.u_cost[u]);
                    self.enqueue(e, c, u as u32);
                }
            }
        }
    }

    /// The relaxed plan and the preferred operators from the goals back (`compute_heuristic`'s rest): h_FF.
    fn relaxed_plan_h(&mut self, t0: Instant) -> Option<i64> {
        // mark_preferred_operators_and_relaxed_plan from each goal (the sets it makes do not depend on the order)
        self.plan_ops.clear();
        for gi in 0..self.goal_props.len() {
            let g = self.goal_props[gi];
            self.stack.push((g, false));
            while let Some((p, done)) = self.stack.pop() {
                let pu = p as usize;
                if !done {
                    if self.marked[pu] {
                        continue;
                    }
                    self.marked[pu] = true;
                    let u = self.reached_by[pu];
                    if u == NO_OP {
                        continue;
                    }
                    self.stack.push((p, true));
                    let (a, b) = (self.u_pre_off[u as usize] as usize, self.u_pre_off[u as usize + 1] as usize);
                    for i in (a..b).rev() {
                        self.stack.push((self.u_pre[i], false));
                    }
                } else {
                    let u = self.reached_by[pu] as usize;
                    let (a, b) = (self.u_pre_off[u] as usize, self.u_pre_off[u + 1] as usize);
                    let pref = self.u_pre[a..b].iter().all(|&q| self.reached_by[q as usize] == NO_OP);
                    let o = self.u_op[u];
                    if !self.relaxed_plan[o as usize] {
                        self.relaxed_plan[o as usize] = true;
                        self.plan_ops.push(o);
                    }
                    if pref && !self.pref_mark[o as usize] {
                        self.pref_mark[o as usize] = true;
                        self.preferred.push(o);
                    }
                }
            }
        }
        let mut h = 0i64;
        for &o in &self.plan_ops {
            self.relaxed_plan[o as usize] = false;
            h += self.op_cost[o as usize] as i64;
        }
        self.eval_s += t0.elapsed().as_secs_f64();
        Some(h)
    }

    pub fn is_preferred(&self, o: u32) -> bool {
        self.pref_mark[o as usize]
    }
}

// --- the lazy search --------------------------------------------------------------------------------------------------

/// The search's answer, as FD's exit codes read in plan_fd ("plan" 0, "unsolvable" 11, "time" 23).
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub status: String,
    pub plan: Vec<u32>,
    pub cost: i64,
    pub expanded: u64,
    pub evaluated: u64,
    pub generated: u64,
    pub evaluations: u64,
    pub h0: Option<i64>,
    pub relax_s: f64,
    pub search_s: f64,
    pub eval_s: f64,
    pub eval_cpu_s: f64,
    pub eval_minstr: f64,
    pub eval_mcyc: f64,
    pub unary: usize,
    pub unary_built: usize,
    /// mode `Watch`: the watch structure's builds
    pub watch_builds: u64,
}

/// FD's successor order (module docstring): the operators sorted by their sorted precondition list, then number.
pub fn successor_order(t: &SasTask) -> Vec<u32> {
    let mut sorted: Vec<Vec<(u32, u32)>> = (0..t.n_ops())
        .map(|o| {
            let mut p = t.pre(o).to_vec();
            p.sort_unstable();
            p
        })
        .collect();
    let mut ids: Vec<u32> = (0..t.n_ops() as u32).collect();
    ids.sort_by(|&a, &b| sorted[a as usize].cmp(&sorted[b as usize]).then(a.cmp(&b)));
    sorted.clear();
    ids
}

/// FD's successor order by first precondition (mode `Watch`): `order` is sorted by the operators' sorted precondition
/// lists, so the operators with no precondition come first, then those whose first (least) precondition is (v, x), in
/// (v, x) order, each group contiguous. An operator applies in s only if its first precondition holds, so the groups
/// (v, s[v]) for v in increasing order, after the empty group, hold every applicable operator in `order`'s order.
pub struct SuccIndex {
    empty: Vec<u32>,
    /// per proposition (prop_off[v] + x): its group's range in `ops`
    off: Vec<u32>,
    ops: Vec<u32>,
    prop_off: Vec<u32>,
}

impl SuccIndex {
    pub fn new(t: &SasTask, order: &[u32]) -> SuccIndex {
        let mut prop_off = Vec::with_capacity(t.sizes.len());
        let mut n = 0u32;
        for &s in &t.sizes {
            prop_off.push(n);
            n += s;
        }
        let first = |o: u32| t.pre(o as usize).iter().min().map(|&(v, x)| prop_off[v as usize] + x);
        let mut cnt = vec![0u32; n as usize + 1];
        let mut empty = Vec::new();
        for &o in order {
            match first(o) {
                None => empty.push(o),
                Some(p) => cnt[p as usize + 1] += 1,
            }
        }
        for i in 0..n as usize {
            cnt[i + 1] += cnt[i];
        }
        let off = cnt.clone();
        let mut ops = vec![0u32; cnt[n as usize] as usize];
        for &o in order {
            if let Some(p) = first(o) {
                ops[cnt[p as usize] as usize] = o;
                cnt[p as usize] += 1;
            }
        }
        SuccIndex { empty, off, ops, prop_off }
    }

    /// The operators of `order` that apply in `s`, in `order`'s order.
    pub fn applicable(&self, t: &SasTask, s: &[u32], f: &mut impl FnMut(u32)) {
        for &o in &self.empty {
            f(o);
        }
        for (v, &x) in s.iter().enumerate() {
            let p = (self.prop_off[v] + x) as usize;
            for &o in &self.ops[self.off[p] as usize..self.off[p + 1] as usize] {
                if t.applicable(o as usize, s) {
                    f(o);
                }
            }
        }
    }
}

/// `successor_order`, the same order: each operator's sorted preconditions in one array, the operators sorted by them
/// (then by number), with no list allocated per operator.
pub fn successor_order_flat(t: &SasTask) -> Vec<u32> {
    let n = t.n_ops();
    let mut flat: Vec<(u32, u32)> = Vec::with_capacity(t.pre.len());
    let mut off: Vec<u32> = Vec::with_capacity(n + 1);
    off.push(0);
    for o in 0..n {
        let a = flat.len();
        flat.extend_from_slice(t.pre(o));
        flat[a..].sort_unstable();
        off.push(flat.len() as u32);
    }
    let mut ids: Vec<u32> = (0..n as u32).collect();
    let key = |o: u32| &flat[off[o as usize] as usize..off[o as usize + 1] as usize];
    ids.sort_by(|&a, &b| key(a).cmp(key(b)).then(a.cmp(&b)));
    ids
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeStatus {
    New,
    Closed,
    Dead,
}

/// `lazy_greedy([hff], preferred=[hff], reopen_closed=false)` from the task's start (module docstring); `deadline`:
/// stop with status "time" past it.
pub fn lazy_greedy(t: &SasTask, deadline: Option<Instant>) -> Outcome {
    lazy_greedy_mode(t, deadline, HffMode::Fd)
}

/// `lazy_greedy` with h_FF's exploration in `mode` (the same plans and counts in every mode).
pub fn lazy_greedy_mode(t: &SasTask, deadline: Option<Instant>, mode: HffMode) -> Outcome {
    let t0 = Instant::now();
    let adj = plusone(t);
    let mut ff = Ff::with_mode(t, &adj, mode);
    let order = if mode == HffMode::Watch { successor_order_flat(t) } else { successor_order(t) };
    let relax_s = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let mut out = lazy_greedy_with(t, &mut ff, &order, deadline);
    out.relax_s = relax_s;
    out.search_s = t1.elapsed().as_secs_f64();
    out.unary = ff.n_unary();
    out.unary_built = ff.n_unary_built;
    out
}

/// The search with a built heuristic and successor order (`lazy_greedy`).
pub fn lazy_greedy_with(t: &SasTask, ff: &mut Ff, order: &[u32], deadline: Option<Instant>) -> Outcome {
    let nv = t.sizes.len();
    let index = if ff.mode == HffMode::Watch { Some(SuccIndex::new(t, order)) } else { None };
    let ev0 = ff.evaluations;
    let es0 = ff.eval_s;
    let ec0 = ff.eval_cpu_s;
    let (ei0, ey0) = (ff.eval_minstr, ff.eval_mcyc);
    // the state registry: states by id in one arena; their ids by content
    let mut arena: Vec<u32> = Vec::with_capacity(nv * 64);
    let mut ids: HashMap<Box<[u32]>, u32, FxBuild> = HashMap::default();
    let mut status: Vec<NodeStatus> = Vec::new();
    let mut parent: Vec<(u32, u32)> = Vec::new();
    // TASKCORE_TRACE: FD's progress lines (search_progress.cc, search_statistics.cc), for the comparison of long searches
    let trace = std::env::var_os("TASKCORE_TRACE").is_some();
    let mut g_of: HashMap<u32, i64> = HashMap::new();
    let register = |s: &[u32], arena: &mut Vec<u32>, ids: &mut HashMap<Box<[u32]>, u32, FxBuild>, status: &mut Vec<NodeStatus>, parent: &mut Vec<(u32, u32)>| -> u32 {
        if let Some(&id) = ids.get(s) {
            return id;
        }
        let id = status.len() as u32;
        arena.extend_from_slice(s);
        ids.insert(s.into(), id);
        status.push(NodeStatus::New);
        parent.push((u32::MAX, u32::MAX));
        id
    };
    // the alternation open list: [h, h preferred only], each h -> FIFO of (parent state, operator)
    let mut lists: [BTreeMap<i64, VecDeque<(u32, u32)>>; 2] = [BTreeMap::new(), BTreeMap::new()];
    let mut sizes = [0usize; 2];
    let mut prio = [0i64; 2];
    let mut best: Option<i64> = None;
    let mut out = Outcome::default();
    let mut cur = register(&t.init, &mut arena, &mut ids, &mut status, &mut parent);
    let mut cur_pred: Option<(u32, u32)> = None;
    let mut buf: Vec<u32> = Vec::with_capacity(nv);
    let mut state: Vec<u32> = Vec::with_capacity(nv);
    loop {
        if let Some(d) = deadline {
            if Instant::now() > d {
                out.status = "time".into();
                break;
            }
        }
        if status[cur as usize] == NodeStatus::New {
            state.clear();
            state.extend_from_slice(&arena[cur as usize * nv..(cur as usize + 1) * nv]);
            out.evaluated += 1;
            match ff.eval(&state) {
                None => status[cur as usize] = NodeStatus::Dead,
                Some(h) => {
                    if let Some(p) = cur_pred {
                        parent[cur as usize] = p;
                    }
                    status[cur as usize] = NodeStatus::Closed;
                    let g = if trace {
                        let g = cur_pred.map_or(0, |(p, o)| g_of[&p] + t.cost[o as usize] as i64);
                        g_of.insert(cur, g);
                        g
                    } else {
                        0
                    };
                    if cur_pred.is_none() {
                        out.h0 = Some(h);
                        best = Some(h);
                        if trace {
                            eprintln!("New best heuristic value for ff: {}", h);
                            eprintln!("g={}, {} evaluated, {} expanded", g, out.evaluated, out.expanded);
                            eprintln!("Initial heuristic value for ff: {}", h);
                        }
                    }
                    if t.is_goal(&state) {
                        let mut plan = Vec::new();
                        let mut x = cur;
                        while parent[x as usize].0 != u32::MAX {
                            plan.push(parent[x as usize].1);
                            x = parent[x as usize].0;
                        }
                        plan.reverse();
                        out.cost = plan.iter().map(|&o| t.cost[o as usize] as i64).sum();
                        out.plan = plan;
                        out.status = "plan".into();
                        break;
                    }
                    if cur_pred.is_some() && best.is_none_or(|b| h < b) {
                        best = Some(h);
                        prio[1] -= 1000; // boost_preferred: the preferred-only list
                        if trace {
                            eprintln!("New best heuristic value for ff: {}", h);
                            eprintln!("g={}, {} evaluated, {} expanded", g, out.evaluated, out.expanded);
                        }
                    }
                    // generate_successors
                    let mut gen = |o: u32| {
                        out.generated += 1;
                        lists[0].entry(h).or_default().push_back((cur, o));
                        sizes[0] += 1;
                        if ff.is_preferred(o) {
                            lists[1].entry(h).or_default().push_back((cur, o));
                            sizes[1] += 1;
                        }
                    };
                    match &index {
                        Some(ix) => ix.applicable(t, &state, &mut gen),
                        None => {
                            for &o in order {
                                if t.applicable(o as usize, &state) {
                                    gen(o);
                                }
                            }
                        }
                    }
                    out.expanded += 1;
                }
            }
        }
        // fetch_next_state
        let k = match (sizes[0] > 0, sizes[1] > 0) {
            (false, false) => {
                // get_finished_search_status: the alternation is safe (its h list is: h_FF is safe), so an empty open
                // list proves no plan exists (FD's exit code 11)
                out.status = "unsolvable".into();
                break;
            }
            (true, false) => 0,
            (false, true) => 1,
            (true, true) => {
                if prio[1] < prio[0] {
                    1
                } else {
                    0
                }
            }
        };
        prio[k] += 1;
        let (p, o) = {
            let mut e = lists[k].first_entry().expect("a non-empty list");
            let v = e.get_mut().pop_front().expect("a non-empty bucket");
            if e.get().is_empty() {
                e.remove();
            }
            v
        };
        sizes[k] -= 1;
        let ps = arena[p as usize * nv..(p as usize + 1) * nv].to_vec();
        t.apply_into(o as usize, &ps, &mut buf);
        cur = register(&buf, &mut arena, &mut ids, &mut status, &mut parent);
        cur_pred = Some((p, o));
    }
    out.evaluations = ff.evaluations - ev0;
    out.eval_s = ff.eval_s - es0;
    out.eval_cpu_s = ff.eval_cpu_s - ec0;
    out.eval_minstr = ff.eval_minstr - ei0;
    out.eval_mcyc = ff.eval_mcyc - ey0;
    out.watch_builds = ff.watch_counts().map_or(0, |w| w.0);
    out
}

/// The plan file FD writes (plan_manager.cc `save_plan`).
pub fn plan_text(t: &SasTask, plan: &[u32]) -> String {
    let mut s = String::new();
    for &o in plan {
        s.push_str(&format!("(a{})\n", o));
    }
    let cost: i64 = plan.iter().map(|&o| t.cost[o as usize] as i64).sum();
    let unit = t.cost.iter().all(|&c| c == 1);
    s.push_str(&format!("; cost = {} ({})\n", cost, if unit { "unit cost" } else { "general cost" }));
    s
}

/// Whether `plan` is a plan of `t` from its start: each operator applicable in turn, the goal at the end (the check
/// every returned plan passes).
pub fn check_plan(t: &SasTask, plan: &[u32]) -> Result<(), String> {
    let mut s = t.init.clone();
    let mut n = Vec::with_capacity(s.len());
    for (i, &o) in plan.iter().enumerate() {
        if o as usize >= t.n_ops() {
            return Err(format!("step {}: no operator a{}", i + 1, o));
        }
        if !t.applicable(o as usize, &s) {
            return Err(format!("step {}: a{} does not apply", i + 1, o));
        }
        t.apply_into(o as usize, &s, &mut n);
        std::mem::swap(&mut s, &mut n);
    }
    if !t.is_goal(&s) {
        return Err("the goal does not hold after the last step".into());
    }
    Ok(())
}

/// A plan file's operator numbers (`(aN)` lines).
pub fn read_plan(text: &str) -> Result<Vec<u32>, String> {
    let mut ops = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix('(') {
            let name = rest.trim_end_matches(')').split_whitespace().next().unwrap_or("");
            ops.push(name.trim_start_matches('a').parse().map_err(|_| format!("plan line {:?}", l))?);
        }
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The heap is libstdc++'s: pops come out in key order, ties in the order its sift rules give.
    #[test]
    fn heap_orders_by_key() {
        let mut q = AQueue::Heap(Vec::new());
        let keys = [5, 3, 9, 3, 1, 7, 1, 2, 8, 3];
        for (i, &k) in keys.iter().enumerate() {
            q.push(k, i as u32);
        }
        let mut last = -1;
        let mut n = 0;
        while !q.empty() {
            let (k, _) = q.pop();
            assert!(k >= last);
            last = k;
            n += 1;
        }
        assert_eq!(n, keys.len());
    }

    #[test]
    fn bucket_is_lifo_and_converts() {
        let mut q = AQueue::new();
        q.push(2, 10);
        q.push(2, 11);
        q.push(0, 12);
        assert_eq!(q.pop(), (0, 12));
        assert_eq!(q.pop(), (2, 11));
        q.push(150, 13); // key >= 100 and > pushes (4): a heap from here on
        assert!(matches!(q, AQueue::Heap(_)));
        assert_eq!(q.pop(), (2, 10));
        assert_eq!(q.pop(), (150, 13));
        q.clear();
        assert!(matches!(q, AQueue::Heap(_)));
    }

    #[test]
    fn tiny_task() {
        // two variables: door (0/1), room (1/2); open the door in room 1, walk through it; goal room 2
        let text = "begin_version\n3\nend_version\nbegin_metric\n0\nend_metric\n2\nbegin_variable\ndoor\n-1\n2\nAtom d(0)\nAtom d(1)\nend_variable\nbegin_variable\nroom\n-1\n2\nAtom r(1)\nAtom r(2)\nend_variable\n0\nbegin_state\n0\n0\nend_state\nbegin_goal\n1\n1 1\nend_goal\n2\nbegin_operator\na0\n1\n1 0\n1\n0 0 -1 1\n1\nend_operator\nbegin_operator\na1\n2\n0 1\n1 0\n1\n0 1 -1 1\n1\nend_operator\n0\n";
        let t = parse_sas(text).unwrap();
        let o = lazy_greedy(&t, None);
        assert_eq!(o.status, "plan");
        assert_eq!(o.plan, vec![0, 1]);
        assert_eq!(plan_text(&t, &o.plan), "(a0)\n(a1)\n; cost = 2 (unit cost)\n");
        check_plan(&t, &o.plan).unwrap();
        let w = lazy_greedy_mode(&t, None, HffMode::Watch);
        assert_eq!((w.status, w.plan), (o.status, o.plan));
    }

    /// A random SAS task: `nv` variables of 2-4 values, `no` operators of 0-3 preconditions and 1-3 effects (some
    /// conditional), costs 0-299 (so h_FF's queue turns into a heap) or all 1.
    fn random_task(seed: u64, nv: usize, no: usize, unit: bool) -> SasTask {
        let mut r = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut next = move |n: u64| {
            r ^= r << 13;
            r ^= r >> 7;
            r ^= r << 17;
            r % n.max(1)
        };
        let mut t = SasTask { metric: !unit, pre_off: vec![0], eff_off: vec![0], ..Default::default() };
        for v in 0..nv {
            t.sizes.push(2 + next(3) as u32);
            t.names.push(format!("v{}", v));
        }
        t.init = t.sizes.iter().map(|&s| next(s as u64) as u32).collect();
        let g = next(nv as u64) as u32;
        t.goal = vec![(g, (t.init[g as usize] + 1) % t.sizes[g as usize])];
        for _ in 0..no {
            let mut pre: Vec<(u32, u32)> = Vec::new();
            for _ in 0..next(4) {
                let v = next(nv as u64) as u32;
                if pre.iter().all(|p| p.0 != v) {
                    pre.push((v, next(t.sizes[v as usize] as u64) as u32));
                }
            }
            let mut effs: Vec<EffSpec> = Vec::new();
            for _ in 0..1 + next(3) {
                let v = next(nv as u64) as u32;
                if effs.iter().any(|e| e.1 == v) {
                    continue;
                }
                let mut c = Vec::new();
                if next(3) == 0 {
                    let cv = next(nv as u64) as u32;
                    c.push((cv, next(t.sizes[cv as usize] as u64) as u32));
                }
                effs.push((c, v, next(t.sizes[v as usize] as u64) as u32));
            }
            let cost = if unit { 1 } else { next(300) as u32 };
            t.push_op(&pre, &effs, cost);
        }
        t
    }

    /// h_FF by watches makes FD's exploration, event for event, on random tasks (the check on in every evaluation), so
    /// the searches give the same outcomes.
    #[test]
    fn watch_equals_fd_on_random_tasks() {
        let mut plans = 0;
        for seed in 0..300u64 {
            let unit = seed % 5 == 0;
            let t = random_task(seed, 4 + (seed % 9) as usize, 10 + (seed * 7 % 150) as usize, unit);
            let adj = plusone(&t);
            let order = successor_order(&t);
            let mut a = Ff::with_mode(&t, &adj, HffMode::Fd);
            let fd = lazy_greedy_with(&t, &mut a, &order, None);
            let mut b = Ff::with_mode(&t, &adj, HffMode::Watch);
            b.check = true;
            if seed % 2 == 1 {
                // the watches built again after every evaluation
                b.watch_policy = (1, 0);
            }
            let w = lazy_greedy_with(&t, &mut b, &order, None);
            assert_eq!((&fd.status, &fd.plan, fd.expanded, fd.evaluated, fd.generated), (&w.status, &w.plan, w.expanded, w.evaluated, w.generated), "seed {}", seed);
            assert_eq!(successor_order_flat(&t), order, "seed {}", seed);
            for th in [2, 3] {
                let c = Ff::new_threads(&t, &adj, th);
                assert_eq!((&c.u_eff, &c.u_base, &c.u_pre_off, &c.u_pre, &c.u_op, c.n_unary_built), (&a.u_eff, &a.u_base, &a.u_pre_off, &a.u_pre, &a.u_op, a.n_unary_built), "seed {}", seed);
            }
            if fd.status == "plan" {
                plans += 1;
            }
        }
        assert!(plans > 50, "{} of 300 random tasks have a plan", plans);
    }
}
