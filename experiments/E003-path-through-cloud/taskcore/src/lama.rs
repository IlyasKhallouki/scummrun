//! Fast Downward's lama-first-costs in Rust (E011 route): the search plan_fd.LAMA_FIRST_COSTS runs,
//!
//! ```text
//! let(hlm, eval_modify_costs(landmark_sum(lm_factory=lm_reasonable_orders_hps(lm_rhw()), pref=false), cost_type=plusone),
//! let(hff, eval_modify_costs(ff(), cost_type=plusone),
//! lazy_greedy([hff, hlm], preferred=[hff, hlm], reopen_closed=false)))
//! ```
//!
//! rule for rule as release 26.6.0 runs it, on the SAS task held in memory (search.rs `SasTask`), so that it returns
//! Fast Downward's plan byte for byte (search.rs `plan_text`) with the same expanded, evaluated and generated counts.
//!
//! What is Fast Downward's here, and must stay so:
//!
//! * **The landmark graph** (landmarks.rs), made on the cost-adapted task: `eval_modify_costs` binds the nested
//!   evaluator to `CostAdaptedTask(plusone)`, so landmark costs are least adapted achiever costs.
//! * **The landmark status** (landmark_status_manager.cc): at the start a landmark is past when it holds, future when
//!   it does not or when a parent of it does not hold; at a transition (made once per state, when the lazy search
//!   first takes it) a landmark future in the parent stays future unless it was added now, a goal that does not hold
//!   is future, the parent of a greedy-necessary ordering whose child is not past is future while it does not hold,
//!   and the child of a reasonable ordering whose parent is not past is future.
//! * **landmark_sum** (landmark_sum_heuristic.cc): the sum over future landmarks of the least cost of a first achiever
//!   (not past) or of a possible achiever (past); a future landmark with no achiever, or a cycle of natural (or
//!   stronger) orderings in the graph, is a dead end. No preferred operators (pref=false).
//! * **h_FF** (search.rs `Ff`, Fast Downward's ff() with plusone costs, its preferred operators).
//! * **The open list** (search_common.cc, alternation_open_list.cc, best_first_open_list.cc): the alternation of
//!   [hff, hff preferred only, hlm, hlm preferred only], boost 1000; each a map of value to FIFO; a successor goes in
//!   under its parent's values, into the preferred lists when its operator is one of the parent's h_FF preferred
//!   operators; `remove_min` takes the non-empty list of lowest priority (the first on a tie) and adds 1 to it.
//! * **The lazy search** (lazy_search.cc): a taken state, when new, gets the landmark transition, then h_FF, then (when
//!   h_FF is finite: both evaluators are safe, so an infinite value is a reliable dead end) landmark_sum; the goal is
//!   tested after the evaluation; a new lowest value of either evaluator (the start's are the first) boosts both
//!   preferred lists by 1000; then the successors, in Fast Downward's successor generator order (search.rs
//!   `successor_order`: here a trie over the sorted preconditions that yields the applicable operators in that order).
//! * **No plan**: an empty open list is "unsolvable" (the alternation is safe: its h_FF list is).

use crate::landmarks::{self, Graph, OrderingType};
use crate::search::{self, Ff, SasTask};
use crate::stdhash::Fact;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::Instant;

/// An h_FF evaluator as the search reads it: search.rs `Ff`, or any evaluator with the same values and preferred
/// operators.
pub trait RelaxedEval {
    /// h_FF of a state (None: a dead end), its preferred operators kept until the next evaluation
    fn eval_state(&mut self, s: &[u32]) -> Option<i64>;
    fn preferred_op(&self, o: u32) -> bool;
}

impl RelaxedEval for Ff {
    fn eval_state(&mut self, s: &[u32]) -> Option<i64> {
        self.eval(s)
    }
    fn preferred_op(&self, o: u32) -> bool {
        self.is_preferred(o)
    }
}

// --- landmark_sum with its status per state -----------------------------------------------------------------------

/// The landmark count's tables (landmark_status_manager.cc, landmark_sum_heuristic.cc) over a graph.
pub struct LmSum {
    n: usize,
    words: usize,
    atoms: Vec<Vec<Fact>>,
    disjunctive: Vec<bool>,
    goal_ids: Vec<u32>,
    gn_children: Vec<(u32, Vec<u32>)>,
    reasonable_parents: Vec<(u32, Vec<u32>)>,
    parents: Vec<Vec<u32>>,
    min_first: Vec<i64>,
    min_possible: Vec<i64>,
    /// the graph has a cycle of natural orderings: every state a dead end
    pub cycle: bool,
    /// per registered state: past bits then future bits
    bits: Vec<u64>,
}

impl LmSum {
    pub fn new(g: &Graph) -> LmSum {
        let n = g.len();
        let words = n.div_ceil(64).max(1);
        let goal_ids = (0..n as u32).filter(|&i| g.goal[i as usize]).collect();
        let mut gn_children = Vec::new();
        let mut reasonable_parents = Vec::new();
        for i in 0..n {
            let c: Vec<u32> = g.children[i].iter().filter(|(_, t)| *t == OrderingType::GreedyNecessary).map(|(c, _)| *c).collect();
            if !c.is_empty() {
                gn_children.push((i as u32, c));
            }
            let p: Vec<u32> = g.parents[i].iter().filter(|(_, t)| *t == OrderingType::Reasonable).map(|(p, _)| *p).collect();
            if !p.is_empty() {
                reasonable_parents.push((i as u32, p));
            }
        }
        LmSum {
            n,
            words,
            atoms: g.atoms.clone(),
            disjunctive: g.disjunctive.clone(),
            goal_ids,
            gn_children,
            reasonable_parents,
            parents: g.parents.iter().map(|p| p.iter().map(|x| x.0).collect()).collect(),
            min_first: g.min_first.clone(),
            min_possible: g.min_possible.clone(),
            cycle: natural_cycle(g),
            bits: Vec::new(),
        }
    }

    #[inline]
    fn holds(&self, id: usize, s: &[u32]) -> bool {
        let tr = |f: &Fact| s[f.0 as usize] == f.1;
        if self.disjunctive[id] {
            self.atoms[id].iter().any(tr)
        } else {
            self.atoms[id].iter().all(tr)
        }
    }

    /// The per-state bits of state `id`, made with the defaults (past all, future none) when new.
    fn ensure(&mut self, id: u32) {
        let need = (id as usize + 1) * 2 * self.words;
        while self.bits.len() < need {
            for w in 0..self.words {
                let lo = w * 64;
                let m = if lo + 64 <= self.n { u64::MAX } else if lo >= self.n { 0 } else { (1u64 << (self.n - lo)) - 1 };
                self.bits.push(m);
            }
            self.bits.extend(std::iter::repeat_n(0, self.words));
        }
    }

    #[inline]
    fn get(&self, id: u32, future: bool, l: usize) -> bool {
        let base = id as usize * 2 * self.words + if future { self.words } else { 0 };
        self.bits[base + l / 64] >> (l % 64) & 1 == 1
    }
    #[inline]
    fn set(&mut self, id: u32, future: bool, l: usize, v: bool) {
        let base = id as usize * 2 * self.words + if future { self.words } else { 0 };
        let w = &mut self.bits[base + l / 64];
        if v {
            *w |= 1 << (l % 64);
        } else {
            *w &= !(1 << (l % 64));
        }
    }

    /// progress_initial_state
    pub fn progress_initial(&mut self, id: u32, s: &[u32]) {
        self.ensure(id);
        for l in 0..self.n {
            if self.holds(l, s) {
                if self.parents[l].iter().any(|&p| !self.holds(p as usize, s)) {
                    self.set(id, true, l, true);
                }
            } else {
                self.set(id, false, l, false);
                self.set(id, true, l, true);
            }
        }
    }

    /// progress(parent, op, state): `id`'s bits are new (the defaults), the parent's made.
    pub fn progress(&mut self, pid: u32, ps: &[u32], id: u32, s: &[u32]) {
        self.ensure(id.max(pid));
        if ps == s {
            return;
        }
        // progress_landmarks
        for l in 0..self.n {
            if self.get(pid, true, l) {
                if !self.holds(l, s) {
                    self.set(id, true, l, true);
                    if !self.get(pid, false, l) {
                        self.set(id, false, l, false);
                    }
                } else if self.holds(l, ps) {
                    self.set(id, true, l, true);
                }
            }
        }
        // progress_goals
        for i in 0..self.goal_ids.len() {
            let g = self.goal_ids[i] as usize;
            if !self.holds(g, s) {
                self.set(id, true, g, true);
            }
        }
        // progress_greedy_necessary_orderings
        for i in 0..self.gn_children.len() {
            let tail = self.gn_children[i].0 as usize;
            let tail_holds = self.holds(tail, s);
            for j in 0..self.gn_children[i].1.len() {
                let c = self.gn_children[i].1[j] as usize;
                if !self.get(id, false, c) && !tail_holds {
                    self.set(id, true, tail, true);
                    break;
                }
            }
        }
        // progress_reasonable_orderings
        for i in 0..self.reasonable_parents.len() {
            let head = self.reasonable_parents[i].0 as usize;
            for j in 0..self.reasonable_parents[i].1.len() {
                let p = self.reasonable_parents[i].1[j] as usize;
                if !self.get(id, false, p) {
                    self.set(id, true, head, true);
                    break;
                }
            }
        }
    }

    /// get_heuristic_value of state `id` (None: a dead end)
    pub fn value(&self, id: u32) -> Option<i64> {
        if self.cycle {
            return None;
        }
        let mut h = 0i64;
        for l in 0..self.n {
            if self.get(id, true, l) {
                let c = if self.get(id, false, l) { self.min_possible[l] } else { self.min_first[l] };
                if c == i64::MAX {
                    return None;
                }
                h += c;
            }
        }
        Some(h)
    }
}

/// landmark_graph_has_cycle_of_natural_orderings (orderings natural or stronger)
fn natural_cycle(g: &Graph) -> bool {
    let n = g.len();
    let mut closed = vec![false; n];
    let mut visited = vec![false; n];
    fn dfs(g: &Graph, v: usize, closed: &mut [bool], visited: &mut [bool]) -> bool {
        if closed[v] {
            return false;
        }
        if visited[v] {
            return true;
        }
        visited[v] = true;
        for &(c, t) in &g.children[v] {
            if t >= OrderingType::Natural && dfs(g, c as usize, closed, visited) {
                return true;
            }
        }
        closed[v] = true;
        false
    }
    (0..n).any(|v| dfs(g, v, &mut closed, &mut visited))
}

// --- Fast Downward's successor order, as a trie ----------------------------------------------------------------

/// The applicable operators of a state in search.rs `successor_order` (Fast Downward's successor generator): a trie
/// over the operators' sorted precondition lists, a node's own operators before its children, children by (var,
/// value); only the child with the state's value of each variable is entered.
pub struct Succ {
    /// per trie node: its operators, then its child groups (per variable: the children by value)
    ops_off: Vec<u32>,
    ops: Vec<u32>,
    grp_off: Vec<u32>,
    /// per group: (variable, first value, children by value from it (u32::MAX: none))
    grp_var: Vec<u32>,
    grp_lo: Vec<u32>,
    grp_kids_off: Vec<u32>,
    grp_kids: Vec<u32>,
}

impl Succ {
    pub fn new(t: &SasTask) -> Succ {
        // the operators' sorted precondition lists in one buffer, the operators sorted by them, then by number
        // (search.rs `successor_order`)
        let n = t.n_ops();
        let mut off: Vec<u32> = Vec::with_capacity(n + 1);
        off.push(0);
        let mut flat: Vec<(u32, u32)> = Vec::with_capacity(t.pre.len());
        for o in 0..n {
            let a = flat.len();
            flat.extend_from_slice(t.pre(o));
            flat[a..].sort_unstable();
            off.push(flat.len() as u32);
        }
        let list = |o: u32| &flat[off[o as usize] as usize..off[o as usize + 1] as usize];
        let mut order: Vec<u32> = (0..n as u32).collect();
        order.sort_unstable_by(|&a, &b| list(a).cmp(list(b)).then(a.cmp(&b)));
        let mut s = Succ {
            ops_off: vec![0],
            ops: Vec::with_capacity(n),
            grp_off: vec![0],
            grp_var: Vec::new(),
            grp_lo: Vec::new(),
            grp_kids_off: vec![0],
            grp_kids: Vec::new(),
        };
        // the trie, node by node in creation order (a node's children get numbers when it is made): a work list of
        // (node, range of `order`, depth); children of a node are its ranges sharing the next (var, value)
        struct Work {
            node: u32,
            lo: usize,
            hi: usize,
            d: usize,
        }
        let mut nodes_made = 1u32;
        let mut work: VecDeque<Work> = VecDeque::new();
        work.push_back(Work { node: 0, lo: 0, hi: n, d: 0 });
        let mut groups: Vec<(u32, Vec<(u32, u32)>)> = Vec::new();
        while let Some(w) = work.pop_front() {
            debug_assert_eq!(w.node as usize, s.ops_off.len() - 1);
            let mut i = w.lo;
            while i < w.hi && list(order[i]).len() == w.d {
                s.ops.push(order[i]);
                i += 1;
            }
            s.ops_off.push(s.ops.len() as u32);
            groups.clear();
            while i < w.hi {
                let key = list(order[i])[w.d];
                let mut j = i;
                while j < w.hi && list(order[j])[w.d] == key {
                    j += 1;
                }
                let child = nodes_made;
                nodes_made += 1;
                work.push_back(Work { node: child, lo: i, hi: j, d: w.d + 1 });
                if groups.last().is_none_or(|g| g.0 != key.0) {
                    groups.push((key.0, Vec::new()));
                }
                groups.last_mut().unwrap().1.push((key.1, child));
                i = j;
            }
            for (var, kids) in &groups {
                let lo = kids.first().unwrap().0;
                let hi = kids.last().unwrap().0;
                let base = s.grp_kids.len();
                s.grp_kids.extend(std::iter::repeat_n(u32::MAX, (hi - lo + 1) as usize));
                for &(val, c) in kids {
                    s.grp_kids[base + (val - lo) as usize] = c;
                }
                s.grp_var.push(*var);
                s.grp_lo.push(lo);
                s.grp_kids_off.push(s.grp_kids.len() as u32);
            }
            s.grp_off.push(s.grp_var.len() as u32);
        }
        s
    }

    /// The applicable operators of `state`, in order, into `out`.
    pub fn applicable(&self, state: &[u32], out: &mut Vec<u32>, stack: &mut Vec<u32>) {
        out.clear();
        stack.clear();
        stack.push(0);
        // a depth-first walk: children are pushed in reverse so the first is taken first
        while let Some(n) = stack.pop() {
            let n = n as usize;
            out.extend_from_slice(&self.ops[self.ops_off[n] as usize..self.ops_off[n + 1] as usize]);
            let (g0, g1) = (self.grp_off[n] as usize, self.grp_off[n + 1] as usize);
            for g in (g0..g1).rev() {
                let v = state[self.grp_var[g] as usize];
                let lo = self.grp_lo[g];
                if v < lo {
                    continue;
                }
                let k = self.grp_kids_off[g] as usize + (v - lo) as usize;
                if k < self.grp_kids_off[g + 1] as usize && self.grp_kids[k] != u32::MAX {
                    stack.push(self.grp_kids[k]);
                }
            }
        }
    }
}

// --- the lazy search --------------------------------------------------------------------------------------------

/// The search's answer, as Fast Downward's exit codes read in plan_fd ("plan" 0, "unsolvable" 11, "time" 23).
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub status: String,
    pub plan: Vec<u32>,
    pub cost: i64,
    pub expanded: u64,
    pub evaluated: u64,
    pub generated: u64,
    pub dead_ends: u64,
    pub h0: Option<(i64, i64)>,
    pub landmarks: usize,
    pub disjunctive: usize,
    pub orderings: usize,
    pub explorations: u64,
    pub graph_s: f64,
    pub relax_s: f64,
    pub search_s: f64,
    pub eval_s: f64,
    pub lm_s: f64,
}

/// The predicate of each variable's atoms for an in-memory task (its names are the keys taskcore writes).
pub fn predicates_of(t: &SasTask) -> Vec<String> {
    t.names.iter().map(|k| landmarks::predicate_of_key(k)).collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    New,
    Closed,
    Dead,
}

/// lama-first-costs on `t` from its start (module docstring); `pred` the predicate of each variable's atoms
/// (`predicates_of` for a task taskcore made); past `deadline` the status is "time".
pub fn lama_first_costs(t: &SasTask, pred: &[String], deadline: Option<Instant>) -> Outcome {
    lama_first_costs_with(t, pred, deadline, Ff::new)
}

/// `lama_first_costs` with the h_FF evaluator `make_ff(task, adapted costs)` makes (search.rs `Ff::new`, or another
/// with the same values and preferred operators).
pub fn lama_first_costs_with<H: RelaxedEval>(
    t: &SasTask,
    pred: &[String],
    deadline: Option<Instant>,
    make_ff: impl FnOnce(&SasTask, &[i32]) -> H,
) -> Outcome {
    let t0 = Instant::now();
    let adj = search::plusone(t);
    let g = match landmarks::build(t, &adj, pred, deadline) {
        Ok(g) => g,
        Err(e) if e == "time" => return Outcome { status: "time".into(), ..Default::default() },
        Err(e) => return Outcome { status: format!("error: {}", e), ..Default::default() },
    };
    let graph_s = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let mut ff = make_ff(t, &adj);
    let ff_s = t1.elapsed().as_secs_f64();
    let succ = Succ::new(t);
    let relax_s = t1.elapsed().as_secs_f64();
    if std::env::var_os("TASKCORE_PROFILE").is_some() {
        eprintln!(
            "profile lama: graph {:.4} s (setup {:.4} rhw {:.4} achievers {:.4} hps {:.4}), h_FF's relaxed task {:.4} s, successor trie {:.4} s",
            graph_s,
            g.stats.setup_s,
            g.stats.rhw_s,
            g.stats.achievers_s,
            g.stats.hps_s,
            ff_s,
            relax_s - ff_s
        );
    }
    let t2 = Instant::now();
    let mut lm = LmSum::new(&g);
    let mut out = search_with(t, &mut ff, &mut lm, &succ, deadline);
    out.search_s = t2.elapsed().as_secs_f64();
    out.graph_s = graph_s;
    out.relax_s = relax_s;
    out.landmarks = g.len();
    out.disjunctive = g.n_disjunctive;
    out.orderings = g.n_orderings;
    out.explorations = g.stats.explorations;
    out
}

/// One open list of the alternation: value -> FIFO of (parent state, operator).
#[derive(Default)]
struct Bf {
    buckets: BTreeMap<i64, VecDeque<(u32, u32)>>,
    size: usize,
}

impl Bf {
    fn push(&mut self, k: i64, e: (u32, u32)) {
        self.buckets.entry(k).or_default().push_back(e);
        self.size += 1;
    }
    fn pop(&mut self) -> (u32, u32) {
        let mut b = self.buckets.first_entry().expect("a non-empty list");
        let v = b.get_mut().pop_front().expect("a non-empty bucket");
        if b.get().is_empty() {
            b.remove();
        }
        self.size -= 1;
        v
    }
}

/// The lazy search with built evaluators (`lama_first_costs`).
pub fn search_with<H: RelaxedEval>(t: &SasTask, ff: &mut H, lm: &mut LmSum, succ: &Succ, deadline: Option<Instant>) -> Outcome {
    let nv = t.sizes.len();
    let trace = std::env::var_os("TASKCORE_TRACE").is_some();
    let mut arena: Vec<u32> = Vec::with_capacity(nv * 1024);
    let mut ids: HashMap<Box<[u32]>, u32> = HashMap::new();
    let mut status: Vec<Status> = Vec::new();
    let mut parent: Vec<(u32, u32)> = Vec::new();
    let mut g_of: Vec<i64> = Vec::new();
    let mut register = |s: &[u32], arena: &mut Vec<u32>, status: &mut Vec<Status>, parent: &mut Vec<(u32, u32)>| -> u32 {
        if let Some(&id) = ids.get(s) {
            return id;
        }
        let id = status.len() as u32;
        arena.extend_from_slice(s);
        ids.insert(s.into(), id);
        status.push(Status::New);
        parent.push((u32::MAX, u32::MAX));
        id
    };
    // [hff, hff preferred only, hlm, hlm preferred only]
    let mut lists: [Bf; 4] = Default::default();
    let mut prio = [0i64; 4];
    let mut best: [Option<i64>; 2] = [None, None];
    let mut out = Outcome::default();
    let mut cur = register(&t.init, &mut arena, &mut status, &mut parent);
    lm.progress_initial(cur, &t.init);
    let mut cur_pred: Option<(u32, u32)> = None;
    let mut buf: Vec<u32> = Vec::with_capacity(nv);
    let mut state: Vec<u32> = Vec::with_capacity(nv);
    let mut pstate: Vec<u32> = Vec::with_capacity(nv);
    let mut app: Vec<u32> = Vec::new();
    let mut stack: Vec<u32> = Vec::new();
    let check = std::env::var_os("TASKCORE_CHECK_SUCC").is_some();
    let order = if check { search::successor_order(t) } else { Vec::new() };
    loop {
        if deadline.is_some_and(|d| Instant::now() > d) {
            out.status = "time".into();
            break;
        }
        if status[cur as usize] == Status::New {
            state.clear();
            state.extend_from_slice(&arena[cur as usize * nv..(cur as usize + 1) * nv]);
            if let Some((p, _)) = cur_pred {
                pstate.clear();
                pstate.extend_from_slice(&arena[p as usize * nv..(p as usize + 1) * nv]);
                let tl = Instant::now();
                lm.progress(p, &pstate, cur, &state);
                out.lm_s += tl.elapsed().as_secs_f64();
            }
            out.evaluated += 1;
            let te = Instant::now();
            let hff = ff.eval_state(&state);
            out.eval_s += te.elapsed().as_secs_f64();
            let tl = Instant::now();
            let hlm = if hff.is_some() { lm.value(cur) } else { None };
            out.lm_s += tl.elapsed().as_secs_f64();
            match (hff, hlm) {
                (Some(hf), Some(hl)) => {
                    if let Some(p) = cur_pred {
                        parent[cur as usize] = p;
                    }
                    status[cur as usize] = Status::Closed;
                    let g = if trace {
                        let g = cur_pred.map_or(0, |(p, o)| g_of[p as usize] + t.cost[o as usize] as i64);
                        if g_of.len() <= cur as usize {
                            g_of.resize(cur as usize + 1, 0);
                        }
                        g_of[cur as usize] = g;
                        g
                    } else {
                        0
                    };
                    if cur_pred.is_none() {
                        out.h0 = Some((hf, hl));
                        best = [Some(hf), Some(hl)];
                        if trace {
                            eprintln!("New best heuristic value for ff: {}", hf);
                            eprintln!("New best heuristic value for landmark_sum_heuristic: {}", hl);
                            eprintln!("g={}, {} evaluated, {} expanded", g, out.evaluated, out.expanded);
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
                    if cur_pred.is_some() {
                        // check_progress: each evaluator's new lowest value; any one boosts
                        let mut boost = false;
                        for (k, v) in [(0usize, hf), (1, hl)] {
                            if best[k].is_none_or(|b| v < b) {
                                best[k] = Some(v);
                                boost = true;
                                if trace {
                                    let name = if k == 0 { "ff" } else { "landmark_sum_heuristic" };
                                    eprintln!("New best heuristic value for {}: {}", name, v);
                                }
                            }
                        }
                        if boost {
                            if trace {
                                eprintln!("g={}, {} evaluated, {} expanded", g, out.evaluated, out.expanded);
                            }
                            prio[1] -= 1000;
                            prio[3] -= 1000;
                        }
                    }
                    // generate_successors
                    succ.applicable(&state, &mut app, &mut stack);
                    if check {
                        let lin: Vec<u32> = order.iter().copied().filter(|&o| t.applicable(o as usize, &state)).collect();
                        assert_eq!(lin, app, "the successor trie's order differs from the linear scan");
                    }
                    for &o in &app {
                        out.generated += 1;
                        let pref = ff.preferred_op(o);
                        lists[0].push(hf, (cur, o));
                        if pref {
                            lists[1].push(hf, (cur, o));
                        }
                        lists[2].push(hl, (cur, o));
                        if pref {
                            lists[3].push(hl, (cur, o));
                        }
                    }
                    out.expanded += 1;
                }
                _ => {
                    status[cur as usize] = Status::Dead;
                    out.dead_ends += 1;
                }
            }
        }
        // fetch_next_state
        let mut k = usize::MAX;
        for (i, l) in lists.iter().enumerate() {
            if l.size > 0 && (k == usize::MAX || prio[i] < prio[k]) {
                k = i;
            }
        }
        if k == usize::MAX {
            out.status = "unsolvable".into();
            break;
        }
        prio[k] += 1;
        let (p, o) = lists[k].pop();
        pstate.clear();
        pstate.extend_from_slice(&arena[p as usize * nv..(p as usize + 1) * nv]);
        t.apply_into(o as usize, &pstate, &mut buf);
        cur = register(&buf, &mut arena, &mut status, &mut parent);
        cur_pred = Some((p, o));
    }
    out
}

/// A SAS file's task, the predicate of each variable's atoms and the value names.
pub type Parsed = (SasTask, Vec<String>, Vec<Vec<String>>);

/// A SAS file's text read for lama-first-costs: the task and the predicates of its atoms ("Atom pred(..)" names, as
/// Fast Downward reads them: landmark_factory_rpg_sasp.cc `get_predicate_for_atom`).
pub fn parse_with_predicates(text: &str) -> Result<Parsed, String> {
    let t = search::parse_sas(text)?;
    // the value names, variable by variable
    let mut names: Vec<Vec<String>> = Vec::with_capacity(t.sizes.len());
    let mut lines = text.lines();
    while let Some(l) = lines.next() {
        if l == "begin_variable" {
            let _name = lines.next();
            let _axiom = lines.next();
            let n: usize = lines.next().ok_or("a variable's size")?.trim().parse().map_err(|_| "a variable's size")?;
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(lines.next().ok_or("a value name")?.to_string());
            }
            names.push(v);
        }
    }
    let mut pred = Vec::with_capacity(names.len());
    for vals in &names {
        let mut p: Option<String> = None;
        for name in vals {
            let q = if name == "<none of those>" {
                String::new()
            } else {
                let pos = if let Some(r) = name.strip_prefix("Atom ") {
                    Some((5, r))
                } else {
                    name.strip_prefix("NegatedAtom ").map(|r| (12, r))
                };
                let Some((_, rest)) = pos else { return Err(format!("cannot extract a predicate from {:?}", name)) };
                let Some(i) = rest.find('(') else { return Err(format!("cannot extract a predicate from {:?}", name)) };
                rest[..i].to_string()
            };
            match &p {
                None => p = Some(q),
                Some(x) if *x == q => {}
                Some(_) => return Err("the values of a variable with different predicates (taskcore writes one per variable)".into()),
            }
        }
        pred.push(p.unwrap_or_default());
    }
    Ok((t, pred, names))
}
