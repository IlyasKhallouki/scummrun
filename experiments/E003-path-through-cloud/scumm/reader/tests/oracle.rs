//! The port against the frozen Python (PORT-NOTES.md has the oracles and how to make them).
//!
//! Fast (seconds): the static oracle (domains.build for four starts, Scope.of for seven scopes, the inventory hook
//! summaries, the Builder's machine-free parts for room 33 and the kitchen), room 33's golden records written back,
//! room 33's whole reading by the Rust Builder over the captured machine runs against golden.py's file, every
//! captured run of room 33 against the Rust machine half's, and room 33's reading with the Rust machine half (no
//! Python) against golden.py's file.
//!
//! Slow (`cargo test --release -p scumm-reader -- --ignored`, about 10 minutes): the kitchen's golden records written
//! back, the kitchen's whole reading against golden.py's file (over the captured runs, and with the Rust machine
//! half), every captured run of the kitchen against the Rust machine half's, and the kitchen's task: Rust build_task
//! (and prune) -> packed file -> taskcore's SAS file, against the sha1 of task.py write_sas's file for each start and
//! variant, and the same for the packed file written as the records are made (RecordStream) and by export_records.

use scumm_reader::oracle::{self, Static};
use scumm_reader::{packed, task, Facts};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn python() -> PathBuf {
    oracle::root().join(".venv").join("bin").join("python")
}

fn sha1_hex(p: &Path) -> String {
    sha1_smol::Sha1::from(std::fs::read(p).unwrap()).digest().to_string()
}

/// Runs reader_port_oracle.py (nice 10) when its output is missing, or older than the sources it reads.
fn ensure(what: &[&str], out: &str) {
    let path = oracle::oracle_dir().join(out);
    let fresh = path.exists() && {
        let st = Static::load();
        let srcs = st.of_kind("check_machine").next().unwrap().get("sources");
        ["reader.py", "domains.py", "model.py", "task.py"].iter().all(|n| srcs.get(n).as_str() == sha1_hex(&here().join(n)))
    };
    if fresh {
        return;
    }
    let mut c = Command::new("nice");
    c.arg("-n").arg("10").arg(python()).arg("reader_port_oracle.py").args(what).current_dir(here());
    let st = c.status().expect("python");
    assert!(st.success(), "reader_port_oracle.py {what:?} failed");
}

fn static_oracle() -> &'static Static {
    static S: OnceLock<Static> = OnceLock::new();
    S.get_or_init(|| {
        ensure(&["static"], "static.jsonl.gz");
        Static::load()
    })
}

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

fn assert_none(what: &str, (diffs, n): (Vec<String>, usize)) {
    assert!(n > 0, "{what}: nothing compared");
    assert!(diffs.is_empty(), "{what}: {} of {n} differ; first:\n{}", diffs.len(), diffs[..diffs.len().min(5)].join("\n"));
    eprintln!("{what}: {n} compared, 0 differences");
}

#[test]
fn domains_against_python() {
    assert_none("domains.build", oracle::compare_domains(game(), static_oracle()));
}

#[test]
fn scopes_against_python() {
    assert_none("Scope.of", oracle::compare_scopes(game(), static_oracle()));
}

#[test]
fn hook_summaries_against_python() {
    assert_none("hook_writes and entry_writes", oracle::compare_hooks(game(), static_oracle()));
}

#[test]
fn builder_parts_against_python() {
    for name in ["room33", "kitchen"] {
        assert_none(&format!("builder {name}"), oracle::compare_builder(game(), static_oracle(), name));
    }
}

/// reader.py's lists of the Machine names and fields it relies on (`check_machine`): the interface the
/// machine-dependent half fills is named after them (PORT-NOTES.md); a change in reader.py shows here.
#[test]
fn check_machine_lists() {
    let l = static_oracle().of_kind("check_machine").next().unwrap();
    let strs = |k: &str| l.get(k).items().iter().map(|x| x.as_str().to_string()).collect::<Vec<_>>();
    assert_eq!(strs("path_fields"), ["freeze", "cut_data", "cleared", "unknown_writes", "assumptions", "notes", "writes", "stack", "end"]);
    assert_eq!(strs("frame_fields"), ["override", "locals", "args_known", "recursive", "resistant", "owned", "pc", "unit", "sid"]);
    assert_eq!(strs("drop_ends"), scumm_reader::builder::DROP_ENDS);
    assert_eq!(
        strs("lazy"),
        ["may_stop", "_live_slots", "_set_freeze", "_kill_room", "_eng_scene_begin", "_eng_scene_load", "_eng_scene_ego", "_eng_scroll_note", "_op_VerbOps"]
    );
    assert_eq!(
        strs("called"),
        [
            "_set_freeze", "_push_frame", "_halt", "_note", "_error", "_done", "_live_slots", "_stop_script", "_kill_room", "read",
            "val", "write", "run_sentence", "_count", "_freeze", "_unfreeze", "getter", "test"
        ]
    );
}

/// size_scaling.py's scopes (the room graph from the IR) against the oracle's k = 0..3, and its start (seed 1, the
/// facts in the file's order) and goal against the oracle's "part1" start.
#[test]
fn size_scaling_scopes_against_python() {
    use scumm_reader::scaling;
    let st = static_oracle();
    let start = scaling::start_facts(&oracle::root(), scaling::SEED);
    let l = st.named("domains", "part1");
    assert_eq!(start, Facts::from_py(l.get("start_items")), "the start's facts and their order");
    let goal = scaling::part1_goal(&oracle::root());
    let want: Vec<(String, Vec<i64>)> =
        l.get("goals").dict_items().iter().map(|(k, v)| (k.clone(), v.items().iter().map(|x| x.int()).collect())).collect();
    assert_eq!(goal, want, "the goal");
    let sc = scaling::scopes(game(), start.get0("room") as i32);
    for k in 0..4 {
        let rooms: Vec<i32> = st.named("scope", &format!("k{k}")).get("rooms").items().iter().map(|r| r.int() as i32).collect();
        assert_eq!(sc[k], rooms, "scope k={k}");
    }
    eprintln!("size_scaling scopes k=0..3, start and goal: equal");
}

/// Room 33's reading on worker threads (par.rs) against golden.py's file, for 1 to 4 workers: the same records in
/// the same order and the same counters as with no workers.
#[test]
fn room33_reading_threads() {
    for threads in 1..=4 {
        let (b, _, _) = oracle::read_par(game(), static_oracle(), "room33", threads);
        assert_none(
            &format!("room33 reading, {threads} threads"),
            oracle::compare_reading(&b, &oracle::golden_dir().join("room33.jsonl.gz"), 5),
        );
    }
}

/// Room 33's records streamed into the packed file and not kept (`keep_records` off), on 3 workers: the same
/// packed file and counters as the reading that keeps them, on no workers.
#[test]
fn room33_streamed_not_kept() {
    let dir = std::env::temp_dir().join(format!("reader-stream-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut out = Vec::new();
    for (threads, keep) in [(0, true), (3, false)] {
        let mut b = oracle::builder(game(), static_oracle(), "room33");
        b.keep_records = keep;
        let path = dir.join(format!("r{threads}.tc"));
        let mut stream = packed::RecordStream::create(&path).unwrap();
        let scope = b.scope.clone();
        scumm_reader::par::with_pool(game(), &scope, threads, |pool| {
            b.read_all_into(pool, true, None, 2, Some(&mut stream));
        });
        assert_eq!(b.records.len(), if keep { 878 } else { 0 });
        let start = Facts::from_py(static_oracle().named("builder", "room33").get("start_items"));
        let keys = stream.fact_keys();
        let s0 = task::task_start(&mut b, &start, &keys);
        stream.finish(&mut b, &[], &[("start".to_string(), s0)]).unwrap();
        out.push((std::fs::read(&path).unwrap(), b.counters_py().text(), b.n_records));
    }
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(out[0].2, 878);
    assert_eq!(out[0], out[1], "the packed file, the counters and the record count");
}

#[test]
fn room33_records_written_back() {
    let (recs, bad) = oracle::golden_records(&oracle::golden_dir().join("room33.jsonl.gz"));
    assert_none("room33 records written back", (bad, recs.len()));
}

#[test]
fn room33_reading_against_golden() {
    let st = static_oracle();
    ensure(&["capture", "room33"], "capture-room33.jsonl.gz");
    let (b, runs) = oracle::replay(game(), st, "room33");
    eprintln!("room33: {runs} runs replayed");
    assert_none("room33 reading", oracle::compare_reading(&b, &oracle::golden_dir().join("room33.jsonl.gz"), 5));
}

/// Every captured machine run of room 33 against the Rust machine half's run of the same inputs (166 runs).
#[test]
fn room33_runs_rust_machine() {
    ensure(&["capture", "room33"], "capture-room33.jsonl.gz");
    let (diffs, runs, differing, _) = oracle::compare_runs(game(), static_oracle(), "room33", 5);
    assert!(runs > 0, "no run compared");
    assert!(differing == 0, "{differing} of {runs} runs differ; first:\n{}", diffs.join("\n"));
    eprintln!("room33: {runs} runs, 0 differ");
}

/// Room 33's reading by the Rust Builder and the Rust machine half (no Python, no playback) against golden.py's.
#[test]
fn room33_reading_rust_machine() {
    let (b, _) = oracle::read(game(), static_oracle(), "room33");
    assert_none("room33 reading, Rust machine", oracle::compare_reading(&b, &oracle::golden_dir().join("room33.jsonl.gz"), 5));
}

/// The switch for the scope-1 finding (PORT-NOTES.md, "ELSEWHERE"), on room 33's scope: off, a place none of the
/// scope's points is written as itself and listed; on, it is written as ELSEWHERE, no precondition set holds
/// ELSEWHERE, and a start with the slot at such a place starts at ELSEWHERE. A place among the points is the same
/// either way.
#[test]
fn elsewhere_switch_positions_and_start() {
    use scumm_reader::domain::Label;
    use scumm_reader::record::{Eff, Pre};
    use scumm_reader::table::{ELSEWHERE, NOWHERE};
    let (name, inside) = {
        let b = oracle::builder(game(), static_oracle(), "room33");
        let (n, ws) = b.table.points["at"].iter().find(|(_, ws)| ws.len() >= 2).expect("a unit with two wait points");
        (n.clone(), ws.iter().copied().collect::<Vec<i64>>())
    };
    let outside = inside.iter().max().unwrap() + 1000;
    let key = format!("pos:{name}");
    for on in [false, true] {
        let mut b = oracle::builder(game(), static_oracle(), "room33");
        b.table.elsewhere = on;
        let dom: Vec<Label> = b.table.domain(&key).unwrap().values.clone();
        assert_eq!(dom.contains(&Label::Int(ELSEWHERE)), on, "the domain holds ELSEWHERE only with the switch on");
        // a write of a place outside the points, with a precondition "not at the first point"
        let pre: Pre = [(format!("at:{name}:{}", inside[0]), vec![Label::Int(0)])].into_iter().collect();
        let eff = vec![(format!("at:{name}:{outside}"), Eff::Set(Label::Int(1)))];
        let (pre, eff) = b.positions(pre, eff);
        let want = if on { ELSEWHERE } else { outside };
        assert_eq!(eff, vec![(key.clone(), Eff::Set(Label::Int(want)))]);
        let set = &pre[&key];
        assert!(set.contains(&Label::Int(NOWHERE)) && !set.contains(&Label::Int(inside[0])));
        assert!(!set.contains(&Label::Int(ELSEWHERE)), "no precondition set holds ELSEWHERE");
        assert_eq!(b.outside_points[&key][&outside], 1, "listed either way");
        // a place among the points, needed at a point: the same either way
        let pre: Pre = [(format!("at:{name}:{}", inside[1]), vec![Label::Int(1)])].into_iter().collect();
        let eff = vec![(format!("at:{name}:{}", inside[0]), Eff::Set(Label::Int(1)))];
        let (pre, eff) = b.positions(pre, eff);
        assert_eq!(eff, vec![(key.clone(), Eff::Set(Label::Int(inside[0])))]);
        assert_eq!(pre[&key], vec![Label::Int(inside[1])]);
        // two places, both outside: one ELSEWHERE (on), a guess of both (off)
        let eff = vec![
            (format!("at:{name}:{outside}"), Eff::Set(Label::Int(1))),
            (format!("at:{name}:{}", outside + 1), Eff::Set(Label::Int(1))),
        ];
        let (_, eff) = b.positions(Pre::new(), eff);
        let want = if on {
            Eff::Set(Label::Int(ELSEWHERE))
        } else {
            Eff::Guess(vec![Label::Int(outside), Label::Int(outside + 1)])
        };
        assert_eq!(eff, vec![(key.clone(), want)]);
        // the start: a slot at a place outside the points, then at a point
        let keys = vec![key.clone()];
        let st = task::task_start(&mut b, &Facts::from_items([(format!("at:{name}:{outside}"), 1)]), &keys);
        assert_eq!(st.get(&key), Some(if on { ELSEWHERE } else { NOWHERE }));
        let st = task::task_start(&mut b, &Facts::from_items([(format!("at:{name}:{}", inside[1]), 1)]), &keys);
        assert_eq!(st.get(&key), Some(inside[1]));
    }
}

/// Room 33 read with the switch on, on 2 workers: no place outside the points, so the reading is golden.py's.
#[test]
fn room33_reading_elsewhere_on() {
    let mut b = oracle::builder(game(), static_oracle(), "room33");
    b.table.elsewhere = true;
    let scope = b.scope.clone();
    scumm_reader::par::with_pool(game(), &scope, 2, |pool| {
        b.read_all_into(pool, true, None, 2, None);
    });
    assert!(b.outside_points.is_empty());
    assert_none("room33 reading, switch on", oracle::compare_reading(&b, &oracle::golden_dir().join("room33.jsonl.gz"), 5));
}

/// The switch --live-locals (PORT-NOTES.md, "Live locals") on scope `name`, on `threads` workers, against golden.py's
/// reading (made without it). Each list of locals the golden reading built, with the slot's flags its event carries,
/// maps to the list of the locals its unit reads with those flags (same unit name, read locals, override, cutscene
/// data, resistant, owned, and recursive as the switch keys it: true or not); that map must be a function and lose no list, and
/// the golden records with
/// their lists renumbered by it must be exactly the switch's records, as sets (lists that differ only in unread locals
/// gave the same records). Returns (golden records, the switch's records, golden lists with their flags, the switch's
/// lists).
fn live_locals_against_golden(name: &str, threads: usize) -> (usize, usize, usize, usize) {
    use scumm_reader::domain::Label;
    use scumm_reader::locals::LiveLocals;
    use scumm_reader::record::{Eff, Event, Val, What};
    use std::collections::{BTreeMap, BTreeSet};
    let ll = std::sync::Arc::new(LiveLocals::of(game()));
    let mut b = oracle::builder(game(), static_oracle(), name);
    b.table.live_locals = Some(ll.clone());
    let scope = b.scope.clone();
    scumm_reader::par::with_pool(game(), &scope, threads, |pool| {
        b.read_all_into(pool, true, None, 2, None);
    });
    let (golden, bad) = oracle::golden_records(&oracle::golden_dir().join(format!("{name}.jsonl.gz")));
    assert!(bad.is_empty(), "golden records written back: {bad:?}");
    let events = |r: &scumm_reader::Record| -> Vec<Event> {
        let mut v = r.next_events.clone();
        if let What::Event(e) = &r.source {
            v.push(e.clone());
        }
        v
    };
    let flags = |e: &Event| format!("{}|{}|{}", scumm_reader::locals::recursive(&e.recursive).text(), e.resistant.text(), e.owned.text());
    let key = |e: &Event, locals: &[(i64, Val)]| {
        let l: Vec<String> = locals.iter().map(|(i, v)| format!("{i}={}", v.show())).collect();
        format!("{}|{}|{}|{:?}|{}", e.name, l.join(","), e.override_, e.cut_data, flags(e))
    };
    let mut off: BTreeMap<(i64, String), BTreeSet<String>> = BTreeMap::new();
    for r in &golden {
        for e in events(r).iter().filter(|e| e.sig != 0) {
            off.entry((e.sig, flags(e))).or_default().insert(key(e, &ll.keep(e.unit, e.locals.clone()).0));
        }
    }
    let mut on: BTreeMap<String, BTreeSet<i64>> = BTreeMap::new();
    for r in &b.records {
        for e in events(r).iter().filter(|e| e.sig != 0) {
            on.entry(key(e, &e.locals)).or_default().insert(e.sig);
        }
    }
    assert!(on.values().all(|s| s.len() == 1), "a list with two numbers");
    let map: BTreeMap<(i64, String), i64> = off
        .iter()
        .map(|((s, f), keys)| {
            assert_eq!(keys.len(), 1, "golden list {s:x}: two lists of read locals");
            let k = keys.iter().next().unwrap();
            let t = on.get(k).unwrap_or_else(|| panic!("golden list {s:x}: no list with its read locals")).iter().next().unwrap();
            ((*s, f.clone()), *t)
        })
        .collect();
    let m = |s: i64, f: &str| if s == 0 { 0 } else { map[&(s, f.to_string())] };
    let renum_event = |e: &Event| -> Event {
        let mut e2 = e.clone();
        e2.sig = m(e.sig, &flags(e));
        e2.locals = ll.keep(e.unit, e.locals.clone()).0;
        e2.recursive = scumm_reader::locals::recursive(&e.recursive);
        e2
    };
    let mut renumbered: BTreeSet<String> = BTreeSet::new();
    for r in &golden {
        let mut r = r.clone();
        let next: BTreeMap<String, String> = r.next_events.iter().map(|e| (format!("args:{}", e.name), flags(e))).collect();
        for (k, e) in r.effects.iter_mut() {
            if let (true, Eff::Set(Label::Int(s))) = (k.starts_with("args:"), &*e) {
                *e = Eff::Set(Label::Int(m(*s, &next[k])));
            }
        }
        if let What::Event(e) = &r.source {
            let (own, f) = (format!("args:{}", e.name), flags(e));
            for (k, set) in r.pre.iter_mut() {
                if *k == own {
                    let mut v: Vec<Label> = set.iter().map(|l| if let Label::Int(s) = l { Label::Int(m(*s, &f)) } else { *l }).collect();
                    v.sort();
                    v.dedup();
                    *set = v;
                }
            }
            let e2 = renum_event(e);
            r.label = r.label.replace(&format!("#{:x})", e.sig), &format!("#{:x})", e2.sig));
            r.source = What::Event(e2);
        }
        r.next_events = r.next_events.iter().map(|e| renum_event(e)).collect();
        renumbered.insert(r.py().text());
    }
    let mine: BTreeSet<String> = b.records.iter().map(|r| r.py().text()).collect();
    let lost = renumbered.difference(&mine).count();
    let added = mine.difference(&renumbered).count();
    assert!(lost == 0 && added == 0, "{name}: {lost} golden records renumbered not in the switch's reading, {added} the other way");
    let on_lists: BTreeSet<i64> = map.values().copied().collect();
    eprintln!(
        "{name}, switch on: {} golden records ({} distinct renumbered), {} records; {} golden lists onto {} lists",
        golden.len(),
        renumbered.len(),
        b.records.len(),
        map.len(),
        on_lists.len()
    );
    (golden.len(), b.records.len(), map.len(), on_lists.len())
}

/// Room 33 read with the switch --live-locals on 2 workers: the golden records, renumbered, exactly (38 lists onto 38:
/// one list drops locals its unit never reads, and no two lists merge).
#[test]
fn room33_live_locals_on() {
    let (g, n, lists, on) = live_locals_against_golden("room33", 2);
    assert_eq!((g, n, lists, on), (878, 878, 38, 38));
}

/// The kitchen read with the switch --live-locals on 3 workers, against golden.py's reading (about 2 minutes):
/// 72,256 golden records, 72,124 of them distinct once renumbered, are the switch's 72,126 records (72,124
/// distinct); 5,298 lists onto 5,221.
#[test]
#[ignore]
fn kitchen_live_locals_on() {
    let (g, n, lists, on) = live_locals_against_golden("kitchen", 3);
    assert_eq!((g, n, lists, on), (72256, 72126, 5298, 5221));
}

#[test]
#[ignore]
fn kitchen_records_written_back() {
    let (recs, bad) = oracle::golden_records(&oracle::golden_dir().join("kitchen.jsonl.gz"));
    assert_none("kitchen records written back", (bad, recs.len()));
}

fn kitchen_capture() {
    let p = oracle::oracle_dir().join("capture-kitchen.jsonl.gz");
    assert!(
        p.exists(),
        "{} is missing: run `nice -n 10 ../../.venv/bin/python reader_port_oracle.py capture kitchen --pickle FILE` \
         from experiments/E003-path-through-cloud (about 15 minutes)",
        p.display()
    );
}

#[test]
#[ignore]
fn kitchen_reading_against_golden() {
    kitchen_capture();
    let (b, runs) = oracle::replay(game(), static_oracle(), "kitchen");
    eprintln!("kitchen: {runs} runs replayed");
    assert_none("kitchen reading", oracle::compare_reading(&b, &oracle::golden_dir().join("kitchen.jsonl.gz"), 5));
}

/// Every captured machine run of the kitchen against the Rust machine half's (18,127 runs; about 2.5 minutes).
#[test]
#[ignore]
fn kitchen_runs_rust_machine() {
    kitchen_capture();
    let (diffs, runs, differing, _) = oracle::compare_runs(game(), static_oracle(), "kitchen", 5);
    assert!(runs > 0, "no run compared");
    assert!(differing == 0, "{differing} of {runs} runs differ; first:\n{}", diffs.join("\n"));
    eprintln!("kitchen: {runs} runs, 0 differ");
}

/// The kitchen's reading by the Rust Builder and the Rust machine half against golden.py's (about a minute).
#[test]
#[ignore]
fn kitchen_reading_rust_machine() {
    let (b, secs) = oracle::read(game(), static_oracle(), "kitchen");
    eprintln!("kitchen: {} records, reading {secs:.1} s", b.records.len());
    assert_none("kitchen reading, Rust machine", oracle::compare_reading(&b, &oracle::golden_dir().join("kitchen.jsonl.gz"), 5));
}

/// The kitchen read with the switch on (PORT-NOTES.md, "ELSEWHERE"), on 4 workers: no place outside the points, so the
/// reading is golden.py's.
#[test]
#[ignore]
fn kitchen_reading_elsewhere_on() {
    let mut b = oracle::builder(game(), static_oracle(), "kitchen");
    b.table.elsewhere = true;
    let scope = b.scope.clone();
    scumm_reader::par::with_pool(game(), &scope, 4, |pool| {
        b.read_all_into(pool, true, None, 2, None);
    });
    assert!(b.outside_points.is_empty(), "{:?}", b.outside_points);
    assert_none("kitchen reading, switch on", oracle::compare_reading(&b, &oracle::golden_dir().join("kitchen.jsonl.gz"), 5));
}

/// The kitchen read by the Rust machine half on 6 worker threads, its records streamed into the packed file on the
/// sink thread and not kept: taskcore's SAS file of it (start s17) against task.py write_sas's on the Python reading.
#[test]
#[ignore]
fn kitchen_streamed_threads_against_write_sas() {
    let py_path = oracle::oracle_dir().join("python-sas.json");
    assert!(py_path.exists(), "{} is missing: run reader_port_oracle.py sas (PORT-NOTES.md)", py_path.display());
    let py: serde_json::Value = serde_json::from_slice(&std::fs::read(&py_path).unwrap()).unwrap();
    let st = static_oracle();
    let kt = oracle::kitchen_task(st);
    let work = std::env::temp_dir().join(format!("scumm-reader-stream-{}", std::process::id()));
    std::fs::create_dir_all(&work).unwrap();
    let tc = taskcore();
    let mut b = oracle::builder(game(), st, "kitchen");
    b.keep_records = false;
    let scope = b.scope.clone();
    let mut sink = scumm_reader::par::SinkThread::new(packed::RecordStream::create(&work.join("made.tc")).unwrap(), 64);
    scumm_reader::par::with_pool(game(), &scope, 6, |pool| {
        b.read_all_into(pool, true, None, 2, Some(&mut sink));
    });
    let stream = sink.finish();
    assert_eq!((b.records.len(), b.n_records), (0, 72256));
    let keys = stream.fact_keys();
    let s17 = task::task_start(&mut b, &kt.s17, keys.iter().chain(kt.goal.iter().map(|g| &g.0)));
    stream.finish(&mut b, &kt.goal, &[("s17".to_string(), s17)]).unwrap();
    let (sha, bytes) = sas_sha1(&tc, &work.join("made.tc"), "s17", &work);
    std::fs::remove_dir_all(&work).unwrap();
    assert_eq!(sha, py["full-all"]["files"]["s17"]["sha1"].as_str().unwrap(), "the SAS file differs from write_sas's");
    eprintln!("kitchen on 6 threads, streamed: SAS s17 {bytes} bytes, sha1 {sha}: equal to write_sas");
}

/// The kitchen's reading on 6 worker threads against golden.py's.
#[test]
#[ignore]
fn kitchen_reading_threads() {
    let (b, secs, stats) = oracle::read_par(game(), static_oracle(), "kitchen", 6);
    eprintln!("kitchen: {} records, reading {secs:.1} s on 6 threads, {} jobs", b.records.len(), stats.jobs);
    assert_none("kitchen reading, 6 threads", oracle::compare_reading(&b, &oracle::golden_dir().join("kitchen.jsonl.gz"), 5));
}

fn taskcore() -> PathBuf {
    let dir = here().join("taskcore");
    let bin = dir.join("target").join("release").join("taskcore");
    let st = Command::new("cargo").args(["build", "--release", "--quiet"]).current_dir(&dir).status().unwrap();
    assert!(st.success(), "taskcore does not build");
    bin
}

/// taskcore's SAS file for a packed task and start, as task.py writes it (no fold, no reduction): (sha1, bytes).
fn sas_sha1(tc: &Path, task: &Path, start: &str, work: &Path) -> (String, u64) {
    use std::io::Read;
    let mut child = Command::new(tc)
        .args(["sas", "--task"])
        .arg(task)
        .args(["--start", start, "--fold", "off", "--reduce", "none", "--encoding", "task", "--out", "/dev/stdout", "--stats"])
        .arg(work.join("stats.json"))
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut out = child.stdout.take().unwrap();
    let mut h = sha1_smol::Sha1::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut n = 0u64;
    loop {
        let k = out.read(&mut buf).unwrap();
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
        n += k as u64;
    }
    assert!(child.wait().unwrap().success());
    (h.digest().to_string(), n)
}

/// The kitchen task from the Rust Builder's reading (replayed), through the packed format and taskcore, against
/// task.py's write_sas on the Python reading (python-sas.json: reader_port_oracle.py sas).
#[test]
#[ignore]
fn kitchen_task_against_write_sas() {
    kitchen_capture();
    let py_path = oracle::oracle_dir().join("python-sas.json");
    assert!(py_path.exists(), "{} is missing: run reader_port_oracle.py sas (PORT-NOTES.md)", py_path.display());
    let py: serde_json::Value = serde_json::from_slice(&std::fs::read(&py_path).unwrap()).unwrap();
    let st = static_oracle();
    let kt = oracle::kitchen_task(st);
    let work = std::env::temp_dir().join(format!("scumm-reader-test-{}", std::process::id()));
    std::fs::create_dir_all(&work).unwrap();
    let tc = taskcore();
    let mut checked = 0;
    // the reading, its records written to a packed file as they are made (RecordStream)
    let mut stream = packed::RecordStream::create(&work.join("made.tc")).unwrap();
    let (mut b, _) = oracle::replay_into(game(), st, "kitchen", Some(&mut stream));
    let keys = task::record_keys(&b.records, &kt.goal);
    let s17 = task::task_start(&mut b, &kt.s17, &keys);
    stream.finish(&mut b, &kt.goal, &[("s17".to_string(), s17)]).unwrap();
    let (sha, _) = sas_sha1(&tc, &work.join("made.tc"), "s17", &work);
    assert_eq!(sha, py["full-all"]["files"]["s17"]["sha1"].as_str().unwrap(), "RecordStream: the SAS file differs");
    std::fs::remove_file(work.join("made.tc")).unwrap();
    checked += 1;
    let recs = std::mem::take(&mut b.records);
    for which in ["all", "no211"] {
        let rs: Vec<_> = recs.iter().filter(|r| which == "all" || r.label != kt.no211).cloned().collect();
        let full = task::build_task(&mut b, &rs, &kt.goal);
        let rel = task::relevant(&full);
        let small = task::prune(&full, &rel);
        for (variant, t) in [("full", &full), ("pruned", &small)] {
            let keys: Vec<String> = t.facts.iter().map(|f| f.0.clone()).collect();
            let mut starts = vec![("s17".to_string(), task::task_start(&mut b, &kt.s17, &keys))];
            if which == "all" {
                starts.push(("replan".to_string(), task::replan_start(&mut b, &kt.replan, &Facts::new(), &keys).0));
            }
            let path = work.join(format!("{variant}-{which}.tc"));
            let meta = |i: usize| (rs[i].kind.clone(), rs[i].end_text().to_string(), packed::cont(&rs[i]));
            packed::export_task(&path, t, &starts, &meta).unwrap();
            for (name, _) in &starts {
                let want = &py[format!("{variant}-{which}")]["files"][name];
                let (sha, bytes) = sas_sha1(&tc, &path, name, &work);
                assert_eq!(sha, want["sha1"].as_str().unwrap(), "{variant} {which} {name}: the SAS file differs from write_sas's");
                assert_eq!(bytes, want["bytes"].as_u64().unwrap());
                eprintln!("{variant} {which} {name}: {bytes} bytes, sha1 {sha}: equal to write_sas");
                checked += 1;
            }
            std::fs::remove_file(&path).unwrap();
        }
        if which == "all" {
            // the streaming writer (no Task): build_task's rules record by record
            let path = work.join("stream-all.tc");
            let keys = task::record_keys(&rs, &kt.goal);
            let starts = vec![("s17".to_string(), task::task_start(&mut b, &kt.s17, &keys))];
            packed::export_records(&path, &mut b, &rs, &kt.goal, &starts).unwrap();
            let (sha, _) = sas_sha1(&tc, &path, "s17", &work);
            assert_eq!(sha, py["full-all"]["files"]["s17"]["sha1"].as_str().unwrap(), "export_records: the SAS file differs");
            std::fs::remove_file(&path).unwrap();
            checked += 1;
        }
    }
    std::fs::remove_dir_all(&work).unwrap();
    assert_eq!(checked, 8);
}
