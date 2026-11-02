//! Tests that need the E011 engine build (FMI_ENGINE_BIN or /var/tmp/fast-mi/svm-e011/...) and the game files:
//! `cargo test --release -- --ignored`.

use fmi_core::*;
use fmi_engine::{Host, HostCfg};
use std::sync::Arc;

fn cfg(out: &str, frame: Option<u64>) -> BootCfg {
    BootCfg {
        seed: 1,
        events: vec![],
        frame,
        dump_locals: true,
        lean: true,
        out_dir: std::env::temp_dir().join(format!("fmi-engine-test-{}-{out}", std::process::id())),
    }
}

/// Forks from four engines on four threads at once: every connection reaches its own engine (the pid match), and
/// every child stands where its parent stands.
#[test]
#[ignore]
fn concurrent_forks_reach_their_engines() {
    let host = Arc::new(Host::new(HostCfg::from_env()).unwrap());
    let mut threads = Vec::new();
    for t in 0..4u64 {
        let host = host.clone();
        threads.push(std::thread::spawn(move || {
            let mut root = host.boot_bin(&cfg(&format!("t{t}"), Some(300 + 50 * t))).unwrap();
            let want = root.facts().unwrap();
            for _ in 0..25 {
                let mut c = root.fork_bin().unwrap();
                assert_eq!(c.status().frame, 300 + 50 * t);
                assert_eq!(c.status().reason, "forked");
                assert!(!c.block_fresh().unwrap().is_empty());
                let f = fmi_engine::block::Block::parse(c.block_fresh().unwrap())
                    .unwrap()
                    .facts();
                assert_eq!(f, want);
                Box::new(c).quit();
            }
            let (st, kids) = root.fork_each_bin(5).unwrap();
            assert_eq!(st.reason, "fork_each");
            let frames: Vec<u64> = kids.iter().map(|k| k.status().frame).collect();
            let f0 = 300 + 50 * t;
            assert_eq!(frames, (f0 + 1..=f0 + 5).collect::<Vec<_>>());
        }));
    }
    for t in threads {
        t.join().unwrap();
    }
}

/// The bridge's checks reach the controller as errors, and the engine serves the next command.
#[test]
#[ignore]
fn errors_come_back_and_the_engine_goes_on() {
    let host = Host::new(HostCfg::from_env()).unwrap();
    let mut e = host.boot_bin(&cfg("err", Some(100))).unwrap();
    assert!(matches!(
        e.run(&RunSpec {
            frame: Some(50),
            ..Default::default()
        }),
        Err(EngineError::Bridge(_))
    ));
    assert!(matches!(e.run(&RunSpec::default()), Err(EngineError::Bridge(_))));
    assert!(matches!(e.set_rng(0), Err(EngineError::Bridge(_))));
    let bad = Event {
        frame: 101,
        what: Input::Key("nokey".into()),
    };
    assert!(matches!(e.inputs(&[bad]), Err(EngineError::Bridge(_))));
    let past = Event {
        frame: 90,
        what: Input::Click(10, 10),
    };
    assert!(matches!(e.inputs(&[past]), Err(EngineError::Bridge(_))));
    let st = e
        .run(&RunSpec {
            frame: Some(101),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(st.frame, 101);
    let s = e.step(3, false, false).unwrap();
    assert_eq!(s.status.frame, 104);
    assert!(e
        .holds(&[Cond {
            kind: CondKind::Room,
            subject: e.status().room,
            op: CondOp::Eq,
            value: 0,
            values: vec![],
            negate: false
        }])
        .unwrap());
}
