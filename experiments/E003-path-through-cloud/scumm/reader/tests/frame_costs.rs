//! The switch --frame-costs (costs.rs): the cost rule on the kitchen's scope (rooms 28 and 41, start s17), the wait
//! points it reads from the scripts, and the costs of the kitchen's golden records. MI1's numbers appear only here.
//!
//! Fast: the scope's clock and the wait points. Slow (`--ignored`): every golden record of the kitchen costed (the
//! counts by kind and cost are printed with `--nocapture`).

use scumm_reader::costs::{delay_frames, FrameCosts};
use scumm_reader::oracle::{self, Static};
use scumm_reader::record::What;
use scumm_ir::UnitKey;
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

fn kitchen_costs() -> FrameCosts {
    let st = Static::load();
    let b = oracle::builder_with(game(), &st, "kitchen", Default::default());
    FrameCosts::of(game(), &b.scope)
}

#[test]
fn kitchen_clock_and_wait_points() {
    let fc = kitchen_costs();
    // the start s17 (run B's S1#7 pre): VAR_TIMER_NEXT 5 (the logo glitch), VAR_CHARINC 0 (talkspeed 255); no unit
    // of the scope writes either
    assert_eq!(fc.dmax, 5);
    assert_eq!(fc.charinc, Some(0));
    // the cook's timer, room 28 local 211: [000D] Local[0] = ((getRandomNr(20) + 30) * 60); [001E] delayVariable
    let w = fc.wait_point(UnitKey::Local(28, 211), 0x21).unwrap();
    assert_eq!(w, "delayVariable(local Some(0)): least Some(1800)");
    // its loop: [0003] breakHere(), resumed at [0004]
    assert_eq!(fc.wait_point(UnitKey::Local(28, 211), 4).unwrap(), "breakHere: 1");
    // local 214: [000B] delay(30) -> [000F]: floor(30 / 5) + 1 = 7 frames
    assert_eq!(fc.wait_point(UnitKey::Local(28, 214), 0x0F).unwrap(), "delay: 7");
    // [000F] print(255,[..,Text("Hey!" + wait() + "You can't come back here!")]); [0039] WaitForMessage():
    // two parts of 60 jiffies, 12 frames each at 5 a frame, and the frame VAR_HAVE_MSG is copied: 25
    assert_eq!(fc.wait_point(UnitKey::Local(28, 214), 0x39).unwrap(), "message: 25");
    // local 216's walks: [0056] WaitForActor(6): 1 frame
    assert_eq!(fc.wait_point(UnitKey::Local(28, 216), 0x56).unwrap(), "wait: 1");
    assert_eq!(delay_frames(1800, 5), 361);
}

#[test]
#[ignore]
fn kitchen_golden_records_costed() {
    let fc = kitchen_costs();
    let (recs, bad) = oracle::golden_records(&oracle::golden_dir().join("kitchen.jsonl.gz"));
    assert!(bad.is_empty());
    let mut by_kind: BTreeMap<(String, u32), usize> = BTreeMap::new();
    let mut labels: BTreeMap<(String, String), (u32, u32)> = BTreeMap::new();
    for r in &recs {
        let c = fc.cost(r);
        assert!(c >= 1, "{}: cost {c}", r.label);
        *by_kind.entry((r.kind.clone(), c.min(30))).or_default() += 1;
        if let What::Event(e) = &r.source {
            if e.name == "local:211" && e.w == 0x21 {
                let x = labels.entry((format!("{:?}", e.unit), r.label.clone())).or_insert((c, c));
                *x = (x.0.min(c), x.1.max(c));
            }
        }
    }
    for ((k, c), n) in &by_kind {
        println!("{k:14} cost {c:>3}{}: {n}", if *c == 30 { "+" } else { "" });
    }
    println!("local 211 at its delay: {labels:?}");
    // every event of the cook's timer (room 28's local 211) at [0021] costs at least floor(1800 / 5) + 1
    assert!(labels.iter().filter(|(k, _)| k.0 == "Local(28, 211)").all(|(_, c)| c.0 >= 361), "{labels:?}");
}
