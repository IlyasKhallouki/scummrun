//! Fix round 1 (fix-round1.md): the Rust machine's rules beyond model.py, each under its switch, on the real game's
//! code with hand-made fact maps. With every switch off the machine is model.py's (the golden tests check that);
//! these check what each rule does when on, and that it does nothing when off.
//!
//! MI1's object, room and script numbers appear here as test data only (D1, plan.md's C2).

use scumm_ir::places::{same_place, set_xy_place};
use scumm_ir::{default_root, Fact, FactMap, Game, UnitKey};
use scumm_machine::path::{Owned, Path};
use scumm_machine::{FactOracle, Features, Machine, Run, Switches, Val};
use std::sync::OnceLock;

fn game() -> &'static Game {
    static G: OnceLock<Game> = OnceLock::new();
    G.get_or_init(|| Game::load(&default_root()).expect("the game"))
}

/// Row 0 (own_entry) or row 2 (own_entry, dispatch, calls, room_change).
fn features(row: u8) -> Features {
    let mut f = Features { own_entry: true, ..Default::default() };
    if row >= 1 {
        (f.dispatch, f.calls) = (true, true);
    }
    if row >= 2 {
        f.room_change = true;
    }
    f
}

fn switches(names: &[&str]) -> Switches {
    Switches::parse(&names.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
}

/// A fact map as the oracle; `kinds` None holds every family (an absent key is 0).
fn oracle(facts: &[(Fact, i32)], kinds: Option<&[&str]>) -> FactOracle {
    let mut m = FactMap::default();
    for &(k, v) in facts {
        m.insert(k, v);
    }
    FactOracle::new(m, kinds.map(|ks| ks.iter().map(|s| s.to_string()).collect()))
}

/// The families of run B's ground truth (check.py `held_families`): no `objpos`, no `actorroom`.
const GROUND_TRUTH: &[&str] = &[
    "at:entry", "at:exit", "at:local", "at:obj", "at:script", "bit", "class", "cutscene", "ego", "esc:entry", "esc:exit",
    "esc:local", "esc:obj", "esc:script", "frozen", "local", "objscript", "owner", "room", "script", "state", "var", "verb",
];

/// `unit` from the instruction at byte offset `offset`, `steps` instructions at most (the path ends "limit" there).
fn run_at(m: &mut Machine, o: &FactOracle, unit: UnitKey, offset: i32, locals: Vec<(i32, Val)>, steps: u32) -> Run {
    let g = game();
    let u = g.id(unit).expect("the unit");
    let pc = g.at(u, offset).expect("an instruction at that offset");
    m.step_limit = steps;
    m.run(o, unit, pc, Some(locals), Owned::Bool(false), Some(false))
}

fn w(p: &Path, k: Fact) -> Option<Val> {
    p.writes.get(&k).copied()
}

fn objs_like(room: i32, ob: i32) -> Vec<i32> {
    let g = game();
    let b = g.object_box[&ob];
    g.room_objects[&room].iter().copied().filter(|&o| o != ob && (g.object_box[&o].w, g.object_box[&o].h) == (b.w, b.h)).collect()
}

// --- F5, draw_clears ---------------------------------------------------------------------------------------------

#[test]
fn f5_engine_tests() {
    assert_eq!(set_xy_place(13, 0), Some((104, 0)));
    assert_eq!(set_xy_place(255, 3), None, "x 255 is no setXY to the engine (script_v5.cpp:1108)");
    assert_eq!(set_xy_place(4096, -1), Some((-32768, -8)), "x * 8 stored as int16");
    assert_eq!(set_xy_place(8192, 1), Some((0, 8)));
    assert!(same_place((104, 0), (104, 0)));
    assert!(!same_place((-8, 0), (-8, 0)), "a negative coordinate matches nothing (int16 against uint16)");
}

/// A frame of the bar's animation: local 208 draws Local[0] (324). On, every other object of room 28 with 324's box
/// (325-329) goes to state 0 and 324 gets 1; off, only 324's state is written (model.py's rule).
#[test]
fn f5_bar_animation_frame() {
    let g = game();
    let bar = UnitKey::Local(28, 208);
    let shared: Vec<i32> = g.room_objects[&28].iter().copied().filter(|&o| o != 324 && g.object_box[&o] == g.object_box[&324]).collect();
    assert_eq!(shared, vec![325, 326, 327, 328, 329], "the objects that share 324's box");
    let mut facts = vec![(Fact::Room, 28)];
    facts.extend((324..=329).map(|o| (Fact::State(o), 1)));
    let o = oracle(&facts, Some(GROUND_TRUTH));
    for on in [false, true] {
        let sw = if on { switches(&["draw_clears"]) } else { Switches::default() };
        let mut m = Machine::new(g, features(0), sw);
        let r = run_at(&mut m, &o, bar, 0, vec![(0, Val::Int(324))], 100);
        assert_eq!(r.paths.len(), 1);
        let p = &r.paths[0];
        assert_eq!(p.end, Some("yield"), "the first frame ends at its breakHere");
        assert_eq!(w(p, Fact::State(324)), Some(Val::Int(1)));
        for &s in &shared {
            assert_eq!(w(p, Fact::State(s)), if on { Some(Val::Int(0)) } else { None }, "state:{s}");
        }
        let not_modelled = p.notes.iter().any(|n| n.0.as_ref() == "drawObject");
        assert_eq!(not_modelled, !on, "model.py's 'not modelled' note only off");
        // no other object of the room changes
        for &o in g.room_objects[&28].iter().filter(|o| !(324..=329).contains(*o)) {
            assert_eq!(w(p, Fact::State(o)), None, "state:{o}");
        }
    }
}

/// The forest's entry (room 58, through pseudo room 201) draws 674 with setXY(0, 0), onto the CDHD place of 658,
/// 660 and others of its size: they go to 0, `objpos:674` is the index of (0, 0) in its places. An object the
/// oracle puts at another place stays; with the ground truth's families (no `objpos`) the places are not known and
/// the clearing is a value set of the old state and 0.
#[test]
fn f5_set_xy_onto_another_place() {
    let g = game();
    let entry = UnitKey::Entry(58);
    assert_eq!(g.resource(201), Some(58), "pseudo room 201 is the forest's resource");
    let at00: Vec<i32> = objs_like(58, 674).into_iter().filter(|o| (g.object_box[o].x, g.object_box[o].y) == (0, 0)).collect();
    assert!(at00.len() >= 2, "objects of 674's size at (0, 0): {at00:?}");
    let places = &g.movable[&674];
    let idx = places.iter().position(|&p| p == (0, 0)).unwrap() as i32;
    // all families held: an absent objpos is 0, the CDHD place
    let moved = at00[0];
    let elsewhere = g.movable[&moved].iter().position(|&p| p != (0, 0)).unwrap() as i32;
    let mut facts = vec![(Fact::Room, 201), (Fact::ObjPos(moved), elsewhere)];
    facts.extend(at00.iter().map(|&o| (Fact::State(o), 1)));
    let o = oracle(&facts, None);
    let mut m = Machine::new(g, features(0), switches(&["draw_clears"]));
    let r = run_at(&mut m, &o, entry, 0x01F7, vec![], 1);
    let p = &r.paths[0];
    assert_eq!(p.end, Some("limit"));
    assert_eq!(w(p, Fact::ObjPos(674)), Some(Val::Int(idx)));
    assert_eq!(w(p, Fact::State(674)), Some(Val::Int(1)));
    for &x in &at00 {
        let want = if x == moved { None } else { Some(Val::Int(0)) };
        assert_eq!(w(p, Fact::State(x)), want, "state:{x}");
    }
    // the ground truth's families: objpos not held, so where 674 and the others are is not known
    let o = oracle(&facts, Some(GROUND_TRUTH));
    let mut m = Machine::new(g, features(0), switches(&["draw_clears"]));
    let r = run_at(&mut m, &o, entry, 0x01F7, vec![], 1);
    let p = &r.paths[0];
    assert_eq!(w(p, Fact::ObjPos(674)), Some(Val::Int(idx)), "setXY's own target is known");
    for &x in &at00 {
        let v = w(p, Fact::State(x)).expect("written");
        if g.movable.contains_key(&x) {
            assert_eq!(r.vals.values_of(v), Some(vec![0, 1]), "state:{x}: its old value or 0");
        } else {
            assert_eq!(v, Val::Int(0), "state:{x}: a fixed place");
        }
    }
    // switch off: model.py's rule, no objpos and no clearing
    let mut m = Machine::new(g, features(0), Switches::default());
    let r = run_at(&mut m, &o, entry, 0x01F7, vec![], 1);
    let p = &r.paths[0];
    assert_eq!(w(p, Fact::ObjPos(674)), None);
    assert!(at00.iter().all(|&x| w(p, Fact::State(x)).is_none()));
}

/// Loading the forest (Mêlée's fork, object 911, walks the ego into pseudo room 218) puts every movable object of
/// resource 58 back at its CDHD place (room.cpp:195) before the entry code moves some of them.
#[test]
fn f5_room_load_resets_places() {
    let g = game();
    let facts = [(Fact::Room, 85), (Fact::Var(1), 1), (Fact::Ego, 1), (Fact::Owner(911), 15), (Fact::Owner(687), 15)];
    let o = oracle(&facts, Some(GROUND_TRUTH));
    let movable: Vec<i32> = g.room_objects[&58].iter().copied().filter(|o| g.movable.contains_key(o)).collect();
    assert!(!movable.is_empty());
    let mut m = Machine::new(g, features(2), switches(&["draw_clears"]));
    let r = m.run_entry(&o, 911, 11, None);
    let entered: Vec<&Path> = r.paths.iter().filter(|p| p.writes.get(&Fact::Room) == Some(&Val::Int(218))).collect();
    assert!(!entered.is_empty(), "a path enters the forest");
    for p in entered {
        for &ob in &movable {
            assert!(w(p, Fact::ObjPos(ob)).is_some(), "objpos:{ob} written at the room load");
        }
    }
    let mut m = Machine::new(g, features(2), Switches::default());
    let r = m.run_entry(&o, 911, 11, None);
    assert!(r.paths.iter().all(|p| movable.iter().all(|&ob| w(p, Fact::ObjPos(ob)).is_none())));
}
