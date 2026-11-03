//! Fast Downward's landmark graph for lama-first (E011 route): `lm_reasonable_orders_hps(lm_rhw())` on the SAS task
//! held in memory (search.rs `SasTask`), rule for rule as release 26.6.0 makes it
//! (third_party/downward/src/search/landmarks/), so that lama.rs's landmark count is Fast Downward's.
//!
//! What is Fast Downward's here, and must stay so:
//!
//! * **The order of discovery** (landmark_factory_rpg_sasp.cc). The goal atoms first, in the goal's order; then a
//!   FIFO of open landmarks: each one not true at the start gets the relaxed reachability without its atoms, then (1)
//!   its shared preconditions (in the iteration order of their `utils::HashSet<FactPair>`, stdhash.rs) as atomic
//!   landmarks ordered greedy-necessary before it, (2) the critical predecessor values of its variable in the domain
//!   transition graph (atomic landmarks only) ordered naturally before it, (3) its disjunctive preconditions (per
//!   disjunction class, the map's order; at most 4 atoms, none true at the start) unless they overlap a landmark. A new
//!   atomic landmark replaces a disjunctive one holding its atom (the disjunctive one's parents become natural
//!   parents). The order matters: whether an atom is already an atomic landmark decides what is skipped, so every
//!   set and map whose iteration order reaches the graph is libstdc++'s (stdhash.rs).
//! * **The forward orderings**: an atom unreachable without a landmark, and not achievable together with it, is ordered
//!   naturally after it when it is an atomic landmark at the end.
//! * **The ids**: the surviving landmarks in the order they were made.
//! * **The achievers** (landmark_factory_relaxation.cc): possible achievers are the operators with an effect on one of
//!   the atoms, first achievers those that can reach it in the relaxed task without its atoms; only their least
//!   cost under the adapted costs is kept (all landmark_sum reads).
//! * **The reasonable orderings** (landmark_factory_reasonable_orders_hps.cc): a goal landmark gets every other
//!   non-disjunctive landmark that interferes with it; another landmark the interfering ancestors of the other
//!   parents of its greedy-necessary children. With no mutex groups (taskcore writes none) two atoms are mutex when
//!   they are values of one variable.
//!
//! Where the order does not reach the graph (the parents and children maps keyed by node pointers, the forward
//! orderings, the domain transition graphs, the reachability fixpoints) plain Rust containers are used. The relaxed
//! reachability is the same fixpoint as Fast Downward's (exploration.cc) computed per operator rather than per
//! effect; achievable-together and the forward orderings are tested only against the atoms that end up atomic
//! landmarks. `dump` prints the graph as Fast Downward's debug dump does (the differential test).

use crate::search::SasTask;
use crate::stdhash::{Fact, StdMap, StdSet};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::time::Instant;

/// landmark_graph.h `OrderingType` (larger is stronger)
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum OrderingType {
    Reasonable = 0,
    Natural = 1,
    GreedyNecessary = 2,
    Necessary = 3,
}

/// landmark.h `LandmarkType` (lm_rhw makes no conjunctive landmarks)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LmType {
    Disjunctive,
    Atomic,
}

/// A landmark node; parents and children by node index (the arena's, not the id).
#[derive(Clone, Debug)]
pub struct Node {
    pub atoms: Vec<Fact>,
    pub kind: LmType,
    pub goal: bool,
    pub alive: bool,
    pub parents: BTreeMap<u32, OrderingType>,
    pub children: BTreeMap<u32, OrderingType>,
}

/// The finished graph: landmarks by id, each with its orderings by id and its achievers' least costs.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub atoms: Vec<Vec<Fact>>,
    pub disjunctive: Vec<bool>,
    pub goal: Vec<bool>,
    /// per id: (child id, type), by child id
    pub children: Vec<Vec<(u32, OrderingType)>>,
    /// per id: (parent id, type), by parent id
    pub parents: Vec<Vec<(u32, OrderingType)>>,
    /// least adapted cost of a first achiever (i64::MAX: none); for a landmark that holds at the start (past in every
    /// state the search makes, so never read there) the least cost of a possible achiever
    pub min_first: Vec<i64>,
    /// least adapted cost of a possible achiever (i64::MAX: none)
    pub min_possible: Vec<i64>,
    pub n_disjunctive: usize,
    pub n_orderings: usize,
    pub stats: Stats,
}

/// Counts and seconds of the graph's making.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub explorations: u64,
    pub rhw_s: f64,
    pub achievers_s: f64,
    pub hps_s: f64,
    pub setup_s: f64,
}

/// CPU seconds of this thread (CLOCK_THREAD_CPUTIME_ID): the profile's measure, less the machine's load.
pub fn thread_cpu() -> f64 {
    #[repr(C)]
    struct Timespec {
        tv_sec: i64,
        tv_nsec: i64,
    }
    extern "C" {
        fn clock_gettime(clk: i32, ts: *mut Timespec) -> i32;
    }
    let mut ts = Timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: clock_gettime writes the timespec it is given; 3 is CLOCK_THREAD_CPUTIME_ID on Linux
    unsafe { clock_gettime(3, &mut ts) };
    ts.tv_sec as f64 + ts.tv_nsec as f64 / 1e9
}

// --- the task as the landmark code reads it ---------------------------------------------------------------------

/// The task's operators as Fast Downward's root task gives them, with flat proposition numbers.
struct Tv<'a> {
    t: &'a SasTask,
    prop_off: Vec<u32>,
    n_props: usize,
    /// operators_providing_effect[var][value], flat by proposition: operator per effect, in operator then effect order
    prov_off: Vec<u32>,
    prov: Vec<u32>,
    /// the effect (its global number) of each entry of `prov`
    prov_eff: Vec<u32>,
}

impl<'a> Tv<'a> {
    fn new(t: &'a SasTask) -> Tv<'a> {
        let mut prop_off = Vec::with_capacity(t.sizes.len() + 1);
        let mut n = 0u32;
        for &s in &t.sizes {
            prop_off.push(n);
            n += s;
        }
        prop_off.push(n);
        let n_props = n as usize;
        let mut cnt = vec![0u32; n_props + 1];
        for o in 0..t.n_ops() {
            for e in t.effs(o) {
                cnt[(prop_off[e.var as usize] + e.post) as usize + 1] += 1;
            }
        }
        for i in 0..n_props {
            cnt[i + 1] += cnt[i];
        }
        let mut fill = cnt.clone();
        let mut prov = vec![0u32; cnt[n_props] as usize];
        let mut prov_eff = vec![0u32; cnt[n_props] as usize];
        for o in 0..t.n_ops() {
            let e0 = t.eff_off[o] as usize;
            for (k, e) in t.effs(o).iter().enumerate() {
                let p = (prop_off[e.var as usize] + e.post) as usize;
                prov[fill[p] as usize] = o as u32;
                prov_eff[fill[p] as usize] = (e0 + k) as u32;
                fill[p] += 1;
            }
        }
        Tv { t, prop_off, n_props, prov_off: cnt, prov, prov_eff }
    }
    #[inline]
    fn prop(&self, f: Fact) -> usize {
        (self.prop_off[f.0 as usize] + f.1) as usize
    }
    /// get_operators_including_effect
    #[inline]
    fn providing(&self, f: Fact) -> &[u32] {
        let p = self.prop(f);
        &self.prov[self.prov_off[p] as usize..self.prov_off[p + 1] as usize]
    }
    #[inline]
    fn init(&self, var: u32) -> u32 {
        self.t.init[var as usize]
    }
    #[inline]
    fn size(&self, var: u32) -> u32 {
        self.t.sizes[var as usize]
    }
}

// --- the relaxed reachability (exploration.cc) -------------------------------------------------------------------

/// Distinct sets of propositions, each interned once (flat storage, an open-addressing table by content).
struct Sets {
    off: Vec<u32>,
    data: Vec<u32>,
    table: Vec<u32>,
}

impl Sets {
    fn new(cap: usize) -> Sets {
        Sets { off: vec![0], data: Vec::new(), table: vec![u32::MAX; (2 * cap.max(8)).next_power_of_two()] }
    }
    fn get(&self, i: u32) -> &[u32] {
        &self.data[self.off[i as usize] as usize..self.off[i as usize + 1] as usize]
    }
    fn len(&self) -> usize {
        self.off.len() - 1
    }
    fn hash(x: &[u32]) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ x.len() as u64;
        for &v in x {
            h = (h.rotate_left(5) ^ v as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
        h ^ (h >> 29)
    }
    /// the set's number (a new one when it is new)
    fn intern(&mut self, x: &[u32]) -> u32 {
        if 2 * (self.len() + 1) > self.table.len() {
            let n = self.table.len() * 2;
            self.table = vec![u32::MAX; n];
            for i in 0..self.len() as u32 {
                let mut slot = Self::hash(self.get(i)) as usize & (n - 1);
                while self.table[slot] != u32::MAX {
                    slot = (slot + 1) & (n - 1);
                }
                self.table[slot] = i;
            }
        }
        let mask = self.table.len() - 1;
        let mut slot = Self::hash(x) as usize & mask;
        loop {
            let i = self.table[slot];
            if i == u32::MAX {
                let id = self.len() as u32;
                self.data.extend_from_slice(x);
                self.off.push(self.data.len() as u32);
                self.table[slot] = id;
                return id;
            }
            if self.get(i) == x {
                return i;
            }
            slot = (slot + 1) & mask;
        }
    }
}

/// Relaxed reachability from the start with some atoms excluded: the same fixpoint as Fast Downward's unary operators
/// (exploration.cc: a unary operator (operator, effect) fires when the operator's preconditions and the effect's
/// conditions are reached, unless its effect atom is excluded or the operator has an unconditional effect on an
/// excluded atom), computed per operator and per set of conditions: an operator fires when its (distinct)
/// preconditions are reached, its unconditional effect atoms then; a conditional effect (its conditions less the
/// operator's preconditions) reaches its atom once its operator fired and its condition set is reached. Operators
/// share precondition sets and conditional effects share condition sets (a few thousand on E011's tasks), so a
/// reached proposition counts down sets, not operators or effects. Conditional effects on an atom an unconditional
/// effect of the same operator reaches are left out (whenever such an effect fires, the unconditional one has).
struct Reach {
    /// precondition sets: per set its size; per proposition the sets it is in; per set its operators
    ps_n: Vec<u32>,
    ps_of_off: Vec<u32>,
    ps_of: Vec<u32>,
    ps_ops_off: Vec<u32>,
    ps_ops: Vec<u32>,
    /// per operator: its precondition set
    op_ps: Vec<u32>,
    /// per operator: its unconditional effect atoms
    unc_off: Vec<u32>,
    unc: Vec<u32>,
    /// per operator: its conditional effects (global numbers): condition set, atom
    op_ce_off: Vec<u32>,
    ce_set: Vec<u32>,
    ce_prop: Vec<u32>,
    /// condition sets: per set its size; per proposition the sets it is in
    cs_n: Vec<u32>,
    cs_of_off: Vec<u32>,
    cs_of: Vec<u32>,
    /// per precondition set, what its operators reach together when none of them is excluded: the union of their
    /// unconditional effect atoms, and of their conditional effects (condition set, atom) on other atoms; per
    /// condition set, the (precondition set, atom) of those unions
    u_off: Vec<u32>,
    u: Vec<u32>,
    c_off: Vec<u32>,
    c: Vec<(u32, u32)>,
    cu_off: Vec<u32>,
    cu: Vec<(u32, u32)>,
    // working state
    reached: Vec<bool>,
    excluded: Vec<bool>,
    op_excluded: Vec<bool>,
    ps_unsat: Vec<u32>,
    cs_unsat: Vec<u32>,
    ps_fired: Vec<bool>,
    /// precondition sets with an excluded operator (their operators fire one by one)
    tainted: Vec<bool>,
    tainted_list: Vec<u32>,
    /// conditional effects of tainted sets' operators that fired before their condition set: per condition set
    pending: Vec<Vec<u32>>,
    pending_list: Vec<u32>,
    queue: Vec<u32>,
    pub runs: u64,
    /// sizes for the profile: preconditions, conditions, conditional effects
    pub sizes: [usize; 5],
}

fn csr(n: usize, pairs: &[(u32, u32)]) -> (Vec<u32>, Vec<u32>) {
    let mut off = vec![0u32; n + 1];
    for &(k, _) in pairs {
        off[k as usize + 1] += 1;
    }
    for i in 0..n {
        off[i + 1] += off[i];
    }
    let mut fill = off.clone();
    let mut out = vec![0u32; pairs.len()];
    for &(k, v) in pairs {
        out[fill[k as usize] as usize] = v;
        fill[k as usize] += 1;
    }
    (off, out)
}

fn sorted_unique(v: &mut Vec<u32>, from: usize) {
    v[from..].sort_unstable();
    let mut k = from;
    for i in from..v.len() {
        if i == from || v[i] != v[k - 1] {
            v[k] = v[i];
            k += 1;
        }
    }
    v.truncate(k);
}

impl Reach {
    fn new(tv: &Tv) -> Reach {
        let t = tv.t;
        let n_ops = t.n_ops();
        let mut psets = Sets::new(n_ops / 2);
        let mut csets = Sets::new(16384);
        let mut op_ps: Vec<u32> = Vec::with_capacity(n_ops);
        let mut unc_off = Vec::with_capacity(n_ops + 1);
        unc_off.push(0u32);
        let mut unc: Vec<u32> = Vec::new();
        let mut op_ce_off = Vec::with_capacity(n_ops + 1);
        op_ce_off.push(0u32);
        let mut ce_set: Vec<u32> = Vec::new();
        let mut ce_prop: Vec<u32> = Vec::new();
        let mut pre: Vec<u32> = Vec::new();
        let mut cbuf: Vec<u32> = Vec::new();
        let n_conds: usize;
        let q0 = thread_cpu();
        // marks of the operator's unconditional effect atoms, cleared after it
        let mut mark = vec![false; tv.n_props];
        let mut misses = 0u64;
        // whether an operator's effects are its predecessor's (variants of one action): then so are its tables
        let same_as_prev = |o: usize| -> bool {
            if o == 0 {
                return false;
            }
            let (a, b) = (t.effs(o - 1), t.effs(o));
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.var == y.var && x.post == y.post && t.conds(x) == t.conds(y))
        };
        for o in 0..n_ops {
            pre.clear();
            pre.extend(t.pre(o).iter().map(|&f| tv.prop(f) as u32));
            sorted_unique(&mut pre, 0);
            op_ps.push(psets.intern(&pre));
            if same_as_prev(o) {
                let (a, b) = (unc_off[o - 1] as usize, unc_off[o] as usize);
                unc.extend_from_within(a..b);
                unc_off.push(unc.len() as u32);
                let (a, b) = (op_ce_off[o - 1] as usize, op_ce_off[o] as usize);
                ce_set.extend_from_within(a..b);
                ce_prop.extend_from_within(a..b);
                op_ce_off.push(ce_prop.len() as u32);
                continue;
            }
            let u0 = unc.len();
            let effs = t.effs(o);
            for e in effs {
                if e.c0 == e.c1 {
                    let p = tv.prop((e.var, e.post));
                    if !mark[p] {
                        mark[p] = true;
                        unc.push(p as u32);
                    }
                }
            }
            // the conditional effects: their condition sets (a condition that is also a precondition is reached when
            // the operator fires, so it is kept); one on an atom an unconditional effect reaches is left out; repeats
            // are kept (they only repeat work). Effects in a row with the same conditions share their set.
            let mut last: (&[(u32, u32)], u32) = (&[], u32::MAX);
            for e in effs {
                if e.c0 == e.c1 {
                    continue;
                }
                let p = tv.prop((e.var, e.post));
                if mark[p] {
                    continue;
                }
                let raw = t.conds(e);
                let cs = if last.1 != u32::MAX && raw == last.0 {
                    last.1
                } else {
                    cbuf.clear();
                    let mut sorted = true;
                    for &f in raw {
                        let c = tv.prop(f) as u32;
                        if cbuf.last().is_some_and(|&l| l >= c) {
                            sorted = false;
                        }
                        cbuf.push(c);
                    }
                    if !sorted {
                        sorted_unique(&mut cbuf, 0);
                    }
                    let cs = csets.intern(&cbuf);
                    misses += 1;
                    last = (raw, cs);
                    cs
                };
                ce_set.push(cs);
                ce_prop.push(p as u32);
            }
            unc[u0..].sort_unstable();
            unc_off.push(unc.len() as u32);
            for &p in &unc[u0..] {
                mark[p as usize] = false;
            }
            op_ce_off.push(ce_prop.len() as u32);
        }
        n_conds = ce_set.iter().map(|&cs| (csets.off[cs as usize + 1] - csets.off[cs as usize]) as usize).sum();
        let q1 = thread_cpu();
        // precondition sets: membership and operators
        let nps = psets.len();
        let ps_n: Vec<u32> = (0..nps as u32).map(|i| psets.get(i).len() as u32).collect();
        let mut pairs: Vec<(u32, u32)> = Vec::new();
        for i in 0..nps as u32 {
            for &p in psets.get(i) {
                pairs.push((p, i));
            }
        }
        let (ps_of_off, ps_of) = csr(tv.n_props, &pairs);
        let ops_pairs: Vec<(u32, u32)> = op_ps.iter().enumerate().map(|(o, &ps)| (ps, o as u32)).collect();
        let (ps_ops_off, ps_ops) = csr(nps, &ops_pairs);
        // condition sets: membership and conditional effects
        let ncs = csets.len();
        let cs_n: Vec<u32> = (0..ncs as u32).map(|i| csets.get(i).len() as u32).collect();
        pairs.clear();
        for i in 0..ncs as u32 {
            for &p in csets.get(i) {
                pairs.push((p, i));
            }
        }
        let (cs_of_off, cs_of) = csr(tv.n_props, &pairs);
        // the unions per precondition set
        let mut u_off = Vec::with_capacity(nps + 1);
        u_off.push(0u32);
        let mut u: Vec<u32> = Vec::new();
        let mut c_off = Vec::with_capacity(nps + 1);
        c_off.push(0u32);
        let mut c: Vec<(u32, u32)> = Vec::new();
        let mut umark = vec![false; tv.n_props];
        for ps in 0..nps {
            let u0 = u.len();
            let c0 = c.len();
            let ops = &ps_ops[ps_ops_off[ps] as usize..ps_ops_off[ps + 1] as usize];
            for &o in ops {
                for &p in &unc[unc_off[o as usize] as usize..unc_off[o as usize + 1] as usize] {
                    if !umark[p as usize] {
                        umark[p as usize] = true;
                        u.push(p);
                    }
                }
            }
            for &o in ops {
                for e in op_ce_off[o as usize] as usize..op_ce_off[o as usize + 1] as usize {
                    if !umark[ce_prop[e] as usize] {
                        c.push((ce_set[e], ce_prop[e]));
                    }
                }
            }
            for &p in &u[u0..] {
                umark[p as usize] = false;
            }
            // repeats only repeat work: a set of one operator keeps its own (few)
            if ops.len() > 1 {
                c[c0..].sort_unstable();
                let mut k = c0;
                for i in c0..c.len() {
                    if i == c0 || c[i] != c[k - 1] {
                        c[k] = c[i];
                        k += 1;
                    }
                }
                c.truncate(k);
            }
            u_off.push(u.len() as u32);
            c_off.push(c.len() as u32);
        }
        let mut cu_pairs: Vec<(u32, (u32, u32))> = Vec::with_capacity(c.len());
        for ps in 0..nps {
            for &(cs, p) in &c[c_off[ps] as usize..c_off[ps + 1] as usize] {
                cu_pairs.push((cs, (ps as u32, p)));
            }
        }
        let mut cu_off = vec![0u32; ncs + 1];
        for &(cs, _) in &cu_pairs {
            cu_off[cs as usize + 1] += 1;
        }
        for i in 0..ncs {
            cu_off[i + 1] += cu_off[i];
        }
        let mut fill = cu_off.clone();
        let mut cu = vec![(0u32, 0u32); cu_pairs.len()];
        for &(cs, x) in &cu_pairs {
            cu[fill[cs as usize] as usize] = x;
            fill[cs as usize] += 1;
        }
        let n_pre = psets.data.len();
        if std::env::var_os("TASKCORE_PROFILE").is_some() {
            eprintln!(
                "profile landmarks (CPU s): reachability tables: per operator {:.4} ({} condition sets looked up), the lists {:.4}",
                q1 - q0,
                misses,
                thread_cpu() - q1
            );
        }
        Reach {
            ps_n,
            ps_of_off,
            ps_of,
            ps_ops_off,
            ps_ops,
            op_ps,
            unc_off,
            unc,
            op_ce_off,
            ce_set,
            ce_prop,
            cs_n,
            cs_of_off,
            cs_of,
            u_off,
            u,
            c_off,
            c,
            cu_off,
            cu,
            reached: vec![false; tv.n_props],
            excluded: vec![false; tv.n_props],
            op_excluded: vec![false; n_ops],
            ps_unsat: vec![0; nps],
            cs_unsat: vec![0; ncs],
            ps_fired: vec![false; nps],
            tainted: vec![false; nps],
            tainted_list: Vec::new(),
            pending: vec![Vec::new(); ncs],
            pending_list: Vec::new(),
            queue: Vec::new(),
            runs: 0,
            sizes: [nps, n_pre, ncs, n_conds, 0],
        }
    }

    #[inline]
    fn reach(&mut self, p: u32) {
        if !self.reached[p as usize] {
            self.reached[p as usize] = true;
            self.queue.push(p);
        }
    }

    /// A tainted set's operator fires (it is not excluded): its own effects, its conditional effects whose condition
    /// set is not reached yet waiting on it.
    fn fire(&mut self, o: usize) {
        for i in self.unc_off[o] as usize..self.unc_off[o + 1] as usize {
            let p = self.unc[i];
            if !self.excluded[p as usize] {
                self.reach(p);
            }
        }
        for e in self.op_ce_off[o] as usize..self.op_ce_off[o + 1] as usize {
            let cs = self.ce_set[e] as usize;
            let p = self.ce_prop[e];
            if self.cs_unsat[cs] == 0 {
                if !self.excluded[p as usize] {
                    self.reach(p);
                }
            } else {
                if self.pending[cs].is_empty() {
                    self.pending_list.push(cs as u32);
                }
                self.pending[cs].push(p);
            }
        }
    }

    fn fire_set(&mut self, ps: usize) {
        self.ps_fired[ps] = true;
        if !self.tainted[ps] {
            for i in self.u_off[ps] as usize..self.u_off[ps + 1] as usize {
                let p = self.u[i];
                if !self.excluded[p as usize] {
                    self.reach(p);
                }
            }
            for i in self.c_off[ps] as usize..self.c_off[ps + 1] as usize {
                let (cs, p) = self.c[i];
                if self.cs_unsat[cs as usize] == 0 && !self.excluded[p as usize] {
                    self.reach(p);
                }
            }
            return;
        }
        for i in self.ps_ops_off[ps] as usize..self.ps_ops_off[ps + 1] as usize {
            let o = self.ps_ops[i] as usize;
            if !self.op_excluded[o] {
                self.fire(o);
            }
        }
    }

    /// compute_relaxed_reachability(excluded, false): reached per flat proposition (in `self.reached`).
    fn compute(&mut self, tv: &Tv, excluded: &[Fact]) -> &[bool] {
        self.runs += 1;
        let t = tv.t;
        self.reached.iter_mut().for_each(|r| *r = false);
        for &f in excluded {
            self.excluded[tv.prop(f)] = true;
        }
        // get_excluded_operators: an unconditional effect on an excluded atom; their precondition sets are tainted
        for &ps in &self.tainted_list {
            self.tainted[ps as usize] = false;
        }
        self.tainted_list.clear();
        for &f in excluded {
            let p = tv.prop(f) as u32;
            for &o in tv.providing(f) {
                let o = o as usize;
                if !self.op_excluded[o] && self.unc[self.unc_off[o] as usize..self.unc_off[o + 1] as usize].binary_search(&p).is_ok() {
                    self.op_excluded[o] = true;
                    let ps = self.op_ps_of(o);
                    if !self.tainted[ps] {
                        self.tainted[ps] = true;
                        self.tainted_list.push(ps as u32);
                    }
                }
            }
        }
        for &cs in &self.pending_list {
            self.pending[cs as usize].clear();
        }
        self.pending_list.clear();
        self.ps_unsat.copy_from_slice(&self.ps_n);
        self.cs_unsat.copy_from_slice(&self.cs_n);
        self.ps_fired.iter_mut().for_each(|x| *x = false);
        self.queue.clear();
        for v in 0..t.sizes.len() {
            let p = (tv.prop_off[v] + t.init[v]) as usize;
            if !self.excluded[p] {
                self.reach(p as u32);
            }
        }
        for ps in 0..self.ps_n.len() {
            if self.ps_n[ps] == 0 {
                self.fire_set(ps);
            }
        }
        let mut qi = 0;
        while qi < self.queue.len() {
            let p = self.queue[qi] as usize;
            qi += 1;
            for i in self.ps_of_off[p] as usize..self.ps_of_off[p + 1] as usize {
                let ps = self.ps_of[i] as usize;
                self.ps_unsat[ps] -= 1;
                if self.ps_unsat[ps] == 0 {
                    self.fire_set(ps);
                }
            }
            for i in self.cs_of_off[p] as usize..self.cs_of_off[p + 1] as usize {
                let cs = self.cs_of[i] as usize;
                self.cs_unsat[cs] -= 1;
                if self.cs_unsat[cs] == 0 {
                    for j in self.cu_off[cs] as usize..self.cu_off[cs + 1] as usize {
                        let (ps, q) = self.cu[j];
                        if self.ps_fired[ps as usize] && !self.tainted[ps as usize] && !self.excluded[q as usize] {
                            self.reach(q);
                        }
                    }
                    for j in 0..self.pending[cs].len() {
                        let q = self.pending[cs][j];
                        if !self.excluded[q as usize] {
                            self.reach(q);
                        }
                    }
                }
            }
        }
        for &f in excluded {
            self.excluded[tv.prop(f)] = false;
        }
        for &o in excluded.iter().flat_map(|&f| tv.providing(f)) {
            self.op_excluded[o as usize] = false;
        }
        &self.reached
    }

    /// the precondition set of operator `o`
    fn op_ps_of(&self, o: usize) -> usize {
        self.op_ps[o] as usize
    }
}

/// util.cc `possibly_reaches_landmark`: the operator's preconditions reached, and an effect on one of the atoms with
/// its conditions reached.
fn possibly_reaches(tv: &Tv, reached: &[bool], o: usize, atoms: &[Fact]) -> bool {
    let t = tv.t;
    if !t.pre(o).iter().all(|&f| reached[tv.prop(f)]) {
        return false;
    }
    t.effs(o).iter().any(|e| atoms.contains(&(e.var, e.post)) && t.conds(e).iter().all(|&f| reached[tv.prop(f)]))
}

// --- lm_rhw (landmark_factory_rpg_sasp.cc) ------------------------------------------------------------------------

/// Scratch sets of the precondition approximations, reused for their memory (a reset set is a new one).
#[derive(Default)]
struct Scratch {
    vars: Vec<u32>,
    pre: StdSet<Fact>,
    shared_tmp: StdSet<Fact>,
    inter: StdSet<Fact>,
    ec: StdSet<Fact>,
    tmp: StdSet<Fact>,
}

/// The graph while it is made: nodes in an arena (creation order), the atom maps of landmark_graph.cc.
struct Builder<'a> {
    tv: &'a Tv<'a>,
    nodes: Vec<Node>,
    atomic: HashMap<Fact, u32>,
    disj: HashMap<Fact, u32>,
    open: VecDeque<u32>,
    /// per processed node: the propositions reached without it (the forward orderings, tested at the end)
    forward: HashMap<u32, Vec<u64>>,
    classes: Vec<i32>,
    /// domain transition graphs, made on first use: per variable, per value its successors, and the values every value
    /// reaches (effects with no condition on the variable)
    dtg: HashMap<u32, (Vec<Vec<u32>>, Vec<u32>)>,
    /// CPU seconds: explorations, shared preconditions, lookahead, disjunctive preconditions
    prof: [f64; 4],
    scratch: Scratch,
}

impl<'a> Builder<'a> {
    fn add_node(&mut self, atoms: Vec<Fact>, kind: LmType, goal: bool) -> u32 {
        let id = self.nodes.len() as u32;
        match kind {
            LmType::Disjunctive => {
                for &a in &atoms {
                    self.disj.insert(a, id);
                }
            }
            LmType::Atomic => {
                self.atomic.insert(atoms[0], id);
            }
        }
        self.nodes.push(Node { atoms, kind, goal, alive: true, parents: BTreeMap::new(), children: BTreeMap::new() });
        id
    }

    /// landmark_factory.cc `add_or_replace_ordering_if_stronger`
    fn add_or_replace(&mut self, from: u32, to: u32, ty: OrderingType) {
        let old = self.nodes[from as usize].children.get(&to).copied();
        match old {
            Some(o) if o >= ty => {}
            _ => {
                self.nodes[from as usize].children.insert(to, ty);
                self.nodes[to as usize].parents.insert(from, ty);
            }
        }
    }

    /// landmark_graph.cc `remove_node` (with `remove_node_occurrences`)
    fn remove_node(&mut self, n: u32) {
        let parents: Vec<u32> = self.nodes[n as usize].parents.keys().copied().collect();
        for p in parents {
            self.nodes[p as usize].children.remove(&n);
        }
        let children: Vec<u32> = self.nodes[n as usize].children.keys().copied().collect();
        for c in children {
            self.nodes[c as usize].parents.remove(&n);
        }
        match self.nodes[n as usize].kind {
            LmType::Disjunctive => {
                for a in self.nodes[n as usize].atoms.clone() {
                    self.disj.remove(&a);
                }
            }
            LmType::Atomic => {
                let a = self.nodes[n as usize].atoms[0];
                self.atomic.remove(&a);
            }
        }
        let node = &mut self.nodes[n as usize];
        node.alive = false;
        node.parents.clear();
        node.children.clear();
    }

    fn is_true_in_init(&self, n: u32) -> bool {
        let node = &self.nodes[n as usize];
        let tr = |f: &Fact| self.tv.init(f.0) == f.1;
        match node.kind {
            LmType::Disjunctive => node.atoms.iter().any(tr),
            LmType::Atomic => node.atoms.iter().all(tr),
        }
    }

    /// remove_disjunctive_landmark_and_rewire_orderings
    fn remove_disjunctive_and_rewire(&mut self, atomic_node: u32) {
        let atom = self.nodes[atomic_node as usize].atoms[0];
        let d = self.disj[&atom];
        // remove_occurrences_of_landmark_node
        if let Some(i) = self.open.iter().position(|&x| x == d) {
            self.open.remove(i);
        }
        self.forward.remove(&d);
        let parents: Vec<u32> = self.nodes[d as usize].parents.keys().copied().collect();
        self.remove_node(d);
        for p in parents {
            self.add_or_replace(p, atomic_node, OrderingType::Natural);
        }
    }

    /// add_atomic_landmark_and_ordering (use_orders on)
    fn add_atomic(&mut self, atom: Fact, node: u32, ty: OrderingType) {
        if let Some(&a) = self.atomic.get(&atom) {
            self.add_or_replace(a, node, ty);
            return;
        }
        let a = self.add_node(vec![atom], LmType::Atomic, false);
        self.open.push_back(a);
        self.add_or_replace(a, node, ty);
        if self.disj.contains_key(&atom) {
            self.remove_disjunctive_and_rewire(a);
        }
    }

    /// deal_with_overlapping_landmarks: true when an overlapping landmark exists already
    fn overlapping(&mut self, atoms: &StdSet<Fact>, node: u32, ty: OrderingType) -> bool {
        if atoms.iter().any(|a| self.atomic.contains_key(&a)) {
            return true;
        }
        if atoms.iter().any(|a| self.disj.contains_key(&a)) {
            // contains_superset_disjunctive_landmark: every atom in one disjunctive landmark
            let mut same: Option<u32> = None;
            let mut superset = true;
            for a in atoms.iter() {
                match self.disj.get(&a) {
                    None => {
                        superset = false;
                        break;
                    }
                    Some(&d) => {
                        if same.is_some_and(|s| s != d) {
                            superset = false;
                            break;
                        }
                        same = Some(d);
                    }
                }
            }
            if superset {
                let other = self.disj[&atoms.first().expect("a non-empty set")];
                self.add_or_replace(other, node, ty);
            }
            return true;
        }
        false
    }

    /// add_disjunctive_landmark_and_ordering
    fn add_disjunctive(&mut self, atoms: &StdSet<Fact>, node: u32, ty: OrderingType) {
        if !self.overlapping(atoms, node, ty) {
            let d = self.add_node(atoms.to_vec(), LmType::Disjunctive, false);
            self.open.push_back(d);
            self.add_or_replace(d, node, ty);
        }
    }

    /// approximate_preconditions_to_achieve_landmark into `out`: (1) the operator's preconditions, (2) the start's
    /// value of a binary variable the operator writes but does not read, where the landmark has another value of it,
    /// (3) the conditions shared by all effects that achieve the landmark. `sc` holds scratch sets (their memory only).
    fn approx_preconditions(&self, atoms: &[Fact], o: usize, out: &mut StdSet<Fact>, sc: &mut Scratch) {
        let t = self.tv.t;
        out.reset();
        sc.vars.clear();
        for &f in t.pre(o) {
            out.insert(f);
            sc.vars.push(f.0);
        }
        // add_binary_variable_conditions
        for e in t.effs(o) {
            let var = e.var;
            if !sc.vars.contains(&var) && self.tv.size(var) == 2 {
                for &a in atoms {
                    if a.0 == var && self.tv.init(var) != a.1 {
                        out.insert((var, self.tv.init(var)));
                        break;
                    }
                }
            }
        }
        // add_effect_conditions
        sc.inter.reset();
        let mut init = true;
        for e in t.effs(o) {
            if !atoms.contains(&(e.var, e.post)) {
                continue;
            }
            if e.c0 == e.c1 {
                return;
            }
            sc.ec.reset();
            for &c in t.conds(e) {
                sc.ec.insert(c);
            }
            if init {
                std::mem::swap(&mut sc.inter, &mut sc.ec);
                init = false;
            } else {
                sc.tmp.assign_intersection(&sc.inter, &sc.ec);
                std::mem::swap(&mut sc.inter, &mut sc.tmp);
            }
            if sc.inter.is_empty() {
                break;
            }
        }
        for f in sc.inter.iter() {
            out.insert(f);
        }
    }

    /// compute_shared_preconditions
    fn shared_preconditions(&self, atoms: &[Fact], reached: &[bool], sc: &mut Scratch) -> StdSet<Fact> {
        let mut shared: StdSet<Fact> = StdSet::new();
        let mut pre = std::mem::take(&mut sc.pre);
        let mut tmp = std::mem::take(&mut sc.shared_tmp);
        let mut init = true;
        'all: for &a in atoms {
            for &o in self.tv.providing(a) {
                let o = o as usize;
                if possibly_reaches(self.tv, reached, o, atoms) {
                    self.approx_preconditions(atoms, o, &mut pre, sc);
                    if init {
                        std::mem::swap(&mut shared, &mut pre);
                        init = false;
                    } else {
                        tmp.assign_intersection(&shared, &pre);
                        std::mem::swap(&mut shared, &mut tmp);
                    }
                    if shared.is_empty() {
                        break 'all;
                    }
                }
            }
        }
        sc.pre = pre;
        sc.shared_tmp = tmp;
        shared
    }

    /// compute_disjunctive_preconditions (with get_disjunctive_preconditions)
    fn disjunctive_preconditions(&self, atoms: &[Fact], reached: &[bool], sc: &mut Scratch) -> Vec<StdSet<Fact>> {
        // get_operators_achieving_landmark: an unordered_set<int> of the achievers, as a vector in its order
        let mut ids: StdSet<i32> = StdSet::new();
        for &a in atoms {
            for &o in self.tv.providing(a) {
                ids.insert(o as i32);
            }
        }
        let mut num_ops = 0usize;
        let mut by_class: StdMap<i32, Vec<Fact>> = StdMap::new();
        // per class: how many operators gave it an atom (used_operators' sizes), and the last of them
        let mut used: HashMap<i32, (usize, u32)> = HashMap::new();
        let mut pre = std::mem::take(&mut sc.pre);
        for o in ids.iter() {
            let o = o as usize;
            if possibly_reaches(self.tv, reached, o, atoms) {
                num_ops += 1;
                self.approx_preconditions(atoms, o, &mut pre, sc);
                // extend_disjunction_class_lookups
                for f in pre.iter() {
                    let class = self.classes[self.tv.prop(f)];
                    if class == -1 {
                        continue;
                    }
                    if !self.atomic.contains_key(&f) {
                        by_class.entry(class).push(f);
                        let u = used.entry(class).or_insert((0, u32::MAX));
                        if u.1 != o as u32 {
                            *u = (u.0 + 1, o as u32);
                        }
                    }
                }
            }
        }
        sc.pre = pre;
        let mut out = Vec::new();
        for (class, fs) in by_class.iter() {
            if used[&class].0 == num_ops {
                let set = StdSet::from_iter_std(fs.iter().copied());
                if set.len() > 1 {
                    out.push(set);
                }
            }
        }
        out
    }

    /// The domain transition graph of `var` (DomainTransitionGraphCollection, compute_successors): per value the
    /// values an effect moves it to, and the values an effect with no condition on `var` sets from any value.
    fn dtg(&mut self, var: u32) -> &(Vec<Vec<u32>>, Vec<u32>) {
        if !self.dtg.contains_key(&var) {
            let t = self.tv.t;
            let size = self.tv.size(var) as usize;
            let mut succ: Vec<Vec<u32>> = vec![Vec::new(); size];
            let mut any: Vec<u32> = Vec::new();
            // the effects on `var`, each once (the providers' entries are per effect)
            for val in 0..size as u32 {
                let p = self.tv.prop((var, val));
                for i in self.tv.prov_off[p] as usize..self.tv.prov_off[p + 1] as usize {
                    let o = self.tv.prov[i] as usize;
                    let e = &t.effs[self.tv.prov_eff[i] as usize];
                    // build_conditions_map: the last precondition on a variable wins
                    let pre = t.pre(o).iter().rev().find(|f| f.0 == var).map(|f| f.1 as i64).unwrap_or(-1);
                    let post = e.post;
                    let ec = t.conds(e).iter().rev().find(|f| f.0 == var).map(|f| f.1 as i64).unwrap_or(-1);
                    if pre != -1 {
                        if ec != -1 && ec != pre {
                            continue;
                        }
                        if pre as u32 != post {
                            succ[pre as usize].push(post);
                        }
                    } else if ec != -1 {
                        if ec as u32 != post {
                            succ[ec as usize].push(post);
                        }
                    } else {
                        any.push(post);
                    }
                }
            }
            for s in &mut succ {
                s.sort_unstable();
                s.dedup();
            }
            any.sort_unstable();
            any.dedup();
            self.dtg.insert(var, (succ, any));
        }
        &self.dtg[&var]
    }

    /// value_critical_to_reach_landmark. The successors every value has (`dtg.1`) are the same for each value taken,
    /// so they are looked at once, with the first value (the start's): what they reach is then reached.
    fn critical(dtg: &(Vec<Vec<u32>>, Vec<u32>), init: u32, lm: u32, excluded: u32, reached: &[bool]) -> bool {
        if excluded == init {
            return true;
        }
        let n = reached.len();
        let mut closed = vec![false; n];
        let mut open: VecDeque<u32> = VecDeque::new();
        open.push_back(init);
        closed[init as usize] = true;
        let mut any_done = false;
        while let Some(v) = open.pop_front() {
            let any: &[u32] = if any_done { &[] } else { &dtg.1 };
            any_done = true;
            for &s in dtg.0[v as usize].iter().chain(any.iter()) {
                if s == lm {
                    return false;
                }
                if !reached[s as usize] || s == excluded {
                    continue;
                }
                if !closed[s as usize] {
                    closed[s as usize] = true;
                    open.push_back(s);
                }
            }
        }
        true
    }

    /// approximate_lookahead_orderings (the forward orderings kept as the reached set, module docstring)
    fn lookahead(&mut self, node: u32, reached: &[bool]) {
        let mut bits = vec![0u64; reached.len().div_ceil(64)];
        for (p, &r) in reached.iter().enumerate() {
            if r {
                bits[p / 64] |= 1 << (p % 64);
            }
        }
        self.forward.insert(node, bits);
        if self.nodes[node as usize].kind != LmType::Atomic {
            return;
        }
        let (var, val) = self.nodes[node as usize].atoms[0];
        let init = self.tv.init(var);
        let off = self.tv.prop_off[var as usize] as usize;
        let size = self.tv.size(var) as usize;
        let rv: Vec<bool> = reached[off..off + size].to_vec();
        let dtg = self.dtg(var);
        let mut crit = Vec::new();
        for value in 0..size as u32 {
            if rv[value as usize] && Self::critical(dtg, init, val, value, &rv) {
                crit.push(value);
            }
        }
        for value in crit {
            self.add_atomic((var, value), node, OrderingType::Natural);
        }
    }

    /// generate_backchaining_landmarks
    fn backchain(&mut self, reach: &mut Reach, deadline: Option<Instant>) -> Result<(), String> {
        while let Some(node) = self.open.pop_front() {
            if deadline.is_some_and(|d| Instant::now() > d) {
                return Err("time".into());
            }
            if self.is_true_in_init(node) {
                continue;
            }
            let atoms = self.nodes[node as usize].atoms.clone();
            let c0 = thread_cpu();
            let reached = reach.compute(self.tv, &atoms).to_vec();
            let c1 = thread_cpu();
            // generate_shared_precondition_landmarks
            let mut sc = std::mem::take(&mut self.scratch);
            let shared = self.shared_preconditions(&atoms, &reached, &mut sc);
            for f in shared.iter() {
                self.add_atomic(f, node, OrderingType::GreedyNecessary);
            }
            if !self.nodes[node as usize].alive {
                return Err(format!("landmark {:?} removed while it was being processed", atoms));
            }
            let c2 = thread_cpu();
            self.lookahead(node, &reached);
            let c3 = thread_cpu();
            // generate_disjunctive_precondition_landmarks
            let sets = self.disjunctive_preconditions(&atoms, &reached, &mut sc);
            self.scratch = sc;
            for set in sets {
                if set.len() < 5 && set.iter().all(|f| self.tv.init(f.0) != f.1) {
                    self.add_disjunctive(&set, node, OrderingType::GreedyNecessary);
                }
            }
            let c4 = thread_cpu();
            self.prof[0] += c1 - c0;
            self.prof[1] += c2 - c1;
            self.prof[2] += c3 - c2;
            self.prof[3] += c4 - c3;
        }
        Ok(())
    }

    /// add_landmark_forward_orderings: an atom not reached without a landmark and not achievable together with it
    /// (atom_and_landmark_achievable_together), when it is an atomic landmark, is ordered naturally after it.
    fn forward_orderings(&mut self) {
        let atomic: Vec<(Fact, u32)> = {
            let mut v: Vec<(Fact, u32)> = self.atomic.iter().map(|(&f, &n)| (f, n)).collect();
            v.sort_unstable();
            v
        };
        let n_ops = self.tv.t.n_ops();
        let mut mark = vec![false; n_ops];
        for n in 0..self.nodes.len() as u32 {
            if !self.nodes[n as usize].alive {
                continue;
            }
            let Some(bits) = self.forward.get(&n) else { continue };
            let bits = bits.clone();
            let atoms = self.nodes[n as usize].atoms.clone();
            let mut marked: Vec<u32> = Vec::new();
            for &a in &atoms {
                for &o in self.tv.providing(a) {
                    if !mark[o as usize] {
                        mark[o as usize] = true;
                        marked.push(o);
                    }
                }
            }
            for &(f, n2) in &atomic {
                let p = self.tv.prop(f);
                if bits[p / 64] >> (p % 64) & 1 == 1 {
                    continue;
                }
                let together = atoms.contains(&f) || self.tv.providing(f).iter().any(|&o| mark[o as usize]);
                if !together {
                    self.add_or_replace(n, n2, OrderingType::Natural);
                }
            }
            for o in marked {
                mark[o as usize] = false;
            }
        }
    }
}

/// build_disjunction_classes from the atoms' predicates (`pred[var]`: the predicate of every value of the variable,
/// as taskcore names them; "" for none): the classes number the predicates in order of first appearance.
fn disjunction_classes(tv: &Tv, pred: &[String]) -> Vec<i32> {
    let mut index: HashMap<&str, i32> = HashMap::new();
    let mut classes = vec![-1i32; tv.n_props];
    for v in 0..tv.t.sizes.len() {
        for x in 0..tv.size(v as u32) {
            let p = pred[v].as_str();
            let c = if p.is_empty() {
                -1
            } else {
                let n = index.len() as i32;
                *index.entry(p).or_insert(n)
            };
            classes[(tv.prop_off[v] + x) as usize] = c;
        }
    }
    classes
}

/// The predicate taskcore writes for every value of a variable named `key` (sas.rs `atom_kind`: the alphanumerics of
/// the key's first part, "fact" when none), which is what Fast Downward reads from "Atom kind(key=value)".
pub fn predicate_of_key(key: &str) -> String {
    let head = key.split(':').next().unwrap_or("");
    let k: String = head.chars().filter(|c| c.is_alphanumeric()).collect();
    if k.is_empty() {
        "fact".to_string()
    } else {
        k
    }
}

// --- the reasonable orderings (landmark_factory_reasonable_orders_hps.cc) ----------------------------------------

/// An operator's effects as get_effects_on_other_variables reads them: its unconditional effect atoms and the atoms its
/// conditional effects always reach (get_effects_that_always_happen), each sorted and distinct.
fn effect_classes(t: &SasTask, o: usize) -> (Vec<Fact>, Vec<Fact>) {
    let mut unc: Vec<Fact> = Vec::new();
    // compute_effect_conditions_by_variable: the conditional effects by variable (a variable written with two values
    // by conditional effects is left out), their conditions together
    let mut ce: Vec<(u32, u32, u32)> = Vec::new();
    for (i, e) in t.effs(o).iter().enumerate() {
        if e.c0 == e.c1 {
            unc.push((e.var, e.post));
        } else {
            ce.push((e.var, e.post, i as u32));
        }
    }
    unc.sort_unstable();
    unc.dedup();
    ce.sort_unstable();
    let effs = t.effs(o);
    let mut always: Vec<Fact> = Vec::new();
    let mut conds: Vec<(u32, u32)> = Vec::new();
    let mut i = 0;
    while i < ce.len() {
        let var = ce[i].0;
        let mut j = i;
        while j < ce.len() && ce[j].0 == var {
            j += 1;
        }
        let value = ce[i].1;
        if ce[i..j].iter().all(|x| x.1 == value) {
            // effect_always_happens: every condition variable (with the effect's value when it is the effect's
            // variable) takes as many values as the effect variable's domain
            conds.clear();
            for x in &ce[i..j] {
                conds.extend_from_slice(t.conds(&effs[x.2 as usize]));
            }
            conds.sort_unstable();
            conds.dedup();
            let domain = t.sizes[var as usize] as usize;
            let mut happens = true;
            let mut k = 0;
            while k < conds.len() {
                let cv = conds[k].0;
                let mut l = k;
                while l < conds.len() && conds[l].0 == cv {
                    l += 1;
                }
                let mut n = l - k;
                if cv == var && !conds[k..l].iter().any(|c| c.1 == value) {
                    n += 1;
                }
                if n < domain {
                    happens = false;
                    break;
                }
                k = l;
            }
            if happens {
                always.push((var, value));
            }
        }
        i = j;
    }
    (unc, always)
}

/// get_effects_on_other_variables(op, var): unconditional effects on other variables and effects that always happen,
/// from the operator's `effect_classes`, sorted and distinct.
fn effects_on_other_variables(classes: &(Vec<Fact>, Vec<Fact>), var_id: u32) -> Vec<Fact> {
    let mut next: Vec<Fact> = classes.0.iter().copied().filter(|f| f.0 != var_id).collect();
    next.extend_from_slice(&classes.1);
    next.sort_unstable();
    next.dedup();
    next
}

struct Hps<'a> {
    tv: &'a Tv<'a>,
    shared: HashMap<Fact, Vec<Fact>>,
    classes: HashMap<u32, (Vec<Fact>, Vec<Fact>)>,
}

impl Hps<'_> {
    /// get_shared_effects_of_achievers (cached per atom)
    fn shared_effects(&mut self, atom: Fact) -> &[Fact] {
        if !self.shared.contains_key(&atom) {
            let mut shared: Vec<Fact> = Vec::new();
            let mut init = true;
            for &o in self.tv.providing(atom) {
                let t = self.tv.t;
                let classes = self.classes.entry(o).or_insert_with(|| effect_classes(t, o as usize));
                let eff = effects_on_other_variables(classes, atom.0);
                if init {
                    shared = eff;
                    init = false;
                } else {
                    shared.retain(|f| eff.binary_search(f).is_ok());
                }
                if shared.is_empty() {
                    break;
                }
            }
            self.shared.insert(atom, shared);
        }
        &self.shared[&atom]
    }

    /// interferes(landmark_a, landmark_b) for atomic landmarks a and b
    fn interferes(&mut self, a: Fact, b: Fact) -> bool {
        if a == b {
            return false;
        }
        // 1. mutex: values of one variable
        if a.0 == b.0 {
            return true;
        }
        // 2. a shared effect E of a's achievers, E mutex with b
        self.shared_effects(a).iter().any(|&e| e != a && e != b && e.0 == b.0 && e.1 != b.1)
    }
}

/// The landmark graph (module docstring) of `t` with the adapted costs `adj`; `pred[var]` the predicate of the
/// variable's atoms.
pub fn build(t: &SasTask, adj: &[i32], pred: &[String], deadline: Option<Instant>) -> Result<Graph, String> {
    let t0 = Instant::now();
    let k0 = thread_cpu();
    let tv = Tv::new(t);
    let k1 = thread_cpu();
    let mut reach = Reach::new(&tv);
    let k2 = thread_cpu();
    let classes = disjunction_classes(&tv, pred);
    let setup_s = t0.elapsed().as_secs_f64();
    if std::env::var_os("TASKCORE_PROFILE").is_some() {
        eprintln!("profile landmarks (CPU s): the providers {:.4}, the reachability's tables {:.4}", k1 - k0, k2 - k1);
        eprintln!(
            "profile landmarks: {} operators, {} effects, {} propositions; reach: {} precondition sets ({} atoms), {} unconditional atoms, {} conditional effects with {} conditions in {} condition sets ({} atoms); setup {:.4} s",
            t.n_ops(),
            t.effs.len(),
            tv.n_props,
            reach.sizes[0],
            reach.sizes[1],
            reach.unc.len(),
            reach.ce_prop.len(),
            reach.sizes[3],
            reach.sizes[2],
            reach.cs_of.len(),
            setup_s
        );
    }
    let t1 = Instant::now();
    let mut b = Builder {
        tv: &tv,
        nodes: Vec::new(),
        atomic: HashMap::new(),
        disj: HashMap::new(),
        open: VecDeque::new(),
        forward: HashMap::new(),
        classes,
        dtg: HashMap::new(),
        prof: [0.0; 4],
        scratch: Scratch::default(),
    };
    // generate_goal_landmarks
    for &g in &t.goal {
        let n = b.add_node(vec![g], LmType::Atomic, true);
        b.open.push_back(n);
    }
    let cpu0 = thread_cpu();
    b.backchain(&mut reach, deadline)?;
    let cpu1 = thread_cpu();
    b.forward_orderings();
    let cpu2 = thread_cpu();
    let rhw_s = t1.elapsed().as_secs_f64();
    if std::env::var_os("TASKCORE_PROFILE").is_some() {
        eprintln!(
            "profile landmarks (CPU s): backchaining {:.4} (explorations {:.4} ({}), shared preconditions {:.4}, lookahead {:.4}, disjunctive {:.4}), forward orderings {:.4}",
            cpu1 - cpu0,
            b.prof[0],
            reach.runs,
            b.prof[1],
            b.prof[2],
            b.prof[3],
            cpu2 - cpu1
        );
    }
    // set_landmark_ids: the surviving nodes in the order they were made
    let t2 = Instant::now();
    let alive: Vec<u32> = (0..b.nodes.len() as u32).filter(|&n| b.nodes[n as usize].alive).collect();
    let mut id_of = vec![u32::MAX; b.nodes.len()];
    for (i, &n) in alive.iter().enumerate() {
        id_of[n as usize] = i as u32;
    }
    // calc_achievers: least adapted costs of the possible and the first achievers
    let mut min_first = Vec::with_capacity(alive.len());
    let mut min_possible = Vec::with_capacity(alive.len());
    for &n in &alive {
        if deadline.is_some_and(|d| Instant::now() > d) {
            return Err("time".into());
        }
        let atoms = b.nodes[n as usize].atoms.clone();
        let mut possible: Vec<u32> = atoms.iter().flat_map(|&a| tv.providing(a).iter().copied()).collect();
        possible.sort_unstable();
        possible.dedup();
        let mut mp = i64::MAX;
        for &o in &possible {
            mp = mp.min(adj[o as usize] as i64);
        }
        // the first achievers: possible achievers that reach the landmark in the relaxed task without its atoms (the
        // backchaining's reachability, made when it processed the landmark). A landmark that holds at the start is
        // past in the start and so in every state the search makes from it (a past landmark stays past), where
        // landmark_sum reads only its possible achievers: its first achievers are not made (`min_first` is
        // `min_possible` there, never read).
        let mf = if b.is_true_in_init(n) {
            mp
        } else {
            let kept: Vec<bool>;
            let reached: &[bool] = match b.forward.get(&n) {
                Some(bits) => {
                    kept = (0..tv.n_props).map(|p| bits[p / 64] >> (p % 64) & 1 == 1).collect();
                    &kept
                }
                None => reach.compute(&tv, &atoms),
            };
            let mut mf = i64::MAX;
            for &o in &possible {
                let c = adj[o as usize] as i64;
                if c < mf && possibly_reaches(&tv, reached, o as usize, &atoms) {
                    mf = c;
                }
            }
            mf
        };
        min_first.push(mf);
        min_possible.push(mp);
    }
    let achievers_s = t2.elapsed().as_secs_f64();
    // approximate_reasonable_orderings
    let t3 = Instant::now();
    let mut hps = Hps { tv: &tv, shared: HashMap::new(), classes: HashMap::new() };
    for &n in &alive {
        if b.nodes[n as usize].kind == LmType::Disjunctive {
            continue;
        }
        let atom = b.nodes[n as usize].atoms[0];
        let candidates: Vec<u32> = if b.nodes[n as usize].goal {
            alive.clone()
        } else {
            // collect_reasonable_ordering_candidates
            let mut set: BTreeSet<u32> = BTreeSet::new();
            let kids: Vec<(u32, OrderingType)> = b.nodes[n as usize].children.iter().map(|(&c, &ty)| (c, ty)).collect();
            for (child, ty) in kids {
                if ty < OrderingType::GreedyNecessary {
                    continue;
                }
                for (&parent, &pty) in &b.nodes[child as usize].parents {
                    if b.nodes[parent as usize].kind == LmType::Disjunctive {
                        continue;
                    }
                    if pty >= OrderingType::Natural && parent != n {
                        set.insert(parent);
                        // collect_ancestors
                        let mut stack = vec![parent];
                        while let Some(x) = stack.pop() {
                            for (&p, &ty2) in &b.nodes[x as usize].parents {
                                if ty2 >= OrderingType::Natural && set.insert(p) {
                                    stack.push(p);
                                }
                            }
                        }
                    }
                }
            }
            set.into_iter().collect()
        };
        for other in candidates {
            if other == n || b.nodes[other as usize].kind == LmType::Disjunctive {
                continue;
            }
            let oa = b.nodes[other as usize].atoms[0];
            if hps.interferes(oa, atom) {
                b.add_or_replace(other, n, OrderingType::Reasonable);
            }
        }
    }
    let hps_s = t3.elapsed().as_secs_f64();
    let mut g = Graph {
        min_first,
        min_possible,
        stats: Stats { explorations: reach.runs, rhw_s, achievers_s, hps_s, setup_s },
        ..Default::default()
    };
    for &n in &alive {
        let node = &b.nodes[n as usize];
        g.atoms.push(node.atoms.clone());
        g.disjunctive.push(node.kind == LmType::Disjunctive);
        g.goal.push(node.goal);
        let mut ch: Vec<(u32, OrderingType)> = node.children.iter().map(|(&c, &ty)| (id_of[c as usize], ty)).collect();
        ch.sort_unstable();
        let mut pa: Vec<(u32, OrderingType)> = node.parents.iter().map(|(&p, &ty)| (id_of[p as usize], ty)).collect();
        pa.sort_unstable();
        g.n_orderings += ch.len();
        g.children.push(ch);
        g.parents.push(pa);
        if node.kind == LmType::Disjunctive {
            g.n_disjunctive += 1;
        }
    }
    Ok(g)
}

impl Graph {
    pub fn len(&self) -> usize {
        self.atoms.len()
    }
    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }

    /// Fast Downward's debug dump of the graph (util.cc `dump_landmark_graph`), the orderings of each node by child id
    /// (Fast Downward prints them in its pointer map's order): `name(var, value)` gives an atom's name.
    pub fn dump(&self, init: &[u32], name: &dyn Fn(u32, u32) -> String) -> String {
        let mut s = String::from("digraph G {\n");
        for id in 0..self.len() {
            let delim = if self.disjunctive[id] { " | " } else { " & " };
            let label: Vec<String> = self.atoms[id].iter().map(|&(v, x)| name(v, x)).collect();
            s.push_str(&format!("  lm{} [label=\"{}\"", id, label.join(delim)));
            let tr = |f: &Fact| init[f.0 as usize] == f.1;
            let holds = if self.disjunctive[id] { self.atoms[id].iter().any(tr) } else { self.atoms[id].iter().all(tr) };
            if holds {
                s.push_str(", style=bold");
            }
            if self.goal[id] {
                s.push_str(", style=filled");
            }
            s.push_str("];\n");
            for &(c, ty) in &self.children[id] {
                let lab = match ty {
                    OrderingType::Necessary => "\"nec\"",
                    OrderingType::GreedyNecessary => "\"gn\"",
                    OrderingType::Natural => "\"n\"",
                    OrderingType::Reasonable => "\"r\", style=dashed",
                };
                s.push_str(&format!("      lm{} -> lm{} [label={}];\n", id, c, lab));
            }
        }
        s.push_str("}\n");
        s
    }
}
