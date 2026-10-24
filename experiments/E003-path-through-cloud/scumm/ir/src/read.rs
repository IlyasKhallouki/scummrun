//! The descumm text into units (ir.py `_read`, `_mark`, `_entries`, `load`). Control flow comes from
//! offsets, not braces; the jumps descumm did not print are recovered by ir.py's three rules; the raw
//! blocks, when given, settle the else-if targets and the ambiguous var names. Every rule, note and
//! order here is ir.py's; its module docstring has the reasons and the ScummVM and descumm cites.

use crate::ast::*;
use crate::parse::parse_statement;
use crate::py;
use crate::sym::{known, Interner, Sym};
use crate::vars;
use rustc_hash::FxHashMap;
use serde_json::Value;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;

/// o5_jumpRelative's opcode (script_v5.cpp:72); its operand is a 2-byte offset, so it is 3 bytes long.
pub const JUMP: u8 = 0x18;
/// The default verb entry (script.cpp:245).
pub const DEFAULT_VERB: i32 = 0xFF;

/// Every unit of the dump (ir.py `load`'s result) and the arenas their statements live in.
#[derive(Clone, Debug)]
pub struct Script {
    pub syms: Interner,
    pub ast: Ast,
    /// every unit's instructions, unit after unit
    pub ins: Vec<Ins>,
    /// in ir.py's load order: globals, then per room its entry, exit and locals, then objects
    pub units: Vec<Unit>,
    pub by_key: FxHashMap<UnitKey, UnitId>,
}

impl Default for Script {
    fn default() -> Self {
        Self::new()
    }
}

impl Script {
    /// A script with no units.
    pub fn new() -> Script {
        Script {
            syms: Interner::new(),
            ast: Ast::default(),
            ins: Vec::with_capacity(48 * 1024),
            units: Vec::new(),
            by_key: FxHashMap::default(),
        }
    }

    /// ir.py `Unit(key, room, file, block)` then `_read(u, text, data)`, and `_entries(u, verbs)` when
    /// `verbs` (the event table: (verb, offset)) is given and not empty. `data` is the unit's raw block.
    /// A test builds its units this way (test_model.py's `unit`: file "test.txt", block "", no data).
    #[allow(clippy::too_many_arguments)]
    pub fn add_unit(
        &mut self,
        key: UnitKey,
        room: Option<i32>,
        file: &str,
        block: &str,
        text: &str,
        data: Option<&[u8]>,
        verbs: Option<&[(i32, i32)]>,
    ) -> UnitId {
        let mut notes = Vec::new();
        let ins = read(&mut self.syms, &mut self.ast, key, text, data, &mut notes);
        let mut u = Unit {
            key,
            room,
            file: self.syms.intern(file),
            block: self.syms.intern(block),
            ins_start: self.ins.len() as u32,
            ins_len: ins.len() as u32,
            entries: Vec::new(),
            shadowed: Vec::new(),
            notes,
        };
        if let Some(vs) = verbs.filter(|v| !v.is_empty()) {
            entries(&mut u, &ins, vs);
        }
        self.ins.extend_from_slice(&ins);
        let id = UnitId(self.units.len() as u32);
        self.by_key.insert(key, id);
        self.units.push(u);
        id
    }

    #[inline]
    pub fn unit(&self, u: UnitId) -> &Unit {
        &self.units[u.0 as usize]
    }
    #[inline]
    pub fn ins(&self, u: UnitId) -> &[Ins] {
        &self.ins[self.unit(u).range()]
    }
    #[inline]
    pub fn id(&self, key: UnitKey) -> Option<UnitId> {
        self.by_key.get(&key).copied()
    }
    /// The unit ids in load order.
    pub fn ids(&self) -> impl Iterator<Item = UnitId> + '_ {
        (0..self.units.len() as u32).map(UnitId)
    }
    /// The index of the instruction at byte offset `offset` (ir.py `Unit.at`: the last one, if two
    /// share it).
    pub fn at(&self, u: UnitId, offset: i32) -> Option<u32> {
        at(self.ins(u), offset)
    }
    /// ir.py `Ins.cite`: `file:line [XXXX]`.
    pub fn cite(&self, u: UnitId, i: u32) -> String {
        let x = &self.ins(u)[i as usize];
        format!("{}:{} [{}]", self.syms.str(self.unit(u).file), x.line, py::hex04(x.offset as i64))
    }
    pub fn text(&self, x: &Ins) -> &str {
        self.syms.str(x.text)
    }
}

/// The last index whose offset is `t` in instructions sorted by offset.
fn at(ins: &[Ins], t: i32) -> Option<u32> {
    let p = ins.partition_point(|x| x.offset <= t);
    (p > 0 && ins[p - 1].offset == t).then(|| (p - 1) as u32)
}

fn hex4(s: &str) -> Option<i32> {
    (s.len() == 4 && s.bytes().all(|c| c.is_ascii_digit() || (b'A'..=b'F').contains(&c)))
        .then(|| i32::from_str_radix(s, 16).unwrap())
}

/// ir.py `_LINE`: `[XXXX] (OP) text`; the offset, the two opcode characters and the text, stripped.
fn line_match(s: &str) -> Option<(i32, &str, &str)> {
    if !s.starts_with('[') {
        return None;
    }
    let offs = hex4(s.get(1..5)?)?;
    let rest = s.get(5..)?.strip_prefix("] (")?;
    let mut cs = rest.char_indices();
    let (_, c1) = cs.next()?;
    let (_, c2) = cs.next()?;
    if c1 == '\n' || c2 == '\n' {
        return None;
    }
    let j = c1.len_utf8() + c2.len_utf8();
    let text = rest[j..].strip_prefix(") ")?;
    Some((offs, &rest[..j], py::strip(text)))
}

/// Python's `int(s, 16)` for the opcode column.
fn opcode(s: &str) -> u8 {
    u8::from_str_radix(py::strip(s), 16).unwrap_or_else(|_| panic!("opcode {s:?} is not a hex byte"))
}

/// `\bname\b` in text (ir.py's search for an ambiguous var name).
fn has_word(text: &str, name: &str) -> bool {
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    text.match_indices(name).any(|(i, _)| !word(text[..i].chars().next_back()) && !word(text[i + name.len()..].chars().next()))
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

struct Raw<'t> {
    offs: i32,
    line: u32,
    op: Option<u8>,
    s: &'t str,
}

fn new_ins(offset: i32, op: u8, text: Sym, line: u32, st: Option<Stmt>) -> Ins {
    Ins { offset, op, line, text, st, next: None, target: None, esc: None, yields: None, ends: None, hidden: None }
}

/// ir.py `_mark`: the engine flags of one instruction, from its statement.
fn mark(ast: &Ast, x: &mut Ins) {
    let st = x.st.unwrap();
    let name = st.name();
    x.yields = match name {
        Some(known::BREAK_HERE) => Some(Yield::Frame),
        Some(known::DELAY | known::DELAY_VARIABLE) => Some(Yield::Delay),
        Some(known::WAIT_FOR_ACTOR | known::WAIT_FOR_MESSAGE | known::WAIT_FOR_CAMERA | known::WAIT_FOR_SENTENCE) => {
            Some(Yield::Wait)
        }
        _ => None,
    };
    let args = match st {
        Stmt::Op { args, .. } => ast.kids(args),
        _ => &[],
    };
    x.ends = match name {
        Some(known::STOP_OBJECT_CODE | known::CHAIN_SCRIPT) => Some(Ends::Always),
        Some(known::STOP_SCRIPT) => {
            if args.len() == 1 && ast.expr(args[0]) == Expr::Const(0) {
                Some(Ends::Always)
            } else {
                Some(Ends::IfSelf)
            }
        }
        Some(known::LOAD_ROOM | known::LOAD_ROOM_WITH_EGO | known::ACTOR_FOLLOW_CAMERA) => Some(Ends::Room),
        Some(n) => {
            let stop = matches!(st, Stmt::Op { flags, .. } if n == known::DO_SENTENCE && ast.flags(flags).contains(&known::STOP));
            let start = matches!(st, Stmt::Start { recursive: false, .. } if n == known::START_SCRIPT);
            let owner = n == known::SET_OWNER_OF && args.len() == 2 && !matches!(ast.expr(args[1]), Expr::Const(v) if v != 0);
            (stop || n == known::START_OBJECT || n == known::STOP_OBJECT_SCRIPT || start || owner).then_some(Ends::IfSelf)
        }
        None => None,
    };
}

/// Fills one unit's instructions from its file's text (ir.py `_read`). `data` is the unit's raw block.
fn read(
    syms: &mut Interner,
    ast: &mut Ast,
    key: UnitKey,
    text: &str,
    data: Option<&[u8]>,
    notes: &mut Vec<Note>,
) -> Vec<Ins> {
    let note = |notes: &mut Vec<Note>, class: &'static str, line: u32, detail: String| {
        notes.push(Note { class, line, detail })
    };
    let mut raw: Vec<Raw> = Vec::new();
    for (k, s) in py::splitlines(text).into_iter().enumerate() {
        if let Some((offs, op, s)) = line_match(s) {
            let op = if op == "**" { None } else { Some(opcode(op)) };
            raw.push(Raw { offs, line: k as u32 + 1, op, s: s.strip_prefix("Exprmode ").unwrap_or(s) });
        }
    }
    let closers: BTreeSet<(i32, u32)> = raw.iter().filter(|r| r.op.is_none()).map(|r| (r.offs, r.line)).collect();
    // instructions in creation order; `seq` names them across the sort by offset
    let mut ins: Vec<Ins> = Vec::with_capacity(raw.len() + 8);
    let mut elseifs: Vec<(usize, i32)> = Vec::new(); // (creation index of the hidden jump, the test's target)
    let mut whiles: Vec<(i32, i32)> = Vec::new(); // the jump back's offset -> the loop's, in insertion order
    for (k, r) in raw.iter().enumerate() {
        let nxt = raw.get(k + 1).map(|r| r.offs);
        let Some(op) = r.op else { continue };
        let st = match parse_statement(r.s, ast, syms) {
            Ok(st) => st,
            Err(e) => {
                note(notes, "unparsed", r.line, format!("{} ({e})", py::prefix_chars(r.s, 60)));
                let t = syms.intern(r.s);
                ins.push(new_ins(r.offs, op, t, r.line, None));
                continue;
            }
        };
        let t = syms.intern(r.s);
        let form = match st {
            Stmt::Jump { form, .. } => Some(form),
            _ => None,
        };
        if form == Some(JumpForm::ElseIf) {
            // the previous branch's jump, then the test
            let mut j = new_ins(r.offs, JUMP, known::EMPTY, r.line, None);
            j.hidden = Some(Hidden::ElseIfJump);
            elseifs.push((ins.len(), match st {
                Stmt::Jump { to, .. } => to,
                _ => unreachable!(),
            }));
            ins.push(j);
            ins.push(new_ins(r.offs + 3, op, t, r.line, Some(st)));
            continue;
        }
        if form == Some(JumpForm::Else) && nxt.is_some_and(|n| n > r.offs + 3) {
            let gap = nxt.unwrap() - r.offs;
            let (k3, rest) = (gap / 3, gap % 3);
            if rest != 0 {
                note(notes, "else gap not a multiple of 3", r.line, r.s.to_string());
            }
            for j in 0..k3 - 1 {
                // jumps that were pending elses, each landing after the next
                let to = r.offs + 3 * (j + 2);
                let text = syms.intern(&format!("goto {};", py::hex04(to as i64)));
                let mut x = new_ins(r.offs + 3 * j, JUMP, text, r.line, Some(Stmt::Jump { cond: None, to, form: JumpForm::ElseChain }));
                x.hidden = Some(Hidden::ElseChain);
                ins.push(x);
            }
            ins.push(new_ins(r.offs + 3 * (k3 - 1), op, t, r.line, Some(st)));
            note(notes, "else chain", r.line, format!("{k3} jumps"));
            continue;
        }
        if matches!(form, Some(JumpForm::Goto | JumpForm::Goto0 | JumpForm::Else)) && nxt.is_some_and(|n| n != r.offs + 3) {
            note(notes, "jump line not 3 bytes", r.line, r.s.to_string());
        }
        if let Stmt::Jump { form: JumpForm::While, to, .. } = st {
            match whiles.iter_mut().find(|w| w.0 == to - 3) {
                Some(w) => w.1 = r.offs,
                None => whiles.push((to - 3, r.offs)),
            }
        }
        ins.push(new_ins(r.offs, op, t, r.line, Some(st)));
    }
    for &(offs, n) in &closers {
        if let Some(p) = whiles.iter().position(|w| w.0 == offs) {
            // the loop's jump back, printed as its closing brace
            let to = whiles.remove(p).1;
            let text = syms.intern(&format!("goto {};", py::hex04(to as i64)));
            let mut x = new_ins(offs, JUMP, text, n, Some(Stmt::Jump { cond: None, to, form: JumpForm::WhileBack }));
            x.hidden = Some(Hidden::WhileBack);
            ins.push(x);
        }
    }
    for &(_, w) in &whiles {
        note(notes, "while without a closer at its end", 0, py::hex04(w as i64));
    }
    // sort by offset (stable), keeping where each creation index went
    let mut order: Vec<usize> = (0..ins.len()).collect();
    order.sort_by_key(|&k| ins[k].offset);
    let mut pos = vec![0usize; ins.len()];
    for (i, &k) in order.iter().enumerate() {
        pos[k] = i;
    }
    let mut ins: Vec<Ins> = order.iter().map(|&k| ins[k]).collect();
    let n = ins.len();
    if n == 0 {
        note(notes, "empty unit", 0, String::new());
    }
    if ins.windows(2).any(|w| w[0].offset == w[1].offset) {
        note(notes, "two instructions at one offset", 0, String::new());
    }
    for i in 0..raw.len().saturating_sub(1) {
        // bytes no instruction or rule accounts for
        let (r, r2) = (&raw[i], &raw[i + 1]);
        if r.op.is_none() && r2.offs > r.offs && at(&ins, r.offs).is_none() {
            note(notes, "closer gap", r.line, format!("{}..{}", py::hex04(r.offs as i64), py::hex04(r2.offs as i64)));
        }
    }
    // the text rule, later ones first: a chain points forward
    let mut guess: FxHashMap<i32, i32> = FxHashMap::default();
    let mut by_offset: Vec<(usize, i32)> = elseifs.clone();
    by_offset.sort_by_key(|&(k, _)| -ins[pos[k]].offset);
    for &(k, t) in &by_offset {
        let x_off = ins[pos[k]].offset;
        let to = match at(&ins, t - 3).map(|j| &ins[j as usize]) {
            None => None,
            Some(prev) if prev.hidden == Some(Hidden::ElseIfJump) => Some(
                *guess.get(&prev.offset).unwrap_or_else(|| panic!("else-if at {x_off:04X}: no guess yet for {:04X}", prev.offset)),
            ),
            Some(prev) => match prev.st {
                Some(Stmt::Jump { cond: None, to, .. }) => Some(to),
                _ => None,
            },
        };
        guess.insert(x_off, match to {
            Some(to) if to >= t => to,
            _ => t,
        });
    }
    let base = key.header();
    for &(k, _) in &elseifs {
        let i = pos[k];
        let (x_off, x_line) = (ins[i].offset, ins[i].line);
        let g = guess[&x_off];
        let mut to = g;
        let byte = data.map(|d| {
            *d.get(base + x_off as usize).unwrap_or_else(|| panic!("else-if at {x_off:04X} past the end of its block"))
        });
        if byte == Some(JUMP) {
            // the operand descumm did not print
            let d = data.unwrap();
            let a = base + x_off as usize;
            to = x_off + 3 + py::from_le(py::slice(d, a + 1, a + 3), true) as i32;
            let class = if to == g {
                "else-if target from bytes, text rule agrees"
            } else {
                "else-if target from bytes, text rule differs"
            };
            note(notes, class, x_line, format!("text {}, bytes {}", py::hex04(g as i64), py::hex04(to as i64)));
        } else {
            note(notes, "else-if target from the text rule, unverified", x_line, py::hex04(to as i64));
        }
        ins[i].st = Some(Stmt::Jump { cond: None, to, form: JumpForm::ElseIfJump });
        ins[i].text = syms.intern(&format!("goto {};", py::hex04(to as i64)));
    }
    for i in 0..n {
        // a name descumm prints for two vars: the 2-byte operand in the bytes decides
        for &(name, ns) in vars::AMBIGUOUS {
            let x = &ins[i];
            if x.st.is_none() || x.hidden.is_some() || !has_word(syms.str(x.text), name) {
                continue;
            }
            let read_as = vars::var_of(name).unwrap();
            let mut found: Vec<i32> = Vec::new();
            if let Some(d) = data {
                let end = if i + 1 < n { base + ins[i + 1].offset as usize } else { d.len() };
                let window = py::slice(d, base + x.offset as usize + 1, end);
                found = ns.iter().copied().filter(|&v| contains(window, &(v as u16).to_le_bytes())).collect();
            }
            if found.len() != 1 {
                note(notes, "ambiguous name, unverified", x.line, format!("{name} read as var {read_as}"));
                continue;
            }
            let line = x.line;
            if found[0] != read_as {
                swap(ast, &ins[i].st.unwrap(), read_as, found[0]);
            }
            note(notes, "ambiguous name from bytes", line, format!("{name} is var {}", found[0]));
        }
    }
    let last = ins.last().map(|x| x.offset);
    for i in 0..n {
        if ins[i].st.is_none() {
            ins[i].next = Some(i as u32 + 1);
            continue;
        }
        mark(ast, &mut ins[i]);
        ins[i].next = if ins[i].ends == Some(Ends::Always) { None } else { Some(i as u32 + 1) };
        if let Some(Stmt::Jump { cond, to, .. }) = ins[i].st {
            let line = ins[i].line;
            ins[i].target = Some(match at(&ins, to) {
                Some(j) => j,
                None if to > last.unwrap() => {
                    note(notes, "jump to the end", line, py::hex04(to as i64));
                    n as u32
                }
                None => {
                    note(notes, "target not an instruction offset", line, py::hex04(to as i64));
                    ins.partition_point(|x| x.offset < to) as u32
                }
            });
            if cond.is_none() {
                ins[i].next = None;
            }
        }
    }
    for i in 0..n {
        if matches!(ins[i].st, Some(Stmt::Op { name: known::BEGIN_OVERRIDE, .. })) {
            let ok = ins.get(i + 1).is_some_and(|a| {
                matches!(a.st, Some(Stmt::Jump { cond: None, .. })) && a.offset == ins[i].offset + 2
            });
            if ok {
                ins[i].next = Some(i as u32 + 2);
                ins[i].esc = Some(i as u32 + 1);
            } else {
                note(notes, "beginOverride not followed by a jump", ins[i].line, String::new());
            }
        }
    }
    ins
}

/// ir.py `_swap`: every Var(a) in the statement read as Var(b), in place (no node is shared between
/// statements).
fn swap(ast: &mut Ast, st: &Stmt, a: i32, b: i32) {
    let mut ids = Vec::new();
    ast.walk_stmt(st, &mut |e| {
        ids.push(e);
        true
    });
    for e in ids {
        if ast.exprs[e.0 as usize] == Expr::Var(a) {
            ast.exprs[e.0 as usize] = Expr::Var(b);
        }
    }
}

/// ir.py `_entries`: object entries from the event table, in table order, first match wins (the
/// verb or 0xFF).
fn entries(u: &mut Unit, ins: &[Ins], table: &[(i32, i32)]) {
    let mut used = vec![false; table.len()];
    let mut seen: Vec<i32> = Vec::new();
    for &(verb, _) in table {
        if seen.contains(&verb) {
            continue;
        }
        seen.push(verb);
        let p = table.iter().position(|&(v, _)| v == verb || v == DEFAULT_VERB).unwrap();
        match at(ins, table[p].1) {
            None => u.notes.push(Note {
                class: "verb entry not an instruction offset",
                line: 0,
                detail: format!("verb {verb} @ {}", py::hex04(table[p].1 as i64)),
            }),
            Some(i) => {
                u.entries.push((verb, i));
                used[p] = true;
            }
        }
    }
    u.shadowed = table
        .iter()
        .enumerate()
        .filter(|(p, _)| !used[*p])
        .map(|(_, &(v, offs))| (v, at(ins, offs).map_or(-1, |i| i as i32)))
        .collect();
}

/// Python's truth value of a JSON value.
pub(crate) fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// The entries of a JSON object sorted by their int keys (ir.py's `sorted(..., key=int)`).
pub(crate) fn by_int_key(v: &Value) -> Vec<(i32, &Value)> {
    let mut out: Vec<(i32, &Value)> = v
        .as_object()
        .map(|o| o.iter().map(|(k, v)| (k.parse().expect("int key"), v)).collect())
        .unwrap_or_default();
    out.sort_by_key(|e| e.0);
    out
}

fn s<'v>(v: &'v Value, k: &str) -> &'v str {
    v[k].as_str().unwrap_or_else(|| panic!("index.json: no string {k:?}"))
}

/// ir.py `load`: every unit of the dump at `scripts` (data/scripts-dos-cd). `blocks` is where the raw
/// block paths of index.json start (the repository root); None, or a missing block, leaves the
/// else-if targets to the text rule and ambiguous var names as read, and the unit's notes say so.
pub fn load(scripts: &Path, blocks: Option<&Path>) -> io::Result<Script> {
    let meta: Value = serde_json::from_slice(&std::fs::read(scripts.join("index.json"))?)?;
    load_with(scripts, blocks, &meta)
}

/// [`load`] with index.json already read.
pub fn load_with(scripts: &Path, blocks: Option<&Path>, meta: &Value) -> io::Result<Script> {
    let mut sc = Script::new();
    // (key, room, file, block, event table)
    #[allow(clippy::type_complexity)]
    let mut todo: Vec<(UnitKey, Option<i32>, String, String, Option<&Vec<Value>>)> = Vec::new();
    for (n, g) in by_int_key(&meta["global_scripts"]) {
        todo.push((UnitKey::Global(n), None, s(g, "file").into(), s(g, "block").into(), None));
    }
    for (r, room) in by_int_key(&meta["rooms"]) {
        let lflf = s(room, "lflf");
        for (kind, tag) in [("entry", "ENCD"), ("exit", "EXCD")] {
            if truthy(room.get(kind)) {
                let key = if kind == "entry" { UnitKey::Entry(r) } else { UnitKey::Exit(r) };
                todo.push((key, Some(r), s(&room[kind], "file").into(), format!("{lflf}/ROOM/{tag}"), None));
            }
        }
        for (n, loc) in by_int_key(&room["local_scripts"]) {
            todo.push((UnitKey::Local(r, n), Some(r), s(loc, "file").into(), format!("{lflf}/ROOM/LSCR_{n:04}"), None));
        }
    }
    for (o, ob) in by_int_key(&meta["objects"]) {
        if truthy(ob.get("verbs")) {
            let room = ob["room"].as_i64().map(|r| r as i32);
            todo.push((UnitKey::Obj(o), room, s(ob, "file").into(), s(ob, "verb_block").into(), ob["verbs"].as_array()));
        }
    }
    // the files first, on a few threads (opening 2 x 1195 small files is most of the load); then the
    // units in order, on this thread (they share the interner and the arenas)
    let read_one = |file: &str, block: &str| -> io::Result<(String, Option<Vec<u8>>)> {
        let text = std::fs::read_to_string(scripts.join(file))?;
        let data = match blocks.map(|b| b.join(block)) {
            Some(p) if p.exists() => Some(std::fs::read(p)?),
            _ => None,
        };
        Ok((text, data))
    };
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 8);
    let chunk = todo.len().div_ceil(threads).max(1);
    let mut files: Vec<io::Result<(String, Option<Vec<u8>>)>> = Vec::with_capacity(todo.len());
    std::thread::scope(|s| {
        let parts: Vec<_> = todo
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().map(|t| read_one(&t.2, &t.3)).collect::<Vec<_>>()))
            .collect();
        for p in parts {
            files.extend(p.join().expect("a file reader thread panicked"));
        }
    });
    for ((key, room, file, block, verbs), got) in todo.into_iter().zip(files) {
        let (text, data) = got?;
        let events: Option<Vec<(i32, i32)>> = verbs.map(|vs| {
            vs.iter()
                .map(|v| {
                    let verb = v["verb"].as_i64().expect("event verb") as i32;
                    let offs = i32::from_str_radix(v["offset"].as_str().expect("event offset"), 16).expect("hex offset");
                    (verb, offs)
                })
                .collect()
        });
        sc.add_unit(key, room, &file, &block, &text, data.as_deref(), events.as_deref());
    }
    Ok(sc)
}
