//! E012 component `tested` (experiments/E012-game-mechanics/plan.md, "Method" 1, and the amendment "component
//! `tested`"): the reading's tested atoms T.
//!
//! [`Tested::of`]: for each action, its record's keys (the line's `pre_keys`) that its precondition holds, with the
//! precondition's value set. An event (source `event`, the Esc events too) drops its own unit's keys: the slot
//! (`local:N`, `script:N`, `objscript:O`) and the unit's `pos`, `escpos`, `args`, `frozen`, `at` and `esc` keys. An
//! input is every other action (`sentence`, which holds the walks, and `verb`, which holds the dialog lines). The
//! goal's atoms are added. The draw checks are in [`crate::draws`], re-exported here.

use crate::draws::check_why;
pub use crate::draws::{check_parts, draws, is_draw, reason_of, Draw, DrawGame, Site, DRAW_WHY};
use crate::Reading;
use fmi_core::{Domain, Key, ReadingSource, Val, ABOVE, BELOW, K_ARGS, K_AT, K_ESC, K_ESCPOS, K_FROZEN, K_LOCAL};
use fmi_core::{K_OBJSCRIPT, K_POS, K_SCRIPT, U_LOCAL, U_OBJ, U_SCRIPT};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

/// Counts of [`Tested::of`] (reported with gate T1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TestedCounts {
    pub actions: usize,
    /// actions whose source is `event` (the Esc events among them)
    pub events: usize,
    pub esc_events: usize,
    /// actions whose source is `sentence` (walks among them) or `verb` (dialog lines, verb clicks)
    pub sentences: usize,
    pub verbs: usize,
    /// other sources (none in the readings measured)
    pub other: usize,
    /// actions with at least one check on a random draw
    pub draw_actions: usize,
    /// checks by their non-fact reason, over every action
    pub checks: BTreeMap<String, usize>,
    /// distinct `pre_keys` texts over every action, before the own-unit keys are dropped
    pub pre_keys_all: usize,
    /// the record's keys the action's precondition does not hold (no value set: not in T)
    pub pre_keys_not_held: usize,
    /// own-unit keys dropped from events (key occurrences)
    pub own_dropped: usize,
    /// T's atoms ((key, value set) pairs) and keys
    pub atoms: usize,
    pub keys: usize,
    /// keys tested by inputs and by events (of other units); both; the goal's keys tested by no action
    pub input_keys: usize,
    pub event_keys: usize,
    pub both_keys: usize,
    pub goal_only_keys: usize,
    /// atoms tested by inputs, by events
    pub input_atoms: usize,
    pub event_atoms: usize,
}

/// T (module docstring).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tested {
    /// per key (sorted), the distinct value sets the preconditions hold it to (each set sorted ascending, as the
    /// task's values are, so `below` first and `above` last; the sets sorted), the goal's among them
    pub atoms: Vec<(Key, Vec<Vec<Val>>)>,
    /// the keys an input's precondition tests, sorted
    pub inputs: Vec<Key>,
    /// the keys an event's precondition tests on a unit other than its own, sorted
    pub events: Vec<Key>,
    /// the goal's keys, sorted
    pub goal: Vec<Key>,
    pub counts: TestedCounts,
}

/// The unit an event's source names (`local:N`, `script:N`, `obj:O`, `entry`, `exit`) as (unit kind, number), as the
/// keys of that unit hold it.
fn unit_of_text(u: &str) -> Option<(u8, u64)> {
    let k = Key::parse(&format!("pos:{u}"));
    (k.kind() == K_POS).then(|| (k.unit(), k.a()))
}

/// Whether `k` is a key of the unit `u`: its slot, or its position, Esc position, arguments, freeze count, wait point
/// or Esc point.
pub fn own_unit_key(k: Key, u: (u8, u64)) -> bool {
    match k.kind() {
        K_POS | K_ESCPOS | K_ARGS | K_FROZEN | K_AT | K_ESC => (k.unit(), k.a()) == u,
        K_LOCAL => u == (U_LOCAL, k.a()),
        K_SCRIPT => u == (U_SCRIPT, k.a()),
        K_OBJSCRIPT => u == (U_OBJ, k.a()),
        _ => false,
    }
}

/// An action's source kind and, for an event, its unit and whether it is an Esc (the source's JSON text:
/// `["event", unit, W, sig, esc]`, `["sentence", ...]`, `["verb", V]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Event { unit: (u8, u64), esc: bool },
    Sentence,
    Verb,
    Other,
}

/// The source of an action line's `source` text.
pub fn source_of(text: &str) -> Source {
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Source::Other,
    };
    match v.get(0).and_then(Value::as_str) {
        Some("event") => match v.get(1).and_then(Value::as_str).and_then(unit_of_text) {
            Some(unit) => Source::Event { unit, esc: v.get(4).and_then(Value::as_bool).unwrap_or(false) },
            None => Source::Other,
        },
        Some("sentence") => Source::Sentence,
        Some("verb") => Source::Verb,
        _ => Source::Other,
    }
}

impl Reading {
    /// Action i's precondition alone: `action(i).pre` (PackedTask._action's: per key its values ascending, a later
    /// set of a key in the earlier one's place) without decoding the effects, the name or the line.
    pub fn pre_of(&self, i: u32) -> Vec<(Key, Vec<Val>)> {
        let pk = &self.packed;
        let p = self.idx.tc_word(i) + 1;
        let npre = pk.word(p + 6);
        let mut q = p + 7;
        let mut out: Vec<(Key, Vec<Val>)> = Vec::with_capacity(npre as usize);
        for _ in 0..npre {
            let (f, m) = (pk.word(q), pk.word(q + 1) as u64);
            let v = crate::tc::sorted((0..m).map(|k| pk.v(pk.word(q + 2 + k))).collect());
            let k = pk.fact_keys[f as usize];
            match out.iter_mut().find(|x| x.0 == k) {
                Some(x) => x.1 = v,
                None => out.push((k, v)),
            }
            q += 2 + m;
        }
        out
    }
}

impl Tested {
    /// T of a reading (module docstring), over any `ReadingSource` (its `action(i).pre`).
    pub fn of(rd: &dyn ReadingSource) -> Tested {
        Tested::build(rd, &|i| rd.action(i).pre)
    }

    /// T of a reading's files: the same as [`Tested::of`], its preconditions decoded alone ([`Reading::pre_of`]).
    pub fn of_reading(rd: &Reading) -> Tested {
        Tested::build(rd, &|i| rd.pre_of(i))
    }

    fn build(rd: &dyn ReadingSource, pre_of: &dyn Fn(u32) -> Vec<(Key, Vec<Val>)>) -> Tested {
        const BY_INPUT: u8 = 1;
        const BY_EVENT: u8 = 2;
        let n = rd.n_actions();
        let mut c = TestedCounts { actions: n as usize, ..Default::default() };
        // per key, its value sets with who tests each (BY_INPUT, BY_EVENT; 0: the goal's alone)
        let mut atoms: HashMap<Key, Vec<(Vec<Val>, u8)>> = HashMap::new();
        let mut key_of: HashMap<String, Key> = HashMap::new();
        let mut checks: HashMap<String, usize> = HashMap::new();
        for i in 0..n {
            let ln = crate::line::parse(rd.action_line(i)).unwrap_or_else(|e| panic!("action {i}: its line: {e}"));
            let src = source_of(ln.source);
            match src {
                Source::Event { esc, .. } => {
                    c.events += 1;
                    c.esc_events += esc as usize;
                }
                Source::Sentence => c.sentences += 1,
                Source::Verb => c.verbs += 1,
                Source::Other => c.other += 1,
            }
            let mut draw = false;
            for ch in &ln.checks {
                if let Some(why) = check_why(ch) {
                    if let Some(r) = reason_of(&why) {
                        match checks.get_mut(r) {
                            Some(x) => *x += 1,
                            None => {
                                checks.insert(r.to_string(), 1);
                            }
                        }
                    }
                    draw |= is_draw(&why);
                }
            }
            c.draw_actions += draw as usize;
            // the record's keys the precondition holds, with its value set (Record::pre: the precondition keeps the
            // last set of a key, and a key's text is the packed file's fact name)
            let pre = pre_of(i);
            for t in &ln.pre_keys {
                let key = match key_of.get(t.as_ref()) {
                    Some(&k) => k,
                    None => {
                        let k = Key::parse(t);
                        key_of.insert(t.to_string(), k);
                        k
                    }
                };
                let Some((k, vs)) = pre.iter().find(|p| p.0 == key) else {
                    c.pre_keys_not_held += 1;
                    continue;
                };
                let by = match src {
                    Source::Event { unit, .. } => {
                        if own_unit_key(*k, unit) {
                            c.own_dropped += 1;
                            continue;
                        }
                        BY_EVENT
                    }
                    _ => BY_INPUT,
                };
                let sets = atoms.entry(*k).or_default();
                match sets.iter_mut().find(|x| x.0 == *vs) {
                    Some(x) => x.1 |= by,
                    None => sets.push((vs.clone(), by)),
                }
            }
        }
        let mut goal: Vec<Key> = Vec::new();
        for (k, vs) in rd.goal() {
            let vs = crate::tc::sorted(vs.clone());
            let sets = atoms.entry(*k).or_default();
            if !sets.iter().any(|x| x.0 == vs) {
                sets.push((vs, 0));
            }
            goal.push(*k);
        }
        goal.sort_unstable();
        goal.dedup();
        let (mut inputs, mut events): (Vec<Key>, Vec<Key>) = (Vec::new(), Vec::new());
        for (k, sets) in &atoms {
            let by = sets.iter().fold(0, |a, x| a | x.1);
            if by & BY_INPUT != 0 {
                inputs.push(*k);
            }
            if by & BY_EVENT != 0 {
                events.push(*k);
            }
            c.input_atoms += sets.iter().filter(|x| x.1 & BY_INPUT != 0).count();
            c.event_atoms += sets.iter().filter(|x| x.1 & BY_EVENT != 0).count();
            c.both_keys += (by == BY_INPUT | BY_EVENT) as usize;
            c.goal_only_keys += (by == 0) as usize;
        }
        inputs.sort_unstable();
        events.sort_unstable();
        c.checks = checks.into_iter().collect();
        c.pre_keys_all = key_of.len();
        c.input_keys = inputs.len();
        c.event_keys = events.len();
        let mut atoms: Vec<(Key, Vec<Vec<Val>>)> = atoms
            .into_iter()
            .map(|(k, s)| {
                let mut v: Vec<Vec<Val>> = s.into_iter().map(|x| x.0).collect();
                v.sort();
                (k, v)
            })
            .collect();
        atoms.sort_unstable_by_key(|a| a.0);
        c.keys = atoms.len();
        c.atoms = atoms.iter().map(|a| a.1.len()).sum();
        Tested { atoms, inputs, events, goal, counts: c }
    }

    /// Every atom as (key, value set), in `atoms`' order.
    pub fn flat(&self) -> Vec<(Key, Vec<Val>)> {
        self.atoms.iter().flat_map(|(k, sets)| sets.iter().map(move |vs| (*k, vs.clone()))).collect()
    }

    /// T's keys, sorted.
    pub fn keys(&self) -> Vec<Key> {
        self.atoms.iter().map(|a| a.0).collect()
    }

    /// Whether some action (or the goal) tests `k`.
    pub fn has(&self, k: Key) -> bool {
        self.atoms.binary_search_by_key(&k, |a| a.0).is_ok()
    }

    /// The counts and the keys as JSON (the T1 report).
    pub fn json(&self) -> Value {
        let c = &self.counts;
        let texts = |ks: &[Key]| -> Vec<String> {
            let mut t: Vec<String> = ks.iter().map(|k| k.text()).collect();
            t.sort();
            t
        };
        json!({
            "actions": c.actions, "events": c.events, "esc_events": c.esc_events, "sentences": c.sentences,
            "verbs": c.verbs, "other": c.other, "draw_actions": c.draw_actions, "checks": c.checks,
            "pre_keys_all": c.pre_keys_all, "pre_keys_not_held": c.pre_keys_not_held, "own_dropped": c.own_dropped,
            "keys": c.keys, "atoms": c.atoms, "input_keys": c.input_keys, "event_keys": c.event_keys,
            "both_keys": c.both_keys, "goal_only_keys": c.goal_only_keys, "input_atoms": c.input_atoms,
            "event_atoms": c.event_atoms,
            "inputs": texts(&self.inputs), "events_keys": texts(&self.events), "goal": texts(&self.goal),
        })
    }
}

/// Whether an engine value `v` of a key with domain `dom` (None: no domain, the value as it is) is in the value set
/// `vs` of an atom: the domain's label of the value (`Domain::label`: `below` under its low bound, `above` over its
/// high bound) is one of the set's values.
pub fn atom_holds(dom: Option<&Domain>, vs: &[Val], v: Val) -> bool {
    let l = match dom {
        Some(d) => d.label(v),
        None => v,
    };
    vs.contains(&l) || (l == BELOW && vs.contains(&BELOW)) || (l == ABOVE && vs.contains(&ABOVE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_unit() {
        let u = unit_of_text("local:206").unwrap();
        for t in
            ["local:206", "pos:local:206", "escpos:local:206", "args:local:206", "frozen:local:206", "at:local:206:49"]
        {
            assert!(own_unit_key(Key::parse(t), u), "{t}");
        }
        for t in ["local:207", "pos:local:207", "script:206", "frozen:script:206", "var:206", "room"] {
            assert!(!own_unit_key(Key::parse(t), u), "{t}");
        }
        let o = unit_of_text("obj:316").unwrap();
        assert!(own_unit_key(Key::parse("objscript:316"), o));
        assert!(own_unit_key(Key::parse("pos:obj:316"), o));
        assert!(!own_unit_key(Key::parse("objscript:315"), o));
        let s = unit_of_text("script:14").unwrap();
        assert!(own_unit_key(Key::parse("script:14"), s) && own_unit_key(Key::parse("args:script:14"), s));
        assert_eq!(source_of(r#"["event", "obj:316", 36, 0, false]"#), Source::Event { unit: o, esc: false });
        assert_eq!(
            source_of(r#"["event", "local:222", 1082, 0, true]"#).clone(),
            Source::Event { unit: unit_of_text("local:222").unwrap(), esc: true }
        );
        assert_eq!(source_of(r#"["sentence", 11, 315, null]"#), Source::Sentence);
        assert_eq!(source_of(r#"["verb", 120]"#), Source::Verb);
    }

    #[test]
    fn holds() {
        let d = Domain { values: vec![BELOW, 0, 1, 2, ABOVE], low: Some(0), high: Some(2) };
        assert!(atom_holds(Some(&d), &[ABOVE], 7));
        assert!(atom_holds(Some(&d), &[BELOW, 0], -3));
        assert!(!atom_holds(Some(&d), &[1, 2], 0));
        assert!(atom_holds(None, &[5], 5));
    }
}
