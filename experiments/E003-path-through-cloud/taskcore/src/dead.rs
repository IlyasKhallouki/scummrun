//! Sound proofs that a request has no plan, before any search (E011, plan.md, amendment "targets with no plan").
//!
//! Each proof works on the SAS task the search would get (`search::SasTask`: Fast Downward's semantics, conditional
//! effects, no axioms) and answers "dead" only when no operator sequence from the start reaches the goal. None of
//! them can answer "dead" for a task with a plan; each can fail to prove a dead task dead ("open").
//!
//! * **h^1** (relaxed reachability, `h1`): the facts reachable when nothing is ever deleted. An effect can fire once
//!   its operator's preconditions and its own conditions are reachable. A goal fact outside the fixpoint is
//!   unreachable: every reachable state's facts are inside it (induction on the path).
//! * **h^2** (`H2::run`, Haslum and Geffner 2000; Alcázar and Torralba 2015 for SAS): the same fixpoint over pairs of
//!   facts. A pair is in the fixpoint when it holds at the start, or when some operator can produce it from a state
//!   whose pairs are all in the fixpoint. With conditional effects the cases are (`H2::run`'s comments): both facts
//!   written by effects that fire together, one written and one kept, or both kept. Every reachable state has all its
//!   pairs inside, by induction on the path, so a goal pair outside proves the goal unreachable.
//!
//! `prove` runs them in that order (h^1 is the cheaper one); `taskcore dead FILE.sas` runs them on a SAS file, and
//! checks the fast h^2 against the plain one (`--reference`) and a plan's states against the rows (`--plan`).

use crate::search::SasTask;
use std::time::Instant;

/// What `prove` found: the method that proves the goal unreachable (None: open), the goal fact or pair outside its
/// fixpoint (SAS variable, value), and each method's seconds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Proof {
    pub dead: Option<&'static str>,
    pub miss: Vec<(u32, u32)>,
    pub h1_s: f64,
    pub h2_s: f64,
    pub h2_passes: u32,
    /// false: h^2 stopped at its watchdog (it proves nothing then)
    pub h2_done: bool,
}

/// h^1, then h^2 when h^1 proves nothing (module docstring); `h2_deadline` is h^2's watchdog.
pub fn prove(t: &SasTask, h2_deadline: Option<Instant>) -> Proof {
    prove_keep(t, h2_deadline).0
}

/// `prove`, with the h^2 fixpoint when it ran (the checks: a plan's states against its rows, the plain fixpoint).
pub fn prove_keep(t: &SasTask, h2_deadline: Option<Instant>) -> (Proof, Option<H2>) {
    let mut p = Proof::default();
    let t0 = Instant::now();
    let fx = FactIdx::new(t);
    let r1 = h1(t);
    p.h1_s = t0.elapsed().as_secs_f64();
    if let Some(&(v, x)) = t.goal.iter().find(|&&(v, x)| !r1[fx.id(v, x) as usize]) {
        p.dead = Some("h1");
        p.miss = vec![(v, x)];
        return (p, None);
    }
    let h = H2::run(t, h2_deadline);
    p.h2_s = h.seconds;
    p.h2_passes = h.passes;
    p.h2_done = h.done;
    if let Some((a, b)) = h.goal_miss(t) {
        p.dead = Some("h2");
        let of = |f: u32| (fx.var_of[f as usize], f - fx.off[fx.var_of[f as usize] as usize]);
        p.miss = if a == b { vec![of(a)] } else { vec![of(a), of(b)] };
    }
    (p, Some(h))
}

/// Facts (variable, value) numbered by variable then value.
#[derive(Clone, Debug)]
pub struct FactIdx {
    pub off: Vec<u32>,
    pub var_of: Vec<u32>,
    pub n: usize,
}

impl FactIdx {
    pub fn new(t: &SasTask) -> FactIdx {
        let mut off = Vec::with_capacity(t.sizes.len() + 1);
        let mut var_of = Vec::new();
        let mut n = 0u32;
        for (v, &s) in t.sizes.iter().enumerate() {
            off.push(n);
            n += s;
            var_of.extend(std::iter::repeat(v as u32).take(s as usize));
        }
        off.push(n);
        FactIdx { off, var_of, n: n as usize }
    }
    #[inline]
    pub fn id(&self, v: u32, x: u32) -> u32 {
        self.off[v as usize] + x
    }
}

/// The task's operators as fact ids: preconditions, effects (conditions, written fact), the variables an
/// unconditional effect writes.
struct Ops {
    pre_off: Vec<u32>,
    pre: Vec<u32>,
    eff_off: Vec<u32>,
    eff_post: Vec<u32>,
    eff_var: Vec<u32>,
    cond_off: Vec<u32>,
    cond: Vec<u32>,
}

impl Ops {
    fn new(t: &SasTask, fx: &FactIdx) -> Ops {
        let mut o = Ops {
            pre_off: vec![0],
            pre: Vec::with_capacity(t.pre.len()),
            eff_off: vec![0],
            eff_post: Vec::with_capacity(t.effs.len()),
            eff_var: Vec::with_capacity(t.effs.len()),
            cond_off: vec![0],
            cond: Vec::with_capacity(t.conds.len()),
        };
        let mut buf = Vec::new();
        for op in 0..t.n_ops() {
            buf.clear();
            buf.extend(t.pre(op).iter().map(|&(v, x)| fx.id(v, x)));
            buf.sort_unstable();
            buf.dedup();
            o.pre.extend_from_slice(&buf);
            o.pre_off.push(o.pre.len() as u32);
            for e in t.effs(op) {
                buf.clear();
                buf.extend(t.conds(e).iter().map(|&(v, x)| fx.id(v, x)));
                buf.sort_unstable();
                buf.dedup();
                o.cond.extend_from_slice(&buf);
                o.cond_off.push(o.cond.len() as u32);
                o.eff_post.push(fx.id(e.var, e.post));
                o.eff_var.push(e.var);
            }
            o.eff_off.push(o.eff_post.len() as u32);
        }
        o
    }
    #[inline]
    fn pre(&self, op: usize) -> &[u32] {
        &self.pre[self.pre_off[op] as usize..self.pre_off[op + 1] as usize]
    }
    #[inline]
    fn effs(&self, op: usize) -> std::ops::Range<usize> {
        self.eff_off[op] as usize..self.eff_off[op + 1] as usize
    }
    #[inline]
    fn cond(&self, e: usize) -> &[u32] {
        &self.cond[self.cond_off[e] as usize..self.cond_off[e + 1] as usize]
    }
    fn n_ops(&self) -> usize {
        self.pre_off.len() - 1
    }
}

#[inline]
fn bit(w: &[u64], i: u32) -> bool {
    w[(i >> 6) as usize] >> (i & 63) & 1 == 1
}
#[inline]
fn set_bit(w: &mut [u64], i: u32) {
    w[(i >> 6) as usize] |= 1u64 << (i & 63);
}
fn clear_range(w: &mut [u64], a: u32, b: u32) {
    let mut i = a;
    while i < b {
        let k = (i >> 6) as usize;
        let lo = i & 63;
        let n = (64 - lo).min(b - i);
        let m = if n == 64 { u64::MAX } else { ((1u64 << n) - 1) << lo };
        w[k] &= !m;
        i += n;
    }
}

/// h^1: the facts reachable in the delete relaxation from the start (module docstring).
pub fn h1(t: &SasTask) -> Vec<bool> {
    let fx = FactIdx::new(t);
    let ops = Ops::new(t, &fx);
    let mut r = vec![false; fx.n];
    for (v, &x) in t.init.iter().enumerate() {
        r[fx.id(v as u32, x) as usize] = true;
    }
    loop {
        let mut changed = false;
        for op in 0..ops.n_ops() {
            if !ops.pre(op).iter().all(|&p| r[p as usize]) {
                continue;
            }
            for e in ops.effs(op) {
                let p = ops.eff_post[e] as usize;
                if !r[p] && ops.cond(e).iter().all(|&c| r[c as usize]) {
                    r[p] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            return r;
        }
    }
}

/// The h^2 fixpoint: `rows[f]` holds g when the pair {f, g} is reachable (f = g: f is reachable).
pub struct H2 {
    pub fx: FactIdx,
    pub w: usize,
    pub rows: Vec<u64>,
    pub passes: u32,
    pub seconds: f64,
    /// false: stopped at the deadline, the rows are not a fixpoint (and prove nothing)
    pub done: bool,
}

impl H2 {
    #[inline]
    pub fn row(&self, f: u32) -> &[u64] {
        &self.rows[f as usize * self.w..(f as usize + 1) * self.w]
    }
    #[inline]
    pub fn pair(&self, f: u32, g: u32) -> bool {
        bit(self.row(f), g)
    }
    #[inline]
    pub fn reached(&self, f: u32) -> bool {
        self.pair(f, f)
    }
    /// Sets {f, g} both ways; true when it was new.
    #[inline]
    fn add(&mut self, f: u32, g: u32) -> bool {
        let w = self.w;
        let a = &mut self.rows[f as usize * w..(f as usize + 1) * w];
        if bit(a, g) {
            return false;
        }
        set_bit(a, g);
        set_bit(&mut self.rows[g as usize * w..(g as usize + 1) * w], f);
        true
    }

    /// The fixpoint from the task's start, the fast way (the same rows as `run_reference`: the tests, and
    /// `taskcore dead --reference`):
    ///
    /// * an operator is looked at again only when a row it reads changed since it was last looked at (semi-naive
    ///   evaluation; the least fixpoint does not depend on the order);
    /// * its precondition's row is made once per look; its conditional effects are taken in groups with the same
    ///   conditions, one row per group;
    /// * pairs of two effects' facts that hold once and for all are added once: two unconditional effects (both fire
    ///   whenever the operator applies) when the operator first applies; an unconditional and a conditional effect,
    ///   or two of one group, when the group first fires; two groups once one's conditions are in the other's row
    ///   (rows only grow).
    ///
    /// `deadline` stops it (done false).
    pub fn run(t: &SasTask, deadline: Option<Instant>) -> H2 {
        let t0 = Instant::now();
        let fx = FactIdx::new(t);
        let ops = Ops::new(t, &fx);
        let n = fx.n;
        let w = (n + 63) / 64;
        let mut h = H2 { w, rows: vec![0u64; n * w], fx, passes: 0, seconds: 0.0, done: false };
        let init: Vec<u32> = t.init.iter().enumerate().map(|(v, &x)| h.fx.id(v as u32, x)).collect();
        for &a in &init {
            for &b in &init {
                h.add(a, b);
            }
        }
        let mut r1 = vec![0u64; w];
        for &a in &init {
            set_bit(&mut r1, a);
        }
        let n_ops = ops.n_ops();
        // per operator: its unconditional effects, and its conditional ones in groups of equal conditions
        let mut unc_off = vec![0u32; n_ops + 1];
        let mut unc: Vec<u32> = Vec::new();
        let mut grp_off = vec![0u32; n_ops + 1];
        let mut grp_eff_off: Vec<u32> = vec![0];
        let mut grp_eff: Vec<u32> = Vec::new();
        let mut cs: Vec<u32> = Vec::new();
        for op in 0..n_ops {
            cs.clear();
            for e in ops.effs(op) {
                if ops.cond(e).is_empty() {
                    unc.push(e as u32);
                } else {
                    cs.push(e as u32);
                }
            }
            cs.sort_by(|&a, &b| ops.cond(a as usize).cmp(ops.cond(b as usize)).then(a.cmp(&b)));
            for (i, &e) in cs.iter().enumerate() {
                if i > 0 && ops.cond(cs[i - 1] as usize) != ops.cond(e as usize) {
                    grp_eff_off.push(grp_eff.len() as u32);
                }
                grp_eff.push(e);
            }
            if !cs.is_empty() {
                grp_eff_off.push(grp_eff.len() as u32);
            }
            unc_off[op + 1] = unc.len() as u32;
            grp_off[op + 1] = (grp_eff_off.len() - 1) as u32;
        }
        let n_grp = grp_eff_off.len() - 1;
        let grp_effs = |g: usize| &grp_eff[grp_eff_off[g] as usize..grp_eff_off[g + 1] as usize];
        let grp_cond = |g: usize| ops.cond(grp_eff[grp_eff_off[g] as usize] as usize);
        // the change clock: ver[f] when row f last changed, r1_ver when a fact was last reached; seen[op] when the
        // operator was last looked at (u64::MAX: never); applied[op] once its unconditional pairs are in, fired[g] once
        // a group's pairs with the unconditional effects and within it are in; joined[op]: its group pairs that are in
        let mut clock: u64 = 1;
        let mut ver = vec![0u64; n];
        let mut r1_ver = 0u64;
        let mut seen = vec![u64::MAX; n_ops];
        let mut applied = vec![false; n_ops];
        let mut fired = vec![false; n_grp];
        let mut joined: Vec<Vec<u64>> = vec![Vec::new(); n_ops];
        let mut row_p = vec![0u64; w];
        let mut base = vec![0u64; w];
        let mut row_c: Vec<u64> = Vec::new();
        let mut fire_g: Vec<u32> = Vec::new();
        let off = h.fx.off.clone();
        let var_of = h.fx.var_of.clone();
        let prof = std::env::var_os("TASKCORE_PROFILE").is_some();
        // row(p) |= src \ (p's variable), both ways; true when something was new
        fn spread(h: &mut H2, ver: &mut [u64], clock: &mut u64, p: u32, lo: u32, hi: u32, src: &dyn Fn(usize) -> u64) -> bool {
            let w = h.w;
            let rp = p as usize * w;
            let mut any = false;
            for i in 0..w {
                let mut new = src(i) & !h.rows[rp + i];
                if new == 0 {
                    continue;
                }
                let (b0, b1) = ((i * 64) as u32, (i * 64 + 64) as u32);
                if lo < b1 && hi > b0 {
                    let a = lo.max(b0) - b0;
                    let z = hi.min(b1) - b0;
                    let m = if z - a == 64 { u64::MAX } else { ((1u64 << (z - a)) - 1) << a };
                    new &= !m;
                    if new == 0 {
                        continue;
                    }
                }
                h.rows[rp + i] |= new;
                if !any {
                    *clock += 1;
                    any = true;
                }
                while new != 0 {
                    let q = (i * 64) as u32 + new.trailing_zeros();
                    new &= new - 1;
                    set_bit(&mut h.rows[q as usize * w..(q as usize + 1) * w], p);
                    ver[q as usize] = *clock;
                }
            }
            if any {
                ver[p as usize] = *clock;
            }
            any
        }
        fn reach(p: u32, h: &mut H2, ver: &mut [u64], clock: &mut u64, r1: &mut [u64], r1_ver: &mut u64) -> bool {
            if bit(r1, p) {
                return false;
            }
            set_bit(r1, p);
            *clock += 1;
            *r1_ver = *clock;
            h.add(p, p);
            ver[p as usize] = *clock;
            true
        }
        fn pair_in(h: &mut H2, ver: &mut [u64], clock: &mut u64, p1: u32, p2: u32) -> bool {
            if !h.add(p1, p2) {
                return false;
            }
            *clock += 1;
            ver[p1 as usize] = *clock;
            ver[p2 as usize] = *clock;
            true
        }
        loop {
            h.passes += 1;
            let mut changed = false;
            let tp = Instant::now();
            let (mut n_looked, mut n_fired_g, mut n_joins) = (0u64, 0u64, 0u64);
            for op in 0..n_ops {
                let pre = ops.pre(op);
                let us = &unc[unc_off[op] as usize..unc_off[op + 1] as usize];
                let gs = grp_off[op] as usize..grp_off[op + 1] as usize;
                let s = seen[op];
                if s != u64::MAX {
                    let dirty = (pre.is_empty() && r1_ver > s)
                        || pre.iter().any(|&b| ver[b as usize] > s)
                        || gs.clone().any(|g| grp_cond(g).iter().any(|&c| ver[c as usize] > s));
                    if !dirty {
                        continue;
                    }
                }
                seen[op] = clock;
                n_looked += 1;
                // the precondition's row: the reachable facts paired with every precondition
                if pre.is_empty() {
                    row_p.copy_from_slice(&r1);
                } else {
                    if !pre.iter().all(|&b| bit(&r1, b)) {
                        continue;
                    }
                    row_p.copy_from_slice(h.row(pre[0]));
                    for &b in &pre[1..] {
                        let rb = &h.rows[b as usize * w..(b as usize + 1) * w];
                        for (x, y) in row_p.iter_mut().zip(rb) {
                            *x &= *y;
                        }
                    }
                    if !pre.iter().all(|&b| bit(&row_p, b)) {
                        continue;
                    }
                }
                // the kept facts' row: no variable an unconditional effect writes
                base.copy_from_slice(&row_p);
                for &e in us {
                    let v = ops.eff_var[e as usize] as usize;
                    clear_range(&mut base, off[v], off[v + 1]);
                }
                // the facts the unconditional effects write: alone, then with the kept facts
                for &e in us {
                    let p = ops.eff_post[e as usize];
                    changed |= reach(p, &mut h, &mut ver, &mut clock, &mut r1, &mut r1_ver);
                    let vp = var_of[p as usize] as usize;
                    changed |= spread(&mut h, &mut ver, &mut clock, p, off[vp], off[vp + 1], &|i| base[i]);
                }
                if !applied[op] {
                    applied[op] = true;
                    for (i, &e1) in us.iter().enumerate() {
                        for &e2 in &us[i + 1..] {
                            if ops.eff_var[e1 as usize] != ops.eff_var[e2 as usize] {
                                changed |= pair_in(&mut h, &mut ver, &mut clock, ops.eff_post[e1 as usize], ops.eff_post[e2 as usize]);
                            }
                        }
                    }
                }
                // the groups that can fire: their conditions in the precondition's row and paired (their row has them)
                fire_g.clear();
                for g in gs.clone() {
                    let c = grp_cond(g);
                    if !c.iter().all(|&b| bit(&row_p, b)) {
                        continue;
                    }
                    let k = fire_g.len();
                    if row_c.len() < (k + 1) * w {
                        row_c.resize((k + 1) * w, 0);
                    }
                    let re = &mut row_c[k * w..(k + 1) * w];
                    re.copy_from_slice(&row_p);
                    for &b in c {
                        let rb = &h.rows[b as usize * w..(b as usize + 1) * w];
                        for (x, y) in re.iter_mut().zip(rb) {
                            *x &= *y;
                        }
                    }
                    if !c.iter().all(|&b| bit(re, b)) {
                        continue;
                    }
                    fire_g.push(g as u32);
                    n_fired_g += 1;
                    let re = &row_c[k * w..(k + 1) * w];
                    for &e in grp_effs(g) {
                        let p = ops.eff_post[e as usize];
                        changed |= reach(p, &mut h, &mut ver, &mut clock, &mut r1, &mut r1_ver);
                        let vp = var_of[p as usize] as usize;
                        changed |= spread(&mut h, &mut ver, &mut clock, p, off[vp], off[vp + 1], &|i| base[i] & re[i]);
                    }
                    if !fired[g] {
                        fired[g] = true;
                        let es = grp_effs(g);
                        for (i, &e) in es.iter().enumerate() {
                            let p = ops.eff_post[e as usize];
                            for &u in us.iter().chain(&es[i + 1..]) {
                                if ops.eff_var[u as usize] != ops.eff_var[e as usize] {
                                    changed |= pair_in(&mut h, &mut ver, &mut clock, p, ops.eff_post[u as usize]);
                                }
                            }
                        }
                    }
                }
                // two fired groups: the second's conditions in the first's row, then every pair of their facts, once
                if fire_g.len() > 1 {
                    let g0 = grp_off[op] as usize;
                    let ng = gs.len();
                    for (i, &g1) in fire_g.iter().enumerate() {
                        let re1 = &row_c[i * w..(i + 1) * w];
                        for &g2 in &fire_g[i + 1..] {
                            let (a, b) = (g1 as usize - g0, g2 as usize - g0);
                            let ix = a * ng + b;
                            let jo = &mut joined[op];
                            if jo.is_empty() {
                                jo.resize((ng * ng + 63) / 64, 0);
                            }
                            if bit(jo, ix as u32) {
                                continue;
                            }
                            n_joins += 1;
                            if !grp_cond(g2 as usize).iter().all(|&c| bit(re1, c)) {
                                continue;
                            }
                            set_bit(jo, ix as u32);
                            for &e1 in grp_effs(g1 as usize) {
                                for &e2 in grp_effs(g2 as usize) {
                                    if ops.eff_var[e1 as usize] != ops.eff_var[e2 as usize] {
                                        changed |= pair_in(&mut h, &mut ver, &mut clock, ops.eff_post[e1 as usize], ops.eff_post[e2 as usize]);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if prof {
                eprintln!("h2 pass {}: {:.3} s, {} operators looked at, {} condition groups fired, {} group pairs checked",
                          h.passes, tp.elapsed().as_secs_f64(), n_looked, n_fired_g, n_joins);
            }
            if !changed {
                h.done = true;
                break;
            }
            if deadline.map_or(false, |d| Instant::now() > d) {
                break;
            }
        }
        h.seconds = t0.elapsed().as_secs_f64();
        h
    }

    /// The fixpoint the plain way (every operator in every pass): the reference `run` is tested against.
    pub fn run_reference(t: &SasTask, deadline: Option<Instant>) -> H2 {
        let t0 = Instant::now();
        let fx = FactIdx::new(t);
        let ops = Ops::new(t, &fx);
        let w = (fx.n + 63) / 64;
        let mut h = H2 { w, rows: vec![0u64; fx.n * w], fx, passes: 0, seconds: 0.0, done: false };
        let init: Vec<u32> = t.init.iter().enumerate().map(|(v, &x)| h.fx.id(v as u32, x)).collect();
        for &a in &init {
            for &b in &init {
                h.add(a, b);
            }
        }
        // the diagonal as one row (the reachable facts), kept in step with the rows
        let mut r1 = vec![0u64; w];
        for &a in &init {
            set_bit(&mut r1, a);
        }
        let mut row_p = vec![0u64; w];
        let mut row_e: Vec<u64> = Vec::new();
        let mut fire: Vec<usize> = Vec::new();
        let mut tmp = vec![0u64; w];
        loop {
            h.passes += 1;
            let mut changed = false;
            for op in 0..ops.n_ops() {
                // the precondition's pairs: row_p = the facts paired with every precondition (and reachable)
                row_p.copy_from_slice(&r1);
                let pre = ops.pre(op);
                let mut ok = true;
                for &b in pre {
                    if !bit(&r1, b) {
                        ok = false;
                        break;
                    }
                    let rb = h.row(b);
                    for (x, y) in row_p.iter_mut().zip(rb) {
                        *x &= *y;
                    }
                }
                if !ok || !pre.iter().all(|&b| bit(&row_p, b)) {
                    continue;
                }
                // the effects that can fire: their conditions paired with the precondition and each other
                let effs = ops.effs(op);
                fire.clear();
                row_e.resize(effs.len() * w, 0);
                for (k, e) in effs.clone().enumerate() {
                    let re = &mut row_e[k * w..(k + 1) * w];
                    re.copy_from_slice(&row_p);
                    let c = ops.cond(e);
                    let mut ok = true;
                    for &b in c {
                        if !bit(re, b) {
                            ok = false;
                            break;
                        }
                        let rb = &h.rows[b as usize * w..(b as usize + 1) * w];
                        for (x, y) in re.iter_mut().zip(rb) {
                            *x &= *y;
                        }
                    }
                    if ok && c.iter().all(|&b| bit(re, b)) {
                        fire.push(k);
                    }
                }
                if fire.is_empty() {
                    continue;
                }
                // a fired effect's fact p with a kept fact q: q paired with the precondition and p's conditions, on a
                // variable no unconditional effect of the operator writes (that one is written, not kept) and not
                // p's variable
                for &k in &fire {
                    let e = effs.start + k;
                    let p = ops.eff_post[e];
                    tmp.copy_from_slice(&row_e[k * w..(k + 1) * w]);
                    for e2 in effs.clone() {
                        if ops.cond(e2).is_empty() {
                            let v = ops.eff_var[e2];
                            clear_range(&mut tmp, h.fx.off[v as usize], h.fx.off[v as usize + 1]);
                        }
                    }
                    let v = h.fx.var_of[p as usize];
                    clear_range(&mut tmp, h.fx.off[v as usize], h.fx.off[v as usize + 1]);
                    if !bit(&r1, p) {
                        set_bit(&mut r1, p);
                        h.add(p, p);
                        changed = true;
                    }
                    let rp = p as usize * w;
                    for i in 0..w {
                        let mut new = tmp[i] & !h.rows[rp + i];
                        if new == 0 {
                            continue;
                        }
                        h.rows[rp + i] |= new;
                        changed = true;
                        while new != 0 {
                            let q = (i * 64) as u32 + new.trailing_zeros();
                            new &= new - 1;
                            set_bit(&mut h.rows[q as usize * w..(q as usize + 1) * w], p);
                        }
                    }
                }
                // two fired effects' facts: the second's conditions paired with the first's row
                for (i, &k1) in fire.iter().enumerate() {
                    let e1 = effs.start + k1;
                    let p1 = ops.eff_post[e1];
                    for &k2 in &fire[i + 1..] {
                        let e2 = effs.start + k2;
                        let p2 = ops.eff_post[e2];
                        if ops.eff_var[e1] == ops.eff_var[e2] || h.pair(p1, p2) {
                            continue;
                        }
                        let re1 = &row_e[k1 * w..(k1 + 1) * w];
                        if ops.cond(e2).iter().all(|&c| bit(re1, c)) {
                            h.add(p1, p2);
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                h.done = true;
                break;
            }
            if deadline.map_or(false, |d| Instant::now() > d) {
                break;
            }
        }
        h.seconds = t0.elapsed().as_secs_f64();
        h
    }

    /// Some(the first goal fact or pair outside the fixpoint): the goal is unreachable. None: h^2 proves nothing.
    pub fn goal_miss(&self, t: &SasTask) -> Option<(u32, u32)> {
        if !self.done {
            return None;
        }
        let g: Vec<u32> = t.goal.iter().map(|&(v, x)| self.fx.id(v, x)).collect();
        for (i, &a) in g.iter().enumerate() {
            for &b in &g[i..] {
                if !self.pair(a, b) {
                    return Some((a, b));
                }
            }
        }
        None
    }
}

/// `taskcore dead FILE.sas [--reference] [--plan FILE]`: the proofs on a SAS file (Fast Downward's translator
/// output of the shape taskcore writes); one JSON line. `--reference`: also the plain h^2, compared row for row.
/// `--plan FILE`: a plan of the task (`sas_plan`), each of its states checked against the h^2 rows.
pub fn dead_cmd(args: &[String]) -> Result<(), String> {
    let path = args.first().ok_or("usage: taskcore dead FILE.sas [--reference] [--plan FILE]")?;
    let t0 = Instant::now();
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path, e))?;
    let t = crate::search::parse_sas(&text)?;
    let parse_s = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let r1 = h1(&t);
    let fx = FactIdx::new(&t);
    let h1_goal = t.goal.iter().all(|&(v, x)| r1[fx.id(v, x) as usize]);
    let h1_s = t1.elapsed().as_secs_f64();
    let h = H2::run(&t, None);
    let pairs: u64 = h.rows.iter().map(|w| w.count_ones() as u64).sum();
    let miss = h.goal_miss(&t);
    let mut out = format!(
        "{{\"file\": {:?}, \"facts\": {}, \"operators\": {}, \"parse_s\": {:.3}, \"h1_goal\": {}, \"h1_s\": {:.3}, \"h2_done\": {}, \"h2_passes\": {}, \"h2_s\": {:.3}, \"h2_pairs\": {}, \"h2_miss\": {}",
        path,
        fx.n,
        t.n_ops(),
        parse_s,
        h1_goal,
        h1_s,
        h.done,
        h.passes,
        h.seconds,
        pairs,
        miss.map_or("null".into(), |(a, b)| format!("[{}, {}]", a, b))
    );
    if args.iter().any(|a| a == "--reference") {
        let r = H2::run_reference(&t, None);
        out += &format!(", \"reference_s\": {:.3}, \"reference_passes\": {}, \"rows_equal\": {}", r.seconds, r.passes, r.rows == h.rows);
    }
    if let Some(i) = args.iter().position(|a| a == "--plan") {
        let p = args.get(i + 1).ok_or("--plan FILE")?;
        let plan = crate::search::read_plan(&std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p, e))?)?;
        let r = h.check_path(&t, &plan);
        out += &format!(", \"plan_steps\": {}, \"plan_states_in_h2\": {}", plan.len(), r.map_or_else(|e| format!("{:?}", e), |_| "true".into()));
    }
    println!("{}}}", out);
    Ok(())
}

impl H2 {
    /// The states of `plan` from the start, each checked against the rows: every fact reached and every pair of facts
    /// in the fixpoint (h^2's invariant, module docstring). Err(the first state and pair outside).
    pub fn check_path(&self, t: &SasTask, plan: &[u32]) -> Result<(), String> {
        let mut s = t.init.clone();
        let mut next = Vec::new();
        for i in 0..=plan.len() {
            let f: Vec<u32> = s.iter().enumerate().map(|(v, &x)| self.fx.id(v as u32, x)).collect();
            for (a, &p) in f.iter().enumerate() {
                for &q in &f[a..] {
                    if !self.pair(p, q) {
                        return Err(format!("state {} of the plan: the pair ({}, {}) is outside h^2", i, p, q));
                    }
                }
            }
            if i == plan.len() {
                break;
            }
            let o = plan[i] as usize;
            if !t.applicable(o, &s) {
                return Err(format!("step {}: a{} is not applicable", i, o));
            }
            t.apply_into(o, &s, &mut next);
            std::mem::swap(&mut s, &mut next);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{EffSpec, SasTask};
    use std::collections::{HashSet, VecDeque};

    fn task(sizes: &[u32], init: &[u32], goal: &[(u32, u32)], ops: &[(Vec<(u32, u32)>, Vec<EffSpec>)]) -> SasTask {
        let mut t = SasTask {
            sizes: sizes.to_vec(),
            names: (0..sizes.len()).map(|i| format!("v{}", i)).collect(),
            init: init.to_vec(),
            goal: goal.to_vec(),
            pre_off: vec![0],
            eff_off: vec![0],
            ..Default::default()
        };
        // as SasTask::push_op builds an operator
        for (pre, effs) in ops {
            t.pre.extend_from_slice(pre);
            t.pre_off.push(t.pre.len() as u32);
            for (c, var, post) in effs {
                let c0 = t.conds.len() as u32;
                t.conds.extend_from_slice(c);
                t.effs.push(crate::search::Eff { c0, c1: t.conds.len() as u32, var: *var, post: *post });
            }
            t.eff_off.push(t.effs.len() as u32);
            t.cost.push(1);
        }
        t
    }

    /// Every state reachable from the start (breadth first).
    fn reachable(t: &SasTask) -> Vec<Vec<u32>> {
        let mut seen: HashSet<Vec<u32>> = HashSet::new();
        let mut q = VecDeque::new();
        seen.insert(t.init.clone());
        q.push_back(t.init.clone());
        let mut out = Vec::new();
        let mut next = Vec::new();
        while let Some(s) = q.pop_front() {
            for o in 0..t.n_ops() {
                if t.applicable(o, &s) {
                    t.apply_into(o, &s, &mut next);
                    if seen.insert(next.clone()) {
                        q.push_back(next.clone());
                    }
                }
            }
            out.push(s);
        }
        out
    }

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, n: u32) -> u32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 33) % n as u64) as u32
        }
    }

    fn random_task(r: &mut Lcg) -> SasTask {
        let nv = 2 + r.next(5) as usize;
        let sizes: Vec<u32> = (0..nv).map(|_| 2 + r.next(3)).collect();
        let init: Vec<u32> = sizes.iter().map(|&s| r.next(s)).collect();
        let gv = r.next(nv as u32);
        let mut goal = vec![(gv, r.next(sizes[gv as usize]))];
        if r.next(3) == 0 {
            let g2 = r.next(nv as u32);
            if g2 != gv {
                goal.push((g2, r.next(sizes[g2 as usize])));
                goal.sort();
            }
        }
        let n_ops = 2 + r.next(12) as usize;
        let mut ops = Vec::new();
        for _ in 0..n_ops {
            let mut pre = Vec::new();
            for v in 0..nv as u32 {
                if r.next(10) < 3 {
                    pre.push((v, r.next(sizes[v as usize])));
                }
            }
            let mut effs: Vec<EffSpec> = Vec::new();
            for _ in 0..1 + r.next(4) {
                let v = r.next(nv as u32);
                let mut c = Vec::new();
                if r.next(2) == 0 {
                    for _ in 0..1 + r.next(2) {
                        let cv = r.next(nv as u32);
                        if !c.iter().any(|&(x, _)| x == cv) {
                            c.push((cv, r.next(sizes[cv as usize])));
                        }
                    }
                    c.sort();
                }
                effs.push((c, v, r.next(sizes[v as usize])));
            }
            ops.push((pre, effs));
        }
        task(&sizes, &init, &goal, &ops)
    }

    #[test]
    fn h2_proves_a_mutex_goal_that_h1_does_not() {
        // v0 and v1 swap a token: exactly one of them holds it, so v0 = 1 and v1 = 1 never hold together
        let t = task(
            &[2, 2],
            &[1, 0],
            &[(0, 1), (1, 1)],
            &[
                (vec![(0, 1), (1, 0)], vec![(vec![], 0, 0), (vec![], 1, 1)]),
                (vec![(0, 0), (1, 1)], vec![(vec![], 0, 1), (vec![], 1, 0)]),
            ],
        );
        let p = prove(&t, None);
        assert_eq!(p.dead, Some("h2"));
        assert_eq!(p.miss, vec![(0, 1), (1, 1)]);
        assert!(h1(&t).iter().all(|&b| b));
    }

    #[test]
    fn a_conditional_effect_keeps_or_writes() {
        // a conditional write of v1 when v2 = 1: v0 = 1 and v1 = 1 come together only through it
        let t = task(
            &[2, 2, 2],
            &[0, 0, 0],
            &[(0, 1), (1, 1)],
            &[
                (vec![], vec![(vec![], 2, 1)]),
                (vec![(0, 0)], vec![(vec![], 0, 1), (vec![(2, 1)], 1, 1)]),
            ],
        );
        assert_eq!(prove(&t, None).dead, None);
        // without the operator that sets v2, the condition never holds: v1 = 1 is unreachable even relaxed
        let t2 = task(
            &[2, 2, 2],
            &[0, 0, 0],
            &[(0, 1), (1, 1)],
            &[(vec![(0, 0)], vec![(vec![], 0, 1), (vec![(2, 1)], 1, 1)])],
        );
        assert_eq!(prove(&t2, None).dead, Some("h1"));
    }

    #[test]
    fn random_tasks_fast_equals_plain_and_every_reachable_state_is_inside() {
        let mut r = Lcg(0x5eed_d0d0);
        let (mut dead, mut live) = (0, 0);
        for _ in 0..20000 {
            let t = random_task(&mut r);
            let h = H2::run(&t, None);
            let hr = H2::run_reference(&t, None);
            assert!(h.done && hr.done);
            assert_eq!(h.rows, hr.rows, "fast and plain h^2 differ on {:?}", t);
            let states = reachable(&t);
            for s in &states {
                let f: Vec<u32> = s.iter().enumerate().map(|(v, &x)| h.fx.id(v as u32, x)).collect();
                for &a in &f {
                    for &b in &f {
                        assert!(h.pair(a, b), "a reachable state's pair is outside h^2 on {:?}", t);
                    }
                }
            }
            let solvable = states.iter().any(|s| t.is_goal(s));
            let p = prove(&t, None);
            if solvable {
                assert_eq!(p.dead, None, "a proof claims no plan for a task with one: {:?}", t);
                live += 1;
            } else if p.dead.is_some() {
                dead += 1;
            }
        }
        // the tests mean something: both kinds occur
        assert!(dead > 1000 && live > 1000, "dead {} live {}", dead, live);
    }
}
