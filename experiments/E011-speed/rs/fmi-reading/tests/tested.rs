//! E012 gate T1 (experiments/E012-game-mechanics/plan.md, component `tested`, and its amendment) on the kept kitchen
//! reading 28-33-41-j00065: the reading's directory in `FMI_T1_READING` (the archive's
//! `/var/tmp/fast-mi/e011-archives/kitchen/readings/28-33-41-j00065`, or a folder of links to its files). Without it
//! the test says so and passes: the reading is not in the repository.
//!
//! * the plan's numbers: 60 keys tested by inputs; 8,201 actions with a check on a random draw; 8,207 such checks and
//!   405 on a timer;
//! * the event keys and the keys in all by the amendment's rule (an object event's slot is `objscript:O`): 46 and 97;
//!   the plan's probe kept an object event's own slot key, and with those keys added back its 49 and 100 come out;
//! * `Tested::of` (any ReadingSource) equals `Tested::of_reading` (the preconditions decoded alone);
//! * every draw check's site resolved from the game agrees with the reader's own text of the check.

use fmi_core::{Key, ReadingSource};
use fmi_reading::tested::{draws, own_unit_key, source_of, DrawGame, Source, Tested};
use fmi_reading::Reading;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn reading() -> Option<Reading> {
    let Some(d) = std::env::var_os("FMI_T1_READING").map(PathBuf::from) else {
        eprintln!("FMI_T1_READING is not set: T1 not run");
        return None;
    };
    Some(Reading::open(&d).expect("the T1 reading"))
}

#[test]
fn t1() {
    let Some(rd) = reading() else { return };
    let t = Tested::of_reading(&rd);
    let c = &t.counts;
    assert_eq!(c.actions, 39_661);
    assert_eq!((c.events, c.sentences, c.verbs, c.other), (23_781, 15_712, 168, 0));
    assert_eq!(c.input_keys, 60, "keys tested by inputs");
    assert_eq!(c.draw_actions, 8_201, "actions with a draw check");
    assert_eq!(c.checks.get("random draw"), Some(&8_207));
    assert_eq!(c.checks.get("timer"), Some(&405));
    assert_eq!((c.event_keys, c.both_keys, c.keys), (46, 9, 97), "the amendment's rule");
    assert_eq!(t.goal, vec![Key::parse("owner:566")]);
    assert!(t.has(Key::parse("owner:566")) && t.has(Key::parse("state:316")) && !t.has(Key::parse("var:100")));
    assert_eq!(c.atoms, t.flat().len());

    // the plan's probe: an object event's own slot key (objscript:O) kept among the event keys
    let mut kept: BTreeSet<Key> = BTreeSet::new();
    for i in 0..rd.n_actions() {
        let r = rd.record(i);
        if let Source::Event { unit, .. } = source_of(&r.source) {
            if unit.0 == fmi_core::U_OBJ {
                for (k, _) in r.pre {
                    if k.kind() == fmi_core::K_OBJSCRIPT && own_unit_key(k, unit) && !t.events.contains(&k) {
                        kept.insert(k);
                    }
                }
            }
        }
    }
    let texts: Vec<String> = kept.iter().map(|k| k.text()).collect();
    assert_eq!(texts, ["objscript:315", "objscript:316", "objscript:339"]);
    let all: BTreeSet<Key> = t.keys().into_iter().chain(kept.iter().copied()).collect();
    assert_eq!((c.event_keys + kept.len(), all.len()), (49, 100), "the plan's numbers under its probe's rule");
}

#[test]
fn of_equals_of_reading() {
    let Some(rd) = reading() else { return };
    assert_eq!(Tested::of(&rd), Tested::of_reading(&rd));
    for i in (0..rd.n_actions()).step_by(97) {
        assert_eq!(rd.pre_of(i), rd.action(i).pre, "action {i}");
    }
}

#[test]
fn draw_sites_agree_with_the_reader() {
    let Some(rd) = reading() else { return };
    let g = DrawGame::load(&scumm_ir::default_root()).expect("the game");
    let (mut checks, mut actions) = (0, 0);
    for i in 0..rd.n_actions() {
        let ds = draws(&rd, i, Some(&g));
        actions += !ds.is_empty() as usize;
        for d in &ds {
            checks += 1;
            if let Ok(s) = g.site(&d.cite) {
                assert_eq!(format!("?(random draw: {})", s.what), d.why, "action {i}: {}", d.cite);
                assert_eq!(d.range, Some((0, s.n)));
            }
        }
        assert_eq!(draws(&rd, i, None).len(), ds.len());
    }
    assert_eq!((actions, checks), (8_201, 8_207));
}
