//! The IR's types: expressions and statements in flat arenas, instructions, units.
//!
//! Layout. Every expression node of the game lives in one arena, [`Ast::exprs`], and is named by an
//! [`ExprId`]. A node's children are either single ids or a [`Span`] into [`Ast::kids`] (a list of
//! ids, contiguous). A statement ([`Stmt`]) is a small `Copy` value held inline in its instruction.
//! Op sub-ops are [`Sub`]s in [`Ast::subs`], op flags are [`Sym`]s in [`Ast::flags`]. No node owns a
//! heap allocation. Instructions of every unit are one vector ([`crate::Script::ins`]); a unit holds
//! the range of its own.
//!
//! The node kinds, fields and field order are ir.py's dataclasses, one for one (ir.py's module
//! docstring has their meaning). Two differences, both of representation only:
//!
//! * ir.py's `Embed(st)` holds an `Assign`; here [`Expr::Embed`] holds the [`AssignSt`] inline;
//! * ir.py's `Index.kind` and the operators are strings; here they are enums ([`RefKind`],
//!   [`ArithOp`], [`CmpOp`], [`AssignOp`], [`JumpForm`]) that print as those strings.

use crate::sym::Sym;

/// An expression node in [`Ast::exprs`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct ExprId(pub u32);

/// A run of children in [`Ast::kids`] (or of subs in [`Ast::subs`], of flags in [`Ast::flags`]).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Span {
    pub start: u32,
    pub len: u32,
}

impl Span {
    #[inline]
    pub fn range(self) -> std::ops::Range<usize> {
        self.start as usize..(self.start + self.len) as usize
    }
    #[inline]
    pub fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// `Var[...]`, `Bit[...]` or `Local[...]`: the kind of an [`Expr::Index`] (ir.py's "var", "bit", "local").
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum RefKind {
    Var,
    Bit,
    Local,
}

impl RefKind {
    /// ir.py's `Index.kind`: "var", "bit" or "local".
    pub fn as_str(self) -> &'static str {
        match self {
            RefKind::Var => "var",
            RefKind::Bit => "bit",
            RefKind::Local => "local",
        }
    }
    /// The capitalised name descumm prints: "Var", "Bit", "Local".
    pub fn title(self) -> &'static str {
        match self {
            RefKind::Var => "Var",
            RefKind::Bit => "Bit",
            RefKind::Local => "Local",
        }
    }
}

/// `+ - * /` in an [`Expr::BinOp`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl ArithOp {
    pub fn as_str(self) -> &'static str {
        match self {
            ArithOp::Add => "+",
            ArithOp::Sub => "-",
            ArithOp::Mul => "*",
            ArithOp::Div => "/",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "+" => ArithOp::Add,
            "-" => ArithOp::Sub,
            "*" => ArithOp::Mul,
            "/" => ArithOp::Div,
            _ => return None,
        })
    }
}

/// `== != < <= > >=` in an [`Expr::Cmp`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl CmpOp {
    pub fn as_str(self) -> &'static str {
        match self {
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "==" => CmpOp::Eq,
            "!=" => CmpOp::Ne,
            "<" => CmpOp::Lt,
            "<=" => CmpOp::Le,
            ">" => CmpOp::Gt,
            ">=" => CmpOp::Ge,
            _ => return None,
        })
    }
}

/// `= += -= *= /= ++ --` of an [`AssignSt`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum AssignOp {
    Set,
    Add,
    Sub,
    Mul,
    Div,
    Inc,
    Dec,
}

impl AssignOp {
    pub fn as_str(self) -> &'static str {
        match self {
            AssignOp::Set => "=",
            AssignOp::Add => "+=",
            AssignOp::Sub => "-=",
            AssignOp::Mul => "*=",
            AssignOp::Div => "/=",
            AssignOp::Inc => "++",
            AssignOp::Dec => "--",
        }
    }
    /// The arithmetic of `x op= v` as `x op v` (model.script_writes' table); None for `=`.
    pub fn arith(self) -> Option<ArithOp> {
        match self {
            AssignOp::Set => None,
            AssignOp::Add | AssignOp::Inc => Some(ArithOp::Add),
            AssignOp::Sub | AssignOp::Dec => Some(ArithOp::Sub),
            AssignOp::Mul => Some(ArithOp::Mul),
            AssignOp::Div => Some(ArithOp::Div),
        }
    }
}

/// How descumm printed a jump (ir.py `Jump.form`). The last three are jumps descumm did not print,
/// recovered by ir.py's hidden-instruction rules.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum JumpForm {
    If,
    While,
    ElseIf,
    Else,
    Goto,
    Goto0,
    Unless,
    ElseIfJump,
    ElseChain,
    WhileBack,
}

impl JumpForm {
    pub fn as_str(self) -> &'static str {
        match self {
            JumpForm::If => "if",
            JumpForm::While => "while",
            JumpForm::ElseIf => "elseif",
            JumpForm::Else => "else",
            JumpForm::Goto => "goto",
            JumpForm::Goto0 => "goto0",
            JumpForm::Unless => "unless",
            JumpForm::ElseIfJump => "elseif-jump",
            JumpForm::ElseChain => "else-chain",
            JumpForm::WhileBack => "while-back",
        }
    }
}

/// An assignment `lhs op rhs`: a statement ([`Stmt::Assign`]) or an opcode run inside an expression
/// ([`Expr::Embed`]). `rhs` is `Const(1)` for `++` and `--`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct AssignSt {
    pub lhs: ExprId,
    pub op: AssignOp,
    pub rhs: ExprId,
}

/// An expression node (ir.py's expression and condition dataclasses).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Expr {
    Const(i32),
    Var(i32),
    Bit(i32),
    Local(i32),
    /// `Var[base + offset]`; a constant offset is folded at parse time, so `offset` is never a Const.
    Index { kind: RefKind, base: i32, offset: ExprId },
    BinOp { op: ArithOp, a: ExprId, b: ExprId },
    Neg(ExprId),
    /// Any call in expression position, also the text codes inside strings.
    Getter { name: Sym, args: Span },
    /// An opcode run inside an expression: it writes `lhs`, and the value is var 0 afterwards.
    Embed(AssignSt),
    /// A string as descumm printed it, escapes kept; an empty argument is `Str("")`.
    Str(Sym),
    Concat(Span),
    List(Span),
    Flag(Sym),
    Cmp { op: CmpOp, a: ExprId, b: ExprId },
    Not(ExprId),
    Truth(ExprId),
}

/// An op's bracketed sub-op (`VerbOps(..,[On(),SetXY(2,3)])`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Sub {
    pub name: Sym,
    pub args: Span,
}

/// A statement: one printed line, or a recovered hidden jump.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Stmt {
    Assign(AssignSt),
    /// Jumps to byte offset `to` when `cond` is false (`None`: always).
    Jump { cond: Option<ExprId>, to: i32, form: JumpForm },
    /// startScript, chainScript (`obj` None) or startObject (`script` is the verb entry).
    Start { name: Sym, script: ExprId, obj: Option<ExprId>, args: Span, freeze: bool, recursive: bool },
    /// Everything else. `subops` spans [`Ast::subs`], `flags` spans [`Ast::flags`].
    Op { name: Sym, args: Span, subops: Span, flags: Span },
}

impl Stmt {
    /// The name of an Op or Start (ir.py `st.name`), None for Assign and Jump.
    #[inline]
    pub fn name(&self) -> Option<Sym> {
        match *self {
            Stmt::Op { name, .. } | Stmt::Start { name, .. } => Some(name),
            _ => None,
        }
    }
}

/// The arenas every [`ExprId`], [`Span`] and [`Stmt`] points into.
#[derive(Clone, Debug, Default)]
pub struct Ast {
    pub exprs: Vec<Expr>,
    pub kids: Vec<ExprId>,
    pub subs: Vec<Sub>,
    pub flags: Vec<Sym>,
}

impl Ast {
    #[inline]
    pub fn expr(&self, e: ExprId) -> Expr {
        self.exprs[e.0 as usize]
    }
    #[inline]
    pub fn kids(&self, s: Span) -> &[ExprId] {
        &self.kids[s.range()]
    }
    #[inline]
    pub fn subs(&self, s: Span) -> &[Sub] {
        &self.subs[s.range()]
    }
    #[inline]
    pub fn flags(&self, s: Span) -> &[Sym] {
        &self.flags[s.range()]
    }
    pub fn push(&mut self, e: Expr) -> ExprId {
        let id = ExprId(self.exprs.len() as u32);
        self.exprs.push(e);
        id
    }
    pub fn push_kids(&mut self, ids: &[ExprId]) -> Span {
        let start = self.kids.len() as u32;
        self.kids.extend_from_slice(ids);
        Span { start, len: ids.len() as u32 }
    }
    pub fn push_subs(&mut self, subs: &[Sub]) -> Span {
        let start = self.subs.len() as u32;
        self.subs.extend_from_slice(subs);
        Span { start, len: subs.len() as u32 }
    }
    pub fn push_flags(&mut self, flags: &[Sym]) -> Span {
        let start = self.flags.len() as u32;
        self.flags.extend_from_slice(flags);
        Span { start, len: flags.len() as u32 }
    }

    /// The constant value of `e`, if it is a Const.
    #[inline]
    pub fn constant(&self, e: ExprId) -> Option<i32> {
        match self.expr(e) {
            Expr::Const(v) => Some(v),
            _ => None,
        }
    }

    /// Calls `f` on every expression node of `st` in ir.py's `_walk` order (pre-order, fields in
    /// declaration order, an Embed before its lhs and rhs). `f` returns false to stop; the result is
    /// false when it stopped.
    pub fn walk_stmt(&self, st: &Stmt, f: &mut impl FnMut(ExprId) -> bool) -> bool {
        match *st {
            Stmt::Assign(a) => self.walk(a.lhs, f) && self.walk(a.rhs, f),
            Stmt::Jump { cond, .. } => cond.is_none_or(|c| self.walk(c, f)),
            Stmt::Start { script, obj, args, .. } => {
                self.walk(script, f)
                    && obj.is_none_or(|o| self.walk(o, f))
                    && self.kids(args).iter().all(|&k| self.walk(k, f))
            }
            Stmt::Op { args, subops, .. } => {
                self.kids(args).iter().all(|&k| self.walk(k, f))
                    && self.subs(subops).iter().all(|s| self.kids(s.args).iter().all(|&k| self.walk(k, f)))
            }
        }
    }

    /// [`Ast::walk_stmt`] for one expression.
    pub fn walk(&self, e: ExprId, f: &mut impl FnMut(ExprId) -> bool) -> bool {
        if !f(e) {
            return false;
        }
        match self.expr(e) {
            Expr::Const(_) | Expr::Var(_) | Expr::Bit(_) | Expr::Local(_) | Expr::Str(_) | Expr::Flag(_) => true,
            Expr::Index { offset, .. } => self.walk(offset, f),
            Expr::BinOp { a, b, .. } | Expr::Cmp { a, b, .. } => self.walk(a, f) && self.walk(b, f),
            Expr::Neg(x) | Expr::Not(x) | Expr::Truth(x) => self.walk(x, f),
            Expr::Getter { args, .. } | Expr::Concat(args) | Expr::List(args) => {
                self.kids(args).iter().all(|&k| self.walk(k, f))
            }
            Expr::Embed(a) => self.walk(a.lhs, f) && self.walk(a.rhs, f),
        }
    }
}

/// How a slot gives up the frame at an instruction (ir.py `Ins.yields`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Yield {
    /// breakHere
    Frame,
    /// delay, delayVariable
    Delay,
    /// the WaitFor* ops, which re-run themselves while they block
    Wait,
}

impl Yield {
    pub fn as_str(self) -> &'static str {
        match self {
            Yield::Frame => "frame",
            Yield::Delay => "delay",
            Yield::Wait => "wait",
        }
    }
}

/// When an instruction ends the running script (ir.py `Ins.ends`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Ends {
    Always,
    Room,
    IfSelf,
}

impl Ends {
    pub fn as_str(self) -> &'static str {
        match self {
            Ends::Always => "always",
            Ends::Room => "room",
            Ends::IfSelf => "if_self",
        }
    }
}

/// Why a jump with no line of its own was recovered (ir.py `Ins.hidden`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Hidden {
    ElseIfJump,
    ElseChain,
    WhileBack,
}

impl Hidden {
    pub fn as_str(self) -> &'static str {
        match self {
            Hidden::ElseIfJump => "elseif-jump",
            Hidden::ElseChain => "else-chain",
            Hidden::WhileBack => "while-back",
        }
    }
}

/// One instruction (ir.py `Ins`). Its file is its unit's. `next`, `target` and `esc` are indices into
/// the unit's instructions, where the unit's length means the end of the script.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Ins {
    /// byte offset, the `[XXXX]` of the decompiled text
    pub offset: i32,
    /// opcode byte
    pub op: u8,
    /// line in the unit's file (1-based)
    pub line: u32,
    /// statement text, `Exprmode ` stripped; a recovered jump's is `goto XXXX;`
    pub text: Sym,
    /// the parsed statement; None for a line that did not parse
    pub st: Option<Stmt>,
    /// fallthrough; None: never falls through
    pub next: Option<u32>,
    /// a jump's destination
    pub target: Option<u32>,
    /// beginOverride: the jump where Esc resumes
    pub esc: Option<u32>,
    pub yields: Option<Yield>,
    pub ends: Option<Ends>,
    pub hidden: Option<Hidden>,
}

/// A unit's key (ir.py's tuples). The derived order is the tuples' (kind name first: entry < exit <
/// global < local < obj, then the numbers).
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum UnitKey {
    /// ("entry", room)
    Entry(i32),
    /// ("exit", room)
    Exit(i32),
    /// ("global", n)
    Global(i32),
    /// ("local", room, n)
    Local(i32, i32),
    /// ("obj", object): the object's whole verb block
    Obj(i32),
}

impl UnitKey {
    /// The tuple's first element.
    pub fn kind(self) -> &'static str {
        match self {
            UnitKey::Entry(_) => "entry",
            UnitKey::Exit(_) => "exit",
            UnitKey::Global(_) => "global",
            UnitKey::Local(..) => "local",
            UnitKey::Obj(_) => "obj",
        }
    }
    /// The tuple's numbers.
    pub fn numbers(self) -> ([i32; 2], usize) {
        match self {
            UnitKey::Entry(r) | UnitKey::Exit(r) => ([r, 0], 1),
            UnitKey::Global(n) | UnitKey::Obj(n) => ([n, 0], 1),
            UnitKey::Local(r, n) => ([r, n], 2),
        }
    }
    /// The code offset of the unit's first opcode in its raw block (ir.py `HEADER`).
    pub fn header(self) -> usize {
        match self {
            UnitKey::Global(_) | UnitKey::Entry(_) | UnitKey::Exit(_) => 8,
            UnitKey::Local(..) => 9,
            UnitKey::Obj(_) => 0,
        }
    }
}

impl std::fmt::Display for UnitKey {
    /// As Python prints the tuple: `('local', 1, 200)`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            UnitKey::Local(r, n) => write!(f, "('local', {r}, {n})"),
            k => write!(f, "('{}', {})", k.kind(), k.numbers().0[0]),
        }
    }
}

/// A unit's index in [`crate::Script::units`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct UnitId(pub u32);

/// An anomaly or a check the reader made (ir.py's `(class, line, detail)` notes).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Note {
    pub class: &'static str,
    pub line: u32,
    pub detail: String,
}

/// One script (ir.py `Unit`).
#[derive(Clone, Debug)]
pub struct Unit {
    pub key: UnitKey,
    pub room: Option<i32>,
    /// the decompiled file, relative to data/scripts-dos-cd
    pub file: Sym,
    /// the raw block, relative to the repository root
    pub block: Sym,
    /// its instructions: `Script::ins[ins_start .. ins_start + ins_len]`
    pub ins_start: u32,
    pub ins_len: u32,
    /// objects: (verb, instruction index), in the event table's order
    pub entries: Vec<(i32, u32)>,
    /// (verb, instruction index or -1) of events never used
    pub shadowed: Vec<(i32, i32)>,
    pub notes: Vec<Note>,
}

impl Unit {
    #[inline]
    pub fn range(&self) -> std::ops::Range<usize> {
        self.ins_start as usize..(self.ins_start + self.ins_len) as usize
    }
    /// The instruction index object code runs for `verb` (its own entry), if any.
    pub fn entry_of(&self, verb: i32) -> Option<u32> {
        self.entries.iter().find(|e| e.0 == verb).map(|e| e.1)
    }
}
