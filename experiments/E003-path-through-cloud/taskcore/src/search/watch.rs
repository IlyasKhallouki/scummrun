//! h_FF's exploration by watched preconditions (E011 round 2, plan component): the same pushes as FD's
//! `relaxed_exploration` (additive_heuristic.cc), in the same order, so the same queue, costs, `reached_by`, relaxed
//! plan and preferred operators, with far fewer steps.
//!
//! FD decrements a counter of every unary operator each time one of its preconditions is popped (on test (a)'s task
//! 3.2 M decrements an evaluation, for about 240 k operators that fire and about 1,000 pushes). Here:
//!
//! * **A watch.** Each unary operator watches one of its preconditions that is not popped yet. When the watched
//!   proposition is popped, the operator looks for another one not popped; if there is none, all its preconditions are
//!   popped and it fires now. So it fires exactly when FD's counter reaches 0: at the pop of its last precondition. The
//!   operators that fire at one pop are taken in increasing number, as FD takes them (its `precondition_of` list of the
//!   popped proposition is in operator order), and each tries its push then, with its cost
//!   `min(10^8, base + the sum of its preconditions' costs)`, which is FD's `increase_cost` sum (all terms are at least
//!   0, so clamping at each step or once at the end gives the same number). A popped proposition's cost is final (every
//!   push after a pop at d has a key above d: base costs are at least 1), so the sum is the one FD added up.
//! * **Dead operators.** At the pop of p with cost c, an operator watching p costs at least `base + c`, now or later
//!   (p is one of its preconditions, and later pops cost no less). If its effect already costs at most that, its push
//!   can never succeed in this evaluation (FD pushes only on a strictly lower cost, and a cost never rises): it is left
//!   out for the rest of the evaluation. A push that fails changes nothing in FD (no cost, no `reached_by`, no queue
//!   entry), so leaving it out changes nothing. The default watchers of a proposition are kept grouped by effect, with
//!   the group's least base cost, so a whole group is left out with one test.
//! * **Which precondition to watch** changes no result, only the work: the default is the precondition popped last in an
//!   earlier evaluation (one not popped then counts as last), so most operators are looked at once, when they fire,
//!   and those whose last precondition is never popped are never looked at. The defaults are built from the first
//!   evaluation's pops (the first uses the highest proposition number) and rebuilt when the guesses go stale.
//!
//! The rest of the evaluation (the setup's pushes, FD's queue, the dead-end test, the relaxed plan) is `Ff::eval`'s, the
//! same code in both modes. TASKCORE_CHECK_HFF set: every evaluation also runs FD's exploration, and the costs,
//! `reached_by` and the queue left behind must be equal (a queue holds the same entries in the same places only after
//! the same pushes and pops).

use super::{Ff, NO_OP, MAX_COST_VALUE};

const NONE: u32 = u32::MAX;

/// A default watcher: the operator, its base cost and its preconditions' range in `u_pre` (read in place of three
/// scattered arrays).
#[derive(Clone, Copy)]
struct SegOp {
    u: u32,
    base: i32,
    lo: u32,
    hi: u32,
}

pub(super) struct Watch {
    /// per proposition: its range of segments (its default watchers grouped by effect, in effect order)
    seg_off: Vec<u32>,
    seg_eff: Vec<u32>,
    /// the least base cost in the segment
    seg_min: Vec<i32>,
    seg_ops_off: Vec<u32>,
    /// the segment's operators, in increasing number, each with what its look reads
    seg_ops: Vec<SegOp>,
    /// the unary operators with no precondition, in increasing number
    zero: Vec<u32>,
    /// the evaluation (epoch) in which the proposition was popped, and its pop number then
    popped: Vec<u32>,
    pop_no: Vec<u32>,
    epoch: u32,
    /// the operators whose watch moved to a proposition in this evaluation: a list per proposition through `next`
    head: Vec<u32>,
    head_epoch: Vec<u32>,
    next: Vec<u32>,
    /// the operators that fire at this pop, with their costs
    fire: Vec<(u32, i32)>,
    /// evaluations since the defaults were built, and the last evaluation's moves and fires
    since: u64,
    pub moves: u64,
    pub fires: u64,
    pub looks: u64,
    pub segs: (u64, u64),
    pub builds: u64,
    policy: (u64, u64),
}

impl Watch {
    /// The watch structure of `ff`'s unary operators, each watching its precondition of highest `rank`.
    pub(super) fn new(ff: &Ff, rank: &[u32], policy: (u64, u64)) -> Watch {
        let n_props = ff.is_goal.len();
        let nu = ff.u_eff.len();
        let mut w = Watch {
            seg_off: Vec::new(),
            seg_eff: Vec::new(),
            seg_min: Vec::new(),
            seg_ops_off: Vec::new(),
            seg_ops: Vec::new(),
            zero: (0..nu as u32).filter(|&u| ff.u_pre_off[u as usize] == ff.u_pre_off[u as usize + 1]).collect(),
            popped: vec![0; n_props],
            pop_no: vec![0; n_props],
            epoch: 0,
            head: vec![NONE; n_props],
            head_epoch: vec![0; n_props],
            next: vec![NONE; nu],
            fire: Vec::new(),
            since: 0,
            moves: 0,
            fires: 0,
            looks: 0,
            segs: (0, 0),
            policy,
            builds: 0,
        };
        w.build(ff, rank);
        w
    }

    /// The default watches from `rank` (module docstring): segments per proposition.
    fn build(&mut self, ff: &Ff, rank: &[u32]) {
        let n_props = ff.is_goal.len();
        let nu = ff.u_eff.len();
        // each operator's watch, then the operators by (watch, effect, number): counting sorts, first on the effect
        // (taking the operators in increasing number), then, stably, on the watch
        let mut wof = vec![NONE; nu];
        let mut cnt = vec![0u32; n_props + 1];
        let mut ecnt = vec![0u32; n_props + 1];
        for u in 0..nu {
            let pre = &ff.u_pre[ff.u_pre_off[u] as usize..ff.u_pre_off[u + 1] as usize];
            if pre.is_empty() {
                continue;
            }
            let mut best = pre[0];
            let mut br = rank[best as usize];
            for &p in &pre[1..] {
                let r = rank[p as usize];
                if r >= br {
                    best = p;
                    br = r;
                }
            }
            wof[u] = best;
            cnt[best as usize + 1] += 1;
            ecnt[ff.u_eff[u] as usize + 1] += 1;
        }
        for i in 0..n_props {
            cnt[i + 1] += cnt[i];
            ecnt[i + 1] += ecnt[i];
        }
        let mut by_eff = vec![0u32; ecnt[n_props] as usize];
        for u in 0..nu {
            if wof[u] != NONE {
                let e = ff.u_eff[u] as usize;
                by_eff[ecnt[e] as usize] = u as u32;
                ecnt[e] += 1;
            }
        }
        let mut fill = cnt.clone();
        let mut by = vec![0u32; cnt[n_props] as usize];
        for &u in &by_eff {
            let p = wof[u as usize] as usize;
            by[fill[p] as usize] = u;
            fill[p] += 1;
        }
        self.seg_off.clear();
        self.seg_eff.clear();
        self.seg_min.clear();
        self.seg_ops_off.clear();
        self.seg_ops.clear();
        self.seg_off.push(0);
        self.seg_ops_off.push(0);
        self.seg_ops.reserve(by.len());
        for p in 0..n_props {
            let l = &by[cnt[p] as usize..cnt[p + 1] as usize];
            let mut i = 0;
            while i < l.len() {
                let e = ff.u_eff[l[i] as usize];
                let mut least = i32::MAX;
                while i < l.len() && ff.u_eff[l[i] as usize] == e {
                    let u = l[i] as usize;
                    least = least.min(ff.u_base[u]);
                    self.seg_ops.push(SegOp { u: u as u32, base: ff.u_base[u], lo: ff.u_pre_off[u], hi: ff.u_pre_off[u + 1] });
                    i += 1;
                }
                self.seg_eff.push(e);
                self.seg_min.push(least);
                self.seg_ops_off.push(self.seg_ops.len() as u32);
            }
            self.seg_off.push(self.seg_eff.len() as u32);
        }
        self.since = 0;
        self.builds += 1;
    }

    /// The defaults again from the last evaluation's pops, when they guess badly: an evaluation moved more watches than
    /// it fired operators, and at least `least`, at least `gap` evaluations after the last build (test (a)'s first
    /// search: 64 and 20,000 made the fewest instructions of the policies tried; TASKCORE_HFF_REBUILD=gap:least sets
    /// them; least 0: every `gap` evaluations).
    pub(super) fn maybe_rebuild(&mut self, ff: &Ff) {
        self.since += 1;
        let (gap, least) = self.policy;
        // (gap, 0): every `gap` evaluations
        let stale = self.since >= gap && self.moves >= least && (least == 0 || self.moves > self.fires + 1024);
        if !stale {
            return;
        }
        let epoch = self.epoch;
        let rank: Vec<u32> = (0..ff.is_goal.len()).map(|p| if self.popped[p] == epoch { self.pop_no[p] } else { NONE }).collect();
        self.build(ff, &rank);
    }
}

impl Ff {
    #[inline]
    fn push_if_lower(&mut self, p: u32, cost: i32, op: u32) {
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

    /// `setup_exploration_queue`, `setup_exploration_queue_state` and `relaxed_exploration` by watched preconditions
    /// (module docstring): the same pushes and pops as `explore_fd`.
    pub(super) fn explore_watch(&mut self, w: &mut Watch, s: &[u32]) {
        w.epoch = w.epoch.wrapping_add(1);
        if w.epoch == 0 {
            w.popped.iter_mut().for_each(|x| *x = 0);
            w.head_epoch.iter_mut().for_each(|x| *x = 0);
            w.epoch = 1;
        }
        let epoch = w.epoch;
        self.queue.clear();
        self.cost.iter_mut().for_each(|c| *c = -1);
        self.marked.iter_mut().for_each(|m| *m = false);
        for i in 0..w.zero.len() {
            let u = w.zero[i] as usize;
            self.push_if_lower(self.u_eff[u], self.u_base[u], u as u32);
        }
        for (v, &x) in s.iter().enumerate() {
            self.push_if_lower(self.prop_off[v] + x, 0, NO_OP);
        }
        let (mut moves, mut fires, mut looks, mut segs, mut live) = (0u64, 0u64, 0u64, 0u64, 0u64);
        let mut n_pop = 0u32;
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
            w.popped[p as usize] = epoch;
            w.pop_no[p as usize] = n_pop;
            n_pop += 1;
            w.fire.clear();
            let pc64 = pc as i64;
            // one watcher u of p (its effect e, base cost and preconditions): left out (dead), moved to a precondition not
            // popped yet, or fired. Dead also when the popped preconditions' costs read so far already bring it to
            // the effect's cost: the rest only add (module docstring)
            let u_pre = &self.u_pre;
            let mut look = |u: u32, e: usize, base: i32, pre: (u32, u32), w: &mut Watch, cost: &[i32]| {
                looks += 1;
                let ce = cost[e] as i64;
                let bound = if ce == -1 { i64::MAX } else { ce };
                let mut sum = base as i64;
                if sum + pc64 >= bound {
                    return;
                }
                let mut open = NONE;
                for &q in &u_pre[pre.0 as usize..pre.1 as usize] {
                    if w.popped[q as usize] != epoch {
                        open = q;
                        break;
                    }
                    sum += cost[q as usize] as i64;
                    if sum >= bound {
                        return;
                    }
                }
                match open {
                    NONE => w.fire.push((u, if sum > MAX_COST_VALUE as i64 { MAX_COST_VALUE } else { sum as i32 })),
                    q => {
                        let ui = u as usize;
                        moves += 1;
                        let qi = q as usize;
                        if w.head_epoch[qi] != epoch {
                            w.head_epoch[qi] = epoch;
                            w.head[qi] = NONE;
                        }
                        w.next[ui] = w.head[qi];
                        w.head[qi] = u;
                    }
                }
            };
            for sg in w.seg_off[p as usize] as usize..w.seg_off[p as usize + 1] as usize {
                let e = w.seg_eff[sg] as usize;
                let ce = self.cost[e];
                segs += 1;
                if ce != -1 && ce as i64 <= w.seg_min[sg] as i64 + pc64 {
                    continue;
                }
                live += 1;
                for i in w.seg_ops_off[sg] as usize..w.seg_ops_off[sg + 1] as usize {
                    let o = w.seg_ops[i];
                    look(o.u, e, o.base, (o.lo, o.hi), w, &self.cost);
                }
            }
            if w.head_epoch[p as usize] == epoch {
                let mut u = w.head[p as usize];
                while u != NONE {
                    let ui = u as usize;
                    let nx = w.next[ui];
                    let pre = (self.u_pre_off[ui], self.u_pre_off[ui + 1]);
                    look(u, self.u_eff[ui] as usize, self.u_base[ui], pre, w, &self.cost);
                    u = nx;
                }
            }
            if w.fire.is_empty() {
                continue;
            }
            w.fire.sort_unstable_by_key(|f| f.0);
            fires += w.fire.len() as u64;
            for i in 0..w.fire.len() {
                let (u, c) = w.fire[i];
                self.push_if_lower(self.u_eff[u as usize], c, u);
            }
        }
        w.moves = moves;
        w.fires = fires;
        w.looks = looks;
        w.segs = (segs, live);
    }
}
