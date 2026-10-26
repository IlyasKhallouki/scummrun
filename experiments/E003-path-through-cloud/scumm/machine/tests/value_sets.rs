//! Fix round 1, F12 (switch value_sets; fix-round1.md): the design's synthetic cases on hand-made units, each run as
//! a click on a verb whose input script is the unit (row 4's run_verb), with the switch off (model.py's rules) and on.
//!
//! * a comparison chain on findObject: on, the chain forks once per object and a later test on the same local is
//!   decided on every path; off, every test forks again;
//! * a mouse division: on, the mouse is a range, a test the whole range agrees on is decided, a test it does not
//!   splits it at the bound, and the band (y - 152) / 24 of the upper part is [0..1];
//! * a walk to the mouse: on, the walk, the put and a stored x fork nothing; the stored x is a non-fact range.

use scumm_ir::{Fact, FactMap, Game, GameData, Script, UnitKey};
use scumm_machine::value::Val;
use scumm_machine::{FactOracle, Features, Machine, Run, Switches};

const VERB: i32 = 120;
const ROOM: i32 = 5;

fn game(units: &[(i32, &str)]) -> Game {
    let mut sc = Script::new();
    for &(n, text) in units {
        sc.add_unit(UnitKey::Global(n), None, "test.txt", "", text, None, None);
    }
    let mut data = GameData { first_local: 200, ..Default::default() };
    for (o, r) in [(101, ROOM), (102, ROOM), (103, ROOM), (104, 6)] {
        data.object_room.insert(o, r);
    }
    data.room_width.insert(ROOM, 640);
    data.room_width.insert(6, 320);
    Game::new(sc, data)
}

/// A left click on verb VERB with input script `script`, row 4's features, the given switches.
fn click(g: &Game, script: i32, switches: &str) -> Run {
    let names = |s: &str| s.split(',').filter(|x| !x.is_empty()).map(String::from).collect::<Vec<_>>();
    let features = Features::parse(&names("own_entry,dispatch,calls,room_change,settle,dialog")).unwrap();
    let mut m = Machine::new(g, features, Switches::parse(&names(switches)).unwrap());
    let mut facts = FactMap::default();
    facts.insert(Fact::Var(32), script); // VAR_VERB_SCRIPT
    facts.insert(Fact::Var(53), 1); // VAR_USERPUT
    facts.insert(Fact::Verb(VERB), 1);
    facts.insert(Fact::Room, ROOM);
    m.run_verb(&FactOracle::new(facts, None), VERB, 1)
}

fn var(r: &Run, p: usize, n: i32) -> Option<Val> {
    r.paths[p].writes.get(&Fact::Var(n)).copied()
}

/// The conditions a path assumed (forked on), as printed.
fn conds(r: &Run, p: usize) -> Vec<String> {
    r.paths[p].assumptions.iter().map(|a| a.cond.to_string()).collect()
}

/// Every condition a run's paths assumed (forked on), as printed.
fn all_conds(r: &Run) -> Vec<String> {
    let mut v: Vec<String> = (0..r.paths.len()).flat_map(|p| conds(r, p)).collect();
    v.sort();
    v.dedup();
    v
}

// The chain jumps out of each branch, so its paths join only after it (0038): the zero test at 002E runs on the
// path where no branch was taken.
const CHAIN: &str = "[0000] (F5) Local[0] = findObject(VAR_VIRT_MOUSE_X,VAR_VIRT_MOUSE_Y);\n\
                     [0007] (48) if (Local[0] == 101) /*0014*/ {\n\
                     [000E] (1A)   Var[300] = 1;\n\
                     [0011] (18)   goto 0038;\n\
                     [0014] (**) }\n\
                     [0014] (48) if (Local[0] == 102) /*0021*/ {\n\
                     [001B] (1A)   Var[300] = 2;\n\
                     [001E] (18)   goto 0038;\n\
                     [0021] (**) }\n\
                     [0021] (48) if (Local[0] == 103) /*002E*/ {\n\
                     [0028] (1A)   Var[300] = 3;\n\
                     [002B] (18)   goto 0038;\n\
                     [002E] (**) }\n\
                     [002E] (A8) if (Local[0]) /*0038*/ {\n\
                     [0033] (1A)   Var[301] = 1;\n\
                     [0038] (**) }\n\
                     [0038] (D5) Local[3] = actorFromPos(VAR_VIRT_MOUSE_X,VAR_VIRT_MOUSE_Y);\n\
                     [003F] (48) if (Local[3] > 12) /*0049*/ {\n\
                     [0046] (1A)   Var[302] = 1;\n\
                     [0049] (**) }\n\
                     [0049] (A0) stopObjectCode();";

#[test]
fn find_object_chain() {
    let g = game(&[(10, CHAIN)]);
    let on = click(&g, 10, "value_sets");
    let c = all_conds(&on);
    // findObject is 0 or one of the room's three objects: the chain forks on each, and where no branch was taken
    // the value left is 0, so the zero test is decided (no fork), and Var[301] is never written
    for t in ["Local[0] == 101", "Local[0] == 102", "Local[0] == 103"] {
        assert!(c.contains(&t.to_string()), "{c:?}");
    }
    assert!(!c.contains(&"Local[0]".to_string()), "on: the zero test is decided: {c:?}");
    assert!((0..on.paths.len()).all(|p| var(&on, p, 301).is_none()));
    // actorFromPos is 0 or an actor below _numActors (13): never above 12
    assert!(!c.contains(&"Local[3] > 12".to_string()), "{c:?}");
    assert!((0..on.paths.len()).all(|p| var(&on, p, 302).is_none()));
    let off = click(&g, 10, "");
    let c = all_conds(&off);
    assert!(c.contains(&"Local[0]".to_string()), "off: the zero test forks: {c:?}");
    assert!(c.contains(&"Local[3] > 12".to_string()), "off: actorFromPos is any value: {c:?}");
}

const DIVISION: &str = "[0000] (48) if (VAR_MOUSE_X < 0) /*000A*/ {\n\
                        [0007] (1A)   Var[304] = 1;\n\
                        [000A] (**) }\n\
                        [000A] (48) if (VAR_MOUSE_Y > 152) /*0030*/ {\n\
                        [0011] (AC)   Exprmode Local[2] = ((VAR_MOUSE_Y - 152) / 24);\n\
                        [001A] (48)   if (Local[2] > 1) /*0024*/ {\n\
                        [0021] (1A)     Var[302] = 9;\n\
                        [0024] (**)   }\n\
                        [0024] (1A)   Var[303] = Local[2];\n\
                        [0030] (**) }\n\
                        [0030] (A0) stopObjectCode();";

#[test]
fn mouse_division() {
    let g = game(&[(11, DIVISION)]);
    let on = click(&g, 11, "value_sets");
    // VAR_MOUSE_X < 0 never holds (the mouse is clipped to the screen); VAR_MOUSE_Y > 152 splits [0..199] at 152, the
    // one check on where to click; in the upper part the band (y - 152) / 24 is 0 or 1, so Local[2] > 1 is decided
    assert_eq!(all_conds(&on), vec!["VAR_MOUSE_Y > 152".to_string()]);
    for p in 0..on.paths.len() {
        assert_eq!(var(&on, p, 304), None);
        assert_eq!(var(&on, p, 302), None);
        let band = var(&on, p, 303).unwrap();
        assert_eq!(on.vals.range(band), Some((0, 1)), "the band joined with the other side's 0");
    }
    let off = click(&g, 11, "");
    let c = all_conds(&off);
    for t in ["VAR_MOUSE_X < 0", "VAR_MOUSE_Y > 152", "Local[2] > 1"] {
        assert!(c.contains(&t.to_string()), "off: {t} forks: {c:?}");
    }
}

const WALK: &str = "[0000] (1E) walkActorTo(1,VAR_VIRT_MOUSE_X,VAR_VIRT_MOUSE_Y);\n\
                    [0005] (01) putActor(11,VAR_VIRT_MOUSE_X,VAR_VIRT_MOUSE_Y);\n\
                    [000A] (1A) Var[164] = VAR_VIRT_MOUSE_X;\n\
                    [000F] (A0) stopObjectCode();";

#[test]
fn walk_to_mouse() {
    let g = game(&[(12, WALK)]);
    for sw in ["value_sets", ""] {
        let r = click(&g, 12, sw);
        assert_eq!(r.paths.len(), 1, "{sw}");
        let p = &r.paths[0];
        assert!(p.assumptions.is_empty() && p.unknown_writes.is_empty(), "{sw}: a walk or put to the mouse forks and writes nothing");
        let x = var(&r, 0, 164).unwrap();
        assert!(r.vals.is_plain(x), "{sw}: the stored x is a non-fact");
        // on: the click's room's x (640 wide); off: model.py's unknown
        assert_eq!(r.vals.range(x), if sw.is_empty() { None } else { Some((0, 639)) }, "{sw}");
    }
}

/// The main screen's left edge by the room's width (camera.cpp:169-192): 0 up to the width less the screen's, and a
/// room narrower than the screen can start left of 0 (its camera is clamped to its width less 160).
#[test]
fn screen_lefts() {
    use scumm_machine::machine::screen_lefts;
    assert_eq!(screen_lefts(320), (0, 0));
    assert_eq!(screen_lefts(640), (0, 320));
    assert_eq!(screen_lefts(1008), (0, 688));
    assert_eq!(screen_lefts(8), (-312, 0));
}
