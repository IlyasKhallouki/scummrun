//! Feature dialog (row 4): model.py's `run_verb` (a click on a verb: a dialog line), its `input` step
//! (`_eng_input`: checkExecVerbs and runInputScript) and `_merge`'s rule for `Path.input_drift`.
//!
//! Each method is the model.py method of the same name, in the same order of reads, writes, notes, forks and ends.
//! model.py's docstrings hold the engine rules and the ScummVM cites.

use super::settle::MOUSE_VARS;
use super::*;

/// kVerbClickArea (verbs.h:35): runInputScript's first argument for a click on a verb (model.py `VERB_CLICK_AREA`).
pub const VERB_CLICK_AREA: i32 = 1;
/// checkExecVerbs' button code for a left click (verbs.cpp:666; model.py `LEFT_CLICK`).
pub const LEFT_CLICK: i32 = 1;

impl<'g> Machine<'g> {
    /// Feature dialog: a click on verb `verb` from the click's frame on (model.py `run_verb`): the mouse moves, every
    /// slot live at pre has its turn, checkExecVerbs runs the input script (`eng_input`), checkAndRunSentenceScript
    /// runs a sentence it queued; then the rounds (settle).
    pub fn run_verb(&mut self, o: &FactOracle, verb: i32, button: i32) -> Run {
        assert!(self.dialogs, "a click on a verb runs only with dialog");
        let g = self.game;
        self.pre = self.pre_slots(o);
        let mut steps = Vec::with_capacity(MOUSE_VARS.len() + self.pre.len() + 2);
        for (v, name) in MOUSE_VARS {
            let u = self.mouse_value(v, name);
            steps.push(Step::Write(Fact::Var(v), u));
        }
        steps.extend(self.pre.iter().map(|&name| Step::Turn(name)));
        steps.push(Step::Input { area: VERB_CLICK_AREA, verb, button });
        steps.push(Step::Sentences);
        let cid = self.cid();
        let call = Call {
            head: rc("click"),
            cite: rc("scumm.cpp:3177-3255"),
            steps: steps.into(),
            caller: None,
            active: false,
            i: 0,
            cid,
        };
        // the Run's unit: the input script installed at pre (model.py reads the room only for a local one)
        let n = self.oget(o, Fact::Var(VAR_VERB_SCRIPT));
        let unit = match n {
            Val::Int(n) if n != 0 => match g.script_key(n) {
                Fact::Script(_) => Some(UnitKey::Global(n)),
                _ => {
                    let room = self.oget(o, Fact::Room);
                    match self.resource(room) {
                        Val::Int(r) => Some(UnitKey::Local(r, n)),
                        Val::Unk(_) => None,
                    }
                }
            },
            _ => None,
        };
        let uid = unit.and_then(|u| g.id(u));
        let start = match uid {
            Some(u) if g.unit(u).ins_len > 0 => g.cite(u, 0),
            _ => format!("input script {}", self.show(n)),
        };
        let mut p = Path::new(vec![Entry::Call(call)]);
        p.settle = true;
        self.drive(o, p, start, unit)
    }

    /// checkExecVerbs' click on a verb and runInputScript (model.py `_eng_input`). The input script is the one
    /// installed at pre; where the path's store differs from pre on VAR_VERB_SCRIPT, VAR_USERPUT or the verb's
    /// `verb:` fact, the difference goes into `Path.input_drift`. Switch input_after_turns reads VAR_USERPUT and
    /// VAR_VERB_SCRIPT from the store instead.
    pub(super) fn eng_input(&mut self, p: &mut Path, ci: usize, o: &FactOracle, area: i32, val: i32, button: i32) -> Vec<Path> {
        p.owner = None;
        p.timed = false;
        let mut drift = Vec::new();
        for k in [Fact::Var(VAR_VERB_SCRIPT), Fact::Var(VAR_USERPUT), Fact::Verb(val)] {
            let now = self.read(p, k, o);
            let before = self.oget(o, k);
            if now != before {
                drift.push((k, before, now));
            }
        }
        p.input_drift = drift;
        let (_, cite) = Self::head_cite(p, ci);
        let mut more = Vec::new();
        let n = if self.switches.input_after_turns {
            // switch (off by default): checkExecVerbs' own reads, on the path's store (verbs.cpp:590, script.cpp:1482)
            let up = self.read(p, Fact::Var(VAR_USERPUT), o);
            let dropped = "_userPut <= 0 after the turns: the click is dropped (verbs.cpp:590)";
            match self.v.decide(&[up], |y| y[0] <= 0) {
                Dec::Unk(off) => {
                    let mut q = p.fork();
                    let why = rc(&self.show(off));
                    let cond = rc("_userPut <= 0: the click is dropped");
                    p.assumptions.push(Assumption {
                        cite: cite.clone(),
                        cond: cond.clone(),
                        side: Side::Bool(false),
                        why: why.clone(),
                    });
                    q.assumptions.push(Assumption { cite: cite.clone(), cond, side: Side::Bool(true), why });
                    self.note(&mut q, cite.clone(), "checkExecVerbs", dropped);
                    more.push(q);
                }
                Dec::Bool(true) => {
                    self.note(p, cite.clone(), "checkExecVerbs", dropped);
                    self.ev_input(p, area, val, button);
                    return Vec::new();
                }
                Dec::Bool(false) => {}
            }
            if self.read(p, Fact::Verb(val), o) != Val::Int(1) {
                let w = format!(
                    "verb {val} may not be shown after the turns: findVerbAtPos would not find it (not modelled)"
                );
                self.note(p, cite.clone(), "checkExecVerbs", &w);
            }
            self.read(p, Fact::Var(VAR_VERB_SCRIPT), o)
        } else {
            self.oget(o, Fact::Var(VAR_VERB_SCRIPT))
        };
        match n {
            Val::Unk(_) => {
                let w = format!("input script {}", self.show(n));
                p.unknown_writes.push((rc("runInputScript"), cite, rc(&w)));
            }
            Val::Int(0) => {
                // if (verbScript) runScript(...) (script.cpp:1509-1510)
                self.note(p, cite, "runInputScript", "VAR_VERB_SCRIPT is 0: no input script runs");
            }
            Val::Int(n) => {
                let args = vec![(0, Val::Int(area)), (1, Val::Int(val)), (2, Val::Int(button))];
                let step = Step::Script { n, resistant: Some(false), recursive: Some(false), args };
                let cid = self.cid();
                p.stack.push(Entry::Call(Call {
                    head: rc("runInputScript"),
                    cite: rc("script.cpp:1478-1511"),
                    steps: vec![step].into(),
                    caller: None,
                    active: false,
                    i: 0,
                    cid,
                }));
            }
        }
        self.ev_input(p, area, val, button);
        more
    }

    /// golden.py's input event, after `_eng_input` returned, on the path it was called with.
    fn ev_input(&mut self, p: &mut Path, area: i32, val: i32, button: i32) {
        if self.trace.is_some() {
            let kw = vec![
                ("area", J::Int(area as i64)),
                ("verb", J::Int(val as i64)),
                ("button", J::Int(button as i64)),
                ("drift", enc::drift(&self.v, &p.input_drift)),
            ];
            self.ev(p, "input", kw);
        }
    }

    /// `_merge`'s `input_drift`: the union of the paths' entries, sorted by Python's `str` of each tuple (`repr` of
    /// its items). Paths that all hold none keep none.
    pub(super) fn merged_drift(&self, group: &[Path]) -> Option<Vec<(Fact, Val, Val)>> {
        if group.iter().all(|gp| gp.input_drift.is_empty()) {
            return None;
        }
        let mut all: Vec<(Fact, Val, Val)> = Vec::new();
        for gp in group {
            for &d in &gp.input_drift {
                if !all.contains(&d) {
                    all.push(d);
                }
            }
        }
        Some(sort_drift(&self.v, all))
    }
}

/// Drift entries sorted by Python's `str` of each (key, pre, now) tuple (model.py `sorted(..., key=str)`).
fn sort_drift(v: &Vals, entries: Vec<(Fact, Val, Val)>) -> Vec<(Fact, Val, Val)> {
    let mut keyed: Vec<(String, (Fact, Val, Val))> = entries
        .into_iter()
        .map(|d| (format!("({}, {}, {})", scumm_ir::py::repr(&d.0.to_string()), v.py_repr(d.1), v.py_repr(d.2)), d))
        .collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    keyed.into_iter().map(|(_, d)| d).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The order CPython gives `sorted(entries, key=str)` for these entries.
    #[test]
    fn drift_order() {
        let mut v = Vals::default();
        let held = v.mk(Reason::Held, "var:53");
        let set = v.mk_set(Reason::Branch, "x", vec![5, 4]);
        let (up, vs, verb) = (Fact::Var(VAR_USERPUT), Fact::Var(VAR_VERB_SCRIPT), Fact::Verb(120));
        let one = Val::Int(1);
        let entries = vec![
            (up, one, held),
            (up, one, Val::Int(0)),
            (up, one, Val::Int(-1)),
            (up, one, Val::Int(12)),
            (verb, one, Val::Int(0)),
            (vs, Val::Int(4), set),
        ];
        let want = vec![entries[5], entries[2], entries[1], entries[3], entries[0], entries[4]];
        assert_eq!(sort_drift(&v, entries), want);
    }
}
