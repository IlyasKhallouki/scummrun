//! One printed statement into the IR (ir.py `parse_statement`, `parse_cond`, `parse_expr` and the
//! recursive descent under them). The grammar, the tokens and the error messages are ir.py's.

use crate::ast::*;
use crate::py;
use crate::sym::{known, Interner, Sym};
use crate::vars;

/// A statement that does not parse: ir.py's ParseError (or ValueError) message.
pub type ParseError = String;

/// ir.py `_TOKEN`: a string literal, digits, a name (letters, digits, `_` and `.`), or an operator.
fn tokenize(text: &str) -> Result<Vec<&str>, ParseError> {
    let text = py::rstrip(text);
    let b = text.as_bytes();
    let mut toks = Vec::with_capacity(16);
    let mut pos = 0;
    while pos < b.len() {
        let start = pos;
        // \s*
        let mut p = pos;
        while p < b.len() {
            let c = text[p..].chars().next().unwrap();
            if !py::is_space(c) {
                break;
            }
            p += c.len_utf8();
        }
        let t0 = p;
        let end = match b.get(p) {
            Some(b'"') => {
                // "(?:[^"\\]|\\.)*"
                let mut q = p + 1;
                let mut closed = None;
                while q < b.len() {
                    match b[q] {
                        b'"' => {
                            closed = Some(q + 1);
                            break;
                        }
                        b'\\' if q + 1 < b.len() && b[q + 1] != b'\n' => {
                            q += 1;
                            q += text[q..].chars().next().unwrap().len_utf8();
                        }
                        b'\\' => break,
                        _ => q += 1,
                    }
                }
                closed
            }
            Some(c) if c.is_ascii_digit() => {
                let mut q = p;
                while q < b.len() && b[q].is_ascii_digit() {
                    q += 1;
                }
                Some(q)
            }
            Some(c) if c.is_ascii_alphabetic() || *c == b'_' => {
                let mut q = p + 1;
                while q < b.len() && (b[q].is_ascii_alphanumeric() || b[q] == b'_' || b[q] == b'.') {
                    q += 1;
                }
                Some(q)
            }
            Some(_) => {
                let two = &b[p..(p + 2).min(b.len())];
                if matches!(two, b"==" | b"!=" | b"<=" | b">=" | b"+=" | b"-=" | b"*=" | b"/=" | b"++" | b"--") {
                    Some(p + 2)
                } else if b"-+*/<>=!(),[];".contains(&b[p]) {
                    Some(p + 1)
                } else {
                    None
                }
            }
            None => None,
        };
        match end {
            Some(e) => {
                toks.push(&text[t0..e]);
                pos = e;
            }
            None => {
                let cpos = text[..start].chars().count();
                let rest: String = text[start..].chars().take(10).collect();
                return Err(format!("bad character at {cpos}: {}", py::repr(&rest)));
            }
        }
    }
    Ok(toks)
}

fn is_digits(t: &str) -> bool {
    !t.is_empty() && t.bytes().all(|c| c.is_ascii_digit())
}

/// Python's `int(t)` on a token: a ValueError message for anything but digits.
fn int_of(t: &str) -> Result<i32, ParseError> {
    if is_digits(t) {
        let v: i64 = t.parse().map_err(|_| format!("constant {t} out of range"))?;
        i32::try_from(v).map_err(|_| format!("constant {t} out of the i32 range"))
    } else {
        Err(format!("invalid literal for int() with base 10: {}", py::repr(t)))
    }
}

struct P<'t, 'a> {
    toks: Vec<&'t str>,
    i: usize,
    ast: &'a mut Ast,
    syms: &'a mut Interner,
}

impl<'t, 'a> P<'t, 'a> {
    fn peek(&self) -> Option<&'t str> {
        self.toks.get(self.i).copied()
    }
    fn take(&mut self) -> Result<&'t str, ParseError> {
        let t = self.peek().ok_or_else(|| "unexpected end".to_string())?;
        self.i += 1;
        Ok(t)
    }
    fn expect(&mut self, t: &str) -> Result<(), ParseError> {
        let got = self.take()?;
        if got != t {
            return Err(format!("expected {}, got {}", py::repr(t), py::repr(got)));
        }
        Ok(())
    }
    fn end(&self) -> Result<(), ParseError> {
        if self.peek().is_some() {
            return Err(format!("trailing {}", py::repr_list(&self.toks[self.i..])));
        }
        Ok(())
    }
    fn push(&mut self, e: Expr) -> ExprId {
        self.ast.push(e)
    }

    /// `Var[n]`, `Var[n + k]` (folded), `Var[n + Var[m]]` (an Index).
    fn reference(&mut self, kind: &str) -> Result<ExprId, ParseError> {
        self.expect("[")?;
        let t = self.take()?;
        let base = int_of(t)?;
        let mut off: Option<ExprId> = None;
        let mut off_const: Option<i32> = None;
        if self.peek() == Some("+") {
            self.take()?;
            let t = self.take()?;
            if is_digits(t) {
                off_const = Some(int_of(t)?);
            } else if matches!(t, "Var" | "Bit" | "Local") {
                off = Some(self.reference(t)?);
            } else {
                return Err(format!("bad index {}", py::repr(t)));
            }
        }
        self.expect("]")?;
        let kind = match kind {
            "Var" => RefKind::Var,
            "Bit" => RefKind::Bit,
            _ => RefKind::Local,
        };
        let leaf = |n: i32| match kind {
            RefKind::Var => Expr::Var(n),
            RefKind::Bit => Expr::Bit(n),
            RefKind::Local => Expr::Local(n),
        };
        Ok(match (off, off_const) {
            (None, None) => self.push(leaf(base)),
            (None, Some(k)) => self.push(leaf(base + k)), // readVar adds a constant index
            (Some(o), _) => self.push(Expr::Index { kind, base, offset: o }),
        })
    }

    /// ir.py `_items`: comma-separated expressions up to `close`; an empty item is `Str("")`.
    fn items(&mut self, close: &str) -> Result<Vec<ExprId>, ParseError> {
        let mut out = Vec::new();
        if self.peek() == Some(close) {
            self.take()?;
            return Ok(out);
        }
        loop {
            let p = self.peek();
            if p == Some(",") || p == Some(close) {
                out.push(self.push(Expr::Str(known::EMPTY)));
            } else {
                out.push(self.expr(false)?);
            }
            let t = self.take()?;
            if t == close {
                return Ok(out);
            }
            if t != "," {
                return Err(format!("expected ',' or {}, got {}", py::repr(close), py::repr(t)));
            }
        }
    }

    fn primary(&mut self) -> Result<ExprId, ParseError> {
        let t = self.take()?;
        if t == "(" {
            let e = self.expr(true)?;
            self.expect(")")?;
            return Ok(e);
        }
        if t == "<" {
            let st = self.assign()?;
            self.expect(">")?;
            return Ok(self.push(Expr::Embed(st)));
        }
        if t == "[" {
            let items = self.items("]")?;
            let span = self.ast.push_kids(&items);
            return Ok(self.push(Expr::List(span)));
        }
        if t.starts_with('"') {
            let y = self.syms.intern(&t[1..t.len() - 1]);
            return Ok(self.push(Expr::Str(y)));
        }
        if is_digits(t) {
            let v = int_of(t)?;
            return Ok(self.push(Expr::Const(v)));
        }
        if matches!(t, "Var" | "Bit" | "Local") && self.peek() == Some("[") {
            return self.reference(t);
        }
        if t.starts_with("VAR_") {
            let n = vars::var_of(t).ok_or_else(|| format!("unknown engine variable {t}"))?;
            return Ok(self.push(Expr::Var(n)));
        }
        if self.peek() == Some("(") {
            self.take()?;
            let name = self.syms.intern(t);
            let items = self.items(")")?;
            let args = self.ast.push_kids(&items);
            return Ok(self.push(Expr::Getter { name, args }));
        }
        if matches!(t, "F" | "R" | "STOP") {
            let y = self.syms.intern(t);
            return Ok(self.push(Expr::Flag(y)));
        }
        Err(format!("unexpected {}", py::repr(t)))
    }

    fn unary(&mut self) -> Result<ExprId, ParseError> {
        if self.peek() == Some("-") {
            self.take()?;
            let x = self.unary()?;
            return Ok(match self.ast.expr(x) {
                Expr::Const(v) => {
                    self.ast.exprs[x.0 as usize] = Expr::Const(v.wrapping_neg());
                    x
                }
                _ => self.push(Expr::Neg(x)),
            });
        }
        self.primary()
    }

    fn term(&mut self) -> Result<ExprId, ParseError> {
        let mut a = self.unary()?;
        while let Some(op @ ("*" | "/")) = self.peek() {
            self.take()?;
            let b = self.unary()?;
            a = self.push(Expr::BinOp { op: ArithOp::parse(op).unwrap(), a, b });
        }
        Ok(a)
    }

    /// ir.py `_expr`: descumm puts every arithmetic operator in parentheses, so a `+` outside them
    /// joins the parts of a string.
    fn expr(&mut self, arith: bool) -> Result<ExprId, ParseError> {
        if !arith {
            // ir.py builds Concat(parts(a) + parts(b)) at each "+"; the last one holds every part
            let a = self.unary()?;
            if self.peek() != Some("+") {
                return Ok(a);
            }
            let mut ps = self.parts(a);
            while self.peek() == Some("+") {
                self.take()?;
                let b = self.unary()?;
                ps.extend(self.parts(b));
            }
            let span = self.ast.push_kids(&ps);
            return Ok(self.push(Expr::Concat(span)));
        }
        let mut a = self.term()?;
        while let Some(op @ ("+" | "-")) = self.peek() {
            self.take()?;
            let b = self.term()?;
            a = self.push(Expr::BinOp { op: ArithOp::parse(op).unwrap(), a, b });
        }
        Ok(a)
    }

    fn parts(&self, e: ExprId) -> Vec<ExprId> {
        match self.ast.expr(e) {
            Expr::Concat(s) => self.ast.kids(s).to_vec(),
            _ => vec![e],
        }
    }

    fn assign(&mut self) -> Result<AssignSt, ParseError> {
        let lhs = self.primary()?;
        if !matches!(self.ast.expr(lhs), Expr::Var(_) | Expr::Bit(_) | Expr::Local(_) | Expr::Index { .. }) {
            let r = crate::show::py_repr(self.ast, self.syms, lhs);
            return Err(format!("cannot assign to {r}"));
        }
        let op = self.take()?;
        let op = match op {
            "++" | "--" => {
                let one = self.push(Expr::Const(1));
                let op = if op == "++" { AssignOp::Inc } else { AssignOp::Dec };
                return Ok(AssignSt { lhs, op, rhs: one });
            }
            "=" => AssignOp::Set,
            "+=" => AssignOp::Add,
            "-=" => AssignOp::Sub,
            "*=" => AssignOp::Mul,
            "/=" => AssignOp::Div,
            _ => return Err(format!("bad assignment {}", py::repr(op))),
        };
        let rhs = self.expr(false)?;
        Ok(AssignSt { lhs, op, rhs })
    }

    fn call(&mut self) -> Result<Stmt, ParseError> {
        let name_s = self.take()?;
        self.expect("(")?;
        let all = self.items(")")?;
        let mut flags: Vec<Sym> = Vec::new();
        let mut args: Vec<ExprId> = Vec::with_capacity(all.len());
        for a in all {
            match self.ast.expr(a) {
                Expr::Flag(y) => flags.push(y),
                _ => args.push(a),
            }
        }
        let mut subops: Vec<Sub> = Vec::new();
        if let Some(&last) = args.last() {
            if let Expr::List(items) = self.ast.expr(last) {
                let ks = self.ast.kids(items);
                if !ks.is_empty() && ks.iter().all(|&k| matches!(self.ast.expr(k), Expr::Getter { .. })) {
                    subops = ks
                        .iter()
                        .map(|&k| match self.ast.expr(k) {
                            Expr::Getter { name, args } => Sub { name, args },
                            _ => unreachable!(),
                        })
                        .collect();
                    args.pop();
                }
            }
        }
        let name = self.syms.intern(name_s);
        let list_items = |p: &Self, e: ExprId| match p.ast.expr(e) {
            Expr::List(s) => Some(s),
            _ => None,
        };
        if (name == known::START_SCRIPT || name == known::CHAIN_SCRIPT) && args.len() == 2 && subops.is_empty() {
            if let Some(items) = list_items(self, args[1]) {
                return Ok(Stmt::Start {
                    name,
                    script: args[0],
                    obj: None,
                    args: items,
                    freeze: flags.contains(&known::F),
                    recursive: flags.contains(&known::R),
                });
            }
        }
        if name == known::START_OBJECT && args.len() == 3 && subops.is_empty() && flags.is_empty() {
            if let Some(items) = list_items(self, args[2]) {
                return Ok(Stmt::Start {
                    name,
                    script: args[1],
                    obj: Some(args[0]),
                    args: items,
                    freeze: false,
                    recursive: false,
                });
            }
        }
        let args = self.ast.push_kids(&args);
        let subops = self.ast.push_subs(&subops);
        let flags = self.ast.push_flags(&flags);
        Ok(Stmt::Op { name, args, subops, flags })
    }

    /// ir.py `parse_cond`: `a op b`, `!a`, `a` or `classOfIs(o,[c])`.
    fn cond(&mut self) -> Result<ExprId, ParseError> {
        let c = if self.peek() == Some("!") {
            self.take()?;
            let x = self.expr(false)?;
            self.push(Expr::Not(x))
        } else {
            let a = self.expr(false)?;
            match self.peek().and_then(CmpOp::parse) {
                Some(op) => {
                    self.take()?;
                    let b = self.expr(false)?;
                    self.push(Expr::Cmp { op, a, b })
                }
                None => self.push(Expr::Truth(a)),
            }
        };
        self.end()?;
        Ok(c)
    }
}

/// Parses with the arenas rolled back when the text does not parse, so a failed line leaves no nodes.
fn guarded<T>(ast: &mut Ast, f: impl FnOnce(&mut Ast) -> Result<T, ParseError>) -> Result<T, ParseError> {
    let marks = (ast.exprs.len(), ast.kids.len(), ast.subs.len(), ast.flags.len());
    let r = f(ast);
    if r.is_err() {
        ast.exprs.truncate(marks.0);
        ast.kids.truncate(marks.1);
        ast.subs.truncate(marks.2);
        ast.flags.truncate(marks.3);
    }
    r
}

/// ir.py `parse_cond`.
pub fn parse_cond(text: &str, ast: &mut Ast, syms: &mut Interner) -> Result<ExprId, ParseError> {
    guarded(ast, |ast| {
        let toks = tokenize(text)?;
        P { toks, i: 0, ast, syms }.cond()
    })
}

/// ir.py `parse_expr`.
pub fn parse_expr(text: &str, ast: &mut Ast, syms: &mut Interner) -> Result<ExprId, ParseError> {
    guarded(ast, |ast| {
        let toks = tokenize(text)?;
        let mut p = P { toks, i: 0, ast, syms };
        let e = p.expr(false)?;
        p.end()?;
        Ok(e)
    })
}

fn hex4(s: &str) -> Option<i32> {
    if s.len() == 4 && s.bytes().all(|c| c.is_ascii_digit() || (b'A'..=b'F').contains(&c)) {
        i32::from_str_radix(s, 16).ok()
    } else {
        None
    }
}

/// `prefix (C) /*XXXX*/ {` with the prefix and the `) /*XXXX*/ {` suffix around C (the greedy `(.*)`
/// of ir.py's `_IF` and `_ELSE_IF`).
fn braced<'s>(text: &'s str, prefix: &str) -> Option<(&'s str, i32)> {
    let rest = text.strip_prefix(prefix)?;
    // ") /*XXXX*/ {" is 12 bytes
    if rest.len() < 12 || !rest.is_char_boundary(rest.len() - 12) {
        return None;
    }
    let (c, tail) = rest.split_at(rest.len() - 12);
    let to = hex4(tail.strip_prefix(") /*")?.strip_suffix("*/ {")?)?;
    Some((c, to))
}

/// ir.py `parse_statement`: one printed line that is not a closer (an else-if line as its test).
pub fn parse_statement(text: &str, ast: &mut Ast, syms: &mut Interner) -> Result<Stmt, ParseError> {
    let text = text.strip_prefix("Exprmode ").unwrap_or(text);
    for (prefix, form) in [("if (", JumpForm::If), ("while (", JumpForm::While), ("} else if (", JumpForm::ElseIf)] {
        if let Some((c, to)) = braced(text, prefix) {
            let cond = parse_cond(c, ast, syms)?;
            return Ok(Stmt::Jump { cond: Some(cond), to, form });
        }
    }
    if let Some(to) = text.strip_prefix("} else /*").and_then(|r| r.strip_suffix("*/ {")).and_then(hex4) {
        return Ok(Stmt::Jump { cond: None, to, form: JumpForm::Else });
    }
    if let Some(to) = text.strip_prefix("goto ").and_then(|r| r.strip_suffix(';')).and_then(hex4) {
        return Ok(Stmt::Jump { cond: None, to, form: JumpForm::Goto });
    }
    if let Some(to) = text.strip_prefix("/* goto ").and_then(|r| r.strip_suffix("; */")).and_then(hex4) {
        return Ok(Stmt::Jump { cond: None, to, form: JumpForm::Goto0 });
    }
    if let Some(rest) = text.strip_prefix("unless (") {
        // ^unless \((.*)\) goto ([0-9A-F]{4});$ : ") goto XXXX;" is 12 bytes
        if rest.len() >= 12 && rest.is_char_boundary(rest.len() - 12) {
            let (c, tail) = rest.split_at(rest.len() - 12);
            if let Some(to) = tail.strip_prefix(") goto ").and_then(|r| r.strip_suffix(';')).and_then(hex4) {
                let cond = parse_cond(c, ast, syms)?;
                return Ok(Stmt::Jump { cond: Some(cond), to, form: JumpForm::Unless });
            }
        }
    }
    let rewritten;
    let text = match draw_object(text) {
        Some(t) => {
            rewritten = t;
            rewritten.as_str()
        }
        None => text,
    };
    guarded(ast, |ast| {
        let toks = tokenize(text)?;
        let mut p = P { toks, i: 0, ast, syms };
        let t = p.peek();
        let st = if matches!(t, Some("Var" | "Bit" | "Local")) || t.is_some_and(|t| t.starts_with("VAR_")) {
            Stmt::Assign(p.assign()?)
        } else {
            p.call()?
        };
        p.expect(";")?;
        p.end()?;
        Ok(st)
    })
}

/// ir.py `_DRAW`: descumm prints drawObject's sub-op glued to the object,
/// `drawObject(OBJsetXY(,X,Y));`; rewritten as `drawObject(OBJ,[setXY(X,Y)]);`.
fn draw_object(text: &str) -> Option<String> {
    let rest = text.strip_prefix("drawObject(")?;
    if !rest.ends_with("));") {
        return None;
    }
    // (.+?)(setXY|setImage)\(,(.*)\)\);$ : the first place, after at least one character, where a
    // sub-op name and "(," start and the "));" suffix still fits after them
    for (i, _) in rest.char_indices().skip(1) {
        for sub in ["setXY", "setImage"] {
            let head = format!("{sub}(,");
            if rest[i..].starts_with(&head) {
                let args_start = i + head.len();
                if args_start + 3 <= rest.len() {
                    let args = &rest[args_start..rest.len() - 3];
                    return Some(format!("drawObject({},[{}({})]);", &rest[..i], sub, args));
                }
            }
        }
    }
    None
}
