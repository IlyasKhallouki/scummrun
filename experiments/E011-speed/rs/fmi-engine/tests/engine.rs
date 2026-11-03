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

/// step_changes (one Step, its delta read into the facts' changes) gives, frame by frame, the status a run to the
/// next frame gives and the diff of the facts before and after it (fmi-core's default), from boot frames with inputs.
#[test]
#[ignore]
fn step_changes_are_a_runs_facts_diff() {
    let host = Host::new(HostCfg::from_env()).unwrap();
    for (f0, frames) in [(400u64, 1200u64), (2000, 1600)] {
        let mut root = host.boot_bin(&cfg(&format!("sc{f0}"), Some(f0))).unwrap();
        let mut a = root.fork_bin().unwrap();
        let mut b = root.fork_bin().unwrap();
        let clicks = [(f0 + 50, (160, 100)), (f0 + 300, (40, 120)), (f0 + 700, (300, 60))];
        for (f, (x, y)) in clicks {
            let ev = [Event { frame: f, what: Input::Click(x, y) }];
            a.inputs(&ev).unwrap();
            b.inputs(&ev).unwrap();
        }
        let (mut ta, mut tb, mut n_changes) = (0.0, 0.0, 0usize);
        let mut prev = a.facts().unwrap();
        for i in 1..=frames {
            let t = std::time::Instant::now();
            let sa = a.run(&RunSpec { frame: Some(f0 + i), ..Default::default() }).unwrap();
            let fa = a.facts().unwrap();
            let want = prev.diff(&fa);
            ta += t.elapsed().as_secs_f64();
            prev = fa;
            let t = std::time::Instant::now();
            let (sb, got) = b.step_changes(1).unwrap();
            tb += t.elapsed().as_secs_f64();
            assert_eq!(sb, sa, "frame {}", f0 + i);
            assert_eq!(got, want, "frame {}", f0 + i);
            n_changes += got.len();
        }
        eprintln!("from {f0}: {frames} frames, {n_changes} changes; run + facts + diff {:.1} us a frame, step_changes {:.1} us",
                  ta / frames as f64 * 1e6, tb / frames as f64 * 1e6);
        assert!(n_changes > 0);
    }
}
