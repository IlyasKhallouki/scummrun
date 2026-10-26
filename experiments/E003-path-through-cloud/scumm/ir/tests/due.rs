//! Fix round 1, F2: the wait classes (`due.rs`) on hand-made units, and their counts over the whole game.

use scumm_ir::due::{exits, loop_facts, poll_shape, survey, wait_class, WaitClass};
use scumm_ir::{default_root, Fact, Game, GameData, Script, UnitKey};

fn game(units: &[(UnitKey, &str)]) -> Game {
    let mut sc = Script::new();
    for (k, text) in units {
        sc.add_unit(*k, None, "test.txt", "", text, None, None);
    }
    Game::new(sc, GameData { first_local: 200, ..Default::default() })
}

/// `while (Var[5] == 0) breakHere;` then a write: the wait after the breakHere is polled on var:5, and the turn
/// leaves the loop exactly when Var[5] is not 0.
#[test]
fn polled_loop() {
    let text = "[0000] (48) if (Var[5] == 0) /*000B*/ {\n\
                [0007] (80)   breakHere();\n\
                [0008] (18)   goto 0000;\n\
                [000B] (**) }\n\
                [000B] (1A) Var[6] = 1;\n\
                [000E] (A0) stopObjectCode();";
    let g = game(&[(UnitKey::Global(50), text)]);
    let u = g.id(UnitKey::Global(50)).unwrap();
    let idx = g.at(u, 0x0008).unwrap();
    let c = wait_class(&g, u, idx, true);
    let WaitClass::Polled(li) = c else { panic!("{c:?}") };
    let lp = &g.loops(u, true)[li as usize];
    assert_eq!(loop_facts(lp), vec![Fact::Var(5)]);
    assert_eq!(poll_shape(&g, u, idx, lp), Ok(()));
    for (v, want) in [(0, false), (1, true), (-3, true), (0x10000, false)] {
        assert_eq!(exits(&g, u, idx, lp, &|k| (k == Fact::Var(5)).then_some(v)), Ok(want), "Var[5] = {v}");
    }
    assert!(exits(&g, u, idx, lp, &|_| None).is_err(), "a missing value is an error, not a guess");
}

/// A loop that resets what it tests before it yields (const_polls) is polled with the switch on; the turn still
/// reads the value another script wrote while it waited.
#[test]
fn const_poll_loop() {
    let text = "[0000] (1A) Var[7] = 0;\n\
                [0003] (80) breakHere();\n\
                [0004] (48) if (Var[7] == 0) /*000E*/ {\n\
                [000B] (18)   goto 0000;\n\
                [000E] (**) }\n\
                [000E] (A0) stopObjectCode();";
    let g = game(&[(UnitKey::Global(51), text)]);
    let u = g.id(UnitKey::Global(51)).unwrap();
    let idx = g.at(u, 0x0004).unwrap();
    assert!(matches!(wait_class(&g, u, idx, false), WaitClass::Time(_)), "a counter without const_polls");
    let WaitClass::Polled(li) = wait_class(&g, u, idx, true) else { panic!() };
    let lp = &g.loops(u, true)[li as usize];
    assert_eq!(exits(&g, u, idx, lp, &|_| Some(0)), Ok(false));
    assert_eq!(exits(&g, u, idx, lp, &|_| Some(2)), Ok(true));
}

/// A breakHere in no loop is plain, a delay is time, a loop on a local is time, a loop with no exit is ambient,
/// and the place after an ordinary opcode is no wait.
#[test]
fn other_classes() {
    let text = "[0000] (80) breakHere();\n\
                [0001] (2E) delay(10);\n\
                [0005] (1A) Local[0] = 1;\n\
                [0008] (48) if (Local[0] == 1) /*0013*/ {\n\
                [000F] (80)   breakHere();\n\
                [0010] (18)   goto 0008;\n\
                [0013] (**) }\n\
                [0013] (80) breakHere();\n\
                [0014] (18) goto 0013;";
    let g = game(&[(UnitKey::Global(52), text)]);
    let u = g.id(UnitKey::Global(52)).unwrap();
    let at = |o: i32| g.at(u, o).unwrap();
    assert_eq!(wait_class(&g, u, at(0x0001), true), WaitClass::Plain);
    assert!(matches!(wait_class(&g, u, at(0x0005), true), WaitClass::Time("a delay")));
    assert!(matches!(wait_class(&g, u, at(0x0010), true), WaitClass::Time(_)));
    assert_eq!(wait_class(&g, u, at(0x0014), true), WaitClass::Ambient);
    assert_eq!(wait_class(&g, u, at(0x0008), true), WaitClass::None);
}

/// An opcode between the wait and the exit test: polled by its loop's reads, but not a pure poll.
#[test]
fn not_a_pure_poll() {
    let text = "[0000] (80) breakHere();\n\
                [0001] (25) animateCostume(3,5);\n\
                [0004] (48) if (Bit[9]) /*000E*/ {\n\
                [000B] (18)   goto 0000;\n\
                [000E] (**) }\n\
                [000E] (A0) stopObjectCode();";
    let g = game(&[(UnitKey::Global(53), text)]);
    let u = g.id(UnitKey::Global(53)).unwrap();
    let idx = g.at(u, 0x0001).unwrap();
    let WaitClass::Polled(li) = wait_class(&g, u, idx, true) else { panic!("{:?}", wait_class(&g, u, idx, true)) };
    assert_eq!(poll_shape(&g, u, idx, &g.loops(u, true)[li as usize]), Err("an opcode before the exit test"));
}

/// The counts over the game's every wait point (printed: the cost the design asks for first, F2's "counted in the
/// ir crate first"), and every pure poll decides with all its facts at 0 and at 1.
#[test]
fn game_survey() {
    let g = Game::load(&default_root()).unwrap();
    for cp in [false, true] {
        let s = survey(&g, cp);
        let total: usize = s.iter().map(|x| x.1).sum();
        println!("const_polls {cp}: {total} wait points");
        for (k, n) in &s {
            println!("  {n:6} {k}");
        }
        assert!(s.iter().any(|(k, _)| k == "polled: a pure poll"));
    }
    for u in g.ids() {
        for idx in 0..=g.ins(u).len() as u32 {
            if let WaitClass::Polled(li) = wait_class(&g, u, idx, true) {
                let lp = &g.loops(u, true)[li as usize];
                if poll_shape(&g, u, idx, lp).is_ok() {
                    for v in [0, 1] {
                        let got = exits(&g, u, idx, lp, &|_| Some(v));
                        assert!(got.is_ok(), "{}: {got:?}", g.cite(u, idx.min(g.ins(u).len() as u32 - 1)));
                    }
                }
            }
        }
    }
}
