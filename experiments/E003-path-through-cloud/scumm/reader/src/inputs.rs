//! Fix round 1, F1 with F11 (fix-round1.md; switch `input_script`): an input is one record that starts at the
//! room's input script. This module works out, from the scope's code and the game's tables alone, what the reading
//! needs for that; the runs are machine.rs's (`Reader::run_full`, `What::Input`) and the jobs builder.rs's
//! (`Builder::inputs`).
//!
//! * **The input scripts**: the scripts VAR_VERB_SCRIPT can hold in the scope (its start value and the constants
//!   the scope's code writes it), a global script or the room's local one (runInputScript, script.cpp:1478-1511;
//!   from _numGlobalScripts on a number is the room's local, script.cpp:70-86).
//! * **The mouse loops**: the units whose code waits inside a loop that reads the mouse (VAR_MOUSE_X/Y,
//!   VAR_VIRT_MOUSE_X/Y). A live, unfrozen slot waiting there takes its turn in the input's frame before
//!   checkExecVerbs (scumm.cpp:3231-3232), so its turn is part of the input's record.
//! * **The sentence-building state** (`input_vars`): the variables both read and written by the input scripts and
//!   the mouse loops, in their live code under the run's settings. They are facts: their domain starts from the
//!   start value and the constants the scope writes them, and the reading's passes add every value a path writes
//!   (`Builder::extend_domains`).
//! * **Targets** (C1): a cell of a scope room's box arrangement (scumm-ir `cells.rs`), clicked with the left or the
//!   right button; a verb the scope shows, clicked with either button (a shown verb's key is the same input as its
//!   left click, verbs.cpp:608-615). The verbs are the scope's verbs and dialog lines (F11: dialog lines are verbs by
//!   number), and the slot verbs: the verbs an input script compares its value argument with as a closed range
//!   (`Local[1] >= a`, then `Local[1] <= b`), which a VerbOps with a number that is not a constant can make (the
//!   inventory's slots). Keys are no input (other keys reach the input script as kKeyClickArea, verbs.cpp:663). Floor
//!   points are no target. An actor under the click is a replay check (findObject and actorFromPos stay as F12 has
//!   them).
//! * **The mouse after the input** (`mouse`): the cell or verb it is over, a fact in a room no wider than the screen
//!   (the camera does not move, camera.cpp:169-192); in a wider room it is not known after the frame (`NOWHERE`).
//!   The mouse loops' later turns read it.
//!
//! Nothing here names a game's room, object, verb, variable or script (D1, plan.md's C2).

use crate::domains;
use crate::facts::Facts;
use crate::scope::{live_bound, made_verbs, Writes};
use crate::table::{MOUSE_VARS, TIME_VARS};
use scumm_ir::{known, CmpOp, Expr, Game, Stmt, UnitId, UnitKey, Yield};
use std::collections::{BTreeMap, BTreeSet};

/// runInputScript's areas (verbs.h:35-39).
pub const VERB_CLICK_AREA: i64 = 1;
pub const SCENE_CLICK_AREA: i64 = 2;
/// checkExecVerbs' button codes for v5 (verbs.cpp:666): left 1, right 2.
pub const BUTTONS: [i64; 2] = [1, 2];
/// The `mouse` fact's value when the reading does not know where the mouse is.
pub const NOWHERE: i64 = -1;
/// The screen's width (scumm.cpp:395).
pub const SCREEN_WIDTH: i32 = 320;
/// The value argument of the input script (runInputScript's second, script.cpp:1497-1500).
const VALUE_ARG: i32 = 1;
/// The longest closed range of verb numbers taken as slot verbs.
const SLOT_RANGE: i32 = 16;
/// `mouse` values of verbs start here (cells are room * CELL_BASE + index).
pub const VERB_MOUSE: i64 = 1 << 24;
pub const CELL_BASE: i64 = 1000;

/// The `mouse` value of a room's cell.
pub fn cell_id(res: i32, idx: usize) -> i64 {
    res as i64 * CELL_BASE + idx as i64
}

/// The (room resource, cell index) of a `mouse` value, or None for a verb or NOWHERE.
pub fn cell_of(id: i64) -> Option<(i32, usize)> {
    (id >= 0 && id < VERB_MOUSE).then(|| ((id / CELL_BASE) as i32, (id % CELL_BASE) as usize))
}

/// The `mouse` value of a verb.
pub fn verb_id(v: i64) -> i64 {
    VERB_MOUSE + v
}

/// The verb of a `mouse` value, or None.
pub fn verb_of(id: i64) -> Option<i64> {
    (id >= VERB_MOUSE).then(|| id - VERB_MOUSE)
}

/// What F1 adds to a scope.
#[derive(Clone, Debug, Default)]
pub struct InputScope {
    /// the input script numbers VAR_VERB_SCRIPT can hold, and their units in the scope
    pub scripts: Vec<i64>,
    pub units: Vec<UnitId>,
    /// units with a wait inside a loop that reads the mouse, and the offsets (`at:` W) of those waits
    pub mouse_loops: Vec<(UnitKey, BTreeSet<i64>)>,
    /// the sentence-building variables
    pub vars: Vec<i32>,
    /// the slot verbs
    pub slot_verbs: Vec<i64>,
    /// the scope rooms' cells: (room, resource, number of cells)
    pub rooms: Vec<(i32, i32, usize)>,
    /// verb -> its place, where every VerbOps that places it gives the same constant SetXY
    pub verb_xy: BTreeMap<i64, (i32, i32)>,
    /// the main screen's top line and height, where every roomOps SetScreen of the game gives the same constants
    /// (gfx.cpp:386: initScreens(top, bottom))
    pub screen: Option<(i32, i32)>,
    /// the highest line the verb screen can start at: the least bottom (below 200) of the game's constant SetScreen
    /// calls (o5_roomOps' SetScreen(b, h) is initScreens(b, h), script_v5.cpp:2397, which puts the verb screen from h
    /// to the screen's bottom, gfx.cpp:386-388; with h at 200 it is empty and no verb can be clicked); 0 where a
    /// SetScreen is not constant
    pub verb_top: i32,
}

fn reads_mouse(game: &Game, st: &Stmt) -> bool {
    let ast = &game.ast;
    let mut hit = false;
    ast.walk_stmt(st, &mut |e| {
        if let Expr::Var(n) = ast.expr(e) {
            if MOUSE_VARS.contains(&n) {
                hit = true;
            }
        }
        !hit
    });
    hit
}

/// The variables an instruction reads (`Var[n]` anywhere in it but its own target) and writes (an assignment's
/// target).
fn var_reads_writes(game: &Game, st: &Stmt, reads: &mut BTreeSet<i32>, writes: &mut BTreeSet<i32>) {
    let ast = &game.ast;
    let target = match st {
        Stmt::Assign(a) => {
            if let Expr::Var(n) = ast.expr(a.lhs) {
                writes.insert(n);
                if a.op != scumm_ir::AssignOp::Set {
                    reads.insert(n);
                }
            }
            Some(a.lhs)
        }
        _ => None,
    };
    ast.walk_stmt(st, &mut |e| {
        if Some(e) != target {
            if let Expr::Var(n) = ast.expr(e) {
                reads.insert(n);
            }
        }
        true
    });
}

/// The input scripts VAR_VERB_SCRIPT can hold: its start value and every constant the scope writes it.
fn input_script_numbers(game: &Game, units: &[UnitId], start: &Facts) -> BTreeSet<i64> {
    let ast = &game.ast;
    let key = format!("var:{}", crate::builder::VAR_VERB_SCRIPT);
    let mut out: BTreeSet<i64> = [start.get0(&key)].into_iter().collect();
    for &u in units {
        for x in game.ins(u) {
            if let Some(Stmt::Assign(a)) = x.st {
                if ast.expr(a.lhs) == Expr::Var(crate::builder::VAR_VERB_SCRIPT) {
                    if let Some(c) = ast.constant(a.rhs) {
                        out.insert(c as i64);
                    }
                }
            }
        }
    }
    out.remove(&0);
    out
}

/// F1's part of a scope (the module's docstring).
pub fn input_scope(game: &Game, units: &[UnitId], rooms: &[i32], start: &Facts, verbs: &[i64], draw_clears: bool) -> InputScope {
    let ast = &game.ast;
    let mut out = InputScope::default();
    // the input scripts and their units
    let nums = input_script_numbers(game, units, start);
    for &n in &nums {
        let keys: Vec<UnitKey> = if (n as i32) < game.first_local {
            vec![UnitKey::Global(n as i32)]
        } else {
            rooms.iter().filter_map(|&r| game.resource(r)).map(|r| UnitKey::Local(r, n as i32)).collect()
        };
        for k in keys {
            if let Some(u) = game.id(k) {
                if units.contains(&u) && !out.units.contains(&u) {
                    out.units.push(u);
                }
            }
        }
    }
    out.scripts = nums.into_iter().collect();
    // the mouse loops
    let cp = true; // const_polls, approved on (D8, F6)
    let mut loop_units: Vec<UnitId> = Vec::new();
    for &u in units {
        let ins = game.ins(u);
        let mut ws: BTreeSet<i64> = BTreeSet::new();
        for lp in game.loops(u, cp) {
            if !lp.body.iter().any(|&i| ins[i as usize].st.is_some_and(|st| reads_mouse(game, &st))) {
                continue;
            }
            // the waits in the loop: after a breakHere or a delay, or at a WaitFor op (a slot gives up the frame
            // there, ir.py `Ins.yields`); a nested call's yield returns to the caller (runScriptNested), so the other
            // stop points are no wait of this slot's turn
            for w in crate::scope::stop_points(game, u) {
                let Some(i) = game.at(u, w) else { continue };
                let after_yield = i > 0 && lp.contains(i as i64 - 1) && ins[i as usize - 1].yields.is_some_and(|y| y != Yield::Wait);
                let wait_op = lp.contains(i as i64) && ins[i as usize].yields == Some(Yield::Wait);
                if after_yield || wait_op {
                    ws.insert(w as i64);
                }
            }
        }
        if !ws.is_empty() {
            out.mouse_loops.push((game.unit(u).key, ws));
            loop_units.push(u);
        }
    }
    // the sentence-building variables: read and written by the input scripts and the mouse loops, in live code
    let mut reads: BTreeSet<i32> = BTreeSet::new();
    let mut writes: BTreeSet<i32> = BTreeSet::new();
    for &u in out.units.iter().chain(loop_units.iter()) {
        let ins = game.ins(u);
        for i in live_bound(game, u, &domains::Env::new()) {
            if let Some(st) = ins[i as usize].st {
                var_reads_writes(game, &st, &mut reads, &mut writes);
            }
        }
    }
    let skip = |n: i32| {
        domains::ENGINE.iter().any(|e| e.0 == n)
            || TIME_VARS.contains(&n)
            || MOUSE_VARS.contains(&n)
            || domains::SETTINGS.iter().any(|s| s.0 == n)
            || domains::HOOKS.contains(&n)
            || domains::ROOM_VARS.contains(&n)
            || n == domains::VAR_EGO
            || n == 0
    };
    out.vars = reads.intersection(&writes).copied().filter(|&n| !skip(n)).collect();
    // the slot verbs: closed ranges an input script compares its value argument with
    let mut ranged: BTreeSet<i64> = BTreeSet::new();
    for &u in &out.units {
        let ins = game.ins(u);
        for (i, x) in ins.iter().enumerate() {
            let Some(Stmt::Jump { cond: Some(c), .. }) = x.st else { continue };
            let Expr::Cmp { op: CmpOp::Ge, a, b } = ast.expr(c) else { continue };
            let (Expr::Local(VALUE_ARG), Some(lo)) = (ast.expr(a), ast.constant(b)) else { continue };
            // the next instruction: Local[1] <= hi
            let Some(Some(Stmt::Jump { cond: Some(c2), .. })) = ins.get(i + 1).map(|y| y.st) else { continue };
            let Expr::Cmp { op: CmpOp::Le, a: a2, b: b2 } = ast.expr(c2) else { continue };
            let (Expr::Local(VALUE_ARG), Some(hi)) = (ast.expr(a2), ast.constant(b2)) else { continue };
            if hi >= lo && hi - lo < SLOT_RANGE {
                ranged.extend((lo..=hi).map(|v| v as i64));
            }
        }
    }
    let wild_verb_ops = game.ins.iter().any(|x| {
        matches!(x.st, Some(Stmt::Op { name: known::VERB_OPS, args, .. }) if ast.constant(ast.kids(args)[0]).is_none())
    });
    if wild_verb_ops {
        out.slot_verbs = ranged.into_iter().filter(|v| !verbs.contains(v)).collect();
    }
    // the rooms' cells
    for &r in rooms {
        if let Some(res) = game.resource(r) {
            let n = game.cells[draw_clears as usize].get(&res).map_or(0, |c| c.len());
            out.rooms.push((r, res, n));
        }
    }
    // the verbs' places: constant SetXY of VerbOps on a constant verb, when they agree
    let made = made_verbs(game);
    let mut xy: BTreeMap<i64, BTreeSet<(i32, i32)>> = BTreeMap::new();
    let mut wild: BTreeSet<i64> = BTreeSet::new();
    for x in &game.ins {
        let Some(Stmt::Op { name: known::VERB_OPS, args, subops, .. }) = x.st else { continue };
        let Some(v) = ast.constant(ast.kids(args)[0]) else { continue };
        if !made.contains_key(&v) {
            continue;
        }
        for s in ast.subs(subops) {
            if game.syms.str(s.name) == "SetXY" {
                let k = ast.kids(s.args);
                match (k.first().and_then(|&a| ast.constant(a)), k.get(1).and_then(|&a| ast.constant(a))) {
                    (Some(px), Some(py)) => {
                        xy.entry(v as i64).or_default().insert((px, py));
                    }
                    _ => {
                        wild.insert(v as i64);
                    }
                }
            }
        }
    }
    for (v, s) in xy {
        if s.len() == 1 && !wild.contains(&v) {
            out.verb_xy.insert(v, *s.iter().next().unwrap());
        }
    }
    // the main screen: every SetScreen(top, bottom) the game has, when they agree
    let mut screens: BTreeSet<Option<(i32, i32)>> = BTreeSet::new();
    let mut seen = |args: scumm_ir::Span| {
        let a: Vec<Option<i32>> = ast.kids(args).iter().map(|&e| ast.constant(e)).collect();
        screens.insert(match a.as_slice() {
            [Some(t), Some(b)] => Some((*t, b - t)),
            _ => None,
        });
    };
    for x in &game.ins {
        if let Some(Stmt::Op { name, args, subops, .. }) = x.st {
            if name == known::SET_SCREEN {
                seen(args);
            }
            for sub in ast.subs(subops) {
                if sub.name == known::SET_SCREEN {
                    seen(sub.args);
                }
            }
        }
    }
    if screens.len() == 1 {
        out.screen = *screens.iter().next().unwrap();
    }
    out.verb_top = if screens.contains(&None) {
        0
    } else {
        screens.iter().flatten().map(|&(t, h)| t + h).filter(|&b| b < 200).min().unwrap_or(0)
    };
    out
}

/// F1's writes, added to the scope's: the `mouse` fact.
pub fn writes(out: &mut Writes) {
    out.exact.insert("mouse".to_string());
}

/// F1b's facts (switch sentence_queue): the sentence queue's state and its top entry (scumm-ir `Fact::Queue`).
pub const QUEUE_KEYS: [&str; 4] = ["queue", "queue:verb", "queue:a", "queue:b"];
/// `queue`'s values: empty, a sentence the next checkAndRunSentenceScript runs, a sentence a live, unfrozen sentence
/// script holds (script.cpp:1186-1194)
pub const QUEUE_EMPTY: i64 = 0;
pub const QUEUE_RUNS: i64 = 1;
pub const QUEUE_HELD: i64 = 2;
/// The machine half's mark for "a held sentence now runs": `queue` 2 becomes 1, 0 stays 0 (a conditional write,
/// `Builder::effect`); never a value of the domain
pub const QUEUE_RELEASE: i64 = 3;
/// `queue:verb`, `queue:a`, `queue:b` where the reading does not know the value doSentence pushed (a value that is
/// no fact): no sentence event reads it, so such a sentence blocks the plan instead of running as a guess
pub const QUEUE_NOT_KNOWN: i64 = -1;

/// F1b's writes, added to the scope's: the queue facts.
pub fn queue_writes(out: &mut Writes) {
    for k in QUEUE_KEYS {
        out.exact.insert(k.to_string());
    }
}

/// The left edge of the main screen in a room `w` wide when the camera cannot move (a room no wider than the
/// screen), else None.
pub fn fixed_left(w: i32) -> Option<i32> {
    (w <= SCREEN_WIDTH).then(|| scumm_machine::machine::screen_lefts(w).0)
}
