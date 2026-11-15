//! E014's switch FMI_READ_WIDEN (experiments/E014-fate-demo/plan.md, amendment (6)): a loop whose exit the machine
//! cannot decide is widened instead of unrolled until the path limit. Off (`Machine::widen_loops` false) nothing here
//! runs and every run is as before.
//!
//! The rule, at a join point a path comes back to in the same frame (a loop; `drive` calls it after `spins`):
//!
//! * the first visit keeps the path's writes and every frame's locals;
//! * at a later visit, each fact and local whose value changed since the last visit and is now unknown becomes a plain
//!   unknown of its own ("varies around the loop", new at each visit: a test the path made on an earlier one says
//!   nothing of it), and the point keeps it as widened; a known value that changed stays when the path took no
//!   unknown branch since the last visit (a loop whose tests all decide runs exactly), and is widened too when it did
//!   (the loop's exit is not known); every key widened before is widened again;
//! * a path back at the point whose every key that changed since the last visit is a widened one is an instance of the
//!   state it widened: it ends ("widened", infeasible as a spin is).
//!
//! Soundness: a plain unknown is any value, so the widened state holds every state the loop reaches from it; the
//! iteration run from it forks both ways on every unknown test, so its paths cover every concrete iteration from those
//! states; a path ended here is an instance of the widened state, and by induction over the iterations every concrete
//! way out of the loop is one of the widened iteration's. Records then hold guesses over the domain where a widened
//! fact is written (`Eff::Guess`): a superset of the values the engine can write.

use super::*;

/// The reason of a widened value: one model.py does not name.
pub const WIDENED: Reason = Reason::Other("widened loop");

/// The facts whose value differs between two stores (a key one holds and the other lacks differs), sorted.
pub fn diff_store(a: &Store, b: &Store) -> Vec<Fact> {
    let mut keys: Vec<Fact> =
        if a.shares_base(b) { a.changed().chain(b.changed()).copied().collect() } else { a.keys().chain(b.keys()).copied().collect() };
    keys.sort_unstable();
    keys.dedup();
    keys.retain(|k| a.get(k) != b.get(k));
    keys
}

/// The locals whose value differs between two stacks' frames, as (frame sid, local number); None when the frames
/// differ (other sids, or another number of frames).
pub fn diff_locals(a: &[(u32, Locals)], b: &[(u32, Locals)]) -> Option<Vec<(u32, i32)>> {
    if a.len() != b.len() || a.iter().zip(b).any(|(x, y)| x.0 != y.0) {
        return None;
    }
    let mut out = Vec::new();
    for ((sid, la), (_, lb)) in a.iter().zip(b) {
        if la == lb {
            continue;
        }
        let mut ns: Vec<i32> = la.iter().chain(lb.iter()).map(|l| l.0).collect();
        ns.sort_unstable();
        ns.dedup();
        out.extend(ns.into_iter().filter(|&n| la.get(n) != lb.get(n)).map(|n| (*sid, n)));
    }
    Some(out)
}

fn frames_locals(p: &Path) -> Vec<(u32, Locals)> {
    p.frames().map(|f| (f.sid, f.locals.clone())).collect()
}

fn put_visit(p: &mut Path, key: (UnitId, u32, u32), e: LoopVisit) {
    Rc::make_mut(p.loops.get_or_insert_with(Default::default)).insert(key, e);
}

impl Machine<'_> {
    /// The widening at a join point the path is at (its top a frame): true when the path ended ("widened").
    pub(super) fn widen(&mut self, p: &mut Path) -> bool {
        let Some(Entry::Frame(f)) = p.stack.last() else { return false };
        let (unit, sid) = (f.unit, f.sid);
        let Some(pc) = f.pc else { return false };
        let key = (unit, pc, sid);
        let now = frames_locals(p);
        let forks = p.assumptions.len();
        let Some(mut e) = p.loops.as_ref().and_then(|l| l.get(&key)).cloned() else {
            put_visit(p, key, LoopVisit { visits: 1, forks, writes: p.writes.clone(), locals: now, ..Default::default() });
            return false;
        };
        e.visits += 1;
        let facts = diff_store(&e.writes, &p.writes);
        let Some(locs) = diff_locals(&e.locals, &now) else {
            // other frames than at the last visit: the point starts again
            put_visit(p, key, LoopVisit { visits: 1, forks, writes: p.writes.clone(), locals: now, ..Default::default() });
            return false;
        };
        if facts.is_empty() && locs.is_empty() {
            // the same state as at the last visit: `spins` tells a repeat
            e.forks = forks;
            put_visit(p, key, e);
            return false;
        }
        let widened_before = !e.facts.is_empty() || !e.locs.is_empty();
        if widened_before && facts.iter().all(|k| e.facts.contains(k)) && locs.iter().all(|l| e.locs.contains(l)) {
            let at = format!("{}: the state again up to its widened values", self.game.cite(unit, pc));
            self.halt(p, "widened", rc(&at));
            return true;
        }
        // a known value is widened only when the path took an unknown branch since the last visit
        let forked = forks > e.forks;
        let local_now = |p: &Path, s: u32, n: i32| p.frames().find(|f| f.sid == s).and_then(|f| f.locals.get(n));
        for k in facts {
            if forked || matches!(p.writes.get(&k), Some(Val::Unk(_))) {
                e.facts.insert(k);
            }
        }
        for (s, n) in locs {
            if forked || matches!(local_now(p, s, n), Some(Val::Unk(_))) {
                e.locs.insert((s, n));
            }
        }
        let cite = self.game.cite(unit, pc);
        let mut fs: Vec<Fact> = e.facts.iter().copied().collect();
        fs.sort_unstable();
        for k in fs {
            let u = self.v.mk(WIDENED, format!("{k}: varies around the loop at {cite} (visit {})", e.visits));
            p.writes.insert(k, u);
        }
        let mut ls: Vec<(u32, i32)> = e.locs.iter().copied().collect();
        ls.sort_unstable();
        for (s, n) in ls {
            let u = self.v.mk(WIDENED, format!("Local[{n}] of frame {s}: varies around the loop at {cite} (visit {})", e.visits));
            for en in p.stack.iter_mut() {
                if let Entry::Frame(f) = en {
                    if f.sid == s {
                        f.set_local(n, u);
                    }
                }
            }
        }
        e.writes = p.writes.clone();
        e.locals = frames_locals(p);
        e.forks = forks;
        put_visit(p, key, e);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(items: &[(i32, i32)]) -> Store {
        let mut s = Store::default();
        for &(k, v) in items {
            s.insert(Fact::Var(k), Val::Int(v));
        }
        s
    }

    #[test]
    fn diff_of_stores() {
        let a = store(&[(1, 1), (2, 2), (3, 3)]);
        let mut b = a.clone();
        assert!(diff_store(&a, &b).is_empty());
        b.insert(Fact::Var(2), Val::Int(5));
        b.insert(Fact::Var(4), Val::Int(0));
        assert_eq!(diff_store(&a, &b), vec![Fact::Var(2), Fact::Var(4)]);
        // stores of other bases are compared key by key
        let c = store(&[(1, 1), (2, 5), (3, 3), (4, 0)]);
        assert_eq!(diff_store(&a, &c), vec![Fact::Var(2), Fact::Var(4)]);
        assert!(diff_store(&b, &c).is_empty());
    }

    #[test]
    fn diff_of_locals() {
        let l = |v: &[(i32, i32)]| Locals::from_vec(v.iter().map(|&(n, x)| (n, Val::Int(x))).collect());
        let a = vec![(1, l(&[(0, 1), (1, 2)])), (2, l(&[(0, 0)]))];
        let b = vec![(1, l(&[(0, 1), (1, 3)])), (2, l(&[(0, 0), (4, 1)]))];
        assert_eq!(diff_locals(&a, &b), Some(vec![(1, 1), (2, 4)]));
        assert_eq!(diff_locals(&a, &a), Some(vec![]));
        assert_eq!(diff_locals(&a, &b[..1]), None, "another number of frames");
        let c = vec![(1, l(&[])), (3, l(&[]))];
        assert_eq!(diff_locals(&a, &c), None, "another frame");
    }
}
