//! E013's component `refuse` in the window search (experiments/E013-robust/plan.md, method B; amendment (3)): when a
//! ready window's try fails at the interface, the try's input log (patch 0007) names the cause, and the parent forks
//! its next try only at a frame where a value the cause names has changed in the parent; when the window's frames end
//! with no such change, the window ends at the refused try's fork frame, from a fork of the parent kept there.
//! Switch FMI_REFUSE=1 (off by default: the window search is E010's and E012's).
//!
//! * **The cause** (`cause_of`, amendment (3) items 1-2): the executor could not form the plan's input ("no input
//!   formed"); or a check after an input it issued failed, and the checked input's log record names the row:
//!   "dropped: gate" when the parent's own gate is shut at that frame (E010's case: B does nothing), "gate shut by the
//!   try" when the parent's gate is open there (the try's own inputs shut it: read as taken over all its records),
//!   "dropped: zone", "dropped: handler" (and "cleared", "unread"), "ignored", or "taken, the check failed".
//! * **The values watched** (`Watch`, item 4): the reads of the cause's records mapped onto the parent's dump and
//!   status, the executor's check variable, and the input part (the status's gate, cursor, cutscene, room and screen
//!   layout, and the verb slots) or the interface part (also the room objects' boxes and states, the actors, the
//!   camera and the inventory).
//! * **A change** (item 3): the parent's watched values at a frame against the parent's own at the refused try's fork
//!   frame.

use fmi_core::*;
use rustc_hash::FxHasher;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

/// The switch.
pub fn on() -> bool {
    std::env::var("FMI_REFUSE").is_ok_and(|v| v == "1")
}

/// The causes of a refused try (the window record's "refuse" and its end reason "refused input: <cause>").
pub const NO_INPUT: &str = "no input formed";
pub const GATE: &str = "dropped: gate";
pub const GATE_BY_TRY: &str = "gate shut by the try";
pub const TAKEN_CHECK: &str = "taken, the check failed";

/// What a refused try's next try waits for: the parent's values the cause names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    pub vars: BTreeSet<usize>,
    pub bits: BTreeSet<u32>,
    pub owners: BTreeSet<u32>,
    pub states: BTreeSet<u32>,
    pub classes: BTreeSet<(u32, u32)>,
    pub actors: BTreeSet<i32>,
    pub scripts: BTreeSet<i32>,
    /// the RNG state (a random draw was read)
    pub rng: bool,
    /// the input part: the status's gate (userput, cutscene), cursor, room and screen layout, and the verb slots
    pub input: bool,
    /// the interface part: the input part, the room objects' boxes and states, the actors, the camera, the inventory
    pub interface: bool,
}

/// The parts a change is named by in the record.
pub const PARTS: [&str; 10] = ["vars", "bits", "owners", "states", "classes", "actors", "scripts", "rng", "input", "interface"];

fn h<T: Hash>(t: &T) -> u64 {
    let mut s = FxHasher::default();
    t.hash(&mut s);
    s.finish()
}

impl Watch {
    /// The reads of a record, mapped onto the dump and the status (amendment (3) item 4).
    pub fn add_reads(&mut self, ts: &[Touch]) {
        for t in ts {
            match t.kind {
                TouchKind::Var => {
                    self.vars.insert(t.a.max(0) as usize);
                }
                TouchKind::Bit => {
                    self.bits.insert(t.a.max(0) as u32);
                }
                TouchKind::Owner => {
                    self.owners.insert(t.a.max(0) as u32);
                }
                TouchKind::State => {
                    self.states.insert(t.a.max(0) as u32);
                }
                TouchKind::Class => {
                    self.classes.insert((t.a.max(0) as u32, t.b.max(0) as u32));
                }
                TouchKind::Actor => {
                    self.actors.insert(t.a);
                }
                TouchKind::ObjectAt | TouchKind::ActorAt => self.interface = true,
                TouchKind::Random => self.rng = true,
                TouchKind::ScriptRunning => {
                    self.scripts.insert(t.a);
                }
                TouchKind::UserPut | TouchKind::Cursor => self.input = true,
                // writes only
                TouchKind::Started
                | TouchKind::Sentence
                | TouchKind::Walk
                | TouchKind::Print
                | TouchKind::Verb
                | TouchKind::Camera => {}
            }
        }
    }

    /// The watched values' hash per part (PARTS' order) at a stop of the parent.
    pub fn parts(&self, st: &Status, o: &Obs) -> [u64; 10] {
        let vars: Vec<i32> = self.vars.iter().map(|&n| o.vars.get(n).copied().unwrap_or(0)).collect();
        let bits: Vec<bool> = self.bits.iter().map(|b| o.bits_set.contains(b)).collect();
        let owners: Vec<Option<i32>> =
            self.owners.iter().map(|x| o.owners.iter().find(|p| p.0 == *x).map(|p| p.1)).collect();
        let states: Vec<Option<i32>> =
            self.states.iter().map(|x| o.states.iter().find(|p| p.0 == *x).map(|p| p.1)).collect();
        let classes: Vec<bool> = self
            .classes
            .iter()
            .map(|(ob, c)| o.classes.iter().any(|p| p.0 == *ob && p.1.contains(c)))
            .collect();
        let actors: Vec<Option<(i32, i32, i32, i32)>> = self
            .actors
            .iter()
            .map(|id| o.actors.iter().find(|a| a.id == *id).map(|a| (a.x, a.y, a.elevation, a.moving)))
            .collect();
        let scripts: Vec<usize> = self
            .scripts
            .iter()
            .map(|n| o.slots.iter().filter(|s| s.number == *n && (s.where_ == WIO_GLOBAL || s.where_ == WIO_LOCAL)).count())
            .collect();
        let input = if self.input || self.interface { input_part(st, o) } else { 0 };
        let interface = if self.interface { interface_part(st, o) } else { 0 };
        [
            h(&vars),
            h(&bits),
            h(&owners),
            h(&states),
            h(&classes),
            h(&actors),
            h(&scripts),
            if self.rng { st.rng } else { 0 },
            input,
            interface,
        ]
    }

    /// The record's counts of what is watched.
    pub fn j(&self) -> crate::pyj::J {
        crate::jobj! {
            "vars" => self.vars.len(), "bits" => self.bits.len(), "owners" => self.owners.len(),
            "states" => self.states.len(), "classes" => self.classes.len(), "actors" => self.actors.len(),
            "scripts" => self.scripts.len(), "rng" => self.rng, "input" => self.input, "interface" => self.interface,
        }
    }
}

/// The input part: the status's gate (userput, cutscene), cursor, room and screen layout, and the dump's verb slots.
pub fn input_part(st: &Status, o: &Obs) -> u64 {
    let verbs: Vec<(i32, i32, i32, i32, i32, i32, i32)> =
        o.verbs.iter().map(|v| (v.slot, v.id, v.mode, v.saveid, v.key, v.x, v.y)).collect();
    h(&(st.up, st.cs, st.cur, st.room, st.screen, verbs))
}

/// The interface part beyond the input part: the room objects' boxes and states, the actors, the camera, the inventory.
pub fn interface_part(st: &Status, o: &Obs) -> u64 {
    let ros: Vec<(i32, i32, i32, i32, i32, i32, i32, (i32, i32))> = o
        .room_objects
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|r| (r.id, r.state, r.parent, r.x, r.y, r.w, r.h, r.walk))
        .collect();
    let actors: Vec<(i32, i32, i32, i32, i32)> = o.actors.iter().map(|a| (a.id, a.x, a.y, a.elevation, a.moving)).collect();
    h(&(ros, actors, st.cam, st.xstart, st.room_w, &o.inventory))
}

/// The cause of a failed try and what its next try waits for, from the try's log records (those after its fork
/// frame), the executor's failed check (None: it could not form its input) and the parent's gate at the checked
/// input's frame (`parent_gate_open`, asked only for a drop at the gate). None: B does nothing (E010's gate wait).
pub fn cause_of(
    records: &[InputRecord],
    check_var: Option<usize>,
    parent_gate_open: &mut dyn FnMut(u64) -> bool,
) -> Option<(String, Watch)> {
    let mut w = Watch::default();
    let Some(cv) = check_var else {
        w.interface = true;
        return Some((NO_INPUT.into(), w));
    };
    w.vars.insert(cv);
    w.input = true;
    let Some(last) = records.last() else {
        // a check failed with no record of its input: the interface part
        w.interface = true;
        return Some((NO_INPUT.into(), w));
    };
    match &last.verdict {
        Verdict::Dropped(why) if why == "gate" => {
            if !parent_gate_open(last.frame) {
                return None;
            }
            for r in records {
                w.add_reads(&r.reads);
            }
            Some((GATE_BY_TRY.into(), w))
        }
        Verdict::Dropped(why) => {
            w.add_reads(&last.reads);
            Some((format!("dropped: {why}"), w))
        }
        Verdict::Ignored => {
            w.add_reads(&last.reads);
            Some(("ignored".into(), w))
        }
        Verdict::Taken => {
            w.add_reads(&last.reads);
            Some((TAKEN_CHECK.into(), w))
        }
    }
}

/// The names of the parts that differ.
pub fn changed(a: &[u64; 10], b: &[u64; 10]) -> Vec<&'static str> {
    PARTS.iter().zip(a.iter().zip(b)).filter(|(_, (x, y))| x != y).map(|(n, _)| *n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(frame: u64, verdict: Verdict, reads: Vec<Touch>) -> InputRecord {
        InputRecord {
            frame,
            input: vec![],
            stat: 0,
            verdict,
            area: 0,
            value: 0,
            mode: 0,
            slot: 0,
            zone: -1,
            key_match: false,
            handler: 4,
            writes: vec![],
            reads,
            ops: vec![],
            ops_dropped: 0,
            touches_dropped: 0,
        }
    }

    fn t(kind: TouchKind, a: i32) -> Touch {
        Touch { kind, a, b: 0, old: 0, now: 0 }
    }

    #[test]
    fn causes_by_row() {
        let mut open = |_f: u64| true;
        let mut shut = |_f: u64| false;
        // no check: the executor could not form its input
        let (c, w) = cause_of(&[], None, &mut open).unwrap();
        assert_eq!(c, NO_INPUT);
        assert!(w.interface);
        // a drop at the gate the parent holds shut: E010's case
        let walk = rec(10, Verdict::Taken, vec![t(TouchKind::Var, 20), t(TouchKind::Actor, 1)]);
        let key = rec(30, Verdict::Dropped("gate".into()), vec![t(TouchKind::UserPut, 0)]);
        assert!(cause_of(&[walk.clone(), key.clone()], Some(107), &mut shut).is_none());
        // ... and the gate the try shut: every record's reads, the check's variable
        let (c, w) = cause_of(&[walk.clone(), key.clone()], Some(107), &mut open).unwrap();
        assert_eq!(c, GATE_BY_TRY);
        assert_eq!(w.vars.iter().copied().collect::<Vec<_>>(), vec![20, 107]);
        assert!(w.actors.contains(&1) && w.input && !w.interface);
        // the other rows read the checked input's record only
        let ign = rec(30, Verdict::Ignored, vec![t(TouchKind::Bit, 7), t(TouchKind::ObjectAt, 0)]);
        let (c, w) = cause_of(&[walk.clone(), ign], Some(107), &mut open).unwrap();
        assert_eq!(c, "ignored");
        assert!(w.bits.contains(&7) && w.interface && !w.vars.contains(&20));
        let (c, _) = cause_of(&[rec(30, Verdict::Dropped("handler".into()), vec![])], Some(107), &mut open).unwrap();
        assert_eq!(c, "dropped: handler");
        let (c, w) = cause_of(&[rec(30, Verdict::Taken, vec![t(TouchKind::Random, 3)])], Some(107), &mut open).unwrap();
        assert_eq!(c, TAKEN_CHECK);
        assert!(w.rng);
    }

    #[test]
    fn parts_change_only_on_watched_values() {
        let mut w = Watch::default();
        w.vars.insert(3);
        w.input = true;
        let st = Status { up: 1, ..Default::default() };
        let mut o = Obs { vars: vec![0, 0, 0, 5, 9], ..Default::default() };
        let a = w.parts(&st, &o);
        o.vars[4] = 10; // not watched
        assert!(changed(&a, &w.parts(&st, &o)).is_empty());
        o.vars[3] = 6;
        assert_eq!(changed(&a, &w.parts(&st, &o)), vec!["vars"]);
        o.vars[3] = 5;
        let st2 = Status { up: 0, ..Default::default() };
        assert_eq!(changed(&a, &w.parts(&st2, &o)), vec!["input"]);
        // the rng is watched only after a draw
        let st3 = Status { up: 1, rng: 77, ..Default::default() };
        assert!(changed(&a, &w.parts(&st3, &o)).is_empty());
    }
}
