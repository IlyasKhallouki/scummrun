//! E012's component `end` (patch 0005): the Boxes reply's decoding, the game's end reasons and the archive chains
//! (plain tests), and the hooks and the Boxes command on a patch 0005 build (`cargo test --release -- --ignored`,
//! FMI_ENGINE_BIN a build with patch 0005).

use fmi_core::*;
use fmi_engine::archive::{self, Entry};
use fmi_engine::wire::{R, W};
use fmi_engine::{Host, HostCfg};
use std::collections::BTreeMap;

/// A Boxes payload as the engine writes it (speedrun_wire.cpp wireBoxes): room, u32 count, per box 8 x i32, u8.
fn payload(room: i32, boxes: &[WalkBox]) -> Vec<u8> {
    let mut w = W::msg(0x12);
    w.i32(room);
    w.u32(boxes.len() as u32);
    for b in boxes {
        for p in [b.ul, b.ur, b.lr, b.ll] {
            w.i32(p.0);
            w.i32(p.1);
        }
        w.u8(b.flags);
    }
    w.finish()[5..].to_vec()
}

#[test]
fn boxes_decode_field_for_field() {
    let want = Boxes {
        room: 28,
        boxes: vec![
            WalkBox {
                ul: (-32000, -32000),
                ur: (-32000, -32000),
                lr: (-32000, -32000),
                ll: (-32000, -32000),
                flags: 0,
            },
            WalkBox {
                ul: (70, 124),
                ur: (98, 124),
                lr: (98, 135),
                ll: (43, 135),
                flags: 0x80,
            },
        ],
    };
    let p = payload(28, &want.boxes);
    let mut r = R::new(&p);
    assert_eq!(r.boxes().unwrap(), want);
    r.done().unwrap();
    // an empty room (no BOXD): the room and a zero count
    let p = payload(10, &[]);
    let mut r = R::new(&p);
    assert_eq!(r.boxes().unwrap(), Boxes { room: 10, boxes: vec![] });
    // a count past the payload is refused, not read
    let mut p = payload(28, &want.boxes);
    p[4] = 3;
    assert!(R::new(&p).boxes().is_err());
}

#[test]
fn game_ends_are_quit_restart_pause() {
    for r in ["quit", "restart", "pause"] {
        assert!(is_game_end(r), "{r}");
    }
    for r in ["goal", "error", "max_ticks", "frame", "forked", "plan_exhausted", "dump_done", ""] {
        assert!(!is_game_end(r), "{r}");
    }
}

fn ent(id: u64, parent: Option<u64>, frame: u64, ev: &[u64], cps: &[u64]) -> Entry {
    Entry {
        id,
        parent,
        frame,
        events: ev
            .iter()
            .map(|&f| Event {
                frame: f,
                what: Input::Click(f as i32, 1),
            })
            .collect(),
        checkpoints: cps
            .iter()
            .map(|&f| Checkpoint {
                frame: f,
                facts: Facts::new(),
                tick: 0,
                rng: 0,
                actors: vec![],
            })
            .collect(),
    }
}

#[test]
fn chains_concatenate_root_first() {
    let mut all = BTreeMap::new();
    all.insert(0, ent(0, None, 2728, &[], &[2728]));
    all.insert(1, ent(1, Some(0), 2767, &[2736, 2767], &[2767]));
    all.insert(2, ent(2, Some(1), 2818, &[2768], &[2768, 2818]));
    all.insert(3, ent(3, Some(2), 2900, &[], &[2900]));
    let c = archive::chained(&all, 3).unwrap();
    assert_eq!(c.frame, 2900);
    assert_eq!(c.events.iter().map(|e| e.frame).collect::<Vec<_>>(), vec![2736, 2767, 2768]);
    assert_eq!(c.checkpoints.iter().map(|e| e.frame).collect::<Vec<_>>(), vec![2728, 2767, 2768, 2818, 2900]);
    let r = archive::chained(&all, 0).unwrap();
    assert!(r.events.is_empty());
    assert_eq!(r.checkpoints.len(), 1);
    // a missing parent and a loop are errors
    all.insert(4, ent(4, Some(9), 3000, &[], &[]));
    assert!(archive::chained(&all, 4).is_err());
    all.insert(5, ent(5, Some(6), 3000, &[], &[]));
    all.insert(6, ent(6, Some(5), 3000, &[], &[]));
    assert!(archive::chained(&all, 5).is_err());
}

fn cfg(out: &str, frame: Option<u64>) -> BootCfg {
    BootCfg {
        seed: 1,
        events: vec![],
        frame,
        dump_locals: true,
        lean: true,
        out_dir: std::env::temp_dir().join(format!("fmi-engine-end-test-{}-{out}", std::process::id())),
    }
}

fn host(env: &[(&str, &str)]) -> Host {
    let mut c = HostCfg::from_env();
    c.extra_env.retain(|(k, _)| k != "SPEEDRUN_REPORT_RESTART");
    c.extra_env.extend(env.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    Host::new(c).unwrap()
}

/// From boot to frame 300 and on to 400: with SPEEDRUN_REPORT_RESTART and no restart, every stop equals the switch
/// off; with SPEEDRUN_TEST_RESTART=350 the run ends at 350 as "restart" (in the root and in a fork), off it goes on.
#[test]
#[ignore]
fn restart_hook_ends_the_run_only_when_on() {
    let run = |e: &mut fmi_engine::BinEngine| {
        e.run(&RunSpec {
            frame: Some(400),
            ..Default::default()
        })
    };
    let mut off = host(&[]).boot_bin(&cfg("off", Some(300))).unwrap();
    let mut on = host(&[("SPEEDRUN_REPORT_RESTART", "1")]).boot_bin(&cfg("on", Some(300))).unwrap();
    let (a, b) = (run(&mut off).unwrap(), run(&mut on).unwrap());
    assert_eq!((a.frame, a.tick, a.rng, a.room), (b.frame, b.tick, b.rng, b.room));
    assert_eq!(off.facts().unwrap(), on.facts().unwrap());
    let trig = [("SPEEDRUN_REPORT_RESTART", "1"), ("SPEEDRUN_TEST_RESTART", "350")];
    let mut e = host(&trig).boot_bin(&cfg("r-on", Some(300))).unwrap();
    let mut c = e.fork_bin().unwrap();
    for x in [&mut c, &mut e] {
        match run(x) {
            Err(EngineError::Ended(st)) => {
                assert_eq!(st.reason, "restart");
                assert_eq!(st.frame, 350);
                assert!(is_game_end(&st.reason));
            }
            r => panic!("expected the end \"restart\", got {r:?}"),
        }
    }
    let mut e = host(&[("SPEEDRUN_TEST_RESTART", "350")]).boot_bin(&cfg("r-off", Some(300))).unwrap();
    assert_eq!(run(&mut e).unwrap().frame, 400);
}

/// The Boxes reply equals the BoxesJson text, and a second call gives the same reply (it changes nothing).
#[test]
#[ignore]
fn boxes_binary_equals_json() {
    let mut e = host(&[]).boot_bin(&cfg("boxes", Some(1000))).unwrap();
    let b = e.boxes().unwrap();
    let text = e.boxes_json().unwrap();
    let j = fmi_engine::json::parse(&text).unwrap();
    assert_eq!(j.at("room").unwrap().i32().unwrap(), b.room);
    let jb = j.at("boxes").unwrap().arr().unwrap();
    assert_eq!(jb.len(), b.boxes.len());
    for (x, y) in b.boxes.iter().zip(jb) {
        let pt = |k: &str| {
            let a = y.at(k).unwrap().arr().unwrap();
            (a[0].i32().unwrap(), a[1].i32().unwrap())
        };
        assert_eq!((x.ul, x.ur, x.lr, x.ll), (pt("ul"), pt("ur"), pt("lr"), pt("ll")));
        assert_eq!(x.flags as i64, y.at("flags").unwrap().int().unwrap());
    }
    assert_eq!(e.boxes().unwrap(), b);
}
