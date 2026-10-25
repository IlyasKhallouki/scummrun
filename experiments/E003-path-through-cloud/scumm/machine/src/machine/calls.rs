//! Features dispatch and calls (row 1): model.py's `run_sentence`, the joins and merges of `_drive`
//! (`_join`, `_spins`, `_merge`), the calls between nested runs (`_call_step` and the `_eng_*` steps),
//! frames and their turns (`_push_frame`, `_finish`), stops, kills and freeze counts.
//!
//! Each method is the model.py method of the same name, in the same order of reads, writes, notes, errors,
//! forks and ends. model.py's docstrings hold the engine rules and the ScummVM cites.

use super::*;
use std::hash::{Hash, Hasher};

/// A slot a room change may kill (model.py `_room_slot`: True, False or an unknown).
pub(super) enum RoomSlot {
    Yes,
    No,
    Unk(Val),
}

/// The sets of entries a settle merge's lists hold (model.py `_merge`'s `have`).
#[derive(Default)]
struct Seen {
    assumptions: Option<FxHashSet<Assumption>>,
    unmodelled: Option<FxHashSet<Tri>>,
    notes: Option<FxHashSet<Tri>>,
    unknown_writes: Option<FxHashSet<Tri>>,
    errors: Option<FxHashSet<Tri>>,
    conflicts: Option<FxHashSet<(u32, Owner, Owner, Rc<[Fact]>, u32)>>,
}

/// model.py's dict equality of two paths' kept slots (`g.slots == base.slots`): the same names, equal frames, in
/// any order.
fn slots_eq(a: &[(Slot, Frame)], b: &[(Slot, Frame)]) -> bool {
    a.len() == b.len() && a.iter().all(|(n, f)| b.iter().any(|(m, h)| m == n && h == f))
}

/// Python's repr of a list of strings: `['a', 'b']`.
fn py_list(items: &[Fact]) -> String {
    let parts: Vec<String> = items.iter().map(|k| format!("'{k}'")).collect();
    format!("[{}]", parts.join(", "))
}

impl<'g> Machine<'g> {
    /// A new call id (`Call.cid`).
    pub(super) fn cid(&mut self) -> u64 {
        self.next_cid += 1;
        self.next_cid
    }

    /// model.py `_push_call`: the caller is the running frame.
    pub(super) fn push_call(&mut self, p: &mut Path, head: S, cite: S, steps: Vec<Step>, active: bool) {
        let caller = if p.stack.is_empty() { None } else { Some(p.frame().expect("a frame").sid) };
        let cid = self.cid();
        p.stack.push(Entry::Call(Call { head, cite, steps: steps.into(), caller, active, i: 0, cid }));
    }

    /// model.py `_scene_steps`: the room change's steps inside an opcode (row 0's without room_change).
    pub(super) fn scene_steps(&self, room: Val, ego: Option<Val>) -> Vec<Step> {
        if self.rooms {
            vec![Step::Scene { room, ego }]
        } else {
            vec![Step::Room1(room)]
        }
    }

    // --- the entry point -----------------------------------------------------------------------------------

    /// Feature dispatch: the sentence (verb, A, B) from the click's frame on (model.py `run_sentence`).
    pub fn run_sentence(&mut self, o: &FactOracle, verb: i32, a: i32, b: Option<i32>) -> Run {
        let g = self.game;
        let n = self.oget(o, Fact::Var(VAR_SENTENCE_SCRIPT));
        let Val::Int(n) = n else {
            let r = Run { start: format!("VAR_SENTENCE_SCRIPT {}", self.show(n)), ..Default::default() };
            return self.finish_run(r);
        };
        let mut steps = Vec::new();
        for k in sentence_slots(g, n) {
            let live = self.oget(o, k);
            let frozen = self.oget(o, Fact::Frozen(slot_name(k)));
            if live == Val::Int(0) {
                continue;
            }
            if live.is_unknown() || frozen.is_unknown() {
                let why = format!("whether the sentence waits: {k} {}, frozen {}", self.show(live), self.show(frozen));
                let mut p = Path::new(Vec::new());
                p.end = Some("deferral unknown");
                p.end_at = Some(rc(&why));
                let r = Run { paths: vec![p], start: why, ..Default::default() };
                return self.finish_run(r);
            }
            if !frozen.truthy() {
                // a frozen slot does not run (script.cpp:979) and does not hold the sentence
                steps.push(Step::Resume(k));
            }
        }
        if steps.len() > 1 {
            // runAllScripts runs them in slot order, which is not a fact
            let w = format!("{} live slots of number {n} run in an order that is not a fact", steps.len());
            steps.insert(0, Step::Note(rc(&w)));
        }
        if self.settles {
            // settle: the click's frame runs every live slot's turn (runAllScripts, scumm.cpp:3231) before the
            // click is taken (checkExecVerbs, 3232) and the sentence runs (checkAndRunSentenceScript, 3255), and
            // the mouse variables already hold the click (processInput 3177, updateScummVars 3198, 3385-3388)
            steps = self.sentence_frame_steps(o, steps);
        }
        let resumed: Vec<String> =
            steps.iter().filter_map(|st| if let Step::Resume(k) = st { Some(k.to_string()) } else { None }).collect();
        steps.push(Step::Sentence { verb, a, b });
        let cid = self.cid();
        let call = Call {
            head: rc("checkAndRunSentenceScript"),
            cite: rc("script.cpp:1244-1246"),
            steps: steps.into(),
            caller: None,
            active: false,
            i: 0,
            cid,
        };
        let unit = match (n != 0).then(|| g.script_key(n)) {
            Some(Fact::Script(_)) => Some(UnitKey::Global(n)),
            _ => None,
        };
        let uid = unit.and_then(|u| g.id(u));
        let mut start = match uid {
            Some(u) if g.unit(u).ins_len > 0 => g.cite(u, 0),
            _ => format!("sentence script {n}"),
        };
        if !resumed.is_empty() {
            start += &format!(" (after the turn of {})", resumed.join(", "));
        }
        self.pre = if self.settles { self.pre_slots(o) } else { Vec::new() };
        let mut p = Path::new(vec![Entry::Call(call)]);
        p.settle = self.settles;
        self.drive(o, p, start, unit)
    }

    // --- joins, spins and merges -----------------------------------------------------------------------------

    /// The path's join signature when it stands at a join, else None (model.py `_join`): between two steps
    /// of an opcode's call, and at an instruction some jump goes to.
    pub(super) fn join(&self, p: &Path, o: &FactOracle) -> Option<Sig> {
        let g = self.game;
        match p.stack.last()? {
            Entry::Call(c) => {
                if c.i == 0 && &*c.head == ROUND {
                    // a round's start: paths in the same state become one (settle)
                    return Some(Sig::round(p.round, self.position_key(p, o)));
                }
                if c.caller.is_none() || c.i == 0 {
                    return None;
                }
            }
            Entry::Frame(f) => match f.pc {
                Some(pc) if pc <= g.unit(f.unit).ins_len && g.joins[g.point(f.unit, pc)] => {}
                _ => return None,
            },
        }
        let mut w: Vec<u64> = Vec::with_capacity(8 + 6 * p.stack.len() + if p.settle { 12 + 4 * p.slots.len() } else { 0 });
        w.push(opt_word(p.cut_index));
        w.push(p.cut_data.len() as u64);
        w.push(p.settle as u64);
        if p.settle {
            // the slots off the stack and the queue are part of the state: paths differing there stay apart
            w.push(p.round as u64);
            w.push(p.slots.len() as u64);
            for (n, f) in &p.slots {
                w.extend([slot_word(*n), f.unit.0 as u64, opt_word(f.pc), origin_word(f.origin)]);
            }
            w.push(p.queue.len() as u64);
            for &(a, b, c, d) in &p.queue {
                w.extend([pair_word(a, b), pair_word(c, d)]);
            }
            let sorted = |w: &mut Vec<u64>, mut items: Vec<u64>| {
                items.sort_unstable();
                w.push(items.len() as u64);
                w.extend(items);
            };
            sorted(&mut w, p.ambient_done.iter().map(|&(n, h)| slot_word(n) | ((h as u64) << 35)).collect());
            sorted(&mut w, p.round_ran.iter().map(|&n| slot_word(n)).collect());
            w.push(owner_word(p.owner));
            sorted(&mut w, p.camera_done.iter().map(|&(n, h)| slot_word(n) | ((h as u64) << 35)).collect());
            w.push(p.timed as u64);
        }
        for e in &p.stack {
            match e {
                Entry::Frame(f) => {
                    w.extend([
                        0,
                        f.sid as u64,
                        f.unit.0 as u64,
                        opt_word(f.pc),
                        killed_word(f.killed) | (bool_word(f.recursive) << 2) | (bool_word(f.resistant) << 4) | ((f.args_known as u64) << 6),
                        pair_word(f.freeze, f.override_),
                        owned_word(f.owned),
                        f.time_loops.len() as u64,
                    ]);
                    w.extend(f.time_loops.iter().map(|&h| h as u64));
                }
                Entry::Call(c) => {
                    w.extend([1, c.cid, c.i as u64 | ((c.active as u64) << 32), opt_word(c.caller)]);
                }
            }
        }
        Some(Sig::from_words(false, w))
    }

    /// A path back at a jump target in a machine state it already had ends as "spin" (model.py `_spins`).
    /// model.py keeps each state as Python's 64-bit hash of (signature, sid, locals, writes, freeze counts);
    /// this keeps a 64-bit hash of the same, the stores hashed as sets (the writes' sum kept by `Store`).
    pub(super) fn spins(&mut self, p: &mut Path, sig: &Sig) -> bool {
        let Some(Entry::Frame(f)) = p.stack.last() else { unreachable!() };
        let (unit, pc) = (f.unit, f.pc.expect("a join point"));
        let mut h = rustc_hash::FxHasher::default();
        sig.hash(&mut h);
        f.sid.hash(&mut h);
        f.locals.as_slice().hash(&mut h); // sorted by local number: the set of its items
        let w = p.writes.hash_sum();
        let mut z: u64 = 0;
        for e in &p.freeze {
            z = z.wrapping_add(fx(&e));
        }
        let state = fmix(fmix(h.finish()) ^ fmix(w ^ 0x9e37_79b9_7f4a_7c15)).wrapping_add(fmix(z ^ 0x5851_f42d_4c95_7f2d));
        if p.seen.contains(&state) {
            let at = format!("{}: the same state again with no yield", self.game.cite(unit, pc));
            self.halt(p, "spin", rc(&at));
            return true;
        }
        p.seen.insert(state);
        false
    }

    /// One path for the paths waiting at one join (model.py `_merge`). model.py's merged path is the first path's
    /// fork, and the first path is dropped after; here the first path itself becomes the merged one, and what the
    /// merge reads from it is worked out before it changes.
    pub(super) fn merge(&mut self, o: &FactOracle, mut group: Vec<Path>, sig: Rc<Sig>) -> Path {
        let g = self.game;
        let tids: Vec<J> = match &mut self.trace {
            Some(t) => group.iter_mut().map(|q| J::Int(t.tid(q) as i64)).collect(),
            None => Vec::new(),
        };
        if group.len() == 1 {
            let mut q = group.pop().unwrap();
            q.joined = Some(sig);
            q.resumed = true;
            return q;
        }
        let n = group.len() as u32 - 1;
        let base = &group[0];
        let settle = base.settle;
        // every key some path wrote gets the join of the paths' values (the oracle's where a path did not write
        // it). model.py goes through the keys sorted; the order shows nothing (the store is a map, and the
        // unknowns a join makes are interned by value), so this goes through the first path's keys, then each
        // other path's keys that no path before it holds. A key whose values all equal the first path's keeps
        // the value the first path has.
        let mut writes: Vec<(Fact, Val)> = Vec::new();
        let mut vals = Vec::with_capacity(group.len());
        let mut join_key = |me: &mut Self, k: Fact, writes: &mut Vec<(Fact, Val)>| {
            vals.clear();
            for gp in &group {
                let v = match gp.writes.get(&k) {
                    Some(&v) => v,
                    None => me.oget(o, k),
                };
                vals.push(v);
            }
            if base.writes.contains_key(&k) && vals.iter().all(|&v| v == vals[0]) {
                return; // the first path holds it already
            }
            writes.push((k, me.v.join_values(&vals)));
        };
        {
            for (j, gj) in group.iter().enumerate() {
                for &k in gj.writes.keys() {
                    if j > 0 && (base.writes.contains_key(&k) || group[1..j].iter().any(|gp| gp.writes.contains_key(&k))) {
                        continue; // done with an earlier path's keys
                    }
                    join_key(self, k, &mut writes);
                }
            }
        }
        let mut frame_locals: Vec<(usize, Vec<(i32, Val)>)> = Vec::new();
        for i in 0..base.stack.len() {
            if !matches!(base.stack[i], Entry::Frame(_)) {
                continue;
            }
            let frames: Vec<&Frame> = group
                .iter()
                .map(|gp| match &gp.stack[i] {
                    Entry::Frame(f) => f,
                    Entry::Call(_) => unreachable!("one signature, one stack shape"),
                })
                .collect();
            if frames[1..].iter().all(|f| f.locals == frames[0].locals) {
                continue; // the same locals in every path: joined, they stay as they are
            }
            let locals = self.join_locals(&frames);
            frame_locals.push((i, locals));
        }
        // settle: a slot off the stack, at the same place in every path: its locals are joined as a frame's
        let mut slot_locals: Vec<(Vec<(i32, Val)>, bool)> = Vec::new();
        for (name, _) in &base.slots {
            let frames: Vec<&Frame> =
                group.iter().map(|gp| gp.slot(*name).expect("one signature, the same kept slots")).collect();
            let locals = self.join_locals(&frames);
            slot_locals.push((locals, frames.iter().all(|f| f.args_known)));
        }
        let same = group[1..].iter().all(|gp| gp.writes == base.writes && slots_eq(&gp.slots, &base.slots));
        let cut_data: Vec<Val> = (0..base.cut_data.len())
            .map(|i| {
                let vs: Vec<Val> = group.iter().map(|gp| gp.cut_data[i]).collect();
                self.v.join_values(&vs)
            })
            .collect();
        let mut names: Vec<Slot> = group.iter().flat_map(|gp| gp.freeze.keys().copied()).collect();
        names.sort_unstable();
        names.dedup();
        let freeze: Vec<(Slot, Option<(i32, bool)>)> = names
            .into_iter()
            .map(|name| {
                let first = base.freeze.get(&name);
                let same = group[1..].iter().all(|gp| gp.freeze.get(&name) == first);
                (name, if same { *first.unwrap() } else { None })
            })
            .collect();
        let input_drift = self.merged_drift(&group);
        let cleared = if group[1..].iter().any(|gp| gp.cleared != base.cleared) { None } else { base.cleared };
        let steps = group.iter().map(|gp| gp.steps).max().unwrap();
        let depth = group.iter().map(|gp| gp.depth).max().unwrap();
        // settle merges recur every round, so a sum would count the shared history again each time: the most
        let reentries = if settle {
            group.iter().map(|gp| gp.reentries).max().unwrap()
        } else {
            group.iter().map(|gp| gp.reentries).sum()
        };
        let next_sid = group.iter().map(|gp| gp.next_sid).max().unwrap();
        let merged = group.iter().map(|gp| gp.merged).max().unwrap() + 1;
        // the first path's lists and immediate snapshot as they are before the merge adds to them
        let (b_assumptions, b_unmodelled, b_notes, b_unknown_writes, b_errors, b_turns, b_conflicts) = (
            base.assumptions.clone(),
            base.unmodelled.clone(),
            base.notes.clone(),
            base.unknown_writes.clone(),
            base.errors.clone(),
            base.turns.clone(),
            base.conflicts.clone(),
        );
        let b_immediate = base.immediate.clone();
        let rest = group.split_off(1);
        let mut q = group.pop().unwrap();
        q.tid = None; // a new path (model.py's fork of the first)
        for (k, v) in writes {
            q.writes.insert(k, v);
        }
        for (i, locals) in frame_locals {
            let f = q.frame_at(i);
            for (n, v) in locals {
                f.set_local(n, v);
            }
        }
        for (i, (locals, args_known)) in slot_locals.into_iter().enumerate() {
            let f = &mut q.slots[i].1;
            f.locals = Locals::from_vec(locals);
            f.args_known = args_known;
        }
        q.cut_data = cut_data;
        for (name, c) in freeze {
            q.freeze.insert(name, c);
        }
        // settle: an entry a merged path already holds is not added again (merges recur every round); the sets
        // are made when first needed (model.py makes them at the first branch: the same sets)
        let mut seen = Seen::default();
        for gp in &rest {
            // each branch's own additions: what follows the history it shares with the first
            if settle {
                q.assumptions.add_branch_new(&b_assumptions, &gp.assumptions, &mut seen.assumptions);
                q.unmodelled.add_branch_new(&b_unmodelled, &gp.unmodelled, &mut seen.unmodelled);
                q.notes.add_branch_new(&b_notes, &gp.notes, &mut seen.notes);
                q.unknown_writes.add_branch_new(&b_unknown_writes, &gp.unknown_writes, &mut seen.unknown_writes);
                q.errors.add_branch_new(&b_errors, &gp.errors, &mut seen.errors);
                q.turns.add_branch(&b_turns, &gp.turns);
                q.conflicts.add_branch_new(&b_conflicts, &gp.conflicts, &mut seen.conflicts);
            } else {
                q.assumptions.add_branch(&b_assumptions, &gp.assumptions);
                q.unmodelled.add_branch(&b_unmodelled, &gp.unmodelled);
                q.notes.add_branch(&b_notes, &gp.notes);
                q.unknown_writes.add_branch(&b_unknown_writes, &gp.unknown_writes);
                q.errors.add_branch(&b_errors, &gp.errors);
                q.turns.add_branch(&b_turns, &gp.turns);
                q.conflicts.add_branch(&b_conflicts, &gp.conflicts);
            }
            // settle: a key counts as moved after the immediate part when it did on any branch merged in
            q.after.extend(gp.after.iter().copied());
            if let (Some(gi), Some(bi)) = (&gp.immediate, &b_immediate) {
                // (one snapshot both paths share from before their fork holds no difference)
                for &k in gi.keys().chain(bi.keys()).filter(|_| !Rc::ptr_eq(gi, bi)) {
                    let differ = match (gi.get(&k), bi.get(&k)) {
                        (Some(x), Some(y)) => x != y,
                        (Some(&x), None) | (None, Some(&x)) => !self.eq_oracle(x, o, k),
                        (None, None) => false,
                    };
                    if differ {
                        q.after.insert(k);
                    }
                }
            }
            q.rw.union(&gp.rw);
            for (name, rule) in &gp.ran_by {
                q.ran_by.entry(*name).or_insert(rule);
            }
            q.skips.extend(gp.skips.iter().cloned());
            q.ambient_pass.extend(gp.ambient_pass.iter().copied());
            q.ambient_slots.extend(gp.ambient_slots.iter().copied());
            q.multi.extend(gp.multi.iter().copied());
            for (k, vs) in &gp.ambient_vals {
                let mine = q.ambient_vals.entry(*k).or_default();
                for v in vs {
                    if !mine.contains(v) {
                        mine.push(*v);
                    }
                }
            }
            q.writer.merge(&gp.writer);
            q.sentences = q.sentences.max(gp.sentences);
        }
        drop(rest);
        if let Some(d) = input_drift {
            q.input_drift = d;
        }
        q.cleared = cleared;
        q.steps = steps;
        q.depth = depth;
        q.reentries = reentries;
        q.next_sid = next_sid;
        q.merged = merged;
        q.joined = Some(sig.clone());
        q.resumed = true;
        self.merges += n;
        if sig.is_round() {
            // a round's start: its fixed point and cycle tests compare with the merged state
            self.settle_merges += n;
            self.value_merges += if same { 0 } else { n };
            let snap = Rc::new(q.writes.clone());
            q.round_start = snap.clone();
            q.round_slots = Self::slots_hash(&q);
            let st = self.state_key(&q);
            q.history.set_last(st, snap);
        }
        if self.trace.is_some() {
            let wh = enc::where_(g, &q);
            let round = J::Int(q.round as i64);
            self.ev(&mut q, "merge", vec![("tids", J::Arr(tids)), ("round", round), ("where", wh)]);
        }
        q
    }

    /// The joined locals of frames at one place (model.py `_merge`'s rule for a stack frame and a kept slot): each
    /// local any of them holds, sorted, its values joined (0 or an unknown local where a frame lacks it).
    fn join_locals(&mut self, frames: &[&Frame]) -> Vec<(i32, Val)> {
        let mut ns: Vec<i32> = frames.iter().flat_map(|f| f.locals.iter().map(|l| l.0)).collect();
        ns.sort_unstable();
        ns.dedup();
        let mut vals = Vec::with_capacity(frames.len());
        let mut out = Vec::with_capacity(ns.len());
        for n in ns {
            vals.clear();
            for f in frames {
                let v = match f.local(n) {
                    Some(v) => v,
                    None if f.args_known => Val::Int(0),
                    None => self.unknown_local(n),
                };
                vals.push(v);
            }
            out.push((n, self.v.join_values(&vals)));
        }
        out
    }

    // --- calls ---------------------------------------------------------------------------------------------------

    /// Runs the call's next step; at its end, pops it and settles its caller (model.py `_call_step`).
    pub(super) fn call_step(&mut self, p: &mut Path, o: &FactOracle) -> Vec<Path> {
        let ci = p.stack.len() - 1;
        let (steps, i) = {
            let c = p.call_at(ci);
            (c.steps.clone(), c.i as usize)
        };
        if i < steps.len() {
            p.call_at(ci).i += 1;
            return self.eng(p, ci, &steps[i], o);
        }
        let Some(Entry::Call(call)) = p.stack.pop() else { unreachable!() };
        if p.stack.is_empty() && p.settle && p.end.is_none() {
            // the frame is over: the next settle round, or the end
            self.next_round(p, o);
            return Vec::new();
        }
        if p.stack.is_empty() {
            // the engine's own call: the first script's turn is the run's end
            if p.end.is_none() {
                p.end = Some(p.turns.last().map_or("nothing ran", |t| t.1));
                p.end_at = Some(call.cite.clone());
            }
            return Vec::new();
        }
        match p.stack.last_mut().unwrap() {
            Entry::Call(top) => {
                // a routine inside an opcode: whether the caller resumes goes up
                top.active = top.active && call.active;
            }
            Entry::Frame(top) => {
                if call.active && top.killed.is_none() && top.freeze == 0 {
                    return Vec::new();
                }
                let how = top.killed.unwrap_or("parked");
                self.finish(p, o, how, call.cite.clone(), None);
            }
        }
        Vec::new()
    }

    /// The call at stack index `ci`'s head and cite.
    pub(super) fn head_cite(p: &mut Path, ci: usize) -> (S, S) {
        let c = p.call_at(ci);
        (c.head.clone(), c.cite.clone())
    }

    /// model.py `getattr(self, "_eng_" + step[0])(p, call, oracle, *step[1:])`.
    fn eng(&mut self, p: &mut Path, ci: usize, step: &Step, o: &FactOracle) -> Vec<Path> {
        match step {
            Step::Write(k, v) => self.write(p, *k, *v),
            Step::Note(w) => {
                let (h, c) = Self::head_cite(p, ci);
                self.note(p, c, &h, w);
            }
            Step::Resume(k) => return self.eng_resume(p, ci, o, *k),
            Step::Sentence { verb, a, b } => self.eng_sentence(p, ci, o, *verb, *a, *b),
            Step::Script { n, resistant, recursive, args } => {
                self.eng_script(p, ci, o, *n, *resistant, *recursive, args)
            }
            Step::Object { obj, verb, args } => self.eng_object(p, ci, o, *obj, *verb, args),
            Step::Hook { var, args } => self.eng_hook(p, ci, o, *var, args),
            Step::Room1(room) => self.eng_room1(p, ci, o, *room),
            Step::ScrollNote => {
                let n = self.read(p, Fact::Var(VAR_SCROLL_SCRIPT), o);
                if n != Val::Int(0) {
                    let (h, c) = Self::head_cite(p, ci);
                    let w = format!("the scroll hook ({}) may run: the camera position is not a fact", self.show(n));
                    self.note(p, c, &h, &w);
                }
            }
            Step::CutIndex(sid) => p.cut_index = *sid,
            Step::Scene { room, ego } => self.eng_scene(p, ci, *room, *ego),
            Step::SceneBegin(room) => self.eng_scene_begin(p, ci, o, *room),
            Step::RoomCode { entry } => self.eng_room_code(p, ci, o, *entry),
            Step::SceneKill => self.kill_room(p, ci, o),
            Step::SceneLoad(room) => self.eng_scene_load(p, ci, o, *room),
            Step::SceneEgo(obj) => self.eng_scene_ego(p, ci, o, *obj),
            Step::Turn(name) => return self.eng_turn(p, ci, o, *name),
            Step::Sentences => self.eng_sentences(p, ci, o),
            Step::Input { area, verb, button } => return self.eng_input(p, ci, o, *area, *verb, *button),
            Step::Abort { armed, code } => self.eng_abort(p, ci, o, armed, *code),
            Step::Override { name, w } => return self.eng_override(p, ci, o, *name, *w),
            Step::Key(code) => return self.eng_key(p, ci, o, *code),
        }
        Vec::new()
    }

    /// runScript(N, F, R, args) (model.py `_eng_script`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn eng_script(
        &mut self,
        p: &mut Path,
        ci: usize,
        o: &FactOracle,
        n: i32,
        resistant: Option<bool>,
        recursive: Option<bool>,
        args: &[(i32, Val)],
    ) {
        let g = self.game;
        if n == 0 {
            return;
        }
        let (head, cite) = Self::head_cite(p, ci);
        let key = g.script_key(n);
        if recursive != Some(true) {
            self.stop_script(p, o, n, &cite, &head);
        } else if p.settle {
            // settle: a recursive start leaves a live instance running (script.cpp:65-66), so two slots run the
            // unit; the path keeps one Frame per unit name (`_close_settle`). An instance on the stack is the
            // running one, not a second live slot
            let live = match p.writes.get(&key) {
                Some(&v) => v,
                None => self.oget(o, key),
            };
            let name = slot_name(key);
            if live != Val::Int(0) && !p.frames().any(|f| Game::unit_name(g.unit(f.unit).key) == name && f.killed.is_none()) {
                p.multi.insert(name);
            }
        }
        let unit = if let Fact::Local(_) = key {
            let room = self.read(p, Fact::Room, o);
            match self.resource(room) {
                res @ Val::Unk(_) => {
                    let w = format!("local script {n} of room {}", self.show(res));
                    p.unknown_writes.push((head, cite, rc(&w)));
                    let u = self.v.mk(Reason::Work, w);
                    self.write(p, key, u);
                    return;
                }
                Val::Int(r) => UnitKey::Local(r, n),
            }
        } else {
            UnitKey::Global(n)
        };
        let Some(uid) = g.id(unit) else {
            let detail = match unit {
                UnitKey::Local(r, _) => {
                    format!("{} is not in room {r} (script.cpp:80-81, an error())", Game::unit_name(unit))
                }
                _ => format!("no global script {n}"),
            };
            self.error(p, "no such script", cite, &detail, true);
            return;
        };
        self.push_frame(p, ci, uid, 0, args, Owned::Bool(false), recursive, resistant, Some(key), true, true);
    }

    /// runObjectScript(O, verb, 0, 0, args) (model.py `_eng_object`).
    fn eng_object(&mut self, p: &mut Path, ci: usize, o: &FactOracle, obj: i32, verb: i32, args: &[(i32, Val)]) {
        let g = self.game;
        if obj == 0 {
            return;
        }
        let (head, cite) = Self::head_cite(p, ci);
        self.kill_object(p, o, obj, &cite, &head);
        let w = self.where_(p, o, obj);
        match w {
            Where::Unk(u) => {
                let s = format!("objscript of {obj} ({})", self.show(u));
                p.unknown_writes.push((head, cite, rc(&s)));
                self.write(p, Fact::ObjScript(obj), u);
            }
            Where::Nowhere => {
                let d = format!("Code for object {obj} not in room (script.cpp:122-125, a warning)");
                self.error(p, "object not here", cite, &d, false);
            }
            Where::Room | Where::Inventory => {
                let Some(idx) = g.entry(obj, verb) else { return };
                let uid = g.id(UnitKey::Obj(obj)).expect("an object unit");
                let owned = Owned::Bool(w == Where::Inventory);
                let key = Some(Fact::ObjScript(obj));
                self.push_frame(p, ci, uid, idx, args, owned, Some(false), Some(false), key, true, true);
            }
        }
    }

    /// runScript(VAR(var), 0, 0, args) when the variable is not 0 (model.py `_eng_hook`).
    fn eng_hook(&mut self, p: &mut Path, ci: usize, o: &FactOracle, var: i32, args: &[(i32, Val)]) {
        let n = self.read(p, Fact::Var(var), o);
        match n {
            Val::Unk(_) => {
                let (head, cite) = Self::head_cite(p, ci);
                let w = format!("hook in var {var}: {}", self.show(n));
                p.unknown_writes.push((head, cite, rc(&w)));
            }
            Val::Int(0) => {}
            Val::Int(n) => self.eng_script(p, ci, o, n, Some(false), Some(false), args),
        }
    }

    /// runAllScripts' turn of the sentence script's live, unfrozen slot `key` in the click's frame (model.py
    /// `_eng_resume`).
    fn eng_resume(&mut self, p: &mut Path, ci: usize, o: &FactOracle, key: Fact) -> Vec<Path> {
        let g = self.game;
        let name = slot_name(key);
        let room = self.read(p, Fact::Room, o);
        let res = self.resource(room);
        let unit = match key {
            Fact::Script(n) => Some(UnitKey::Global(n)),
            Fact::Local(n) => res.int().map(|r| UnitKey::Local(r, n)),
            Fact::ObjScript(ob) => Some(UnitKey::Obj(ob)),
            _ => unreachable!("not a slot fact"),
        };
        let uid = unit.and_then(|u| g.id(u));
        let mut waits = Vec::new();
        for k in o.keys(Prefix::At(name)).to_vec() {
            if self.read(p, k, o) == Val::Int(1) {
                waits.push(k);
            }
        }
        sort_str(&mut waits);
        let idx = match (uid, waits.as_slice()) {
            (Some(u), [Fact::At(_, w)]) => g.at(u, *w),
            _ => None,
        };
        let Some(idx) = idx else {
            let u = unit.map_or("None".to_string(), |u| u.to_string());
            let at = format!(
                "{name} is live and not frozen, and its turn cannot be run (unit {u}, wait points {})",
                py_list(&waits)
            );
            self.halt(p, "deferral unknown", rc(&at));
            return Vec::new();
        };
        let uid = uid.unwrap();
        let mut owned = Owned::Bool(false);
        if let Some(UnitKey::Obj(ob)) = unit {
            let owner = self.read(p, Fact::Owner(ob), o);
            owned = match owner {
                Val::Unk(_) => Owned::Unk(owner),
                Val::Int(n) => Owned::Bool(n != OF_OWNER_ROOM),
            };
        }
        let mut more = Vec::new();
        if idx > 0 {
            let prev = &g.ins(uid)[idx as usize - 1];
            let delay = matches!(prev.st, Some(Stmt::Op { name: known::DELAY | known::DELAY_VARIABLE, .. }));
            if prev.next == Some(idx) && delay {
                let mut q = p.fork();
                let c = self.cite(X { u: uid, i: idx - 1 });
                let why = rc(&format!("?(timer: {name}'s delay count)"));
                let cond = rc(&format!("{name}'s delay is over"));
                p.assumptions.push(Assumption { cite: c.clone(), cond: cond.clone(), side: Side::Bool(true), why: why.clone() });
                q.assumptions.push(Assumption { cite: c, cond, side: Side::Bool(false), why });
                more.push(q); // still paused: no turn this frame
            }
        }
        if p.settle {
            // settle: its turn is one of the click frame's runAllScripts turns (order conflicts), with rows 1-2's
            // rules for what it meets (a WaitFor op forks)
            p.owner = Some(Owner::Slot(name));
            p.owner_rule = "a: started or resumed by the path";
            p.timed = false;
            p.rw.add_owner(Owner::Slot(name));
            p.round_ran.insert(name);
        }
        self.clear(p, o, Prefix::At(name)); // it runs from its wait point now
        let resistant = g.resistant(key, res.int());
        if resistant.is_none() {
            let (_, c) = Self::head_cite(p, ci);
            self.note(p, c, "resume", &format!("{name}: F flag not known"));
        }
        self.push_frame(p, ci, uid, idx, &[], owned, None, resistant, Some(key), false, false);
        more
    }

    /// checkAndRunSentenceScript on the path's own store after the turns before it (model.py `_eng_sentence`).
    fn eng_sentence(&mut self, p: &mut Path, ci: usize, o: &FactOracle, verb: i32, a: i32, b: Option<i32>) {
        p.owner = None;
        p.timed = false;
        let n = self.read(p, Fact::Var(VAR_SENTENCE_SCRIPT), o);
        match n {
            Val::Unk(_) => {
                let at = format!("VAR_SENTENCE_SCRIPT {}", self.show(n));
                self.halt(p, "deferral unknown", rc(&at));
            }
            Val::Int(0) => {
                if p.settle {
                    // time still passes (settle); the immediate part ran nothing
                    p.first_end = Some("no sentence script");
                } else {
                    self.halt(p, "no sentence script", rc("VAR_SENTENCE_SCRIPT is 0: nothing runs (script.cpp:1245)"));
                }
            }
            Val::Int(n) => {
                let (busy, unknown) = self.sentence_waits(p, o, n);
                let dropped = b.is_some_and(|b| b != 0 && b == a);
                if !busy.is_empty() && p.settle {
                    // it stays queued; a later frame's checkAndRunSentenceScript runs it (settle)
                    p.queue.push((verb, a, b.unwrap_or(0), 0));
                    p.first_end = Some("deferred");
                } else if !busy.is_empty() {
                    let at = format!("{} live and not frozen: the sentence waits (script.cpp:1186-1191)", busy.join(", "));
                    self.halt(p, "deferred", rc(&at));
                } else if !unknown.is_empty() {
                    let at = format!("whether the sentence waits: {}", unknown.join("; "));
                    self.halt(p, "deferral unknown", rc(&at));
                } else if dropped && p.settle {
                    p.first_end = Some("dropped");
                } else if dropped {
                    self.halt(p, "dropped", rc("object B is object A (script.cpp:1203-1204)"));
                } else {
                    let args = [(0, Val::Int(verb)), (1, Val::Int(a)), (2, Val::Int(b.unwrap_or(0)))];
                    self.eng_script(p, ci, o, n, Some(false), Some(false), &args);
                }
            }
        }
        if self.trace.is_some() {
            let kw = vec![
                ("verb", J::Int(verb as i64)),
                ("a", J::Int(a as i64)),
                ("b", b.map_or(J::Null, |b| J::Int(b as i64))),
                ("first_end", enc::opt_s(p.first_end)),
                ("end", enc::opt_s(p.end)),
                ("queue", J::Int(p.queue.len() as i64)),
            ];
            self.ev(p, "sentence", kw);
        }
    }

    /// checkAndRunSentenceScript's first test (model.py `_sentence_waits`): the slots of the sentence
    /// script's number live and not frozen, and those whose state is not known.
    pub(super) fn sentence_waits(&mut self, p: &mut Path, o: &FactOracle, n: i32) -> (Vec<String>, Vec<String>) {
        let (mut busy, mut unknown) = (Vec::new(), Vec::new());
        for k in sentence_slots(self.game, n) {
            let live = self.read(p, k, o);
            let frozen = self.read(p, Fact::Frozen(slot_name(k)), o);
            if live == Val::Int(0) {
                continue;
            }
            if live.is_unknown() || frozen.is_unknown() {
                unknown.push(format!("{k} {}, frozen {}", self.show(live), self.show(frozen)));
            } else if !frozen.truthy() {
                busy.push(k.to_string());
            }
        }
        (busy, unknown)
    }

    /// Without room_change: the room fact, and the running slot ends when the change kills it (model.py
    /// `_eng_room1`).
    fn eng_room1(&mut self, p: &mut Path, ci: usize, o: &FactOracle, room: Val) {
        self.write(p, Fact::Room, room);
        let (head, cite) = Self::head_cite(p, ci);
        self.note(
            p,
            cite.clone(),
            &head,
            "the room change's kills of other slots and its exit and entry scripts are not run (feature room_change)",
        );
        let (caller, active) = {
            let c = p.call_at(ci);
            (c.caller, c.active)
        };
        let Some(fi) = p.frame_of(caller) else { return };
        if !active {
            return;
        }
        match self.room_slot(p.frame_at(fi)) {
            RoomSlot::Unk(u) => {
                let w = format!("slot kind unknown ({}); taken as surviving the room change", self.show(u));
                self.note(p, cite, &head, &w);
            }
            RoomSlot::Yes => {
                let f = p.frame_at(fi);
                f.killed = Some("room");
                let unit = f.unit;
                p.call_at(ci).active = false;
                self.gone(p, o, unit);
            }
            RoomSlot::No => {}
        }
    }

    // --- frames and turns --------------------------------------------------------------------------------------

    /// model.py `_push_frame`: a nested run starts (or, `nested` false, a runAllScripts turn).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn push_frame(
        &mut self,
        p: &mut Path,
        ci: usize,
        unit: UnitId,
        pc: u32,
        locals: &[(i32, Val)],
        owned: Owned,
        recursive: Option<bool>,
        resistant: Option<bool>,
        key: Option<Fact>,
        args_known: bool,
        nested: bool,
    ) {
        let g = self.game;
        let depth = p.frames().filter(|f| f.nested).count(); // vm.numNestedScripts before this run
        if nested && depth >= MAX_NESTING {
            let (_, cite) = Self::head_cite(p, ci);
            let d = format!("Too many nested scripts starting {} (script.cpp:351-352)", Game::unit_name(self.unit_key(unit)));
            self.error(p, "nesting", cite.clone(), &d, false);
            self.halt(p, "nesting", cite);
            return;
        }
        if p.frames().any(|f| f.unit == unit) {
            p.reentries += 1;
        }
        let mut f = Frame::new(unit, Some(pc), args_known, owned, recursive);
        f.locals = Locals::from_vec(locals.to_vec());
        f.resistant = resistant;
        f.sid = p.next_sid;
        f.nested = nested;
        p.next_sid += 1;
        let sid = f.sid;
        p.stack.push(Entry::Frame(f));
        if p.settle && (p.round != 0 || p.timed) {
            // it executes now: no turn of its own later in this frame (didexec, script.cpp:506-507, 979)
            p.round_ran.insert(Game::unit_name(self.unit_key(unit)));
        }
        p.depth = p.depth.max(p.frames().count() as u32);
        if let Some(k) = key {
            self.write(p, k, Val::Int(1));
        }
        if self.trace.is_some() {
            let ukey = self.unit_key(unit);
            let u = g.unit(unit);
            let offset = if pc < u.ins_len { J::Int(g.ins(unit)[pc as usize].offset as i64) } else { J::Null };
            let (head, _) = Self::head_cite(p, ci);
            let kw = vec![
                ("sid", J::Int(sid as i64)),
                ("unit", enc::unit_key(ukey)),
                ("name", enc::slot(Game::unit_name(ukey))),
                ("pc", J::Int(pc as i64)),
                ("offset", offset),
                ("nested", J::Bool(nested)),
                ("by", enc::s(&head)),
            ];
            self.ev(p, "push", kw);
        }
    }

    /// The running frame's turn is over (model.py `_finish`); then runScriptNested's resume test for the
    /// call below.
    pub(super) fn finish(&mut self, p: &mut Path, o: &FactOracle, how: &'static str, at: S, resume: Option<i32>) {
        let g = self.game;
        let Some(Entry::Frame(f)) = p.stack.pop() else { panic!("finish: the top is not a frame") };
        let ukey = self.unit_key(f.unit);
        let (name, key) = (Game::unit_name(ukey), g.lifecycle_key(ukey));
        match how {
            "yield" | "parked" => {
                if let Some(k) = key {
                    self.write(p, k, Val::Int(1));
                }
                let r = if how == "parked" {
                    f.pc.filter(|&pc| pc < g.unit(f.unit).ins_len).map(|pc| g.ins(f.unit)[pc as usize].offset)
                } else {
                    resume
                };
                match r {
                    None => {
                        let w = format!("{at}: the resume offset is past the code; at: not predicted");
                        self.note(p, rc(""), how, &w);
                    }
                    Some(r) => self.write(p, Fact::At(name, r), Val::Int(1)),
                }
                self.set_freeze(p, o, name, Some((f.freeze, true)), "");
                if p.settle {
                    // a recursive start's instance stays live while another runs on the stack
                    if p.frames().any(|h| Game::unit_name(g.unit(h.unit).key) == name && h.killed.is_none()) {
                        p.multi.insert(name);
                    }
                    // settle resumes it from here with the locals it has (its freeze count is in p.freeze)
                    match r.and_then(|r| g.at(f.unit, r)) {
                        None => p.pop_slot(name),
                        Some(idx) => {
                            let mut kept = f.clone();
                            kept.pc = Some(idx);
                            kept.killed = None;
                            kept.nested = false;
                            kept.time_loops = Vec::new();
                            p.set_slot(name, kept);
                        }
                    }
                }
            }
            "end" => {
                if let UnitKey::Obj(ob) = ukey {
                    self.kill_object(p, o, ob, &at, "stopObjectCode");
                } else {
                    if f.override_ != 0 {
                        let d = format!("{name}: stopObjectCode's error() (script.cpp:879-880)");
                        self.error(p, "ended with an open cutscene", at.clone(), &d, true);
                    }
                    if let Some(k) = key {
                        self.write(p, k, Val::Int(0));
                    }
                    self.clear(p, o, Prefix::Esc(name));
                    self.set_freeze(p, o, name, Some((0, true)), "");
                    p.pop_slot(name);
                    if f.recursive == Some(true) {
                        let w = format!("{name} ended; another instance started with R may still run");
                        self.note(p, rc(""), "end", &w);
                    }
                }
            }
            _ => {}
        }
        p.turns.push((name, how));
        let (sid, pc) = (f.sid, f.pc);
        let last = match p.stack.last() {
            None => true,
            Some(Entry::Call(c)) => c.caller.is_none() && p.stack.len() == 1,
            Some(Entry::Frame(_)) => false,
        };
        if last {
            p.last = Some(Box::new(f));
        }
        match p.stack.last() {
            None => {
                if p.end.is_none() {
                    p.end = Some(how);
                    p.end_at = Some(at.clone());
                }
            }
            Some(Entry::Call(c)) if c.active => {
                let caller = c.caller;
                let stays = match p.frame_of(caller) {
                    None => false,
                    Some(fi) => {
                        let fr = p.frame_at(fi);
                        fr.killed.is_none() && fr.freeze <= 0
                    }
                };
                if !stays {
                    let ci = p.stack.len() - 1;
                    p.call_at(ci).active = false;
                }
            }
            _ => {}
        }
        if self.trace.is_some() {
            let kw = vec![
                ("sid", J::Int(sid as i64)),
                ("name", enc::slot(name)),
                ("how", enc::s(how)),
                ("at", enc::s(&at)),
                ("resume", resume.map_or(J::Null, |r| J::Int(r as i64))),
                ("pc", pc.map_or(J::Null, |r| J::Int(r as i64))),
            ];
            self.ev(p, "finish", kw);
        }
    }

    // --- stops, kills and freezes --------------------------------------------------------------------------------

    /// stopScript(N) (model.py `_stop_script`): every global or local slot of N dies, frames on the stack too.
    pub(super) fn stop_script(&mut self, p: &mut Path, o: &FactOracle, n: i32, cite: &S, head: &str) {
        if n == 0 {
            return;
        }
        let g = self.game;
        let key = g.script_key(n);
        let name = slot_name(key);
        let mut opened = 0;
        for f in p.frames_mut() {
            let k = g.unit(f.unit).key;
            if Game::unit_name(k) == name && matches!(k, UnitKey::Global(_) | UnitKey::Local(..)) && f.killed.is_none() {
                opened += (f.override_ != 0) as u32;
                f.killed = Some("stopped");
            }
        }
        for _ in 0..opened {
            let d = format!("{name} by {head} (script.cpp:273-275, an error())");
            self.error(p, "stopped with an open cutscene", cite.clone(), &d, true);
        }
        self.stopped(p, o, key);
    }

    /// stopObjectScript(O) (model.py `_kill_object`): every object slot of O dies.
    pub(super) fn kill_object(&mut self, p: &mut Path, o: &FactOracle, obj: i32, cite: &S, head: &str) {
        let uid = self.game.id(UnitKey::Obj(obj));
        let mut opened = 0;
        for f in p.frames_mut() {
            if Some(f.unit) == uid && f.killed.is_none() {
                opened += (f.override_ != 0) as u32;
                f.killed = Some("stopped");
            }
        }
        for _ in 0..opened {
            let d = format!("obj:{obj} by {head} (script.cpp:307-309, an error())");
            self.error(p, "stopped with an open cutscene", cite.clone(), &d, true);
        }
        self.stopped(p, o, Fact::ObjScript(obj));
    }

    /// The live slots off the stack, by unit name, in model.py's dict order (model.py `_live_slots`):
    /// (name, slot fact or None, liveness).
    pub(super) fn live_slots(&mut self, p: &mut Path, o: &FactOracle) -> Vec<(Slot, Option<Fact>, Val)> {
        let mut out = Vec::new();
        for pre in [Prefix::Script, Prefix::Local, Prefix::ObjScript] {
            let mut keys = self.prefixed(p, o, pre);
            sort_str(&mut keys); // in a fixed order: the notes and errors it leads to are listed in it
            for k in keys {
                let v = self.read(p, k, o);
                if v != Val::Int(0) {
                    out.push((slot_name(k), Some(k), v));
                }
            }
        }
        for code in [Slot::Entry, Slot::Exit] {
            let mut keys = self.prefixed(p, o, Prefix::At(code));
            sort_str(&mut keys);
            let vals: Vec<Val> = keys.iter().map(|&k| self.read(p, k, o)).collect();
            if vals.contains(&Val::Int(1)) {
                out.push((code, None, Val::Int(1)));
            } else if let Some(&u) = vals.iter().find(|v| v.is_unknown()) {
                out.push((code, None, u));
            }
        }
        let g = self.game;
        let on_stack: Vec<Slot> = p.frames().map(|f| Game::unit_name(g.unit(f.unit).key)).collect();
        out.retain(|e| !on_stack.contains(&e.0));
        out
    }

    /// A slot's freeze count off the stack (model.py `_count`): (count, exact), or None when not known.
    fn count(&mut self, p: &mut Path, o: &FactOracle, name: Slot) -> Option<(i32, bool)> {
        if let Some(&c) = p.freeze.get(&name) {
            return c;
        }
        match self.read(p, Fact::Frozen(name), o) {
            Val::Unk(_) => None,
            Val::Int(0) => Some((0, true)),
            Val::Int(_) => Some((1, false)),
        }
    }

    /// A slot's freeze count off the stack and its `frozen:` fact (model.py `_set_freeze`).
    pub(super) fn set_freeze(&mut self, p: &mut Path, o: &FactOracle, name: Slot, n: Option<(i32, bool)>, why: &str) {
        p.freeze.insert(name, n);
        let v = match n {
            None => self.v.mk(Reason::Freeze, if why.is_empty() { name.to_string() } else { why.to_string() }),
            Some((c, _)) if c > 0 => Val::Int(1),
            Some((_, true)) => Val::Int(0),
            Some((_, false)) => {
                let w = format!("{name}: frozen before the move, then unfrozen once; its count is not a fact");
                self.v.mk(Reason::Freeze, w)
            }
        };
        if !(v == Val::Int(0) && self.read(p, Fact::Frozen(name), o) == Val::Int(0)) {
            self.write(p, Fact::Frozen(name), v);
        }
    }

    /// freezeScripts(flag) (model.py `_freeze`).
    pub(super) fn freeze(&mut self, p: &mut Path, o: &FactOracle, flag: i32, x: X) {
        let g = self.game;
        let c = self.cite(x);
        let cur = p.top_frame_index();
        let every = flag >= 0x80;
        let mut unknown_f = Vec::new();
        for (i, e) in p.stack.iter_mut().enumerate() {
            let Entry::Frame(f) = e else { continue };
            if Some(i) != cur && (every || f.resistant == Some(false)) {
                f.freeze += 1;
            } else if Some(i) != cur && f.resistant.is_none() {
                unknown_f.push(Game::unit_name(g.unit(f.unit).key));
            }
        }
        for name in unknown_f {
            self.note(p, c.clone(), "freezeScripts", &format!("{name}: F flag not known; taken as not frozen"));
        }
        for (name, key, live) in self.live_slots(p, o) {
            let kept = p.slots.iter().find(|s| s.0 == name).map(|s| s.1.resistant);
            let kept_known = matches!(kept, Some(Some(_)));
            let mut room: Option<Val> = None;
            let res = if every {
                Some(false)
            } else if let Some(Some(r)) = kept {
                Some(r)
            } else {
                if matches!(name, Slot::Local(_)) {
                    let r = self.read(p, Fact::Room, o);
                    room = Some(self.resource(r));
                }
                match key {
                    Some(k) => g.resistant(k, room.and_then(|r| r.int())),
                    None => Some(false), // room code: no F flag
                }
            };
            if res == Some(true) {
                continue;
            }
            if res.is_none() || live.is_unknown() {
                let why = if res.is_some() {
                    format!("live {}", self.show(live))
                } else if let Some(r @ Val::Unk(_)) = room {
                    format!("room resource {}", self.show(r))
                } else {
                    "its F flag differs between start sites".to_string()
                };
                self.set_freeze(p, o, name, None, &format!("{name}: {why}"));
                continue;
            }
            if !every && matches!(key, Some(Fact::Script(_) | Fact::Local(_))) && !kept_known {
                let w = format!(
                    "{name}: F flag from the constant start sites ({} starts with a variable number not counted)",
                    g.wild_starts.len()
                );
                self.note(p, c.clone(), "assumption: F flag", &w);
            }
            let n = self.count(p, o, name);
            self.set_freeze(p, o, name, n.map(|(k, e)| (k + 1, e)), &format!("{name}: freeze count not known"));
        }
        if let Some(fi) = p.frame_of(p.cut_index) {
            p.frame_at(fi).freeze = 0; // script.cpp:937-940
        }
        if p.settle {
            // every queued sentence's count goes up too (script.cpp:934-935)
            for e in p.queue.iter_mut() {
                e.3 += 1;
            }
        }
        self.note(p, c, "freezeScripts", "the queued sentences' freeze counts are not facts");
    }

    /// unfreezeScripts (model.py `_unfreeze`): every frozen slot loses 1 from its count.
    pub(super) fn unfreeze(&mut self, p: &mut Path, o: &FactOracle) {
        for f in p.frames_mut() {
            if f.freeze > 0 {
                f.freeze -= 1;
            }
        }
        for (name, _, _) in self.live_slots(p, o) {
            let Some(n) = self.count(p, o, name) else { continue };
            if n.0 > 0 {
                self.set_freeze(p, o, name, Some((n.0 - 1, n.1)), "");
            }
        }
        if p.settle {
            // and every queued sentence's count above 0 goes down (script.cpp:962-965)
            for e in p.queue.iter_mut() {
                e.3 = 0.max(e.3 - 1);
            }
        }
    }

    /// Whether a room change kills the slot (model.py `_room_slot`).
    pub(super) fn room_slot(&self, f: &Frame) -> RoomSlot {
        match self.unit_key(f.unit) {
            UnitKey::Entry(_) | UnitKey::Exit(_) | UnitKey::Local(..) => RoomSlot::Yes,
            UnitKey::Obj(_) => match f.owned {
                Owned::Unk(u) => RoomSlot::Unk(u),
                Owned::Bool(false) => RoomSlot::Yes,
                Owned::Bool(true) => RoomSlot::No,
            },
            UnitKey::Global(_) => RoomSlot::No,
        }
    }

    /// A frame's slot died: its fact, wait points, override and frozen: go (model.py `_gone`).
    pub(super) fn gone(&mut self, p: &mut Path, o: &FactOracle, unit: UnitId) {
        let ukey = self.unit_key(unit);
        let (key, name) = (self.game.lifecycle_key(ukey), Game::unit_name(ukey));
        if let Some(k) = key {
            self.write(p, k, Val::Int(0));
        }
        self.clear(p, o, Prefix::At(name));
        self.clear(p, o, Prefix::Esc(name));
        self.set_freeze(p, o, name, Some((0, true)), "");
        p.pop_slot(name);
    }

    // --- getters --------------------------------------------------------------------------------------------------

    /// findInventory(owner, n) with calls on (model.py `_find_inventory`).
    pub(super) fn find_inventory(&mut self, p: &mut Path, o: &FactOracle, args: Span, x: Option<X>) -> Val {
        let ks = self.game.ast.kids(args);
        assert_eq!(ks.len(), 2, "findInventory takes two values");
        let (a0, a1) = (ks[0], ks[1]);
        let owner = self.val(p, o, a0, x);
        let n = self.val(p, o, a1, x);
        let (Val::Int(ow), Val::Int(k)) = (owner, n) else {
            let w = format!("findInventory({},{})", self.show(owner), self.show(n));
            return self.v.mk(Reason::Inventory, w);
        };
        match self.owned_by(p, o, ow) {
            Err((key, v)) => {
                let w = format!("findInventory({ow},{k}): {key} {}", self.show(v));
                self.v.mk(Reason::Inventory, w)
            }
            Ok(owned) => {
                let c = self.cite_of(x);
                self.note(p, c, "findInventory", "the owner's objects from owner facts; the engine walks its inventory list");
                if k < 1 || k as usize > owned.len() {
                    return Val::Int(0);
                }
                self.v.one_of(owned, Reason::Inventory, format!("findInventory({ow},{k})"))
            }
        }
    }
}

/// isScriptInUse matches the number in any slot (model.py `_sentence_slots`).
pub(super) fn sentence_slots(g: &Game, n: i32) -> Vec<Fact> {
    if n == 0 {
        Vec::new()
    } else {
        vec![g.script_key(n), Fact::ObjScript(n)]
    }
}
