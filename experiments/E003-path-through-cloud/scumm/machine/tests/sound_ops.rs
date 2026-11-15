//! E014 amendment (8)'s switch FMI_READ_SOUND (`Machine::sound_ops`, heads.rs `effect_with`) on a hand-made unit, run
//! with the reader's features (rows 0-2): a script that sends a sound command, then waits in a variable delay.
//!
//! * off, the sound command has no rule: the path ends "unmodelled" there (the reader drops such a path);
//! * on, it has no tracked effect: the path goes on and parks at the delay, its write before it kept;
//! * the other ops keep their effect either way (a yield is a yield).

use scumm_ir::{known, Fact, FactMap, Game, GameData, Script, UnitKey};
use scumm_machine::heads::{effect, effect_with, Effect};
use scumm_machine::path::Owned;
use scumm_machine::value::Val;
use scumm_machine::{FactOracle, Features, Machine, Run, Switches};

const SOUND_THEN_DELAY: &str = "[0000] (1A) Local[1] = 60;\n\
                                [0005] (1A) Var[301] = 1;\n\
                                [000A] (4C) soundKludge([268,80,0,Local[1]]);\n\
                                [0018] (2B) delayVariable(Local[1]);\n\
                                [001B] (1A) Var[302] = 1;\n\
                                [0020] (A0) stopObjectCode();\n\
                                END";

fn game(text: &str) -> Game {
    let mut sc = Script::new();
    sc.add_unit(UnitKey::Global(1), None, "test.txt", "", text, None, None);
    Game::new(sc, GameData { first_local: 200, ..Default::default() })
}

fn run(g: &Game, sound: bool) -> Run {
    let names: Vec<String> = ["own_entry", "dispatch", "calls", "room_change"].iter().map(|s| s.to_string()).collect();
    let mut m = Machine::new(g, Features::parse(&names).unwrap(), Switches::default());
    m.sound_ops = sound;
    let mut facts = FactMap::default();
    facts.insert(Fact::Var(301), 0);
    facts.insert(Fact::Var(302), 0);
    m.run(&FactOracle::new(facts, None), UnitKey::Global(1), 0, Some(vec![]), Owned::Bool(false), Some(false))
}

#[test]
fn off_a_sound_command_ends_the_path_unmodelled() {
    let g = game(SOUND_THEN_DELAY);
    let r = run(&g, false);
    assert_eq!(r.paths.len(), 1);
    assert_eq!(r.paths[0].end, Some("unmodelled"));
}

#[test]
fn on_the_path_passes_it_and_parks_at_the_delay() {
    let g = game(SOUND_THEN_DELAY);
    let r = run(&g, true);
    assert_eq!(r.paths.len(), 1);
    let p = &r.paths[0];
    assert_eq!(p.end, Some("yield"), "parked at the variable delay");
    assert!(p.unmodelled.is_empty());
    assert_eq!(p.writes.get(&Fact::Var(301)).copied(), Some(Val::Int(1)), "the write before the sound command");
    assert_eq!(p.writes.get(&Fact::Var(302)).copied(), None, "nothing past the delay");
}

#[test]
fn the_table_with_and_without_the_switch() {
    for name in [known::SOUND_KLUDGE, known::SET_ROOM_SHADOW, known::DEBUG] {
        assert_eq!(effect(name), Effect::Unmodelled("no rule"));
        assert_eq!(effect_with(name, false), Effect::Unmodelled("no rule"));
        assert_eq!(effect_with(name, true), Effect::None);
    }
    for (name, e) in [(known::DELAY_VARIABLE, Effect::Yield), (known::START_SOUND, Effect::None)] {
        assert_eq!(effect_with(name, false), e);
        assert_eq!(effect_with(name, true), e);
    }
    assert_eq!(effect_with(known::SYSTEM_OPS, true), Effect::Unmodelled("restart, pause or quit"));
}
