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
    fn push_op(&mut self, pre: &[(u32, u32)], effs: &[(Vec<(u32, u32)>, u32, u32)], cost: u32) {
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
    let mut effs: Vec<(Vec<(u32, u32)>, u32, u32)> = Vec::new();
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

// --- the adapted costs ----------------------------------------------------------------------------------------------

/// operator_cost.cc PLUSONE on the task: cost + 1, or 1 for every operator when every operator costs 1.
pub fn plusone(t: &SasTask) -> Vec<i32> {
    let unit = t.cost.iter().all(|&c| c == 1);
    t.cost.iter().map(|&c| if unit { 1 } else { c as i32 + 1 }).collect()
}

// --- FD's AdaptiveQueue ---------------------------------------------------------------------------------------------

const MIN_BUCKETS_BEFORE_SWITCH: i32 = 100;

/// algorithms/priority_queues.h `AdaptiveQueue<PropID>`: a bucket queue that turns into a heap (and stays one).
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
}

/// A small key for `simplify`'s map: the sorted preconditions (at most `MAX_PRECONDITIONS_TO_TEST + ..` are hashed
/// whole: any length) and the effect.
type SKey = (Box<[u32]>, u32);

impl Ff {
    /// The relaxed task of `t` with the adapted costs `adj` (one per operator).
    pub fn new(t: &SasTask, adj: &[i32]) -> Ff {
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
        let mut b_eff = Vec::with_capacity(t.effs.len());
        let mut b_base = Vec::with_capacity(t.effs.len());
        let mut b_pre_off = Vec::with_capacity(t.effs.len() + 1);
        b_pre_off.push(0u32);
        let mut b_pre: Vec<u32> = Vec::with_capacity(t.effs.len() * 4);
        let mut b_op = Vec::with_capacity(t.effs.len());
        let mut buf: Vec<u32> = Vec::new();
        for o in 0..t.n_ops() {
            let opre: Vec<u32> = t.pre(o).iter().map(|&(v, x)| prop(v, x)).collect();
            for e in t.effs(o) {
                buf.clear();
                buf.extend_from_slice(&opre);
                buf.extend(t.conds(e).iter().map(|&(v, x)| prop(v, x)));
                buf.sort_unstable();
                buf.dedup();
                b_pre.extend_from_slice(&buf);
                b_pre_off.push(b_pre.len() as u32);
                b_eff.push(prop(e.var, e.post));
                b_base.push(adj[o]);
                b_op.push(o as u32);
            }
        }
        let nb = b_eff.len();
        let pre_of = |u: usize| &b_pre[b_pre_off[u] as usize..b_pre_off[u + 1] as usize];
        // simplify: the map from (preconditions, effect) to the least (cost, number)
        let mut map: HashMap<SKey, (i32, u32), FxBuild> = HashMap::with_capacity_and_hasher(nb, FxBuild::default());
        for u in 0..nb {
            let key: SKey = (pre_of(u).into(), b_eff[u]);
            let val = (b_base[u], u as u32);
            map.entry(key).and_modify(|old| if val < *old { *old = val }).or_insert(val);
        }
        let mut keep = vec![true; nb];
        let mut probe: Vec<u32> = Vec::with_capacity(MAX_PRECONDITIONS_TO_TEST);
        for u in 0..nb {
            let pre = pre_of(u);
            let eff = b_eff[u];
            let cost = b_base[u];
            let own = map.get(&(pre.into(), eff) as &SKey).map(|v| v.1);
            if own != Some(u as u32) {
                keep[u] = false;
                continue;
            }
            if pre.len() > MAX_PRECONDITIONS_TO_TEST {
                continue;
            }
            let full = (1u32 << pre.len()) - 1;
            for mask in 0..full {
                probe.clear();
                for (i, &p) in pre.iter().enumerate() {
                    if mask & (1 << i) != 0 {
                        probe.push(p);
                    }
                }
                let key: SKey = (probe.as_slice().into(), eff);
                if let Some(&(c, _)) = map.get(&key) {
                    if c <= cost {
                        keep[u] = false;
                        break;
                    }
                }
            }
        }
        drop(map);
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
        }
    }

    pub fn n_unary(&self) -> usize {
        self.u_eff.len()
    }

    #[inline]
    fn enqueue(&mut self, p: u32, cost: i32, op: u32) {
        let c = &mut self.cost[p as usize];
        if *c == -1 || *c > cost {
            *c = cost;
            self.reached_by[p as usize] = op;
            self.queue.push(cost, p);
        }
    }

    /// h_FF of `s` (None: a dead end), with `preferred` set (ff_heuristic.cc `compute_heuristic`).
    pub fn eval(&mut self, s: &[u32]) -> Option<i64> {
        let t0 = Instant::now();
        self.evaluations += 1;
        for p in &self.preferred {
            self.pref_mark[*p as usize] = false;
        }
        self.preferred.clear();
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
        if self.goal_props.iter().any(|&g| self.cost[g as usize] == -1) {
            self.eval_s += t0.elapsed().as_secs_f64();
            return None;
        }
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

/// The search's answer, as FD's exit codes read in plan_fd ("plan" 0, "incomplete" 12, "time" 23).
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
    pub unary: usize,
    pub unary_built: usize,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeStatus {
    New,
    Closed,
    Dead,
}

/// `lazy_greedy([hff], preferred=[hff], reopen_closed=false)` from the task's start (module docstring); `deadline`:
/// stop with status "time" past it.
pub fn lazy_greedy(t: &SasTask, deadline: Option<Instant>) -> Outcome {
    let t0 = Instant::now();
    let adj = plusone(t);
    let mut ff = Ff::new(t, &adj);
    let order = successor_order(t);
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
    let ev0 = ff.evaluations;
    let es0 = ff.eval_s;
    // the state registry: states by id in one arena; their ids by content
    let mut arena: Vec<u32> = Vec::with_capacity(nv * 64);
    let mut ids: HashMap<Box<[u32]>, u32, FxBuild> = HashMap::default();
    let mut status: Vec<NodeStatus> = Vec::new();
    let mut parent: Vec<(u32, u32)> = Vec::new();
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
                    if cur_pred.is_none() {
                        out.h0 = Some(h);
                        best = Some(h);
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
                    if cur_pred.is_some() && best.map_or(true, |b| h < b) {
                        best = Some(h);
                        prio[1] -= 1000; // boost_preferred: the preferred-only list
                    }
                    // generate_successors
                    for &o in order {
                        if !t.applicable(o as usize, &state) {
                            continue;
                        }
                        out.generated += 1;
                        lists[0].entry(h).or_default().push_back((cur, o));
                        sizes[0] += 1;
                        if ff.is_preferred(o) {
                            lists[1].entry(h).or_default().push_back((cur, o));
                            sizes[1] += 1;
                        }
                    }
                    out.expanded += 1;
                }
            }
        }
        // fetch_next_state
        let k = match (sizes[0] > 0, sizes[1] > 0) {
            (false, false) => {
                out.status = "incomplete".into();
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
    }
}
