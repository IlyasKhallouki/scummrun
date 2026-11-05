//! E022 (`FMI_PLACE_CTX=1`): a place's context. A place is a room and the objects ego owns (E016 amendment (11)). Its
//! context is the set of the room's input keys that the state holds nonzero, appended to the place's text
//! (`#k1+k2`), so a state that first holds information the room's code tests is at a fresh place: nothing held there,
//! its interactions novel and targets again (experiments/E022-place-context/plan.md).
//!
//! A room's input keys, read from the game's code once per process: the vars and bits its own code (its entry, exit,
//! local scripts and the verbs of its objects) tests in a jump's condition, less those its own code writes and the
//! engine's variables. The archive then leaves out each key it sees cleared from a parent's state to its child's
//! (`note_edge`): a dialog's topic or a menu's state comes and goes; information, once found, stays.

use crate::archive::FactLook;
use scumm_ir::{Expr, ExprId, Game, Stmt, UnitKey};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::{OnceLock, RwLock};

pub fn on() -> bool {
    std::env::var("FMI_PLACE_CTX").is_ok_and(|v| v == "1")
}

/// Per room, its input keys (`static_keys_of`), from the game the process reads.
fn keys() -> &'static BTreeMap<i64, Vec<String>> {
    static K: OnceLock<BTreeMap<i64, Vec<String>>> = OnceLock::new();
    K.get_or_init(|| match crate::guide::load_game(&crate::guide::default_root()) {
        Ok(g) => static_keys_of(&g),
        Err(e) => {
            eprintln!("E022: no game for the place contexts ({e}); every context is the base");
            BTreeMap::new()
        }
    })
}

fn cleared() -> &'static RwLock<HashSet<String>> {
    static C: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
    C.get_or_init(|| RwLock::new(HashSet::new()))
}

/// The vars and bits an expression reads (`Var[n]`, `Bit[n]`; indexed ones are not known statically).
fn reads(g: &Game, e: ExprId, out: &mut BTreeSet<String>) {
    let ast = &g.ast;
    match ast.expr(e) {
        Expr::Var(n) => {
            out.insert(format!("var:{n}"));
        }
        Expr::Bit(n) => {
            out.insert(format!("bit:{n}"));
        }
        Expr::BinOp { a, b, .. } | Expr::Cmp { a, b, .. } => {
            reads(g, a, out);
            reads(g, b, out);
        }
        Expr::Neg(x) | Expr::Not(x) | Expr::Truth(x) => reads(g, x, out),
        Expr::Getter { args, .. } | Expr::Concat(args) | Expr::List(args) => {
            for &k in ast.kids(args) {
                reads(g, k, out);
            }
        }
        Expr::Embed(a) => reads(g, a.rhs, out),
        _ => {}
    }
}

/// The var or bit an assignment's left side writes.
fn written(g: &Game, lhs: ExprId) -> Option<String> {
    match g.ast.expr(lhs) {
        Expr::Var(n) => Some(format!("var:{n}")),
        Expr::Bit(n) => Some(format!("bit:{n}")),
        _ => None,
    }
}

/// Per room, the keys its own code tests in a jump's condition and does not write, the engine's variables aside.
pub fn static_keys_of(g: &Game) -> BTreeMap<i64, Vec<String>> {
    let engine: HashSet<String> = scumm_ir::vars::VARS.iter().map(|&(_, n)| format!("var:{n}")).collect();
    let mut tested: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();
    let mut wrote: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();
    for u in g.ids() {
        let room = match g.unit(u).key {
            UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => r as i64,
            UnitKey::Obj(o) => match g.object_room.get(&o) {
                Some(&r) => r as i64,
                None => continue,
            },
            UnitKey::Global(_) => continue,
        };
        for x in g.ins(u) {
            match x.st {
                Some(Stmt::Jump { cond: Some(c), .. }) => reads(g, c, tested.entry(room).or_default()),
                Some(Stmt::Assign(a)) => {
                    if let Some(k) = written(g, a.lhs) {
                        wrote.entry(room).or_default().insert(k);
                    }
                }
                _ => {}
            }
        }
    }
    tested
        .into_iter()
        .map(|(r, ks)| {
            let w = wrote.get(&r).cloned().unwrap_or_default();
            (r, ks.into_iter().filter(|k| !w.contains(k) && !engine.contains(k)).collect())
        })
        .collect()
}

/// The keys of every room (`note_edge`'s).
fn all_keys() -> &'static BTreeSet<String> {
    static A: OnceLock<BTreeSet<String>> = OnceLock::new();
    A.get_or_init(|| keys().values().flatten().cloned().collect())
}

/// A row's state against its parent's: an input key nonzero in the parent and 0 in the child is cleared, and stops
/// being an input key.
pub fn note_edge<P: FactLook + ?Sized, C: FactLook + ?Sized>(parent: &P, child: &C) {
    if !on() {
        return;
    }
    let gone: Vec<&String> = all_keys()
        .iter()
        .filter(|k| parent.look(k).unwrap_or(0) != 0 && child.look(k).unwrap_or(0) == 0)
        .collect();
    if gone.is_empty() {
        return;
    }
    let mut c = cleared().write().unwrap_or_else(|e| e.into_inner());
    for k in gone {
        c.insert(k.clone());
    }
}

/// The context of a state in room `room`: the room's input keys not seen cleared that it holds nonzero (a count
/// merges a state that found the information with one that holds other keys as often).
pub fn context_of<F: FactLook + ?Sized>(room: i64, f: &F) -> Vec<String> {
    if !on() {
        return vec![];
    }
    let Some(ks) = keys().get(&room) else { return vec![] };
    let c = cleared().read().unwrap_or_else(|e| e.into_inner());
    ks.iter().filter(|k| !c.contains(*k) && f.look(k).unwrap_or(0) != 0).cloned().collect()
}

/// The text a place takes for its context: empty for the base.
pub fn suffix<F: FactLook + ?Sized>(room: i64, f: &F) -> String {
    match context_of(room, f) {
        ks if ks.is_empty() => String::new(),
        ks => format!("#{}", ks.join("+")),
    }
}

/// The input keys of `room` and the ones seen cleared (the report's).
pub fn report(room: i64) -> (Vec<String>, Vec<String>) {
    let c = cleared().read().unwrap_or_else(|e| e.into_inner());
    let ks = keys().get(&room).cloned().unwrap_or_default();
    let gone = ks.iter().filter(|k| c.contains(*k)).cloned().collect();
    (ks, gone)
}
