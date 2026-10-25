//! Feature esc (row 5): model.py's `run_esc` (the Esc key: a skipped cutscene), its steps `abort` (`_eng_abort`:
//! processInput's abortCutscene), `override` (`_eng_override`: the turn of the slot abortCutscene rewound) and `key`
//! (`_eng_key`: checkExecVerbs and runInputScript with the key code), and `_key_route` (`Run.key_routes`).
//!
//! Each method is the model.py method of the same name, in the same order of reads, writes, notes, forks and ends.
//! model.py's docstrings (the module's feature esc, and each method's) hold the engine rules and the ScummVM cites.

use super::settle::slot_key;
use super::*;

/// kKeyClickArea (verbs.h:38): runInputScript's first argument for a key (verbs.cpp:664; model.py `KEY_CLICK_AREA`).
pub const KEY_CLICK_AREA: i32 = 4;
/// scumm.h:161: a _mouseAndKeyboardStat below it is a key (verbs.cpp:593; model.py `MBS_MAX_KEY`).
pub const MBS_MAX_KEY: i32 = 0x200;
/// Common::KEYCODE_RETURN and KEYCODE_TAB (common/keyboard.h:54, 52; model.py `KEYCODE_RETURN`, `KEYCODE_TAB`).
pub const KEYCODE_RETURN: i32 = 13;
pub const KEYCODE_TAB: i32 = 9;
/// Common::KEYCODE_ESCAPE (common/keyboard.h:56; model.py `KEYCODE_ESCAPE`).
pub const KEYCODE_ESCAPE: i32 = 27;
/// The rule of the rewound slot's turn (model.py's literal in `_eng_override`).
const RULE_ESC: &str = "a: rewound by Esc (abortCutscene)";
const ESC_CITE: &str = "scumm.cpp:3177-3255";

impl<'g> Machine<'g> {
    /// Feature esc: the Esc key from its frame on (model.py `run_esc`): abortCutscene rewinds the slot of the
    /// override armed at pre (`eng_abort`), every slot live at pre has its turn (the rewound one from its override
    /// point, `eng_override`), checkExecVerbs takes the key code (`eng_key`), checkAndRunSentenceScript runs a
    /// sentence queued in the frame; then the rounds (settle).
    pub fn run_esc(&mut self, o: &FactOracle) -> Run {
        assert!(self.escs, "the Esc key runs only with esc");
        let g = self.game;
        self.pre = self.pre_slots(o);
        // the esc: keys of the fact map whose value is 1, sorted as strings (model.py `sorted(...)`)
        let mut armed: Vec<Fact> =
            o.keys(Prefix::EscAll).iter().copied().filter(|&k| o.get(k) == OVal::Int(1)).collect();
        sort_str(&mut armed);
        let code = self.oget(o, Fact::Var(VAR_CUTSCENEEXIT_KEY));
        let mut start = "Esc: no override armed at the top level".to_string();
        let mut unit = None;
        let mut over: Option<(Slot, i32)> = None;
        if let (Some(&Fact::Esc(name, w)), Val::Int(c)) = (armed.first(), code) {
            if c != 0 {
                over = Some((name, w));
                // model.py `self._unit_named(Path([]), name, oracle)`: the room is read from a fresh path
                let mut fresh = Path::new(Vec::new());
                unit = self.unit_named(&mut fresh, o, name);
                let uid = unit.and_then(|u| g.id(u));
                let at = match uid.and_then(|u| g.at(u, w).map(|i| (u, i))) {
                    Some((u, i)) => g.cite(u, i),
                    None => format!("[{w}]"),
                };
                start = format!("Esc: abortCutscene rewinds {name} to {at}");
            }
        }
        let mut steps = Vec::with_capacity(self.pre.len() + 4);
        steps.push(Step::Abort { armed, code });
        for &n in &self.pre {
            steps.push(match over {
                Some((name, w)) if name == n => Step::Override { name, w },
                _ => Step::Turn(n),
            });
        }
        if let Some((name, w)) = over {
            if !self.pre.contains(&name) {
                // the override's slot is not a slot live at pre (not expected)
                steps.push(Step::Override { name, w });
            }
        }
        steps.push(Step::Key(code));
        steps.push(Step::Sentences);
        let cid = self.cid();
        let call = Call {
            head: rc("Esc"),
            cite: rc(ESC_CITE),
            steps: steps.into(),
            caller: None,
            active: false,
            i: 0,
            cid,
        };
        let mut p = Path::new(vec![Entry::Call(call)]);
        p.settle = true;
        self.drive(o, p, start, unit)
    }

    /// processInput's Esc and abortCutscene (model.py `_eng_abort`); golden.py's abort event after it.
    pub(super) fn eng_abort(&mut self, p: &mut Path, ci: usize, o: &FactOracle, armed: &[Fact], code: Val) {
        self.eng_abort_(p, ci, o, armed, code);
        if self.trace.is_some() {
            let kw = vec![
                ("armed", enc::tuple(armed.iter().map(|&k| enc::fact(k)).collect())),
                ("code", enc::val(&self.v, code)),
            ];
            self.ev(p, "abort", kw);
        }
    }

    fn eng_abort_(&mut self, p: &mut Path, ci: usize, o: &FactOracle, armed: &[Fact], code: Val) {
        p.owner = None;
        p.timed = false;
        let (_, cite) = Self::head_cite(p, ci);
        let code = match code {
            Val::Unk(_) => {
                let w = format!(
                    "VAR_CUTSCENEEXIT_KEY {}: whether Esc is the cutscene exit key is not known (input.cpp:966, 1423), \
                     abortCutscene does not run here (the action's usability is unknown, actions.py)",
                    self.show(code)
                );
                self.note(p, cite, "Esc", &w);
                return;
            }
            Val::Int(c) => c,
        };
        if code == 0 {
            let w = "VAR_CUTSCENEEXIT_KEY 0: Esc is not the cutscene exit key (input.cpp:966, 1423), abortCutscene does \
                     not run";
            self.note(p, cite, "Esc", w);
            return;
        }
        if armed.is_empty() {
            let w = "no override armed at the top level: abortCutscene does nothing (script.cpp:1693-1694)";
            self.note(p, cite, "Esc", w);
            return;
        }
        if armed.len() > 1 {
            // facts.py gives the top level's override only
            let w = format!("{} esc: facts are 1; the first is taken", armed.len());
            self.note(p, cite, "Esc", &w);
        }
        let key = armed[0];
        let Fact::Esc(name, w) = key else { unreachable!("an esc: key") };
        self.clear(p, o, Prefix::At(name));
        self.write(p, Fact::At(name, w), Val::Int(1));
        self.set_freeze(p, o, name, Some((0, true)), "");
        let what = format!(
            "{name}: its cutsceneOverride count is not a fact in facts.py (the bridge's dump holds it per slot; adding \
             it is held); kept at 0 as for every slot live at pre"
        );
        self.note(p, rc("script.cpp:1703-1704"), "assumption: cutsceneOverride", &what);
        self.write(p, Fact::Var(VAR_OVERRIDE), Val::Int(1));
        self.write(p, key, Val::Int(0));
    }

    /// The turn of the slot abortCutscene rewound, from its override point W by rule a (model.py `_eng_override`).
    /// An earlier turn in the frame that moved it leaves it to settle's own rule (`eng_turn`).
    pub(super) fn eng_override(&mut self, p: &mut Path, ci: usize, o: &FactOracle, name: Slot, w: i32) -> Vec<Path> {
        let g = self.game;
        p.owner = None;
        if self.read(p, Fact::At(name, w), o) != Val::Int(1) || p.round_ran.contains(&name) {
            return self.eng_turn(p, ci, o, name);
        }
        let unit = self.unit_named(p, o, name);
        let uid = unit.and_then(|u| g.id(u));
        let idx = uid.and_then(|u| g.at(u, w));
        let key = slot_key(name);
        let live = match key {
            Some(k) => self.read(p, k, o),
            None => Val::Int(1),
        };
        let frozen = self.read(p, Fact::Frozen(name), o);
        let (Some(u), Some(idx), Val::Int(1), Val::Int(0)) = (uid, idx, live, frozen) else {
            let why = if idx.is_none() {
                "its override point is no instruction of its unit".to_string()
            } else {
                format!("its liveness {} or freeze {} changed before its turn", self.show(live), self.show(frozen))
            };
            self.skip(p, name, &why);
            return Vec::new();
        };
        let unit = unit.expect("a unit when its id is known");
        let owner = Owner::Slot(name);
        p.owner = Some(owner);
        p.owner_rule = RULE_ESC;
        p.timed = false;
        p.rw.add_owner(owner);
        p.round_ran.insert(name);
        p.ran_by.entry(name).or_insert(RULE_ESC);
        p.not_run.remove(&name);
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
        if resistant.is_none() {
            let (_, c) = Self::head_cite(p, ci);
            self.note(p, c, "resume", &format!("{name}: F flag not known"));
        }
        self.clear(p, o, Prefix::At(name)); // it runs from its override point now
        self.push_frame(p, ci, u, idx, &[], owned, None, resistant, key, false, false);
        Vec::new()
    }

    /// checkExecVerbs takes the key code the Esc left (model.py `_eng_key`): wiped by clearClickedStatus, dropped with
    /// input off, or runInputScript(kKeyClickArea, code, 1). VAR_USERPUT and VAR_VERB_SCRIPT come from the path's
    /// store (switch esc_key_at_pre: from pre).
    pub(super) fn eng_key(&mut self, p: &mut Path, ci: usize, o: &FactOracle, code: Val) -> Vec<Path> {
        p.owner = None;
        p.timed = false;
        let at_pre = self.switches.esc_key_at_pre;
        let (_, cite) = Self::head_cite(p, ci);
        let mut more = Vec::new();
        if p.cleared != Some(false) {
            // a doSentence(STOP) in the frame's turns: clearClickedStatus wiped the key
            let gone = "a doSentence(STOP) in the frame's turns ran clearClickedStatus: the key is gone before \
                        checkExecVerbs (script_v5.cpp:1004-1008, input.cpp:378-386)";
            if p.cleared.is_none() {
                // merged paths disagree: both sides
                let mut q = p.fork();
                let why = rc(&format!("?({}: whether a doSentence(STOP) ran in the frame)", Reason::Branch.as_str()));
                let cond = rc("clearClickedStatus ran in the frame");
                p.assumptions.push(Assumption {
                    cite: cite.clone(),
                    cond: cond.clone(),
                    side: Side::Bool(false),
                    why: why.clone(),
                });
                q.assumptions.push(Assumption { cite: cite.clone(), cond, side: Side::Bool(true), why });
                self.note(&mut q, cite.clone(), "Esc key", gone);
                self.key_route(&mut q, "wiped by clearClickedStatus");
                more.push(q);
            } else {
                self.note(p, cite, "Esc key", gone);
                self.key_route(p, "wiped by clearClickedStatus");
                return Vec::new();
            }
        }
        // Esc is not the exit key: processKeyboard's default branch gives the key's ASCII code
        let code = if code == Val::Int(0) { Val::Int(KEYCODE_ESCAPE) } else { code };
        let code = match code {
            Val::Unk(_) => {
                let w = format!("the key code {} is not known: checkExecVerbs not run", self.show(code));
                self.note(p, cite, "Esc key", &w);
                self.key_route(p, "key code not known");
                return more;
            }
            Val::Int(c) => c,
        };
        if code == KEYCODE_RETURN || code == KEYCODE_TAB {
            let cursor = self.oget(o, Fact::Var(VAR_CURSORSTATE));
            if !matches!(cursor, Val::Int(c) if c <= 0) {
                let w = format!(
                    "key code {code} with cursor state {}: a click (input.cpp:1566-1570), not modelled",
                    self.show(cursor)
                );
                self.note(p, cite, "Esc key", &w);
                self.key_route(p, "a click (Enter or Tab), not modelled");
                return more;
            }
        }
        if !(0..MBS_MAX_KEY).contains(&code) {
            let w = format!("key code {code} is not below MBS_MAX_KEY (verbs.cpp:593): not modelled");
            self.note(p, cite, "Esc key", &w);
            self.key_route(p, "not a key, not modelled");
            return more;
        }
        let userput = if at_pre {
            self.oget(o, Fact::Var(VAR_USERPUT))
        } else {
            self.read(p, Fact::Var(VAR_USERPUT), o)
        };
        let dropped = "input off at checkExecVerbs: the key is dropped (verbs.cpp:590)";
        match self.v.decide(&[userput], |y| y[0] <= 0) {
            Dec::Unk(off) => {
                let mut q = p.fork();
                let why = rc(&self.show(off));
                let cond = rc("_userPut <= 0: the key is dropped");
                p.assumptions.push(Assumption {
                    cite: cite.clone(),
                    cond: cond.clone(),
                    side: Side::Bool(false),
                    why: why.clone(),
                });
                q.assumptions.push(Assumption { cite: cite.clone(), cond, side: Side::Bool(true), why });
                self.note(&mut q, cite.clone(), "Esc key", dropped);
                self.key_route(&mut q, "dropped with input off");
                more.push(q);
            }
            Dec::Bool(true) => {
                self.note(p, cite, "Esc key", dropped);
                self.key_route(p, "dropped with input off");
                return more;
            }
            Dec::Bool(false) => {}
        }
        let n = if at_pre {
            self.oget(o, Fact::Var(VAR_VERB_SCRIPT))
        } else {
            self.read(p, Fact::Var(VAR_VERB_SCRIPT), o)
        };
        match n {
            Val::Unk(_) => {
                let shown = self.show(n);
                p.unknown_writes.push((rc("runInputScript"), cite.clone(), rc(&format!("input script {shown}"))));
                self.note(p, cite, "Esc key", &format!("the input script is not known ({shown}): not run"));
                self.key_route(p, "input script not known");
            }
            Val::Int(0) => {
                // if (verbScript) runScript(...) (script.cpp:1509-1510)
                self.note(p, cite, "Esc key", "VAR_VERB_SCRIPT is 0: no input script runs");
                self.key_route(p, "no input script");
            }
            Val::Int(n) => {
                let w = format!("input script {n} runs with [{KEY_CLICK_AREA}, {code}, 1]");
                self.note(p, cite, "Esc key", &w);
                self.key_route(p, &format!("input script {n}"));
                let args = vec![(0, Val::Int(KEY_CLICK_AREA)), (1, Val::Int(code)), (2, Val::Int(1))];
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
        more
    }

    /// Where the Esc key went on `p`, with the assumptions `p` holds then (model.py `_key_route`); golden.py's key
    /// event, which gives a fork its trace id before the fork event names it.
    fn key_route(&mut self, p: &mut Path, fate: &str) {
        self.key_routes.push((rc(fate), p.assumptions.iter().cloned().collect()));
        if self.trace.is_some() {
            self.ev(p, "key", vec![("fate", enc::s(fate))]);
        }
    }
}
