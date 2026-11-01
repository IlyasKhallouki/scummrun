//! The switch shared_runs (experiments/E004-fast-exact/shared-runs.md has the claim and its proof): within a batch,
//! the click on a verb whose run never reads the verb runs once, and every other job that differs from it only in the
//! verb takes that run with the verb renamed. The files and counters are byte for byte those without the switch.

use crate::builder::RunOut;
use crate::machine::PKey;
use rustc_hash::FxHashMap;
use scumm_ir::{AssignOp, Expr, ExprId, Fact, Game, RefKind, Stmt, UnitId};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

enum State {
    /// a worker is making it
    Running,
    /// made: the run and the verb it was made for, or None when it read its verb (each job runs itself)
    Done(Option<(RunOut, i64)>),
}

/// The runs kept for the batch, by key ([`crate::machine::Reader`] makes the key).
#[derive(Default)]
pub struct SharedRuns {
    batch: AtomicU64,
    runs: Mutex<FxHashMap<String, (u64, State)>>,
    cv: Condvar,
    /// the keys, without their guides, whose run read its verb in an earlier batch: a job with one runs at once
    /// rather than wait for another's run (a choice of when to run; what is written is the same)
    refused: Mutex<rustc_hash::FxHashSet<String>>,
    /// each input script's instructions where `Local[1]` is written on every path ([`arg_written`])
    units: Mutex<FxHashMap<UnitId, Option<Arc<Vec<bool>>>>>,
    /// runs made and kept, taken by another job, made and not kept (they read their verb)
    pub made: AtomicU64,
    pub taken: AtomicU64,
    pub not_kept: AtomicU64,
    /// nanoseconds workers waited for a run another worker was making
    pub waited_ns: AtomicU64,
}

/// What a run with a key does.
pub enum Look {
    /// another job's run, and its verb
    Hit(RunOut, i64),
    /// this one makes the run and hands it on with [`SharedRuns::fill`]
    Mine,
    /// the first run with the key read its verb: run alone
    Alone,
}

impl SharedRuns {
    /// A batch starts: the runs kept go (a later batch may read other values).
    pub fn next_batch(&self) {
        self.batch.fetch_add(1, Ordering::SeqCst);
        self.runs.lock().unwrap().clear();
    }

    pub fn look(&self, key: &str, coarse: &str) -> Look {
        if self.refused.lock().unwrap().contains(coarse) {
            return Look::Alone;
        }
        let b = self.batch.load(Ordering::SeqCst);
        let t0 = std::time::Instant::now();
        let mut m = self.runs.lock().unwrap();
        loop {
            match m.get(key) {
                Some((kb, State::Running)) if *kb == b => {
                    m = self.cv.wait(m).unwrap();
                }
                Some((kb, State::Done(Some((out, v))))) if *kb == b => {
                    self.waited_ns.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
                    self.taken.fetch_add(1, Ordering::Relaxed);
                    return Look::Hit(out.clone(), *v);
                }
                Some((kb, State::Done(None))) if *kb == b => return Look::Alone,
                _ => {
                    m.insert(key.to_string(), (b, State::Running));
                    return Look::Mine;
                }
            }
        }
    }

    /// The run with `key` is made: `out` is it with its verb, or None when it read the verb.
    pub fn fill(&self, key: &str, coarse: &str, out: Option<(RunOut, i64)>) {
        let b = self.batch.load(Ordering::SeqCst);
        if out.is_some() {
            self.made.fetch_add(1, Ordering::Relaxed);
        } else {
            self.not_kept.fetch_add(1, Ordering::Relaxed);
            self.refused.lock().unwrap().insert(coarse.to_string());
        }
        self.runs.lock().unwrap().insert(key.to_string(), (b, State::Done(out)));
        self.cv.notify_all();
    }

    /// Condition 1: the unit's code never reads `Local[1]` before it writes it. Some(the instructions where it is
    /// written on every path from the unit's start), or None when some instruction may read it before (or reads a
    /// local through an index, or did not parse).
    pub fn arg_written(&self, game: &Game, u: UnitId) -> Option<Arc<Vec<bool>>> {
        if let Some(v) = self.units.lock().unwrap().get(&u) {
            return v.clone();
        }
        let v = arg_written(game, u, ARG).map(Arc::new);
        self.units.lock().unwrap().insert(u, v.clone());
        v
    }
}

/// Where `Local[n]` of unit `u` is written on every path from the unit's start (a must analysis over the IR's
/// control flow: `next`, `target`, `esc`), or None when an instruction may read it where it is not, reads a local
/// through an index, or did not parse. A plain `Local[n] = e` writes it after `e` is read; every other mention reads it.
pub fn arg_written(game: &Game, u: UnitId, n: i32) -> Option<Vec<bool>> {
    let ast = &game.ast;
    let ins = game.ins(u);
    let len = ins.len();
    let mut reads = vec![false; len];
    let mut writes = vec![false; len];
    for (i, x) in ins.iter().enumerate() {
        let st = x.st.as_ref()?;
        let (mut rd, mut bad) = (false, false);
        let mut visit = |e: ExprId| {
            match ast.expr(e) {
                Expr::Local(k) if k == n => rd = true,
                Expr::Index { kind: RefKind::Local, .. } => bad = true,
                _ => {}
            }
            true
        };
        match st {
            Stmt::Assign(a) if a.op == AssignOp::Set && matches!(ast.expr(a.lhs), Expr::Local(k) if k == n) => {
                ast.walk(a.rhs, &mut visit);
                writes[i] = true;
            }
            _ => {
                ast.walk_stmt(st, &mut visit);
            }
        }
        if bad {
            return None;
        }
        reads[i] = rd;
    }
    let mut def = vec![true; len];
    let starts: Vec<u32> = match game.unit(u).key {
        scumm_ir::UnitKey::Obj(_) => game.unit(u).entries.iter().map(|e| e.1).collect(),
        _ => vec![0],
    };
    for &s in &starts {
        if (s as usize) < len {
            def[s as usize] = false;
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..len {
            if def[i] || writes[i] {
                continue;
            }
            let x = &ins[i];
            for s in [x.next, x.target, x.esc].into_iter().flatten() {
                if (s as usize) < len && def[s as usize] {
                    def[s as usize] = false;
                    changed = true;
                }
            }
        }
    }
    (0..len).all(|i| !reads[i] || def[i]).then_some(def)
}

/// The input script's argument that holds the verb (machine.rs `eng_input`: area, verb, button).
pub const ARG: i32 = 1;

/// σ: a run made for verb `from`, as the run for verb `to` (shared-runs.md, the proof's last step).
pub fn rename(mut out: RunOut, from: i64, to: i64) -> RunOut {
    if from == to {
        return out;
    }
    let (a, b) = (format!("verb:{from}"), format!("verb:{to}"));
    let (fa, fb) = (Fact::Verb(from as i32), Fact::Verb(to as i32));
    for p in &mut out.paths {
        if let Some(s) = p.pre.remove(&a) {
            p.pre.insert(b.clone(), s);
        }
    }
    for h in &mut out.held {
        for (k, _) in h.guide.iter_mut() {
            if *k == PKey::F(fa) {
                *k = PKey::F(fb);
            }
        }
        if h.key == a {
            h.key = b.clone();
        }
    }
    for t in &mut out.touched {
        if *t == a {
            *t = b.clone();
        }
    }
    for q in &mut out.queries {
        if q.key == a {
            q.key = b.clone();
        }
    }
    if out.first_fork.as_deref() == Some(a.as_str()) {
        out.first_fork = Some(b);
    }
    out
}
