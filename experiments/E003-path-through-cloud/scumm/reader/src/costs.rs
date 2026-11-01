//! Frame costs (the switch `--frame-costs`, off by default): each record's cost in engine frames, a lower bound of
//! its real duration, from the engine source (third_party/scummvm/engines/scumm, the patched tree the project cites)
//! and the scripts. With the switch off every record costs 1, as build_task has always written.
//!
//! **A record's duration.** A record runs inside one frame. Game time passes before it: an input takes the frames
//! its keys and clicks land on, and an event waits for its slot's yield to end. So a record's cost is
//!
//! * an input (sentence, walk, dialog line, Esc): one frame per key or click the harness plays for it (replay.py
//!   `step_input`): a sentence is the verb's key and one click per object (2, or 3 with B), a walk one click, a
//!   dialog line one click, a skipped cutscene the Esc key. The engine takes one input a frame: processInput keeps one
//!   button or key state (input.cpp:439-459) and checkExecVerbs reads it once (scumm.cpp:3177, 3232);
//! * an event: the frames from its slot's yield to its turn, a lower bound by the yield it resumes from (below);
//! * a skipped cutscene's override event runs in the Esc's own frame (abortCutscene from processInput, before the
//!   frame's scripts): its cost is the Esc's frame.
//!
//! **The frame clock.** A frame waits VAR_TIMER_NEXT jiffies (var 19, vars.cpp:55; scumm.cpp:2821), at least 1
//! (2834-2835). Script delays and the talk delay go down by that amount, at most 15 (scumm.cpp:3129-3136). So a frame
//! takes off at most `d = min(15, max(1, VAR_TIMER_NEXT))`. VAR_TIMER_NEXT keeps its start value where no unit of the
//! scope writes it; otherwise d is taken as 15.
//!
//! **The yields** (an event resumes at its slot's wait point W; the instruction before W in the code, or at W for a
//! wait):
//!
//! * breakHere: the slot runs again in the next frame's runAllScripts (didexec is reset each frame, script.cpp:971-979,
//!   506-507): 1 frame.
//! * delay N (jiffies; o5_delay, script_v5.cpp:972-980): the slot is paused with delay N; each frame takes off d
//!   before the scripts run, and the slot runs in the first frame where the delay is below 0 (script.cpp:1513-1542,
//!   scumm.cpp:3132). That is frame `floor(N / d) + 1` after the yield: with d at most `dmax`, at least
//!   `floor(N / dmax) + 1`.
//! * delayVariable(V): the same with V's value. The event's own locals give it when the reading knows them; otherwise
//!   the last assignment to V on the straight code before the delay (no jump, entry or Esc point between them) gives
//!   its least value, by interval arithmetic over constants, + - * /, and getRandomNr(n), which returns 0..n
//!   (script_v5.cpp:1438-1442, common/random.cpp:62-67). Otherwise 1 frame.
//! * WaitForMessage after a talk: a print on text slot 0 (print to an actor other than 252-254, or printEgo;
//!   script_v5.cpp decodeParseString) on the straight code before the wait, with no yield between, starts the message
//!   in the same frame p: actorTalk sets VAR_HAVE_MSG to 0xFF and calls displayDialog (actor.cpp:3529-3540), which
//!   sets the talk delay to `T = 60 + n * VAR_CHARINC` jiffies for the part's n characters (string.cpp:1142, 1312;
//!   v5 has no VAR_DEFAULT_TALK_DELAY, scumm.h:1918, so 60). Each frame takes d off (scumm.cpp:3134-3136). When it
//!   reaches 0, the frame's displayDialog prints the next part (a wait code, string.cpp:280-284) or, at the end of
//!   the text, stops the talk (string.cpp:1120-1131, actor.cpp:3569); VAR_HAVE_MSG is copied at the next frame's
//!   start (scumm.cpp:3198, 3382) and the wait passes in that frame (script_v5.cpp:3288-3291). So the event comes
//!   `sum over parts of max(1, ceil(T_j / dmax)) + 1` frames after p. A keepText code ends the message where it
//!   stands (string.cpp:275-279). Characters inserted from variables are counted as 0, a lower bound. VAR_CHARINC
//!   (var 37, vars.cpp:72) keeps its start value where no unit of the scope writes it; otherwise each part counts 1
//!   frame. The bound assumes no other script prints on text slot 0 while the slot waits (a new talk replaces the
//!   message, actor.cpp:3529-3540), and no talk-stop key ('.', input.cpp:1418-1421), which the model's inputs lack.
//! * any other wait (an actor's walk, the camera, the sentence queue) or a yield the code does not show: the slot ran
//!   in an earlier frame, so 1 frame. A walk's length is not known to the model (actor positions are not facts).
//!
//! Every cost is at least 1. It is a lower bound of the record's own duration. A plan's sum is not a lower bound of
//! the plan's game time where two slots wait at once, or where a slot waiting at the plan's start has already spent
//! part of its wait (replay.py's cost caps lower such an event's cost at a replan to what the engine's dump shows).

use crate::py::{Counter, Py};
use crate::record::{Event, Record, Val, What};
use crate::scope::Scope;
use rustc_hash::FxHashMap;
use scumm_ir::ast::{ArithOp, AssignOp, Expr, ExprId, Stmt};
use scumm_ir::{known, Game, UnitId, UnitKey};

/// VAR_TIMER_NEXT in SCUMM v3-v6 (vars.cpp:55).
pub const VAR_TIMER_NEXT: i32 = 19;
/// VAR_CHARINC in SCUMM v3-v6 (vars.cpp:72).
pub const VAR_CHARINC: i32 = 37;
/// The talk delay a message part starts with where the game has no VAR_DEFAULT_TALK_DELAY (string.cpp:1142).
pub const TALK_DELAY: i64 = 60;
/// The most jiffies a frame takes off a delay (scumm.cpp:3129-3130).
pub const DELTA_CAP: i64 = 15;

/// What an event resuming at a wait point waited for.
#[derive(Clone, Debug, PartialEq)]
enum Wait {
    /// a fixed number of frames, and the rule that gave it
    Frames(u32, &'static str),
    /// delayVariable: the operand (a local's index, if a local) and its least value from the code, if known
    DelayVar { local: Option<i64>, least: Option<i64> },
}

/// The frame cost of every record (module docstring).
#[derive(Clone, Debug)]
pub struct FrameCosts {
    /// the most jiffies a frame takes off a delay in this scope
    pub dmax: i64,
    /// VAR_CHARINC in this scope, where no unit writes it
    pub charinc: Option<i64>,
    /// (unit, wait point) -> what an event resuming there waited for
    waits: FxHashMap<(UnitKey, i64), Wait>,
    /// the wait points by rule (for the report)
    pub rules: Counter,
}

/// The least frame `floor(n / dmax) + 1` a delay of `n` jiffies can end on.
pub fn delay_frames(n: i64, dmax: i64) -> u32 {
    (n.max(0) / dmax + 1).min(u32::MAX as i64) as u32
}

impl FrameCosts {
    /// The costs for a scope: its start's VAR_TIMER_NEXT and VAR_CHARINC where no unit of the scope writes them, and
    /// the wait points of every unit of the game.
    pub fn of(game: &Game, scope: &Scope) -> FrameCosts {
        let tn = format!("var:{VAR_TIMER_NEXT}");
        let ci = format!("var:{VAR_CHARINC}");
        let dmax = if scope.writes.has(&tn) { DELTA_CAP } else { scope.start.get0(&tn).clamp(1, DELTA_CAP) };
        let charinc = (!scope.writes.has(&ci)).then(|| scope.start.get0(&ci));
        let mut fc = FrameCosts { dmax, charinc, waits: FxHashMap::default(), rules: Counter::default() };
        for u in game.ids() {
            fc.add_unit(game, u);
        }
        fc
    }

    fn put(&mut self, key: (UnitKey, i64), w: Wait) {
        // two yields with the same wait point (a breakHere before a wait): the lesser bound holds for both
        let new = match self.waits.get(&key) {
            None => w,
            Some(old) if *old == w => return,
            Some(Wait::Frames(a, _)) => match w {
                Wait::Frames(b, _) if *a <= b => return,
                Wait::Frames(..) => w,
                _ => Wait::Frames(1, "two yields with one wait point"),
            },
            Some(_) => Wait::Frames(1, "two yields with one wait point"),
        };
        self.waits.insert(key, new);
    }

    fn add_unit(&mut self, game: &Game, u: UnitId) {
        let unit = game.unit(u);
        let ins = game.ins(u);
        let key = unit.key;
        for (i, x) in ins.iter().enumerate() {
            let Some(Stmt::Op { name, args, subops: _, flags: _ }) = x.st else { continue };
            let resume_next = || x.next.filter(|&j| (j as usize) < ins.len()).map(|j| ins[j as usize].offset as i64);
            match name {
                known::BREAK_HERE => {
                    if let Some(w) = resume_next() {
                        self.rules.inc("breakHere: 1 frame");
                        self.put((key, w), Wait::Frames(1, "breakHere"));
                    }
                }
                known::DELAY => {
                    if let Some(w) = resume_next() {
                        let n = game.ast.kids(args).first().and_then(|&e| game.ast.constant(e));
                        match n {
                            Some(n) => {
                                self.rules.inc("delay N: floor(N / dmax) + 1 frames");
                                self.put((key, w), Wait::Frames(delay_frames(n as i64, self.dmax), "delay"));
                            }
                            None => {
                                self.rules.inc("delay, operand not a constant: 1 frame");
                                self.put((key, w), Wait::Frames(1, "delay"));
                            }
                        }
                    }
                }
                known::DELAY_VARIABLE => {
                    if let Some(w) = resume_next() {
                        let op = game.ast.kids(args).first().copied();
                        let local = op.and_then(|e| match game.ast.expr(e) {
                            Expr::Local(n) => Some(n as i64),
                            _ => None,
                        });
                        let least = op.and_then(|e| least_before(game, u, i as u32, e));
                        self.rules.inc(if least.is_some() {
                            "delayVariable, least value from the code before it"
                        } else {
                            "delayVariable, value from the event's locals or 1 frame"
                        });
                        self.put((key, w), Wait::DelayVar { local, least });
                    }
                }
                known::WAIT_FOR_MESSAGE => {
                    let w = x.offset as i64;
                    match self.message_before(game, u, i as u32) {
                        Some(n) => {
                            self.rules.inc("WaitForMessage after a talk: the talk delay's frames + 1");
                            self.put((key, w), Wait::Frames(n, "message"));
                        }
                        None => {
                            self.rules.inc("WaitForMessage, message not known: 1 frame");
                            self.put((key, w), Wait::Frames(1, "message"));
                        }
                    }
                }
                known::WAIT_FOR_ACTOR | known::WAIT_FOR_CAMERA | known::WAIT_FOR_SENTENCE => {
                    self.rules.inc("WaitForActor, WaitForCamera, WaitForSentence: 1 frame");
                    self.put((key, x.offset as i64), Wait::Frames(1, "wait"));
                }
                _ => {}
            }
        }
    }

    /// The frames from a talk on the straight code before the WaitForMessage at `i` to the wait's end (module
    /// docstring), or None.
    fn message_before(&self, game: &Game, u: UnitId, i: u32) -> Option<u32> {
        for j in straight_before(game, u, i) {
            let x = &game.ins(u)[j as usize];
            let Some(st) = x.st else { return None };
            match st {
                Stmt::Op { name, args, subops, .. } if name == known::PRINT || name == known::PRINT_EGO => {
                    let text = game.ast.subs(subops).iter().find(|s| s.name == known::TEXT);
                    if name == known::PRINT {
                        let actor = game.ast.kids(args).first().and_then(|&e| game.ast.constant(e));
                        match actor {
                            // text slots 1-3: no talk (decodeParseString); the message is an older one
                            Some(252..=254) => continue,
                            Some(_) => {}
                            None => return None,
                        }
                    }
                    let Some(text) = text else { continue }; // no text: no talk
                    let parts = text_parts(game, game.ast.kids(text.args).first().copied()?)?;
                    return Some(self.message_frames(&parts));
                }
                Stmt::Op { name, .. } if is_yield(name) => return None,
                _ => {}
            }
        }
        None
    }

    /// `sum over parts of max(1, ceil(T_j / dmax)) + 1`, T_j = 60 + n_j * VAR_CHARINC; a part that ends in keepText
    /// ends the message (no talk delay after it is waited for).
    fn message_frames(&self, parts: &[(i64, bool)]) -> u32 {
        let mut frames: i64 = 0;
        for &(n, keep) in parts {
            if keep {
                break;
            }
            frames += match self.charinc {
                Some(c) => {
                    let t = TALK_DELAY + n * c;
                    ((t + self.dmax - 1).div_euclid(self.dmax)).max(1)
                }
                None => 1,
            };
        }
        (frames + 1).min(u32::MAX as i64) as u32
    }

    /// An event's cost: the frames its slot waited, from the yield its wait point follows.
    pub fn event_cost(&self, e: &Event) -> u32 {
        match self.waits.get(&(e.unit, e.w)) {
            Some(Wait::Frames(n, _)) => *n,
            Some(Wait::DelayVar { local, least }) => {
                let known = match local.map(|l| e.locals.iter().find(|(k, _)| *k == l)) {
                    // the event's own value of the local: an int, or an unknown's least value if it has values
                    Some(Some((_, Val::Int(n)))) => Some(*n),
                    Some(Some((_, Val::Unk(u)))) => u.values.as_ref().and_then(|vs| vs.first().copied()),
                    // a local the list does not hold reads 0 where the args are known (Builder::list_number)
                    Some(None) if e.args_known => Some(0),
                    _ => None,
                };
                // the event's value and the code's least value both bound the same delay from below: the greater
                let known = match (known, *least) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (a, b) => a.or(b),
                };
                match known {
                    Some(n) => delay_frames(n, self.dmax),
                    None => 1,
                }
            }
            None => 1,
        }
    }

    /// A record's cost in frames (module docstring).
    pub fn cost(&self, r: &Record) -> u32 {
        match (&r.source, r.kind.as_str()) {
            (What::Sentence { .. }, "walk_to") => 1,
            (What::Sentence { b, .. }, _) => 2 + matches!(b, Some(b) if *b != 0) as u32,
            (What::Verb(_), _) => 1,
            (What::Event(e), _) if e.abort => 1,
            (What::Event(e), _) => self.event_cost(e),
        }
    }

    /// The parameters, for the reading's head (`frame_costs`) and reading.json.
    pub fn py(&self) -> Py {
        Py::dict(vec![
            ("switch", Py::str("--frame-costs")),
            ("dmax", Py::Int(self.dmax)),
            ("charinc", self.charinc.map_or(Py::None, Py::Int)),
            ("talk_delay", Py::Int(TALK_DELAY)),
            ("wait_points", Py::Int(self.waits.len() as i64)),
            ("rules", self.rules.py()),
        ])
    }

    /// The cost of a wait point as the module docstring gives it with no locals known (tests and reports).
    pub fn wait_point(&self, unit: UnitKey, w: i64) -> Option<String> {
        self.waits.get(&(unit, w)).map(|x| match x {
            Wait::Frames(n, why) => format!("{why}: {n}"),
            Wait::DelayVar { local, least } => format!("delayVariable(local {local:?}): least {least:?}"),
        })
    }
}

fn is_yield(name: scumm_ir::Sym) -> bool {
    matches!(
        name,
        known::BREAK_HERE
            | known::DELAY
            | known::DELAY_VARIABLE
            | known::WAIT_FOR_ACTOR
            | known::WAIT_FOR_MESSAGE
            | known::WAIT_FOR_CAMERA
            | known::WAIT_FOR_SENTENCE
    )
}

/// The instructions before `i` that every path to `i` passes, nearest first: back while the next one is reached only
/// by falling through (no jump goes to it, it is no entry of an object's verb and no Esc point).
fn straight_before(game: &Game, u: UnitId, i: u32) -> Vec<u32> {
    let ins = game.ins(u);
    let unit = game.unit(u);
    let entry = |k: u32| unit.entries.iter().any(|e| e.1 == k) || ins.iter().any(|x| x.esc == Some(k));
    let mut out = Vec::new();
    let mut k = i;
    while k > 0 {
        if game.joins[game.point(u, k)] || entry(k) {
            break;
        }
        let j = k - 1;
        if ins[j as usize].next != Some(k) {
            break;
        }
        out.push(j);
        k = j;
    }
    out
}

/// The least value the operand `e` of the instruction at `i` can hold: a constant, or the last assignment to it on
/// the straight code before (a local through yields too: no other script writes it; a variable up to a yield).
fn least_before(game: &Game, u: UnitId, i: u32, e: ExprId) -> Option<i64> {
    let ast = &game.ast;
    if let Some(c) = ast.constant(e) {
        return Some(c as i64);
    }
    let target = ast.expr(e);
    if !matches!(target, Expr::Local(_) | Expr::Var(_)) {
        return None;
    }
    for j in straight_before(game, u, i) {
        let x = &game.ins(u)[j as usize];
        let st = x.st?;
        if let Stmt::Assign(a) = st {
            if ast.expr(a.lhs) == target {
                if a.op != AssignOp::Set {
                    return None;
                }
                return interval(game, a.rhs).map(|(lo, _)| lo);
            }
        }
        // any other write of it (an opcode run inside an expression): not followed
        let mut other = false;
        ast.walk_stmt(&st, &mut |x| {
            if let Expr::Embed(a) = ast.expr(x) {
                if ast.expr(a.lhs) == target {
                    other = true;
                    return false;
                }
            }
            true
        });
        if other {
            return None;
        }
        if matches!(target, Expr::Var(_)) {
            if let Stmt::Op { name, .. } = st {
                if is_yield(name) {
                    return None;
                }
            }
            if matches!(st, Stmt::Start { .. }) {
                return None; // a started script runs at once and may write the variable
            }
        }
    }
    None
}

/// The interval of an expression's values: constants, + - * /, an opcode's value inside an expression, and
/// getRandomNr(n) in 0..n.
fn interval(game: &Game, e: ExprId) -> Option<(i64, i64)> {
    let ast = &game.ast;
    match ast.expr(e) {
        Expr::Const(c) => Some((c as i64, c as i64)),
        Expr::Neg(x) => interval(game, x).map(|(a, b)| (-b, -a)),
        Expr::Embed(a) if a.op == AssignOp::Set => interval(game, a.rhs),
        Expr::Getter { name, args } if name == known::GET_RANDOM_NR => {
            let n = ast.kids(args).first().and_then(|&x| ast.constant(x))?;
            (n >= 0).then_some((0, n as i64))
        }
        Expr::BinOp { op, a, b } => {
            let (a0, a1) = interval(game, a)?;
            let (b0, b1) = interval(game, b)?;
            match op {
                ArithOp::Add => Some((a0 + b0, a1 + b1)),
                ArithOp::Sub => Some((a0 - b1, a1 - b0)),
                ArithOp::Mul => {
                    let ps = [a0 * b0, a0 * b1, a1 * b0, a1 * b1];
                    Some((*ps.iter().min().unwrap(), *ps.iter().max().unwrap()))
                }
                // C's division truncates; with a positive constant divisor it keeps the order
                ArithOp::Div if b0 == b1 && b0 > 0 => Some((a0 / b0, a1 / b0)),
                ArithOp::Div => None,
            }
        }
        _ => None,
    }
}

/// A Text sub-op's parts, split at its wait codes: (characters, ends in keepText). A string's characters as descumm
/// printed them (an escape `\xNN` is one); a code that inserts a value counts 0; newline adds none (string.cpp:270-274
/// gives it no talk delay).
fn text_parts(game: &Game, e: ExprId) -> Option<Vec<(i64, bool)>> {
    let ast = &game.ast;
    let items: Vec<ExprId> = match ast.expr(e) {
        Expr::Concat(s) => ast.kids(s).to_vec(),
        Expr::Str(_) => vec![e],
        _ => return None,
    };
    let mut parts = vec![(0i64, false)];
    for x in items {
        match ast.expr(x) {
            Expr::Str(s) => parts.last_mut().unwrap().0 += chars(game.syms.str(s)),
            Expr::Getter { name, .. } if name == known::WAIT => parts.push((0, false)),
            Expr::Getter { name, .. } if name == known::KEEP_TEXT => {
                parts.last_mut().unwrap().1 = true;
                break;
            }
            Expr::Getter { .. } => {}
            _ => return None,
        }
    }
    Some(parts)
}

/// Characters of a string as descumm printed it: `\xNN` is one, any other backslash pair one.
fn chars(s: &str) -> i64 {
    let b = s.as_bytes();
    let (mut i, mut n) = (0, 0);
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            i += if b[i + 1] == b'x' && i + 3 < b.len() { 4 } else { 2 };
        } else {
            i += 1;
        }
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_rule() {
        assert_eq!(delay_frames(0, 5), 1);
        assert_eq!(delay_frames(4, 5), 1);
        assert_eq!(delay_frames(5, 5), 2);
        assert_eq!(delay_frames(30, 5), 7);
        assert_eq!(delay_frames(1800, 5), 361);
    }

    #[test]
    fn string_chars() {
        assert_eq!(chars("Hey!"), 4);
        assert_eq!(chars("M\\x88l\\x82e"), 5);
        assert_eq!(chars(""), 0);
    }
}
