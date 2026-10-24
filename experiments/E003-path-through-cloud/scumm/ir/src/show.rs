//! Printing the IR back (ir.py `show` and `head`), for citations, the round trip and the machine's
//! notes; and Python's dataclass `repr` of a node, for parse error messages.

use crate::ast::*;
use crate::py;
use crate::sym::{known, Interner};
use crate::vars;
use std::fmt::Write;

/// ir.py `show` of an expression.
pub fn show_expr(ast: &Ast, syms: &Interner, e: ExprId) -> String {
    let mut s = String::new();
    expr(ast, syms, e, &mut s);
    s
}

/// ir.py `show` of a statement.
pub fn show_stmt(ast: &Ast, syms: &Interner, st: &Stmt) -> String {
    let mut s = String::new();
    stmt(ast, syms, st, &mut s);
    s
}

fn list(ast: &Ast, syms: &Interner, ids: &[ExprId], out: &mut String) {
    for (k, &i) in ids.iter().enumerate() {
        if k > 0 {
            out.push(',');
        }
        expr(ast, syms, i, out);
    }
}

fn expr(ast: &Ast, syms: &Interner, e: ExprId, out: &mut String) {
    match ast.expr(e) {
        Expr::Const(v) => write!(out, "{v}").unwrap(),
        Expr::Var(n) => match vars::var_name(n) {
            Some(name) => out.push_str(name),
            None => write!(out, "Var[{n}]").unwrap(),
        },
        Expr::Bit(n) => write!(out, "Bit[{n}]").unwrap(),
        Expr::Local(n) => write!(out, "Local[{n}]").unwrap(),
        // descumm prints the index variable by number (descumm.cpp:502-509)
        Expr::Index { kind, base, offset } => {
            let (name, n) = match ast.expr(offset) {
                Expr::Var(n) => ("Var", n),
                Expr::Bit(n) => ("Bit", n),
                Expr::Local(n) => ("Local", n),
                // ir.py's show fails here (an Index has no .n); the scripts have none
                _ => ("Index", 0),
            };
            write!(out, "{}[{base} + {name}[{n}]]", kind.title()).unwrap()
        }
        Expr::BinOp { op, a, b } => {
            out.push('(');
            expr(ast, syms, a, out);
            write!(out, " {} ", op.as_str()).unwrap();
            expr(ast, syms, b, out);
            out.push(')');
        }
        Expr::Neg(x) => {
            out.push('-');
            expr(ast, syms, x, out);
        }
        Expr::Getter { name, args } => {
            out.push_str(syms.str(name));
            out.push('(');
            list(ast, syms, ast.kids(args), out);
            out.push(')');
        }
        Expr::Embed(a) => {
            let s = show_stmt(ast, syms, &Stmt::Assign(a));
            out.push('<');
            out.push_str(&s[..s.len() - 1]);
            out.push('>');
        }
        Expr::Str(y) => {
            let s = syms.str(y);
            if !s.is_empty() {
                out.push('"');
                out.push_str(s);
                out.push('"');
            }
        }
        Expr::Concat(parts) => {
            for (k, &p) in ast.kids(parts).iter().enumerate() {
                if k > 0 {
                    out.push_str(" + ");
                }
                expr(ast, syms, p, out);
            }
        }
        Expr::List(items) => {
            out.push('[');
            list(ast, syms, ast.kids(items), out);
            out.push(']');
        }
        Expr::Flag(y) => out.push_str(syms.str(y)),
        Expr::Cmp { op, a, b } => {
            expr(ast, syms, a, out);
            write!(out, " {} ", op.as_str()).unwrap();
            expr(ast, syms, b, out);
        }
        Expr::Not(x) => {
            out.push('!');
            expr(ast, syms, x, out);
        }
        Expr::Truth(x) => expr(ast, syms, x, out),
    }
}

fn sub(ast: &Ast, syms: &Interner, s: &Sub, out: &mut String) {
    out.push_str(syms.str(s.name));
    out.push('(');
    list(ast, syms, ast.kids(s.args), out);
    out.push(')');
}

fn stmt(ast: &Ast, syms: &Interner, st: &Stmt, out: &mut String) {
    match *st {
        Stmt::Assign(a) => {
            expr(ast, syms, a.lhs, out);
            if matches!(a.op, AssignOp::Inc | AssignOp::Dec) {
                write!(out, "{};", a.op.as_str()).unwrap();
            } else {
                write!(out, " {} ", a.op.as_str()).unwrap();
                expr(ast, syms, a.rhs, out);
                out.push(';');
            }
        }
        Stmt::Start { name, script, obj, args, freeze, recursive } => {
            out.push_str(syms.str(name));
            out.push('(');
            if let Some(o) = obj {
                expr(ast, syms, o, out);
                out.push(',');
            }
            expr(ast, syms, script, out);
            out.push_str(",[");
            list(ast, syms, ast.kids(args), out);
            out.push(']');
            if freeze {
                out.push_str(",F");
            }
            if recursive {
                out.push_str(",R");
            }
            out.push_str(");");
        }
        Stmt::Op { name, args, subops, flags } => {
            let subs = ast.subs(subops);
            if name == known::DRAW_OBJECT && !subs.is_empty() {
                // descumm's own spelling: drawObject(OBJsetXY(,X,Y));
                out.push_str("drawObject(");
                if let Some(&a0) = ast.kids(args).first() {
                    expr(ast, syms, a0, out);
                }
                out.push_str(syms.str(subs[0].name));
                out.push_str("(,");
                list(ast, syms, ast.kids(subs[0].args), out);
                out.push_str("));");
                return;
            }
            out.push_str(syms.str(name));
            out.push('(');
            let mut first = true;
            let mut sep = |out: &mut String| {
                if !first {
                    out.push(',');
                }
                first = false;
            };
            for &a in ast.kids(args) {
                sep(out);
                expr(ast, syms, a, out);
            }
            if !subs.is_empty() {
                sep(out);
                out.push('[');
                for (k, s) in subs.iter().enumerate() {
                    if k > 0 {
                        out.push(',');
                    }
                    sub(ast, syms, s, out);
                }
                out.push(']');
            }
            for &f in ast.flags(flags) {
                sep(out);
                out.push_str(syms.str(f));
            }
            out.push_str(");");
        }
        Stmt::Jump { cond, to, form } => {
            let c = cond.map(|c| show_expr(ast, syms, c)).unwrap_or_default();
            let h = py::hex04(to as i64);
            match form {
                JumpForm::If => write!(out, "if ({c}) /*{h}*/ {{"),
                JumpForm::While => write!(out, "while ({c}) /*{h}*/ {{"),
                JumpForm::ElseIf => write!(out, "}} else if ({c}) /*{h}*/ {{"),
                JumpForm::Else => write!(out, "}} else /*{h}*/ {{"),
                JumpForm::Goto0 => write!(out, "/* goto {h}; */"),
                JumpForm::Unless => write!(out, "unless ({c}) goto {h};"),
                _ => write!(out, "goto {h};"),
            }
            .unwrap();
        }
    }
}

/// ir.py `head`: a statement's name for counting, and the key of model.py's rule table (HEADS):
/// `jump <form>`, `assign <op>` (with the getter's name when the value is a getter), or the op's name.
pub fn head(ast: &Ast, syms: &Interner, st: &Stmt) -> String {
    match *st {
        Stmt::Jump { form, .. } => format!("jump {}", form.as_str()),
        Stmt::Assign(a) => match ast.expr(a.rhs) {
            Expr::Getter { name, .. } => format!("assign {} {}", a.op.as_str(), syms.str(name)),
            _ => format!("assign {}", a.op.as_str()),
        },
        Stmt::Op { name, .. } | Stmt::Start { name, .. } => syms.str(name).to_string(),
    }
}

/// Python's dataclass repr of an expression node (`Getter(name='x', args=(Const(value=1),))`), as
/// ir.py's "cannot assign to ..." message prints it.
pub fn py_repr(ast: &Ast, syms: &Interner, e: ExprId) -> String {
    let tuple = |ids: &[ExprId]| -> String {
        let parts: Vec<String> = ids.iter().map(|&i| py_repr(ast, syms, i)).collect();
        match parts.len() {
            1 => format!("({},)", parts[0]),
            _ => format!("({})", parts.join(", ")),
        }
    };
    let assign = |a: AssignSt| {
        format!(
            "Assign(lhs={}, op={}, rhs={})",
            py_repr(ast, syms, a.lhs),
            py::repr(a.op.as_str()),
            py_repr(ast, syms, a.rhs)
        )
    };
    match ast.expr(e) {
        Expr::Const(v) => format!("Const(value={v})"),
        Expr::Var(n) => format!("Var(n={n})"),
        Expr::Bit(n) => format!("Bit(n={n})"),
        Expr::Local(n) => format!("Local(n={n})"),
        Expr::Index { kind, base, offset } => format!(
            "Index(kind={}, base={base}, offset={})",
            py::repr(kind.as_str()),
            py_repr(ast, syms, offset)
        ),
        Expr::BinOp { op, a, b } => format!(
            "BinOp(op={}, a={}, b={})",
            py::repr(op.as_str()),
            py_repr(ast, syms, a),
            py_repr(ast, syms, b)
        ),
        Expr::Neg(x) => format!("Neg(x={})", py_repr(ast, syms, x)),
        Expr::Getter { name, args } => {
            format!("Getter(name={}, args={})", py::repr(syms.str(name)), tuple(ast.kids(args)))
        }
        Expr::Embed(a) => format!("Embed(st={})", assign(a)),
        Expr::Str(y) => format!("Str(text={})", py::repr(syms.str(y))),
        Expr::Concat(s) => format!("Concat(parts={})", tuple(ast.kids(s))),
        Expr::List(s) => format!("List(items={})", tuple(ast.kids(s))),
        Expr::Flag(y) => format!("Flag(name={})", py::repr(syms.str(y))),
        Expr::Cmp { op, a, b } => format!(
            "Cmp(op={}, a={}, b={})",
            py::repr(op.as_str()),
            py_repr(ast, syms, a),
            py_repr(ast, syms, b)
        ),
        Expr::Not(x) => format!("Not(x={})", py_repr(ast, syms, x)),
        Expr::Truth(x) => format!("Truth(x={})", py_repr(ast, syms, x)),
    }
}
