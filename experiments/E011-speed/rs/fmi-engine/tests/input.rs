//! E013's patch 0007 (experiments/E013-robust/plan.md A.2-A.3, amendment (2) (a)): the InputLog and Digest replies'
//! decoding (plain tests), and on a build with patch 0007 (`cargo test --release -- --ignored`, FMI_ENGINE_BIN such a
//! build): the input log's paths, its binary reply against its JSON twin, the switch's inertness, and the digest's
//! A0 checks (gate B4's first two parts). The engine tests start from the E012 kitchen archive's root (FMI_TEST_ROOT,
//! default /var/tmp/fast-mi/e012-archives/kitchen entry 0: frame 2728 from the boot, no input).

use fmi_core::*;
use fmi_engine::wire::{R, W};
use fmi_engine::{BinEngine, Host, HostCfg};

fn rec(frame: u64, verdict: Verdict) -> InputRecord {
    InputRecord {
        frame,
        input: vec![Input::Click(10, 130), Input::Key("t".into())],
        stat: 0x8000 | 1,
        verdict,
        area: 4,
        value: 0,
        mode: 1,
        slot: 0,
        zone: 0,
        key_match: false,
        handler: 4,
        writes: vec![Touch { kind: TouchKind::Var, a: 107, b: 0, old: 11, now: 10 }],
        reads: vec![
            Touch { kind: TouchKind::UserPut, a: 0, b: 0, old: 1, now: 1 },
            Touch { kind: TouchKind::ActorAt, a: 20, b: 30, old: 0, now: 0 },
        ],
        ops: vec![LogOp { slot: 3, number: 4, where_: 2, code: 0x12, opcode: 0x1A }],
        ops_dropped: 0,
        touches_dropped: 2,
    }
}

/// An InputLog payload as the engine writes it (speedrun_wire.cpp wireInputLog).
fn payload(l: &InputLog) -> Vec<u8> {
    let mut w = W::msg(0x14);
    w.u32(l.records.len() as u32);
    for r in &l.records {
        w.u64(r.frame);
        w.u32(r.input.len() as u32);
        for i in &r.input {
            match i {
                Input::Click(x, y) | Input::RClick(x, y) => {
                    w.u8(if matches!(i, Input::Click(..)) { 0 } else { 1 });
                    w.i32(*x);
                    w.i32(*y);
                }
                Input::Key(k) => {
                    w.u8(3);
                    w.str(k);
                }
                _ => unreachable!(),
            }
        }
        w.i32(r.stat);
        match &r.verdict {
            Verdict::Dropped(why) => {
                w.u8(0);
                w.str(why);
            }
            Verdict::Ignored => w.u8(1),
            Verdict::Taken => w.u8(2),
        }
        for v in [r.area, r.value, r.mode, r.slot, r.zone] {
            w.i32(v);
        }
        w.b(r.key_match);
        w.i32(r.handler);
        for ts in [&r.writes, &r.reads] {
            w.u32(ts.len() as u32);
            for t in ts.iter() {
                w.u8(t.kind as u8);
                for v in [t.a, t.b, t.old, t.now] {
                    w.i32(v);
                }
            }
        }
        w.u32(r.ops.len() as u32);
        for o in &r.ops {
            w.i32(o.slot);
            w.i32(o.number);
            w.i32(o.where_);
            w.i64(o.code);
            w.u8(o.opcode);
        }
        w.u32(r.ops_dropped);
        w.u32(r.touches_dropped);
    }
    w.u32(l.dropped);
    w.u64(l.frame);
    w.finish()[5..].to_vec()
}

#[test]
fn input_log_decodes_field_for_field() {
    let want = InputLog {
        records: vec![rec(2889, Verdict::Taken), rec(2923, Verdict::Dropped("gate".into())), rec(2930, Verdict::Ignored)],
        dropped: 3,
        frame: 2931,
    };
    let p = payload(&want);
    let mut r = R::new(&p);
    assert_eq!(r.input_log().unwrap(), want);
    r.done().unwrap();
    let empty = InputLog { records: vec![], dropped: 0, frame: 7 };
    let p = payload(&empty);
    assert_eq!(R::new(&p).input_log().unwrap(), empty);
    // a count past the payload is refused, not read
    let mut p = payload(&want);
    p[0] = 9;
    assert!(R::new(&p).input_log().is_err());
    // the touch kinds are the engine's order
    for (i, k) in TouchKind::ALL.iter().enumerate() {
        assert_eq!(TouchKind::of_u8(i as u8), Some(*k));
    }
    assert_eq!(TouchKind::of_u8(18), None);
    assert_eq!(Verdict::Dropped("zone".into()).text(), "dropped: zone");
    assert!(Verdict::Ignored.refused() && !Verdict::Taken.refused());
}

#[test]
fn digest_decodes() {
    let mut w = W::msg(0x15);
    w.u64(2728);
    w.u64(0x0123_4567_89ab_cdef);
    w.u32(81234);
    let p = w.finish()[5..].to_vec();
    let mut r = R::new(&p);
    assert_eq!(r.digest().unwrap(), Digest { frame: 2728, digest: 0x0123_4567_89ab_cdef, bytes: 81234 });
    r.done().unwrap();
}

// --- on a build with patch 0007 ----------------------------------------------------------------------------------------

fn root_dir() -> std::path::PathBuf {
    std::env::var_os("FMI_TEST_ROOT").map(Into::into).unwrap_or_else(|| "/var/tmp/fast-mi/e012-archives/kitchen".into())
}

fn cfg(out: &str, frame: u64, events: Vec<Event>) -> BootCfg {
    BootCfg {
        seed: 1,
        events,
        frame: Some(frame),
        dump_locals: true,
        lean: true,
        out_dir: std::env::temp_dir().join(format!("fmi-engine-input-test-{}-{out}", std::process::id())),
    }
}

fn host(input: bool) -> Host {
    let mut c = HostCfg::from_env();
    c.extra_env.retain(|(k, _)| k != "SPEEDRUN_INPUT");
    if input {
        c.extra_env.push(("SPEEDRUN_INPUT".into(), "1".into()));
    }
    Host::new(c).unwrap()
}

/// The archive root's engine (E012's kitchen: frame 2728, the boot's own movie).
fn root(input: bool, out: &str) -> BinEngine {
    let e = fmi_engine::archive::chain_of(&root_dir(), 0).unwrap();
    host(input).boot_bin(&cfg(out, e.frame, e.events)).unwrap()
}

fn to(e: &mut BinEngine, f: u64) -> Status {
    e.run(&RunSpec { frame: Some(f), ..Default::default() }).unwrap()
}

/// The verb slots the dump lists (visible), with a point inside each (the dump's top left + (2, 2)).
fn verb_points(e: &mut BinEngine) -> Vec<(i32, i32, i32)> {
    let o = e.obs(&ObsReq::default()).unwrap();
    o.verbs.iter().map(|v| (v.slot, v.x + 2, v.y + 2)).collect()
}

/// At the root (a sentence idle: the gate open, the handler set): a click on each visible verb slot reaches the handler
/// with that slot (taken or ignored, never dropped); a key while the gate is shut is "dropped: gate"; the binary reply
/// equals the JSON twin's text; the log is taken by the call.
#[test]
#[ignore]
fn input_log_paths() {
    let mut e = root(true, "paths");
    let f0 = e.status().frame;
    assert!(e.status().up > 0, "the root's gate is open");
    let verbs = verb_points(&mut e);
    assert!(!verbs.is_empty());
    for &(slot, x, y) in &verbs {
        let mut c = e.fork_bin().unwrap();
        c.input_log().unwrap(); // what the parent's movie left
        c.inputs(&[Event { frame: f0 + 1, what: Input::Click(x, y) }]).unwrap();
        to(&mut c, f0 + 2);
        let l = c.input_log().unwrap();
        assert_eq!(l.records.len(), 1, "one record for one click");
        let r = &l.records[0];
        assert_eq!(r.frame, f0 + 1);
        assert_eq!(r.input, vec![Input::Click(x, y)]);
        assert_eq!(r.slot, slot, "the click's verb slot");
        assert!(!matches!(r.verdict, Verdict::Dropped(_)), "a verb click at an open gate reaches the handler: {r:?}");
        assert!(r.reads.iter().any(|t| t.kind == TouchKind::UserPut && t.old > 0));
        assert!(r.reads.iter().any(|t| t.kind == TouchKind::Var && t.old == r.handler));
        eprintln!("slot {slot}: {} area {} value {} writes {} reads {} ops {}", r.verdict.text(), r.area, r.value, r.writes.len(), r.reads.len(), r.ops.len());
        assert!(c.input_log().unwrap().records.is_empty(), "the call takes the records");
        drop(c);
    }
    // binary against JSON: two forks, the same click
    let (slot, x, y) = verbs[0];
    let mut a = e.fork_bin().unwrap();
    let mut b = e.fork_bin().unwrap();
    for c in [&mut a, &mut b] {
        c.input_log().unwrap();
        c.inputs(&[Event { frame: f0 + 1, what: Input::Click(x, y) }]).unwrap();
        to(c, f0 + 2);
    }
    let lb = a.input_log().unwrap();
    let text = b.input_log_json().unwrap();
    let j = fmi_engine::json::parse(&text).unwrap();
    let recs = j.at("records").unwrap().arr().unwrap();
    assert_eq!(recs.len(), lb.records.len());
    let (rb, rj) = (&lb.records[0], &recs[0]);
    assert_eq!(rj.at("frame").unwrap().int().unwrap() as u64, rb.frame);
    assert_eq!(rj.at("slot").unwrap().i32().unwrap(), slot);
    assert_eq!(rj.at("handler").unwrap().i32().unwrap(), rb.handler);
    assert_eq!(rj.at("verdict").unwrap().text().unwrap(), match rb.verdict {
        Verdict::Taken => "taken",
        Verdict::Ignored => "ignored",
        Verdict::Dropped(_) => "dropped",
    });
    for (k, ts) in [("writes", &rb.writes), ("reads", &rb.reads)] {
        let a = rj.at(k).unwrap().arr().unwrap();
        assert_eq!(a.len(), ts.len(), "{k}");
        for (t, x) in ts.iter().zip(a) {
            let x = x.arr().unwrap();
            assert_eq!(x[0].text().unwrap(), t.kind.name());
            let v: Vec<i32> = x[1..].iter().map(|v| v.i32().unwrap()).collect();
            assert_eq!(v, vec![t.a, t.b, t.old, t.now]);
        }
    }
    assert_eq!(rj.at("ops").unwrap().arr().unwrap().len(), rb.ops.len());
    drop(a);
    drop(b);
    // the gate shut: run on to a frame where userput is 0, then a key there
    let mut c = e.fork_bin().unwrap();
    c.input_log().unwrap();
    let mut shut = None;
    for _ in 0..3000 {
        if c.status().up <= 0 {
            shut = Some(c.status().frame);
            break;
        }
        let f = c.status().frame + 1;
        if c.run(&RunSpec { frame: Some(f), ..Default::default() }).is_err() {
            break;
        }
    }
    if let Some(f) = shut {
        c.inputs(&[Event { frame: f + 1, what: Input::Key("z".into()) }]).unwrap();
        to(&mut c, f + 2);
        let l = c.input_log().unwrap();
        let r = l.records.iter().find(|r| r.frame == f + 1).expect("the key's record");
        eprintln!("gate shut at {f}: the key {}", r.verdict.text());
        if c.status().up <= 0 || r.reads.iter().any(|t| t.kind == TouchKind::UserPut && t.old <= 0) {
            assert_eq!(r.verdict, Verdict::Dropped("gate".into()), "{r:?}");
        }
    }
    drop(c);
    drop(e);
}

/// SPEEDRUN_INPUT=1 is inert: the same clicks from the root with the switch off and on give the same stops, facts and
/// dumps at every frame of the next 200.
#[test]
#[ignore]
fn input_switch_is_inert() {
    let mut off = root(false, "off");
    let mut on = root(true, "on");
    let f0 = off.status().frame;
    let verbs = verb_points(&mut off);
    let evs: Vec<Event> = verbs
        .iter()
        .take(3)
        .enumerate()
        .map(|(k, &(_, x, y))| Event { frame: f0 + 1 + 20 * k as u64, what: Input::Click(x, y) })
        .chain([Event { frame: f0 + 80, what: Input::Click(160, 70) }, Event { frame: f0 + 90, what: Input::Key("t".into()) }])
        .collect();
    for e in [&mut off, &mut on] {
        e.inputs(&evs).unwrap();
    }
    for f in f0 + 1..=f0 + 200 {
        let (a, b) = (to(&mut off, f), to(&mut on, f));
        assert_eq!((a.frame, a.tick, a.qf, a.rng, a.room, a.up, a.idle.clone(), a.blk.clone()),
                   (b.frame, b.tick, b.qf, b.rng, b.room, b.up, b.idle.clone(), b.blk.clone()), "frame {f}");
        assert_eq!(off.facts().unwrap(), on.facts().unwrap(), "frame {f}");
        if f % 20 == 0 {
            assert_eq!(off.obs(&ObsReq::default()).unwrap(), on.obs(&ObsReq::default()).unwrap(), "frame {f}");
        }
    }
    assert!(!on.input_log().unwrap().records.is_empty());
    assert!(off.input_log().is_err(), "the log is off without the switch");
    drop(off);
    drop(on);
}

/// Gate B4's first two parts, and that the digest reads only: a fork and a replay from boot at the same frame give
/// equal digests (A0); two frames of an idle run differ; asking the digest at every frame changes no later state.
#[test]
#[ignore]
fn digest_a0() {
    let mut a = root(false, "da");
    let f0 = a.status().frame;
    let mut b = a.fork_bin().unwrap();
    to(&mut a, f0 + 100);
    // b asks the digest at every frame on the way
    let mut ds = Vec::new();
    for f in f0 + 1..=f0 + 100 {
        to(&mut b, f);
        ds.push(b.digest().unwrap());
    }
    let (da, db) = (a.digest().unwrap(), b.digest().unwrap());
    assert_eq!(da.digest, db.digest, "a fork asked at every frame against one that was not");
    assert_eq!(a.facts().unwrap(), b.facts().unwrap());
    assert_eq!(a.obs(&ObsReq { exact: true, ..Default::default() }).unwrap(), b.obs(&ObsReq { exact: true, ..Default::default() }).unwrap());
    assert_eq!(a.digest().unwrap(), da, "a second call gives the same digest");
    let e = fmi_engine::archive::chain_of(&root_dir(), 0).unwrap();
    let mut c = host(false).boot_bin(&cfg("dc", f0 + 100, e.events)).unwrap();
    assert_eq!(c.digest().unwrap().digest, da.digest, "a replay from boot to the same frame");
    // an idle run: consecutive frames differ
    let distinct: std::collections::HashSet<u64> = ds.iter().map(|d| d.digest).collect();
    let pairs = ds.windows(2).filter(|w| w[0].digest == w[1].digest).count();
    assert_eq!(pairs, 0, "two consecutive frames of an idle run with equal digests ({} distinct of {})", distinct.len(), ds.len());
    let t = std::time::Instant::now();
    for _ in 0..50 {
        a.digest().unwrap();
    }
    eprintln!("digest: {:.3} ms each, {} bytes of save stream", t.elapsed().as_secs_f64() * 1e3 / 50.0, da.bytes);
    for x in [a, b, c] {
        drop(x);
    }
}
