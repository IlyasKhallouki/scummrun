//! Feature settle (row 3): time after the action's immediate part, in rounds (model.py `run_wait`, the settle
//! parts of `run_sentence`, and the settle methods: `_pre_slots`, `_slots_sig`, `_position_key`, `_state_key`,
//! `_changed`, `_unsettle`, `_next_round`, `_round_order`, `_round_end`, `_round_conflicts`, `_loosen`,
//! `_close_settle`, `_unit_named`, `_slot_frame`, `_changed_any`, `_pre_rule`, `_origin`, `_skip`, `_time_wait`,
//! `_eng_turn`, `_eng_sentences`, `_sentence_busy`).
//!
//! Each method is the model.py method of the same name, in the same order of reads, writes, notes, forks and
//! ends. model.py's module docstring (feature settle) holds the rules and the ScummVM cites.
//!
//! model.py's state hashes (`_position_key`, `_state_key`, `round_slots`) are Python `hash()` values that it
//! compares as ints. The port keeps 64-bit hashes of the same contents, with Python's one structural collision:
//! `hash(-1) == hash(-2)` (`value::py_int`, used by `Val`'s hash). A frozenset is hashed as a sum of its items'
//! hashes, so that equal sets hash alike in any order.

use super::calls::sentence_slots;
use super::*;
use rustc_hash::{FxHashSet, FxHasher};
use scumm_ir::analysis::{LoopKind, Read, WaitKind, WaitPoint};
use std::hash::{Hash, Hasher};

/// The head of a settle round's Call (model.py `ROUND`).
pub const ROUND: &str = "runAllScripts";
const ROUND_CITE: &str = "script.cpp:968-987; scumm.cpp:3231-3255";
const RULE_A: &str = "a: started or resumed by the path";
/// model.py `R_SETTLE`, as `Path.writer` keeps it
const R_SETTLE: &str = "not settled";
/// model.py `MOUSE_VARS`, sorted by number.
pub(super) const MOUSE_VARS: [(i32, &str); 4] =
    [(20, "VAR_VIRT_MOUSE_X"), (21, "VAR_VIRT_MOUSE_Y"), (44, "VAR_MOUSE_X"), (45, "VAR_MOUSE_Y")];

/// The slot fact of a unit name: script:N and local:N stay, obj:O is objscript:O, entry and exit code have none.
pub(super) fn slot_key(name: Slot) -> Option<Fact> {
    match name {
        Slot::Script(n) => Some(Fact::Script(n)),
        Slot::Local(n) => Some(Fact::Local(n)),
        Slot::Obj(o) => Some(Fact::ObjScript(o)),
        Slot::Entry | Slot::Exit => None,
    }
}

/// model.py `POSITION_KEYS` and `POSITION_PREFIXES`: the facts a merge at a round's start keeps exact.
fn is_position_key(k: Fact) -> bool {
    matches!(
        k,
        Fact::Room
            | Fact::Var(VAR_ROOM_RESOURCE)
            | Fact::Cutscene
            | Fact::Script(_)
            | Fact::Local(_)
            | Fact::ObjScript(_)
            | Fact::At(..)
            | Fact::Frozen(_)
            | Fact::Esc(..)
    )
}

/// Whether key `k` starts with `kind` + ":" (an exit test's read of any key of a kind, `Read::Any`).
fn kind_matches(kind: &str, k: Fact) -> bool {
    match kind {
        "var" => matches!(k, Fact::Var(_)),
        "bit" => matches!(k, Fact::Bit(_)),
        "owner" => matches!(k, Fact::Owner(_)),
        "state" => matches!(k, Fact::State(_)),
        "class" => matches!(k, Fact::Class(..)),
        "script" => matches!(k, Fact::Script(_)),
        "local" => matches!(k, Fact::Local(_)),
        _ => {
            let s = k.to_string();
            s.starts_with(kind) && s.as_bytes().get(kind.len()) == Some(&b':')
        }
    }
}

/// An order-free hash of a set's items (model.py's frozensets).
fn set_hash<T: Hash>(items: impl Iterator<Item = T>) -> u64 {
    let (mut s, mut n) = (0u64, 0u64);
    for t in items {
        s = s.wrapping_add(fx(&t));
        n += 1;
    }
    fmix(s ^ n.wrapping_mul(0x9e37_79b9_7f4a_7c15))
}

/// A queue entry's hash, its ints as Python hashes them.
fn queue_hash(q: &[(i32, i32, i32, i32)], h: &mut FxHasher) {
    q.len().hash(h);
    for &(a, b, c, d) in q {
        (py_int(a), py_int(b), py_int(c), py_int(d)).hash(h);
    }
}

/// Unit names sorted as model.py sorts their strings.
fn sort_names(names: &mut [Slot]) {
    names.sort_by_cached_key(|n| n.to_string());
}

impl<'g> Machine<'g> {
    // --- values against the oracle ------------------------------------------------------------------------

    /// `v == oracle.get(k)`, without making the oracle's unknown when it is one.
    pub(super) fn eq_oracle(&self, v: Val, o: &FactOracle, k: Fact) -> bool {
        let reason = match o.get(k) {
            OVal::Int(n) => return v == Val::Int(n),
            OVal::Held => Reason::Held,
            OVal::Timer => Reason::Timer,
        };
        match self.v.unk(v) {
            Some(u) => u.reason == reason && u.values.is_none() && *u.what == *k.to_string(),
            None => false,
        }
    }

    /// `store.get(k, oracle.get(k))`.
    fn store_or_oracle(&mut self, s: &Store, o: &FactOracle, k: Fact) -> Val {
        match s.get(&k) {
            Some(&v) => v,
            None => self.oget(o, k),
        }
    }

    /// `p.writes[k] = v` with no read or write log (model.py's direct writes to the store).
    pub(super) fn put_raw(p: &mut Path, k: Fact, v: Val) {
        p.writes.insert(k, v);
    }

    // --- entry points -------------------------------------------------------------------------------------

    /// Feature settle: the wait action ('.'), settle from pre with no immediate part (model.py `run_wait`).
    pub fn run_wait(&mut self, o: &FactOracle) -> Run {
        assert!(self.settles, "a wait runs only with settle");
        self.pre = self.pre_slots(o);
        let mut p = Path::new(Vec::new());
        p.settle = true;
        p.first_end = Some("wait");
        p.immediate = Some(Rc::new(Store::default()));
        self.next_round(&mut p, o);
        self.drive(o, p, "wait: runAllScripts each round (script.cpp:968-987)".to_string(), None)
    }

    /// The settle steps of `run_sentence`'s call: the mouse variables, the notes, and every slot live at pre's
    /// turn in the order of their keys (a live slot of the sentence script's number by its resume step; one not
    /// live at pre is dropped).
    pub(super) fn sentence_frame_steps(&mut self, o: &FactOracle, steps: Vec<Step>) -> Vec<Step> {
        let mut out = Vec::new();
        for (v, name) in MOUSE_VARS {
            let u = self.mouse_value(o, v, name);
            out.push(Step::Write(Fact::Var(v), u));
        }
        out.extend(steps.iter().filter(|st| matches!(st, Step::Note(_))).cloned());
        for name in self.pre_slots(o) {
            let resumed = steps.iter().find(|st| matches!(st, Step::Resume(k) if slot_name(*k) == name));
            out.push(resumed.cloned().unwrap_or(Step::Turn(name)));
        }
        out
    }

    // --- the state between frames -------------------------------------------------------------------------

    /// The unit names of the slots live at pre, in the order of their fact keys (model.py `_pre_slots`).
    pub(super) fn pre_slots(&self, o: &FactOracle) -> Vec<Slot> {
        let mut keys: Vec<(String, Slot)> = Vec::new();
        for pre in [Prefix::Script, Prefix::Local, Prefix::ObjScript] {
            for &k in o.keys(pre) {
                if o.get(k) == OVal::Int(1) {
                    keys.push((k.to_string(), slot_name(k)));
                }
            }
        }
        if o.keys(Prefix::At(Slot::Entry)).iter().any(|&k| o.get(k) == OVal::Int(1)) {
            keys.push(("at:entry".to_string(), Slot::Entry));
        }
        keys.sort();
        keys.into_iter().map(|k| k.1).collect()
    }

    /// `_slots_sig(p, full=False)`: where each kept slot resumes and by which rule.
    pub(super) fn slots_sig_short(p: &Path) -> Vec<(Slot, UnitId, Option<u32>, &'static str)> {
        p.slots.iter().map(|(n, f)| (*n, f.unit, f.pc, f.origin)).collect()
    }

    /// `hash(_slots_sig(p))`: also each kept slot's locals and flags.
    pub(super) fn slots_hash(p: &Path) -> u64 {
        let mut h = FxHasher::default();
        p.slots.len().hash(&mut h);
        for (n, f) in &p.slots {
            (n, f.unit, f.pc, f.locals.as_slice(), f.args_known, f.origin, f.owned, f.recursive, f.resistant).hash(&mut h);
        }
        fmix(h.finish())
    }

    fn freeze_hash(p: &Path) -> u64 {
        set_hash(p.freeze.iter().map(|(n, c)| (n, c.map(|(k, e)| (py_int(k), e)))))
    }

    /// model.py `_position_key`: what paths at a round's start must share to merge.
    pub(super) fn position_key(&self, p: &Path, o: &FactOracle) -> u64 {
        let slots = set_hash(p.writes.iter().filter(|(&k, &v)| is_position_key(k) && !self.eq_oracle(v, o, k)));
        let mut h = FxHasher::default();
        slots.hash(&mut h);
        Self::slots_sig_short(p).hash(&mut h);
        Self::freeze_hash(p).hash(&mut h);
        queue_hash(&p.queue, &mut h);
        set_hash(p.ambient_done.iter()).hash(&mut h);
        set_hash(p.camera_done.iter()).hash(&mut h);
        p.cut_index.hash(&mut h);
        p.cut_data.len().hash(&mut h);
        fmix(h.finish())
    }

    /// model.py `_state_key`: the path's state between two frames.
    pub(super) fn state_key(&self, p: &Path) -> u64 {
        let mut h = FxHasher::default();
        (p.writes.hash_sum(), p.writes.len()).hash(&mut h);
        Self::slots_hash(p).hash(&mut h);
        Self::freeze_hash(p).hash(&mut h);
        queue_hash(&p.queue, &mut h);
        p.cut_data.hash(&mut h);
        p.cut_index.hash(&mut h);
        set_hash(p.ambient_done.iter()).hash(&mut h);
        set_hash(p.camera_done.iter()).hash(&mut h);
        fmix(h.finish())
    }

    /// model.py `_changed(p, before)`, as a truth value: whether any fact differs between `before` and the path's
    /// writes (pre's value where one does not hold it). Only its truth is read.
    fn changed(&self, p: &Path, o: &FactOracle, before: &Store) -> bool {
        for (&k, &v) in p.writes.iter() {
            let same = match before.get(&k) {
                Some(&b) => b == v,
                None => self.eq_oracle(v, o, k),
            };
            if !same {
                return true;
            }
        }
        before.iter().any(|(&k, &b)| !p.writes.contains_key(&k) && !self.eq_oracle(b, o, k))
    }

    /// model.py `_unsettle`: each fact that differs between the snapshots becomes unknown with the values it took.
    fn unsettle(&mut self, p: &mut Path, o: &FactOracle, snaps: &[Rc<Store>], why: &str) {
        let mut keys: Vec<Fact> = snaps.iter().flat_map(|s| s.keys().copied()).collect();
        keys.sort_unstable();
        keys.dedup();
        sort_str(&mut keys);
        for k in keys {
            let vals: Vec<Val> = snaps.iter().map(|s| self.store_or_oracle(s, o, k)).collect();
            let last = *vals.last().unwrap();
            if vals.iter().all(|&v| v == last) {
                continue;
            }
            let what = format!("{k} {why}");
            let v = self.settle_unknown(&vals, what);
            Self::put_raw(p, k, v);
            p.writer.insert(k, R_SETTLE);
        }
    }

    /// `_unsettle`'s and `_loosen`'s value: a plain R_SETTLE unknown when a value is one, else the values' set (a
    /// set of one when they agree).
    fn settle_unknown(&mut self, vals: &[Val], what: String) -> Val {
        if vals.iter().any(|&v| self.v.is_plain(v)) {
            return self.v.mk(Reason::Settle, what);
        }
        let mut all = Vec::new();
        for &v in vals {
            all.extend(self.v.values_of(v).expect("no plain unknown"));
        }
        match self.v.one_of(all, Reason::Settle, what.clone()) {
            Val::Int(n) => self.v.mk_set(Reason::Settle, what, vec![n]),
            u => u,
        }
    }

    /// model.py `_loosen`: fact `k` becomes unknown with the values `vals`, unless they agree on a known one.
    fn loosen(&mut self, p: &mut Path, k: Fact, vals: &[Val], why: &str) {
        if vals[1..].iter().all(|&v| v == vals[0]) && !vals[0].is_unknown() {
            return;
        }
        let v = self.settle_unknown(vals, format!("{k}: {why}"));
        Self::put_raw(p, k, v);
        p.writer.insert(k, R_SETTLE);
    }

    /// model.py `_close_settle`: what a path that reached settle can no longer claim when it ends.
    pub(super) fn close_settle(&mut self, p: &mut Path, o: &FactOracle) {
        if p.closed || p.immediate.is_none() {
            return;
        }
        p.closed = true;
        let mut av: Vec<(Fact, Vec<Val>)> = p.ambient_vals.iter().map(|(k, v)| (*k, v.clone())).collect();
        av.sort_by_cached_key(|e| e.0.to_string());
        for (k, mut vals) in av {
            let now = self.store_or_oracle(&p.writes, o, k);
            vals.push(now);
            self.loosen(p, k, &vals, "an ambient loop settle ran once writes it every frame");
        }
        let mut names: Vec<Slot> = p.ambient_slots.iter().chain(p.multi.iter()).copied().collect();
        names.sort_unstable();
        names.dedup();
        sort_names(&mut names);
        let (zero, one) = (Val::Int(0), Val::Int(1));
        for name in names {
            let key = slot_key(name);
            let mut ats = self.prefixed(p, o, Prefix::At(name));
            sort_str(&mut ats);
            let live = match key {
                Some(k) => self.store_or_oracle(&p.writes, o, k),
                None => {
                    let mut any = false;
                    for &a in &ats {
                        any |= self.store_or_oracle(&p.writes, o, a) == one;
                    }
                    Val::Int(any as i32)
                }
            };
            let why = if p.multi.contains(&name) {
                if let (Some(k), true) = (key, live == zero) {
                    self.loosen(p, k, &[zero, one], "a second instance started with R may still run");
                }
                "two instances run it; the path tracks one"
            } else if live == zero {
                continue; // a capped slot that died stands nowhere: its at: keys are 0
            } else {
                "an ambient loop settle ran once: the slot stands anywhere in it"
            };
            for k in ats {
                self.loosen(p, k, &[zero, one], why);
            }
        }
    }

    // --- rounds ---------------------------------------------------------------------------------------------

    /// Between two frames (model.py `_next_round`): the immediate part's snapshot, or the fixed point, the cycle
    /// and the round limit; else the next round's call.
    pub(super) fn next_round(&mut self, p: &mut Path, o: &FactOracle) {
        self.round_end(p, o);
        let mut state = None;
        if p.round == 0 {
            if p.immediate.is_none() {
                p.immediate = Some(Rc::new(p.writes.clone()));
                if p.first_end.is_none() {
                    p.first_end = Some(p.turns.last().map_or("nothing ran", |t| t.1));
                }
            }
        } else {
            let rs = p.round_start.clone();
            if !self.changed(p, o, &rs) {
                // the task's fixed point; a kept slot whose locals still move is told apart
                let moving = Self::slots_hash(p) != p.round_slots;
                p.end = Some("settled");
                let more = if moving { "; a kept slot's locals still moved" } else { "" };
                p.end_at = Some(rc(&format!("round {} changed no fact{more}", p.round)));
                self.ev_settled(p);
                return;
            }
            let st = self.state_key(p);
            if let Some(earlier) = p.history.find(st) {
                let mut snaps = p.history.writes_from(earlier);
                snaps.push(Rc::new(p.writes.clone()));
                self.unsettle(p, o, &snaps, "varies around a cycle");
                p.end = Some("cycle");
                p.end_at = Some(rc(&format!("after round {}: the state at round {}'s start", p.round, earlier + 1)));
                self.ev_settled(p);
                return;
            }
            if p.round >= self.round_limit {
                let snaps = [rs, Rc::new(p.writes.clone())];
                self.unsettle(p, o, &snaps, &format!("still changing at round {}", p.round));
                p.end = Some("round limit");
                p.end_at = Some(rc(&format!("{} rounds", self.round_limit)));
                self.ev_settled(p);
                return;
            }
            state = Some(st);
        }
        let st = state.unwrap_or_else(|| self.state_key(p));
        let snap = Rc::new(p.writes.clone());
        p.history.push(st, snap.clone());
        p.round_start = snap;
        p.round_slots = Self::slots_hash(p);
        p.round += 1;
        p.steps0 = p.steps;
        p.round_ran.clear();
        p.rw.clear();
        p.owner = None;
        let order = self.round_order(p);
        let mut steps: Vec<Step> = order.iter().map(|&n| Step::Turn(n)).collect();
        steps.push(Step::Sentences);
        let cid = self.cid();
        p.stack.push(Entry::Call(Call {
            head: rc(ROUND),
            cite: rc(ROUND_CITE),
            steps: steps.into(),
            caller: None,
            active: false,
            i: 0,
            cid,
        }));
        if self.trace.is_some() {
            let kw = vec![
                ("round", J::Int(p.round as i64)),
                ("order", J::Arr(order.iter().map(|&n| enc::slot(n)).collect())),
            ];
            self.ev(p, "round", kw);
        }
    }

    fn ev_settled(&mut self, p: &mut Path) {
        if self.trace.is_some() {
            let kw = vec![
                ("end", enc::opt_s(p.end)),
                ("end_at", enc::opt_s(p.end_at.as_deref())),
                ("round", J::Int(p.round as i64)),
            ];
            self.ev(p, "settled", kw);
        }
    }

    /// The order runAllScripts runs the slots in (model.py `_round_order`): the slots live at pre, then the
    /// others the path keeps, by sid.
    fn round_order(&self, p: &Path) -> Vec<Slot> {
        let mut out = self.pre.clone();
        let mut rest: Vec<(u32, String, Slot)> =
            p.slots.iter().filter(|(n, _)| !self.pre.contains(n)).map(|(n, f)| (f.sid, n.to_string(), *n)).collect();
        rest.sort();
        out.extend(rest.into_iter().map(|r| r.2));
        out
    }

    /// A frame's end (model.py `_round_end`): its order conflicts, and the values around each ambient pass.
    fn round_end(&mut self, p: &mut Path, o: &FactOracle) {
        self.round_conflicts(p);
        let mut names: Vec<Slot> = p.ambient_pass.iter().copied().collect();
        sort_names(&mut names);
        for name in names {
            let mut keys: Vec<Fact> = p.rw.sets(Owner::Slot(name)).1.into_iter().collect();
            sort_str(&mut keys);
            for k in keys {
                if matches!(k, Fact::At(n, _) if n == name) {
                    continue;
                }
                let rs = p.round_start.clone();
                let before = self.store_or_oracle(&rs, o, k);
                let now = self.store_or_oracle(&p.writes, o, k);
                let vals = p.ambient_vals.entry(k).or_default();
                for v in [before, now] {
                    if !vals.contains(&v) {
                        vals.push(v);
                    }
                }
            }
        }
        p.ambient_pass.clear();
    }

    /// Two slots of one frame, one writing a fact the other read or wrote (model.py `_round_conflicts`).
    fn round_conflicts(&mut self, p: &mut Path) {
        if p.rw.owners().iter().filter(|&&k| k != Owner::Sentence).count() < 2 {
            return;
        }
        let mut items: Vec<(String, Owner, FxHashSet<Fact>, FxHashSet<Fact>)> = p
            .rw
            .all_sets()
            .into_iter()
            .filter(|e| e.0 != Owner::Sentence)
            .map(|(o, r, w)| (enc::owner_name(o), o, r, w))
            .collect();
        items.sort_by(|x, y| x.0.cmp(&y.0));
        let mut found = Vec::new();
        for (i, (_, a, ra, wa)) in items.iter().enumerate() {
            for (_, b, rb, wb) in &items[i + 1..] {
                // (wa & (rb | wb)) | (wb & ra)
                let mut keys: Vec<Fact> = wa.iter().filter(|k| rb.contains(k) || wb.contains(k)).copied().collect();
                let more: Vec<Fact> = wb.iter().filter(|k| ra.contains(k) && !wa.contains(k)).copied().collect();
                keys.extend(more);
                if !keys.is_empty() {
                    sort_str(&mut keys);
                    let n = keys.len() as u32;
                    found.push((p.round, *a, *b, Rc::from(keys), n));
                }
            }
        }
        for c in found {
            p.conflicts.push(c);
        }
    }

    // --- one slot's turn ---------------------------------------------------------------------------------------

    /// The unit of a slot's name (model.py `_unit_named`): a local or the room's code is the room in use's.
    pub(super) fn unit_named(&mut self, p: &mut Path, o: &FactOracle, name: Slot) -> Option<UnitKey> {
        match name {
            Slot::Script(n) => Some(UnitKey::Global(n)),
            Slot::Obj(ob) => Some(UnitKey::Obj(ob)),
            _ => {
                let room = self.read(p, Fact::Room, o);
                let Val::Int(res) = self.resource(room) else { return None };
                Some(match name {
                    Slot::Local(n) => UnitKey::Local(res, n),
                    Slot::Entry => UnitKey::Entry(res),
                    _ => UnitKey::Exit(res),
                })
            }
        }
    }

    /// The Frame settle would resume for slot `name`, or why not ("" for a dead slot or room code that is not
    /// running) (model.py `_slot_frame`).
    fn slot_frame(&mut self, p: &mut Path, o: &FactOracle, name: Slot) -> Result<Frame, &'static str> {
        let g = self.game;
        let key = slot_key(name);
        if let Some(k) = key {
            let live = self.read(p, k, o);
            if live.is_unknown() {
                return Err("its liveness is not known");
            }
            if !live.truthy() {
                p.pop_slot(name);
                return Err("");
            }
        }
        let frozen = self.read(p, Fact::Frozen(name), o);
        if frozen.is_unknown() {
            return Err("its freeze count is not known");
        }
        if frozen.truthy() {
            return Err("frozen");
        }
        let mut keys = self.prefixed(p, o, Prefix::At(name));
        sort_str(&mut keys);
        let vals: Vec<(Fact, Val)> = keys.iter().map(|&k| (k, self.read(p, k, o))).collect();
        let waits: Vec<Fact> = vals.iter().filter(|e| e.1 == Val::Int(1)).map(|e| e.0).collect();
        if let Some(f) = p.slot(name) {
            let pc = f.pc.expect("a kept slot resumes at an instruction");
            if pc >= g.unit(f.unit).ins_len {
                return Err("its position is not known");
            }
            let at = Fact::At(name, g.ins(f.unit)[pc as usize].offset);
            if vals.iter().find(|e| e.0 == at).map(|e| e.1) != Some(Val::Int(1)) {
                return Err("its position is not known");
            }
            return Ok(f.clone());
        }
        if key.is_none() && waits.is_empty() {
            return Err(""); // room code that is not running
        }
        if waits.len() != 1 || vals.iter().any(|e| e.1.is_unknown()) {
            return Err("it has no single wait point");
        }
        let unit = self.unit_named(p, o, name);
        let uid = unit.and_then(|u| g.id(u));
        let Fact::At(_, w) = waits[0] else { unreachable!() };
        let Some(idx) = uid.and_then(|u| g.at(u, w)) else { return Err("its wait point is no instruction of its unit") };
        let (unit, uid) = (unit.unwrap(), uid.unwrap());
        let mut owned = Owned::Bool(false);
        if let UnitKey::Obj(ob) = unit {
            let owner = self.read(p, Fact::Owner(ob), o);
            owned = match owner {
                Val::Unk(_) => Owned::Unk(owner),
                Val::Int(n) => Owned::Bool(n != OF_OWNER_ROOM),
            };
        }
        let res = if let UnitKey::Local(..) = unit {
            let r = self.read(p, Fact::Room, o);
            self.resource(r).int()
        } else {
            None
        };
        let resistant = match key {
            Some(k) => g.resistant(k, res),
            None => Some(false),
        };
        let mut f = Frame::new(uid, Some(idx), false, owned, None);
        f.resistant = resistant;
        f.nested = false;
        f.origin = "pre";
        Ok(f)
    }

    /// Whether the path changed one of these facts from pre (model.py `_changed_any`).
    fn changed_any(&self, p: &Path, o: &FactOracle, reads: &[Read]) -> bool {
        for r in reads {
            let hit = match *r {
                Read::Fact(k) => p.writes.get(&k).is_some_and(|&v| !self.eq_oracle(v, o, k)),
                Read::Any(kind) => p.writes.iter().any(|(&k, &v)| kind_matches(kind, k) && !self.eq_oracle(v, o, k)),
                Read::ClassOf(ob) => {
                    p.writes.iter().any(|(&k, &v)| matches!(k, Fact::Class(c, _) if c == ob) && !self.eq_oracle(v, o, k))
                }
            };
            if hit {
                return true;
            }
        }
        false
    }

    /// The rule a slot live at pre runs its first turn by, from its pre wait point, or why not (model.py
    /// `_pre_rule`).
    fn pre_rule(&mut self, p: &mut Path, o: &FactOracle, u: UnitId, wp: WaitPoint) -> Result<&'static str, &'static str> {
        let g = self.game;
        match wp.kind {
            WaitKind::Wait => {
                if wp.op != known::WAIT_FOR_SENTENCE {
                    return Ok("b: a WaitFor op");
                }
                let n = self.read(p, Fact::Var(VAR_SENTENCE_SCRIPT), o);
                let reads: Vec<Read> = match n {
                    Val::Int(n) => sentence_slots(g, n).into_iter().map(Read::Fact).collect(),
                    Val::Unk(_) => Vec::new(),
                };
                if self.changed_any(p, o, &reads) {
                    Ok("c: WaitForSentence, the sentence slot changed")
                } else {
                    Err("WaitForSentence: the sentence script's slot has not changed")
                }
            }
            WaitKind::Delay => Ok("b: a delay"),
            WaitKind::Parked => Ok("b: parked after an opcode, waits for nothing"),
            WaitKind::Frame => {
                let Some(li) = wp.loop_ else { return Ok("b: a breakHere outside any loop") };
                let lp = &g.loops(u, self.switches.const_polls)[li as usize];
                match lp.kind {
                    LoopKind::Ambient => Ok("b: an ambient loop"),
                    LoopKind::Time if lp.camera() => Ok("b: a loop on the camera position (its exit forks once)"),
                    LoopKind::Time => Ok("b: a time-only loop"),
                    LoopKind::Facts => {
                        if self.changed_any(p, o, &lp.reads) {
                            Ok("c: polls facts the path changed")
                        } else {
                            Err("polls facts the path has not changed")
                        }
                    }
                }
            }
            WaitKind::Start => Err("its wait point is a start"),
            WaitKind::Other => Err("its wait point is a other"),
        }
    }

    /// Where a slot settle meets comes from, for the skip counts (model.py `_origin`).
    fn origin(&self, p: &Path, name: Slot) -> &'static str {
        if let Some(kept) = p.slot(name) {
            return if kept.origin == "pre" { "pre, ran before" } else { "path" };
        }
        if self.pre.contains(&name) {
            return if p.ran_by.contains_key(&name) { "pre, ran before" } else { "pre, first turn" };
        }
        "path"
    }

    /// A slot a round passes over, with why (model.py `_skip`); golden.py's skip event.
    pub(super) fn skip(&mut self, p: &mut Path, name: Slot, why: &str) {
        let origin = (!why.is_empty()).then(|| self.origin(p, name));
        p.owner = None;
        if !why.is_empty() {
            let w = rc(why);
            p.skips.insert((name, rc(origin.unwrap()), w.clone()));
            if self.pre.contains(&name) && !p.ran_by.contains_key(&name) {
                p.not_run.insert(name, w);
            }
        }
        if self.trace.is_some() {
            let kw = vec![
                ("name", enc::slot(name)),
                ("why", enc::s(why)),
                ("origin", enc::opt_s(origin)),
                ("round", J::Int(p.round as i64)),
            ];
            self.ev(p, "skip", kw);
        }
    }

    /// Whether a wait point is a time-only wait (model.py `_time_wait`).
    fn time_wait(&self, u: UnitId, wp: WaitPoint) -> bool {
        match wp.kind {
            WaitKind::Wait => wp.op != known::WAIT_FOR_SENTENCE,
            WaitKind::Delay => true,
            WaitKind::Frame => wp
                .loop_
                .is_some_and(|li| self.game.loops(u, self.switches.const_polls)[li as usize].kind == LoopKind::Time),
            _ => false,
        }
    }

    /// One slot's turn in runAllScripts (model.py `_eng_turn`).
    pub(super) fn eng_turn(&mut self, p: &mut Path, ci: usize, o: &FactOracle, name: Slot) -> Vec<Path> {
        let g = self.game;
        let cp = self.switches.const_polls;
        p.owner = None;
        if p.round_ran.contains(&name) {
            if self.trace.is_some() {
                let kw = vec![("name", enc::slot(name)), ("ran", J::Bool(false)), ("why", enc::s("didexec"))];
                self.ev(p, "turn", kw);
            }
            return Vec::new();
        }
        let owner = Owner::Slot(name);
        p.owner = Some(owner);
        p.owner_rule = "";
        p.rw.add_owner(owner);
        let frame = match self.slot_frame(p, o, name) {
            Ok(f) => f,
            Err(why) => {
                self.skip(p, name, why);
                return Vec::new();
            }
        };
        let u = frame.unit;
        let pc = frame.pc.expect("a slot's pc");
        let wp = g.wait_point(u, pc, cp);
        let rule: &'static str = if frame.origin == "pre" && frame.sid == 0 {
            // its first turn, from its pre wait point
            let r = match self.pre_rule(p, o, u, wp) {
                Ok(r) => r,
                Err(why) => {
                    self.skip(p, name, why);
                    return Vec::new();
                }
            };
            if p.round == 0 && self.time_wait(u, wp) {
                // the click's frame is one frame after pre: a time-only wait the slot was in at pre still holds
                self.skip(p, name, "a time-only wait, held in the click's frame");
                return Vec::new();
            }
            r
        } else {
            p.ran_by.get(&name).copied().unwrap_or(RULE_A)
        };
        if let Some(li) = wp.loop_ {
            let lp = &g.loops(u, cp)[li as usize];
            if lp.kind == LoopKind::Ambient {
                if p.ambient_done.contains(&(name, lp.head)) {
                    self.skip(p, name, "the ambient cap: its loop with no exit ran once");
                    return Vec::new();
                }
                p.ambient_done.insert((name, lp.head));
                p.ambient_slots.insert(name);
                p.ambient_pass.insert(name);
            }
        }
        p.ran_by.entry(name).or_insert(rule);
        p.not_run.remove(&name);
        let mut gf = frame;
        gf.nested = false;
        gf.killed = None;
        gf.freeze = 0;
        gf.time_loops = g.time_loops_at(u, pc, cp).to_vec();
        if gf.sid == 0 {
            gf.sid = p.next_sid;
            p.next_sid += 1;
            if gf.resistant.is_none() {
                let (_, c) = Self::head_cite(p, ci);
                self.note(p, c, "resume", &format!("{name}: F flag not known"));
            }
        }
        p.owner_rule = rule;
        p.timed = true;
        self.clear(p, o, Prefix::At(name)); // it runs from its wait point now
        let x = &g.ins(u)[pc as usize];
        let xi = X { u, i: pc };
        match wp.kind {
            WaitKind::Wait if wp.op != known::WAIT_FOR_SENTENCE => {
                let cite = self.cite(xi);
                let cond = rc(&format!("{} is over", scumm_ir::py::strip(g.text(x))));
                let why = rc(&format!("?({}: {})", Reason::WaitOver.as_str(), waits(wp.op).1));
                p.assumptions.push(Assumption { cite, cond, side: Side::Bool(true), why });
                gf.pc = x.next;
            }
            WaitKind::Delay => {
                let cite = self.cite(X { u, i: pc - 1 });
                let cond = rc(&format!("{name}'s delay is over"));
                let why = rc(&format!("?({}: delay count)", Reason::WaitOver.as_str()));
                p.assumptions.push(Assumption { cite, cond, side: Side::Bool(true), why });
            }
            _ => {}
        }
        let (sid, gpc) = (gf.sid, gf.pc);
        p.stack.push(Entry::Frame(gf));
        p.depth = p.depth.max(p.frames().count() as u32);
        p.round_ran.insert(name);
        p.seen.clear();
        if self.trace.is_some() {
            let offset = match gpc {
                Some(i) if i < g.unit(u).ins_len => J::Int(g.ins(u)[i as usize].offset as i64),
                _ => J::Null,
            };
            let kw = vec![
                ("name", enc::slot(name)),
                ("sid", J::Int(sid as i64)),
                ("unit", enc::unit_key(self.unit_key(u))),
                ("pc", gpc.map_or(J::Null, |i| J::Int(i as i64))),
                ("offset", offset),
                ("rule", enc::s(p.owner_rule)),
                ("round", J::Int(p.round as i64)),
            ];
            self.ev(p, "turn", kw);
        }
        Vec::new()
    }

    /// checkAndRunSentenceScript at a round's end, on the sentence queue (model.py `_eng_sentences`); golden.py's
    /// queue event when the queue was not empty.
    pub(super) fn eng_sentences(&mut self, p: &mut Path, ci: usize, o: &FactOracle) {
        let before = p.queue.len();
        self.eng_sentences_(p, ci, o);
        if before > 0 && self.trace.is_some() {
            let kw = vec![
                ("before", J::Int(before as i64)),
                ("after", J::Int(p.queue.len() as i64)),
                ("round", J::Int(p.round as i64)),
            ];
            self.ev(p, "queue", kw);
        }
    }

    fn eng_sentences_(&mut self, p: &mut Path, ci: usize, o: &FactOracle) {
        p.owner = None;
        if p.queue.is_empty() {
            return;
        }
        let n = self.read(p, Fact::Var(VAR_SENTENCE_SCRIPT), o);
        let n = match n {
            Val::Unk(_) => {
                let (_, c) = Self::head_cite(p, ci);
                let w = format!("VAR_SENTENCE_SCRIPT {}: the queue is not run", self.show(n));
                self.note(p, c, "sentence queue", &w);
                return;
            }
            Val::Int(n) => n,
        };
        let (busy, unknown) = if n != 0 { self.sentence_waits(p, o, n) } else { (Vec::new(), Vec::new()) };
        if !busy.is_empty() {
            if self.switches.input_script && !p.settle {
                // F1: the sentence stays queued past the input's frame (inputs.rs)
                self.sentence_held(p, ci, &busy);
            }
            return;
        }
        if !unknown.is_empty() {
            let (_, c) = Self::head_cite(p, ci);
            let w = format!("whether the sentence waits: {}", unknown.join("; "));
            self.note(p, c, "sentence queue", &w);
            return;
        }
        let (verb, a, b, frozen) = *p.queue.last().unwrap();
        if frozen != 0 {
            return;
        }
        p.queue.pop();
        if b != 0 && b == a {
            let (_, c) = Self::head_cite(p, ci);
            self.note(p, c, "sentence queue", "a queued sentence whose B is its A is dropped");
            return;
        }
        if n != 0 {
            p.sentences += 1;
            p.owner = Some(Owner::Sentence);
            p.owner_rule = "the sentence queue";
            p.rw.add_owner(Owner::Sentence);
            let args = [(0, Val::Int(verb)), (1, Val::Int(a)), (2, Val::Int(b))];
            self.eng_script(p, ci, o, n, Some(false), Some(false), &args);
        }
    }

    /// WaitForSentence's test (model.py `_sentence_busy`): True, False or an unknown.
    pub(super) fn sentence_busy(&mut self, p: &mut Path, o: &FactOracle) -> Dec {
        let n = self.read(p, Fact::Var(VAR_SENTENCE_SCRIPT), o);
        let Val::Int(n) = n else { return Dec::Unk(n) };
        let mut used = Vec::new();
        for k in sentence_slots(self.game, n) {
            used.push(self.read(p, k, o));
        }
        if let Some(&bad) = used.iter().find(|v| v.is_unknown()) {
            return Dec::Unk(bad);
        }
        let in_use = used.iter().any(|v| v.truthy());
        Dec::Bool(match p.queue.last() {
            Some(&(_, _, _, frozen)) => !(frozen != 0 && !in_use),
            None => in_use,
        })
    }
}
