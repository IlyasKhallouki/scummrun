//! E014's switch FMI_READ_WIDEN (`Machine::widen_loops`, machine/widen.rs; experiments/E014-fate-demo/plan.md,
//! amendment (6)) on hand-made units, run with the reader's features (rows 0-2: joins and merges, no settle):
//!
//! * a loop whose exit tests a value the oracle does not hold passes the path limit with the switch off; on, it ends
//!   under the limit: its first iteration's way out keeps exact values, the widened iteration's has plain unknowns
//!   where the loop moved, and the path that came round again ends "widened";
//! * a loop whose every test decides runs exactly with the switch on (one path, the counter's last value);
//! * a run with no loop is the same with the switch on and off.

use scumm_ir::{Fact, FactMap, Game, GameData, Script, UnitKey};
use scumm_machine::path::Owned;
use scumm_machine::value::Val;
use scumm_machine::{FactOracle, Features, Machine, Run, Switches};

// Local[0] counts the iterations (known); Local[1] starts at a random draw (unknown) and steps; the exit compares it
// with another draw.
const UNKNOWN_EXIT: &str = "[0000] (1A) Local[0] = 0;\n\
                            [0005] (16) Local[1] = getRandomNr(10);\n\
                            [0009] (16) Local[2] = getRandomNr(10);\n\
                            [000D] (46) Local[1]++;\n\
                            [0010] (46) Local[0]++;\n\
                            [0013] (9A) Var[301] = Local[0];\n\
                            [0018] (C4) unless (Local[1] > Local[2]) goto 000D;\n\
                            [001F] (1A) Var[303] = 1;\n\
                            [0024] (A0) stopObjectCode();\n\
                            END";

const KNOWN_EXIT: &str = "[0000] (1A) Local[0] = 0;\n\
                          [0005] (46) Local[0]++;\n\
                          [0008] (9A) Var[301] = Local[0];\n\
                          [000D] (44) unless (Local[0] > 5) goto 0005;\n\
                          [0014] (1A) Var[303] = 1;\n\
                          [0019] (A0) stopObjectCode();\n\
                          END";

const NO_LOOP: &str = "[0000] (A8) if (Local[1]) /*000C*/ {\n\
                       [0005] (1A)   Var[301] = 1;\n\
                       [000A] (18) } else /*0011*/ {\n\
                       [000C] (1A)   Var[301] = 2;\n\
                       [0011] (**) }\n\
                       [0011] (1A) Var[303] = 1;\n\
                       [0016] (A0) stopObjectCode();\n\
                       END";

fn game(text: &str) -> Game {
    let mut sc = Script::new();
    sc.add_unit(UnitKey::Global(1), None, "test.txt", "", text, None, None);
    Game::new(sc, GameData { first_local: 200, ..Default::default() })
}

fn run(g: &Game, widen: bool) -> Run {
    let names: Vec<String> = ["own_entry", "dispatch", "calls", "room_change"].iter().map(|s| s.to_string()).collect();
    let mut m = Machine::new(g, Features::parse(&names).unwrap(), Switches::default());
    m.widen_loops = widen;
    let mut facts = FactMap::default();
    facts.insert(Fact::Var(301), 0);
    facts.insert(Fact::Var(303), 0);
    m.run(&FactOracle::new(facts, None), UnitKey::Global(1), 0, Some(vec![]), Owned::Bool(false), Some(false))
}

fn ends(r: &Run, end: &str) -> usize {
    r.paths.iter().filter(|p| p.end == Some(end)).count()
}

fn var(r: &Run, p: usize, n: i32) -> Option<Val> {
    r.paths[p].writes.get(&Fact::Var(n)).copied()
}

/// The paths that left the loop (they set Var[303]).
fn exits(r: &Run) -> Vec<usize> {
    (0..r.paths.len()).filter(|&i| var(r, i, 303) == Some(Val::Int(1))).collect()
}

#[test]
fn an_unknown_exit_passes_the_limit_unwidened() {
    let g = game(UNKNOWN_EXIT);
    let r = run(&g, false);
    assert!(r.limit > 0, "the loop runs on to the path limit");
    assert_eq!(ends(&r, "widened"), 0);
}

#[test]
fn an_unknown_exit_ends_widened() {
    let g = game(UNKNOWN_EXIT);
    let r = run(&g, true);
    assert_eq!(r.limit, 0, "under the limit");
    assert!(ends(&r, "widened") >= 1, "the path that came round again ends");
    let out = exits(&r);
    assert!(out.len() >= 2, "a way out of the first iteration and of the widened one: {}", out.len());
    // the first iteration's way out keeps the counter exact; a later one holds a plain unknown
    let counts: Vec<Option<Val>> = out.iter().map(|&i| var(&r, i, 301)).collect();
    assert!(counts.contains(&Some(Val::Int(1))), "{counts:?}");
    let widened: Vec<&Option<Val>> = counts.iter().filter(|v| matches!(v, Some(Val::Unk(_)))).collect();
    assert!(!widened.is_empty(), "{counts:?}");
    for v in widened {
        let Some(Val::Unk(u)) = v else { unreachable!() };
        let text = r.vals.show(Val::Unk(*u));
        assert!(text.contains("varies around the loop"), "{text}");
    }
}

#[test]
fn a_known_exit_runs_exactly() {
    let g = game(KNOWN_EXIT);
    for widen in [false, true] {
        let r = run(&g, widen);
        assert_eq!(r.limit, 0);
        assert_eq!(ends(&r, "widened"), 0);
        let out = exits(&r);
        assert_eq!(out.len(), 1, "one path");
        assert_eq!(var(&r, out[0], 301), Some(Val::Int(6)), "the counter's last value");
    }
}

#[test]
fn no_loop_is_the_same() {
    let g = game(NO_LOOP);
    let (a, b) = (run(&g, false), run(&g, true));
    assert_eq!(a.paths.len(), b.paths.len());
    for (p, q) in a.paths.iter().zip(&b.paths) {
        assert_eq!(p.end, q.end);
        assert_eq!(p.writes, q.writes);
    }
}
