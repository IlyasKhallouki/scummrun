//! The `--live-locals` switch (PORT-NOTES.md, "Live locals"): which of a slot's locals its unit's code can read.
//!
//! A SCUMM v5 slot's locals are read only by its own code: the engine's readVar reads `vm.localvar[_currentScript]`
//! for a variable number with bit 0x4000 (script.cpp:568-706), and every such number comes from the running
//! instruction's operands, directly or as the index of an indexed variable (script.cpp:573-580, script_v5.cpp:383-396),
//! or from a code inside a text it prints (string.cpp:1573-1706, [`text_code_exact`]). So a local no instruction of the
//! unit reads cannot change any of its paths. [`unit_reads`] gives the locals a unit's instructions read, from the IR
//! over all its code (every verb entry of an object, every path), or "all" when the IR does not show them exactly.

use crate::py::Py;
use crate::record::Val;
use crate::scope::Scope;
use rustc_hash::FxHashMap;
use scumm_ir::{AssignOp, AssignSt, Ast, Expr, ExprId, Game, Interner, RefKind, Stmt, Sym, UnitId, UnitKey};
use std::collections::{BTreeMap, BTreeSet};

/// Why a unit keeps every local: the IR does not show its reads exactly.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum All {
    /// a local read through an index (`Local[base + x]`, or `Local[x] += ...` on one): its number is not fixed
    Indexed,
    /// an instruction the IR did not parse
    Unparsed,
    /// a code descumm did not decode (`unknownN(..)`, `...Unknown...`) with an operand that is not a constant
    Undecoded,
    /// a code inside a text whose operand bytes the engine may read as text that starts an escape ([`text_code_exact`])
    TextBytes,
    /// an IR note that the engine may run bytes no instruction shows: any note but [`EXACT_NOTES`] ("closer gap",
    /// "jump to the end", "target not an instruction offset", "verb entry not an instruction offset", ...)
    Gap,
}

impl All {
    pub fn as_str(self) -> &'static str {
        match self {
            All::Indexed => "a local read through an index",
            All::Unparsed => "an instruction the IR did not parse",
            All::Undecoded => "a code descumm did not decode, with an operand that is not a constant",
            All::TextBytes => "a text code whose operand bytes the engine may read as text",
            All::Gap => "an IR note that the engine may run bytes no instruction shows",
        }
    }
}

/// The locals a unit's code reads (their numbers), or why it keeps all of them.
pub type Reads = Result<BTreeSet<i32>, All>;

/// The IR notes (ir/src/read.rs) that leave every byte the engine runs in an instruction the IR shows: a jump target
/// read from the bytes (it is an instruction's start, or the note "target not an instruction offset" follows), the
/// hidden jumps of an else chain (their bytes a multiple of 3, or "else gap not a multiple of 3" follows), a global
/// variable's name read from the bytes or left as printed (no local), and a unit with no instruction (no slot of it
/// ever waits, so no list of it is keyed). Any other note keeps all locals ([`All::Gap`]): a gap ("closer gap", "else
/// gap not a multiple of 3", "jump line not 3 bytes", "while without a closer at its end", "two instructions at one
/// offset"), a start the engine can take inside or past the shown code ("jump to the end", "target not an instruction
/// offset", "verb entry not an instruction offset", "else-if target from the text rule, unverified", "beginOverride not
/// followed by a jump": the engine skips the next 3 bytes as that jump, script.cpp:1711-1722), and any note
/// added later. "unparsed" is the instruction's own ([`All::Unparsed`]).
pub const EXACT_NOTES: [&str; 6] = [
    "else-if target from bytes, text rule agrees",
    "else-if target from bytes, text rule differs",
    "else chain",
    "ambiguous name from bytes",
    "ambiguous name, unverified",
    "empty unit",
];

/// A name descumm gives a code it could not decode (dump_scripts.py's SOFT_WARN_RE: `unknownN(`,
/// UnknownCursorCommand, SetBoxUnknown, UnknownWait, StringFuncUnknown).
fn undecoded(syms: &Interner, name: Sym) -> bool {
    let s = syms.str(name);
    s.strip_prefix("unknown").is_some_and(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit())) || s.contains("Unknown")
}

/// descumm's names for the codes inside a text (descumm-common.cpp get_string, 0xFF or 0xFE then the code): newline(),
/// keepText(), wait(), getInt(v), getVerb(v), getName(v), getString(v), startAnim(k), sound(..), setColor(k),
/// unknown13(k), setFont(k), and unknownN(k) for any other code N.
fn text_code(syms: &Interner, name: Sym) -> bool {
    const CODES: [&str; 11] = ["newline", "keepText", "wait", "getInt", "getVerb", "getName", "getString", "startAnim", "sound", "setColor", "setFont"];
    let s = syms.str(name);
    CODES.contains(&s) || s.strip_prefix("unknown").is_some_and(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit()))
}

/// Whether a text code reads in the engine as descumm printed it, so the text's local reads are the IR's.
///
/// The engine converts a text in convertMessageToString (string.cpp:1573-1706): a byte 0xFF, then a code; codes 1, 2,
/// 3 and 8 take no operand, 4 to 7, 9, 10 and 12 to 14 a 2-byte word, and any other code none (string.cpp:1690-1693:
/// `num -= 2`, then `+= 2`); a byte 0xFE is plain text there. descumm takes a 2-byte word after 8, 11, 0 and 15 and up
/// (its default branch, `unknownN(k)`), and reads 0xFE as 0xFF. So the engine reads some bytes descumm took as an
/// operand as plain text, and a plain text byte 0xFF there starts an escape the IR does not show: bytes FF 04 then
/// 0x4003 read Local[3]. Exact when no operand byte is 0xFF (each operand a constant or a plain variable: Var[n] is
/// the word n, Local[n] 0x4000 + n, Bit[n] 0x8000 + n), the code is not 0xFF (`unknown255`, the 0xFE form), the operand
/// has no index (descumm reads the index word from the text, the engine's readVar fetches it from the script,
/// script.cpp:573-580), and the code is not `sound` (descumm skips bytes it does not show).
fn text_code_exact(ast: &Ast, syms: &Interner, name: Sym, args: &[ExprId]) -> bool {
    let s = syms.str(name);
    if s == "sound" || s == "unknown255" {
        return false;
    }
    args.iter().all(|&k| {
        let w = match ast.expr(k) {
            Expr::Const(c) => c as u32 & 0xFFFF,
            Expr::Var(n) => n as u32,
            Expr::Local(n) => 0x4000 | n as u32,
            Expr::Bit(n) => 0x8000 | n as u32,
            _ => return false,
        };
        w & 0xFF != 0xFF && (w >> 8) & 0xFF != 0xFF
    })
}

struct Walk<'a> {
    ast: &'a Ast,
    syms: &'a Interner,
    out: BTreeSet<i32>,
}

impl Walk<'_> {
    fn args_const(&self, args: &[ExprId]) -> bool {
        args.iter().all(|&k| matches!(self.ast.expr(k), Expr::Const(_)))
    }

    /// Every local an expression reads.
    fn expr(&mut self, e: ExprId) -> Result<(), All> {
        let ast = self.ast;
        match ast.expr(e) {
            Expr::Local(n) => {
                self.out.insert(n);
            }
            Expr::Index { kind, offset, .. } => {
                if kind == RefKind::Local {
                    return Err(All::Indexed);
                }
                self.expr(offset)?;
            }
            Expr::Const(_) | Expr::Var(_) | Expr::Bit(_) | Expr::Str(_) | Expr::Flag(_) => {}
            Expr::BinOp { a, b, .. } | Expr::Cmp { a, b, .. } => {
                self.expr(a)?;
                self.expr(b)?;
            }
            Expr::Neg(x) | Expr::Not(x) | Expr::Truth(x) => self.expr(x)?,
            Expr::Getter { name, args } => {
                if text_code(self.syms, name) {
                    if !text_code_exact(ast, self.syms, name, ast.kids(args)) {
                        return Err(All::TextBytes);
                    }
                } else if undecoded(self.syms, name) && !self.args_const(ast.kids(args)) {
                    return Err(All::Undecoded);
                }
                for &k in ast.kids(args) {
                    self.expr(k)?;
                }
            }
            Expr::Concat(args) | Expr::List(args) => {
                for &k in ast.kids(args) {
                    self.expr(k)?;
                }
            }
            // an opcode inside an expression: the machine reads its target back as the value (model.py)
            Expr::Embed(a) => self.assign(a, true)?,
        }
        Ok(())
    }

    /// An assignment: its target is read when the op reads it (`+=`, `++`, ...) or its value is read back (an
    /// Embed); an indexed target's index is read either way; the value is read.
    fn assign(&mut self, a: AssignSt, read_back: bool) -> Result<(), All> {
        let reads_target = read_back || a.op != AssignOp::Set;
        match self.ast.expr(a.lhs) {
            Expr::Local(n) => {
                if reads_target {
                    self.out.insert(n);
                }
            }
            Expr::Index { kind: RefKind::Local, offset, .. } => {
                self.expr(offset)?;
                if reads_target {
                    return Err(All::Indexed);
                }
            }
            _ => self.expr(a.lhs)?,
        }
        self.expr(a.rhs)
    }

    fn stmt(&mut self, st: &Stmt) -> Result<(), All> {
        let ast = self.ast;
        match *st {
            Stmt::Assign(a) => self.assign(a, false)?,
            Stmt::Jump { cond, .. } => {
                if let Some(c) = cond {
                    self.expr(c)?;
                }
            }
            Stmt::Start { script, obj, args, .. } => {
                self.expr(script)?;
                if let Some(o) = obj {
                    self.expr(o)?;
                }
                for &k in ast.kids(args) {
                    self.expr(k)?;
                }
            }
            Stmt::Op { name, args, subops, .. } => {
                if undecoded(self.syms, name) && !self.args_const(ast.kids(args)) {
                    return Err(All::Undecoded);
                }
                for &k in ast.kids(args) {
                    self.expr(k)?;
                }
                for s in ast.subs(subops) {
                    if undecoded(self.syms, s.name) && !self.args_const(ast.kids(s.args)) {
                        return Err(All::Undecoded);
                    }
                    for &k in ast.kids(s.args) {
                        self.expr(k)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// The locals unit `u`'s code reads: every `Local[n]` in every instruction of the unit (all of an object's verb
/// entries), but the target of a plain `Local[n] = ...`; "all" when the IR does not show the reads exactly ([`All`]).
pub fn unit_reads(game: &Game, u: UnitId) -> Reads {
    let unit = game.unit(u);
    if unit.notes.iter().any(|n| n.class != "unparsed" && !EXACT_NOTES.contains(&n.class)) {
        return Err(All::Gap);
    }
    let mut w = Walk { ast: &game.ast, syms: &game.syms, out: BTreeSet::new() };
    for x in game.ins(u) {
        match &x.st {
            Some(st) => w.stmt(st)?,
            // a recovered jump always has its statement; a line with none did not parse
            None => return Err(All::Unparsed),
        }
    }
    Ok(w.out)
}

/// A kept slot's recursive flag as the switch keys it: true, or None for anything else. The model reads the flag only
/// as "is it true" (a start that is not recursive stops the running instance, calls.rs and machine.rs `started`; the
/// note at the slot's end, calls.rs), and so does the engine (chainScript passes the slot's flag to runScript, which
/// tests it, script.cpp:65-66, script_v5.cpp:834-850), so false and unknown behave alike.
pub fn recursive(flag: &Py) -> Py {
    match flag {
        Py::Bool(true) => Py::Bool(true),
        _ => Py::None,
    }
}

/// Every unit's reads ([`unit_reads`]): the switch's table (`FactTable::live_locals`).
#[derive(Debug)]
pub struct LiveLocals {
    reads: FxHashMap<UnitKey, Reads>,
}

impl LiveLocals {
    /// The reads of every unit of the game.
    pub fn of(game: &Game) -> LiveLocals {
        LiveLocals { reads: game.ids().map(|u| (game.unit(u).key, unit_reads(game, u))).collect() }
    }

    /// The locals unit `u` reads; None: all of them (or a unit the game lacks).
    pub fn get(&self, u: UnitKey) -> Option<&BTreeSet<i32>> {
        self.reads.get(&u).and_then(|r| r.as_ref().ok())
    }

    /// A list of locals as the switch keys it: the locals unit `u` reads, the others left out (they read 0 with known
    /// args, unknown without, and no instruction reads them). True when one was left out.
    pub fn keep(&self, u: UnitKey, locals: Vec<(i64, Val)>) -> (Vec<(i64, Val)>, bool) {
        let Some(reads) = self.get(u) else { return (locals, false) };
        let n = locals.len();
        let kept: Vec<(i64, Val)> = locals.into_iter().filter(|(i, _)| i32::try_from(*i).is_ok_and(|i| reads.contains(&i))).collect();
        let dropped = kept.len() != n;
        (kept, dropped)
    }

    /// Every unit of the game by the name a slot of theirs has in the engine's state: `script:N`, `obj:N`, and for a
    /// room's code `local:N@R`, `entry@R`, `exit@R` with R its room and each pseudo room on its resource; each with the
    /// locals it reads, or None (all). Every unit, not only the scope's: a scope's records can name events of a room's
    /// code outside the scope (a script started by number from a variable, PORT-NOTES.md "ELSEWHERE"). The reading's
    /// head carries it (golden.py's file, then reading.json), so the harness can tell which locals a list was keyed on.
    pub fn slots(&self, game: &Game) -> BTreeMap<String, Option<Vec<i64>>> {
        let mut out = BTreeMap::new();
        let val = |k: UnitKey| self.get(k).map(|s| s.iter().map(|&i| i as i64).collect::<Vec<i64>>());
        for u in game.ids() {
            let key = game.unit(u).key;
            let name = Game::unit_name(key).to_string();
            match key {
                UnitKey::Global(_) | UnitKey::Obj(_) => {
                    out.insert(name, val(key));
                }
                UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => {
                    out.insert(format!("{name}@{r}"), val(key));
                    for (&p, _) in game.pseudo.iter().filter(|&(_, &res)| res == r) {
                        out.insert(format!("{name}@{p}"), val(key));
                    }
                }
            }
        }
        out
    }

    /// [`LiveLocals::slots`] as golden.py's encoding: a dict.
    pub fn slots_py(&self, game: &Game) -> Py {
        Py::Dict(
            self.slots(game)
                .into_iter()
                .map(|(k, v)| (k, v.map_or(Py::None, |s| Py::List(s.into_iter().map(Py::Int).collect()))))
                .collect(),
        )
    }

    /// The scope's units by what the switch knows of them: reads known exactly (none, some), or all locals and why.
    pub fn counts(&self, game: &Game, scope: &Scope) -> BTreeMap<String, usize> {
        let mut out = BTreeMap::new();
        for &u in &scope.units {
            let k = match self.reads.get(&game.unit(u).key) {
                Some(Ok(s)) if s.is_empty() => "reads no local".to_string(),
                Some(Ok(_)) => "reads some locals, known".to_string(),
                Some(Err(why)) => format!("all locals: {}", why.as_str()),
                None => "all locals: not in the game".to_string(),
            };
            *out.entry(k).or_default() += 1;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{Unknown, R_LOCAL};
    use std::sync::OnceLock;

    fn game() -> &'static Game {
        static G: OnceLock<Game> = OnceLock::new();
        G.get_or_init(crate::oracle::load_game)
    }

    fn reads(k: UnitKey) -> Reads {
        unit_reads(game(), game().id(k).expect("a unit"))
    }

    fn set(v: &[i32]) -> Reads {
        Ok(v.iter().copied().collect())
    }

    /// MI1's units: the forms the IR gives a local in.
    #[test]
    fn reads_of_units() {
        // room 31's local 202 (the jail dialog): it reads only Local[0], its saved VAR_VERB_SCRIPT, not the
        // Local[1] and Local[2] script 2 starts it with
        assert_eq!(reads(UnitKey::Local(31, 202)), set(&[0]));
        // `Local[1 + 0]` and `Local[1 + 1]`: a constant index folds to Local[1] and Local[2]
        assert_eq!(reads(UnitKey::Global(64)), set(&[0, 1, 2]));
        assert_eq!(reads(UnitKey::Local(49, 200)), set(&[2]));
        // `Exprmode Local[0] = ...` and `Local[0] = 10` only write Local[0]
        assert!(!reads(UnitKey::Local(48, 203)).unwrap().contains(&0));
        // `Local[0 + Var[300]]`, `Local[6 + Local[5]]`: a local whose number the IR does not fix
        assert_eq!(reads(UnitKey::Global(153)), Err(All::Indexed));
        assert_eq!(reads(UnitKey::Local(75, 200)), Err(All::Indexed));
        // the six decompiles with descumm warnings: `unknown8(8224)` inside a text, bytes 20 20 (no 0xFF), which the
        // engine reads as plain text after code 8
        for k in [
            UnitKey::Local(38, 202),
            UnitKey::Local(19, 204),
            UnitKey::Local(29, 210),
            UnitKey::Local(35, 216),
            UnitKey::Global(138),
            UnitKey::Local(51, 207),
        ] {
            assert!(reads(k).is_ok(), "{k:?}");
        }
    }

    /// Over the whole game: 2 units keep all locals (an indexed local), 1,193 have their reads known.
    #[test]
    fn whole_game() {
        let ll = LiveLocals::of(game());
        let all: Vec<UnitKey> = game().ids().map(|u| game().unit(u).key).filter(|&k| ll.get(k).is_none()).collect();
        assert_eq!(all, vec![UnitKey::Global(153), UnitKey::Local(75, 200)]);
        assert_eq!(game().ids().count(), 1195);
        // the reading's head: every unit by its slot's name in the engine's state
        let slots = ll.slots(game());
        assert_eq!(slots["local:202@31"], Some(vec![0]));
        assert_eq!(slots["script:2"], Some(vec![0, 1, 2, 3, 4, 6, 7]));
        assert_eq!(slots["script:153"], None);
        assert_eq!(slots["obj:486"], Some(vec![]));
        assert!(slots.contains_key("entry@33") && slots.contains_key("exit@33"));
    }

    /// Room 31's local 202 (reads {0}) with one IR note added: a note of [`EXACT_NOTES`] leaves its reads; any other,
    /// the three that say the engine can start decoding where no shown instruction starts among them, keeps all.
    #[test]
    fn notes_keep_all_but_exact_ones() {
        let k = UnitKey::Local(31, 202);
        let with_note = |class: &'static str| {
            let mut g = game().clone();
            let u = g.id(k).unwrap();
            g.script.units[u.0 as usize].notes.push(scumm_ir::Note { class, line: 1, detail: String::new() });
            unit_reads(&g, u)
        };
        for class in EXACT_NOTES {
            assert_eq!(with_note(class), set(&[0]), "{class}");
        }
        for class in [
            "jump to the end",
            "target not an instruction offset",
            "verb entry not an instruction offset",
            "closer gap",
            "else-if target from the text rule, unverified",
            "beginOverride not followed by a jump",
            "a note added later",
        ] {
            assert_eq!(with_note(class), Err(All::Gap), "{class}");
        }
        // MI1's notes are all of EXACT_NOTES
        for u in game().ids() {
            for n in &game().unit(u).notes {
                assert!(EXACT_NOTES.contains(&n.class), "{:?}: {}", game().unit(u).key, n.class);
            }
        }
    }

    /// The operands of the codes inside a unit's texts, by code name: (expression, its node).
    fn text_operands(g: &Game, k: UnitKey, code: &str) -> Vec<(Expr, ExprId)> {
        let mut out = Vec::new();
        for x in g.ins(g.id(k).unwrap()) {
            if let Some(st) = &x.st {
                g.ast.walk_stmt(st, &mut |e| {
                    if let Expr::Getter { name, args } = g.ast.expr(e) {
                        if g.syms.str(name) == code {
                            out.extend(g.ast.kids(args).iter().map(|&a| (g.ast.expr(a), a)));
                        }
                    }
                    true
                });
            }
        }
        out
    }

    /// A code inside a text keeps its unit's reads exact only when the engine reads its bytes as descumm did: room 38's
    /// local 202 has `unknown8(8224)` (bytes 20 20: plain text to the engine, which takes no operand after code 8);
    /// with bytes FF 04 (1279) or 20 FF the engine would start an escape there, so the unit keeps all locals. The same
    /// for a variable operand whose word has a byte 0xFF (Var[255]), an indexed operand, and `sound(..)`.
    #[test]
    fn text_codes_by_their_bytes() {
        let k = UnitKey::Local(38, 202);
        let ops = text_operands(game(), k, "unknown8");
        assert_eq!(ops.iter().map(|o| o.0).collect::<Vec<_>>(), vec![Expr::Const(8224)]);
        let with = |e: ExprId, x: Expr| {
            let mut g = game().clone();
            g.script.ast.exprs[e.0 as usize] = x;
            let u = g.id(k).unwrap();
            unit_reads(&g, u)
        };
        let exact = reads(k);
        assert!(exact.is_ok());
        let e = ops[0].1;
        assert_eq!(with(e, Expr::Const(0x2020)), exact);
        assert_eq!(with(e, Expr::Const(0x04FF)), Err(All::TextBytes));
        assert_eq!(with(e, Expr::Const(0xFF20)), Err(All::TextBytes));
        assert_eq!(with(e, Expr::Const(-1)), Err(All::TextBytes));
        // a variable operand: Var[255] is the word 00FF; Local[n] (0x4000 + n) and Var[100] have no byte 0xFF
        let (k2, v) = game()
            .ids()
            .map(|u| game().unit(u).key)
            .find_map(|k| text_operands(game(), k, "getInt").into_iter().find(|o| matches!(o.0, Expr::Var(_))).map(|o| (k, o.1)))
            .expect("a getInt(Var[..])");
        let with2 = |x: Expr| {
            let mut g = game().clone();
            g.script.ast.exprs[v.0 as usize] = x;
            unit_reads(&g, g.id(k2).unwrap())
        };
        assert!(reads(k2).is_ok());
        assert_eq!(with2(Expr::Var(100)), reads(k2));
        assert_eq!(with2(Expr::Var(255)), Err(All::TextBytes));
        assert_eq!(with2(Expr::Var(0x1FF)), Err(All::TextBytes));
        // MI1: no code inside a text has an operand byte 0xFF, an index, code 0xFF or `sound`
        let ll = LiveLocals::of(game());
        assert!(game().ids().all(|u| !matches!(ll.reads[&game().unit(u).key], Err(All::TextBytes))));
    }

    /// The recursive flag: only whether it is true.
    #[test]
    fn recursive_flag() {
        assert_eq!(recursive(&Py::Bool(false)), Py::None);
        assert_eq!(recursive(&Py::None), Py::None);
        assert_eq!(recursive(&Py::Bool(true)), Py::Bool(true));
    }

    #[test]
    fn keep_drops_only_unread_locals() {
        let ll = LiveLocals::of(game());
        let u = Val::Unk(Unknown::new(R_LOCAL, "Local[2] from a fact", None));
        let l = vec![(0, Val::Int(202)), (1, Val::Int(7)), (2, u.clone())];
        assert_eq!(ll.keep(UnitKey::Local(31, 202), l.clone()), (vec![(0, Val::Int(202))], true));
        assert_eq!(ll.keep(UnitKey::Global(64), l.clone()), (l.clone(), false));
        // all locals: kept as they are
        assert_eq!(ll.keep(UnitKey::Global(153), l.clone()), (l.clone(), false));
        // a unit the game lacks: kept as they are
        assert_eq!(ll.keep(UnitKey::Global(9999), l.clone()), (l, false));
    }
}
