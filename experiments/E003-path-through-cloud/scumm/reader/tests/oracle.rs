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

/// `reader-port read room33 --packed F` (the switch off): the reading is golden.py's, 0 differences, though the
/// packed file's start and `finish` add a builder counter after the reading ("steps that need room in their domain").
#[test]
fn reader_port_read_packed_room33() {
    let dir = std::env::temp_dir().join(format!("reader-port-packed-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let packed = dir.join("room33.tc");
    let out = Command::new(env!("CARGO_BIN_EXE_reader-port"))
        .args(["read", "room33", "--packed"])
        .arg(&packed)
        .current_dir(here())
        .output()
        .expect("reader-port");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let wrote = packed.exists();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(out.status.success(), "reader-port read room33 --packed failed:\n{text}{}", String::from_utf8_lossy(&out.stderr));
    assert!(text.contains("room33 reading against golden.py's: 880 compared, 0 differences"), "{text}");
    assert!(wrote && text.contains(" actions ("), "no packed file:\n{text}");
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

/// Under the switch --live-locals a slot's list is numbered only through `Builder::list_number`, which puts the slot's
/// flags in the number: `sig` without them stops (fix round 1's F10 must not give a dumped slot a number no record's
/// list has).
#[test]
#[should_panic(expected = "Builder::list_number")]
fn live_locals_sig_needs_the_flags() {
    use scumm_reader::record::Val;
    let mut b = oracle::builder(game(), static_oracle(), "room33");
    b.table.live_locals = Some(std::sync::Arc::new(scumm_reader::locals::LiveLocals::of(game())));
    b.sig("local:202", &[(0, Val::Int(1))], 0, &[], true, &[]);
}

/// `Builder::list_number` and `Builder::limit` (SIG_LIMIT) with the switch --live-locals, on room 31's local 202,
/// which reads only Local[0]. Off, list_number is `sig` on the list as it is. On, an unread local gives no other list,
/// the R flag gives another number, and the limit counts a list's flag variants once: with a limit of 2, the lists
/// Local[0] = 5 (R false and R true) and Local[0] = 6 are kept and Local[0] = 7 is folded; counting numbers, as
/// without the switch, would fold Local[0] = 6, which the reading without the switch keeps.
#[test]
fn live_locals_list_number_and_limit() {
    use scumm_ir::UnitKey;
    use scumm_reader::builder::Builder;
    use scumm_reader::record::{Event, Val};
    use scumm_reader::Py;
    let unit = UnitKey::Local(31, 202);
    let l = |a: i64, c: i64| vec![(0, Val::Int(a)), (1, Val::Int(c))];
    let f = Py::Bool(false);
    let mut off = oracle::builder(game(), static_oracle(), "room33");
    let (lo, so, ro) = off.list_number("local:202", unit, l(5, 7), 0, &[], true, &f, &f, &f);
    assert_eq!((lo, ro), (l(5, 7), f.clone()));
    assert_eq!(so, off.sig("local:202", &l(5, 7), 0, &[], true, &[]));
    let mut b = oracle::builder(game(), static_oracle(), "room33");
    b.table.live_locals = Some(std::sync::Arc::new(scumm_reader::locals::LiveLocals::of(game())));
    let num = |b: &mut Builder, a: i64, c: i64, r: bool| b.list_number("local:202", unit, l(a, c), 0, &[], true, &f, &Py::Bool(r), &f);
    let (l1, s1, r1) = num(&mut b, 5, 7, false);
    assert_eq!((l1, r1), (vec![(0, Val::Int(5))], Py::None));
    assert_eq!(num(&mut b, 5, 8, false).1, s1, "an unread local: the same list");
    let s1r = num(&mut b, 5, 7, true).1;
    let s2 = num(&mut b, 6, 7, false).1;
    let s3 = num(&mut b, 7, 7, false).1;
    assert!(s1r != s1 && s2 != s1 && s3 != s2);
    let ev = |sig: i64, a: i64, r: bool| Event {
        name: "local:202".into(),
        unit,
        w: 3,
        sig,
        locals: vec![(0, Val::Int(a))],
        args_known: true,
        recursive: Py::None,
        resistant: Py::Bool(r),
        owned: f.clone(),
        abort: false,
        override_: 0,
        cut_data: vec![],
    };
    b.sig_limit = 2;
    let kept: Vec<i64> = [ev(s1, 5, false), ev(s1r, 5, true), ev(s2, 6, false), ev(s3, 7, false), ev(s1r, 5, true)]
        .iter()
        .map(|e| b.limit(e).sig)
        .collect();
    assert_eq!(kept, vec![s1, s1r, s2, 0, s1r]);
    assert_eq!(b.folded.iter().copied().collect::<Vec<_>>(), vec![s3]);
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

/// F10: room 33 read with the dumped locals of the slots live at its start (`Builder.slot_locals`): the
/// reading keeps every record of the reading without them and adds the records of those slots' events from their
/// locals; at a replan each slot's locals match the list its events were read with (`task::replan_start_locals`),
/// and with no locals in the dump the rule is reader.py's.
#[test]
fn room33_slot_locals() {
    use scumm_reader::builder::SlotLocals;
    use scumm_reader::{Eff, Label, Record};
    use std::collections::BTreeMap;
    let st = static_oracle();
    let (b0, _) = oracle::read(game(), st, "room33");
    let mut b1 = oracle::builder(game(), st, "room33");
    let start = b1.table.start.clone();
    // the slots live at the start (room 33, seed 1: globals 159 and 23 and room 33's local 202, their locals all 0
    // in the engine), here with Local[0] = 7
    let slots: Vec<SlotLocals> = start
        .iter()
        .filter(|(k, v)| k.starts_with("at:") && *v == 1)
        .map(|(k, _)| {
            let (n, w) = k[3..].rsplit_once(':').unwrap();
            let mut locals = vec![0i64; 25];
            locals[0] = 7;
            SlotLocals { name: n.to_string(), room: start.get0("room") as i32, w: w.parse().unwrap(), esc: None, locals, override_: 0, cut_data: vec![], owned: false, flags: None }
        })
        .collect();
    assert!(!slots.is_empty());
    b1.slot_locals = slots.clone();
    let mut m = scumm_reader::machine::Reader::new(game());
    b1.read_all_into(&mut m, true, None, 2, None);
    assert_eq!(b1.counts.get("F10: dumped slots read from their locals"), slots.len() as i64);
    assert_eq!(b1.slot_events.len(), slots.len());
    assert!(b1.slot_events.iter().all(|e| e.sig != 0 && e.args_known));
    // every record of the reading without the slots' locals is still there. The domain of args:<unit> now holds
    // the list's number, so a record that read the unit with args 0 says so in its precondition (an engine
    // condition of an event), and its write args:<unit> := 0 is then no change and goes, as `record` drops any write
    // of the value its precondition holds; a record whose one effect that was changes nothing and is dropped
    let argkeys: Vec<String> = slots.iter().map(|s| format!("args:{}", s.name)).collect();
    let zero = |k: &String, e: &Eff| argkeys.contains(k) && *e == Eff::Set(Label::Int(0));
    let noop = |r: &Record| r.effects.iter().all(|(k, e)| zero(k, e));
    let text = |r: &Record| {
        let mut r = r.clone();
        for k in &argkeys {
            r.pre.remove(k);
        }
        r.engine_pre.retain(|k| !argkeys.contains(k));
        r.effects.retain(|(k, e)| !zero(k, e));
        r.py().text()
    };
    let mut have: BTreeMap<String, i64> = BTreeMap::new();
    for r in &b1.records {
        *have.entry(text(r)).or_default() += 1;
    }
    let mut dropped = 0;
    for r in &b0.records {
        match have.get_mut(&text(r)) {
            Some(n) if *n > 0 => *n -= 1,
            _ if noop(r) => dropped += 1,
            _ => {
                let same: Vec<String> = b1.records.iter().filter(|x| x.label == r.label).map(|x| text(x)).collect();
                panic!("record {} is gone: {}\nWITH LOCALS:\n{}", r.label, text(r), same.join("\n"))
            }
        }
    }
    let own: Vec<(String, usize)> = b1
        .slot_events
        .iter()
        .map(|e| (e.label(), b1.records.iter().filter(|r| r.label == e.label()).count()))
        .collect();
    eprintln!("room33 with dumped locals: {} records ({} before, {dropped} of them now no-ops); the slots' own: {own:?}", b1.records.len(), b0.records.len());
    assert!(own.iter().any(|o| o.1 > 0), "no record of a slot's events from its locals");
    // the replan: each slot whose events made records has its list's number; a slot with no record has no args:
    // fact in the task
    let mut lists = task::built_lists(&b1.records);
    for (k, v) in task::slot_lists(&b1) {
        lists.entry(k).or_default().extend(v);
    }
    let keys = task::record_keys(&b1.records, &[]);
    let by_name: BTreeMap<String, SlotLocals> = slots.iter().map(|s| (s.name.clone(), s.clone())).collect();
    let (s1, counts, unmatched) = task::replan_start_locals(&mut b1, &start, &Facts::new(), &keys, &by_name, &lists);
    let in_task: Vec<&scumm_reader::Event> = b1.slot_events.iter().filter(|e| keys.contains(&format!("args:{}", e.name))).collect();
    assert!(!in_task.is_empty());
    for e in &in_task {
        assert_eq!(s1.get0(&format!("args:{}", e.name)), e.sig, "{}", e.name);
    }
    assert!(unmatched.is_empty());
    assert_eq!(counts.get("live slots whose dumped locals match a built list"), in_task.len() as i64);
    assert_eq!(counts.get("live slots with unknown locals (args 0)"), 0);
    // no locals in the dump: reader.py's replan_start
    let (s2, c2, _) = task::replan_start_locals(&mut b1, &start, &Facts::new(), &keys, &BTreeMap::new(), &lists);
    let (s3, c3) = task::replan_start(&mut b1, &start, &Facts::new(), &keys);
    for k in keys.iter().filter(|k| k.starts_with("args:")) {
        assert_eq!(s2.get(k), s3.get(k), "{k}");
    }
    assert_eq!(c2.get("live slots with unknown locals (args 0)"), c3.get("live slots with unknown locals (args 0)"));
}

/// F10 under --live-locals: room 33 read with the switch and the start's slots dumped with their flags. Each slot's
/// event keeps only its unit's read locals and carries the dump's flags; at a replan each slot matches the list its
/// events were read with, also when a local its unit never reads differs, and no list when a flag differs (the
/// number covers the flags), so it is read again.
#[test]
fn room33_slot_locals_live_locals() {
    use scumm_reader::builder::{SlotFlags, SlotLocals};
    use scumm_reader::Py;
    use std::collections::BTreeMap;
    let st = static_oracle();
    let ll = std::sync::Arc::new(scumm_reader::locals::LiveLocals::of(game()));
    let mut b = oracle::builder(game(), st, "room33");
    b.table.live_locals = Some(ll.clone());
    let start = b.table.start.clone();
    let flags = SlotFlags { resistant: false, recursive: false, owned: false };
    let room = start.get0("room") as i32;
    let slots: Vec<SlotLocals> = start
        .iter()
        .filter(|(k, v)| k.starts_with("at:") && *v == 1)
        .map(|(k, _)| {
            let (n, w) = k[3..].rsplit_once(':').unwrap();
            let mut locals = vec![0i64; 25];
            locals[0] = 7;
            SlotLocals { name: n.to_string(), room, w: w.parse().unwrap(), esc: None, locals, override_: 0, cut_data: vec![], owned: false, flags: Some(flags) }
        })
        .collect();
    assert!(!slots.is_empty());
    b.slot_locals = slots.clone();
    let mut m = scumm_reader::machine::Reader::new(game());
    b.read_all_into(&mut m, true, None, 2, None);
    assert_eq!(b.slot_events.len(), slots.len());
    let res = game().resource(room);
    for e in &b.slot_events {
        let reads = ll.get(e.unit);
        assert!(e.locals.iter().all(|(i, _)| reads.is_none_or(|r| r.contains(&(*i as i32)))), "{}: an unread local", e.name);
        assert_eq!((&e.recursive, &e.resistant, &e.owned), (&Py::None, &Py::Bool(false), &Py::Bool(false)), "{}", e.name);
        assert_eq!(scumm_reader::Builder::unit_of_name(&e.name, res), Some(e.unit));
    }
    let mut lists = task::built_lists(&b.records);
    for (k, v) in task::slot_lists(&b) {
        lists.entry(k).or_default().extend(v);
    }
    let keys = task::record_keys(&b.records, &[]);
    let by_name: BTreeMap<String, SlotLocals> = slots.iter().map(|s| (s.name.clone(), s.clone())).collect();
    let in_task: Vec<scumm_reader::Event> =
        b.slot_events.iter().filter(|e| keys.contains(&format!("args:{}", e.name))).cloned().collect();
    assert!(!in_task.is_empty());
    let (s1, counts, unmatched) = task::replan_start_locals(&mut b, &start, &Facts::new(), &keys, &by_name, &lists);
    for e in &in_task {
        assert_eq!(s1.get0(&format!("args:{}", e.name)), e.sig, "{}", e.name);
    }
    assert!(unmatched.is_empty());
    assert_eq!(counts.get("live slots whose dumped locals match a built list"), in_task.len() as i64);
    // a local the unit never reads, other in the dump: the same list
    let mut unread = by_name.clone();
    let mut changed = 0;
    for s in unread.values_mut() {
        let unit = scumm_reader::Builder::unit_of_name(&s.name, res).unwrap();
        if let Some(r) = ll.get(unit) {
            if let Some(n) = (0..25).find(|n| !r.contains(n)) {
                s.locals[n as usize] = 99;
                changed += 1;
            }
        }
    }
    assert!(changed > 0);
    let (s2, _, un2) = task::replan_start_locals(&mut b, &start, &Facts::new(), &keys, &unread, &lists);
    for e in &in_task {
        assert_eq!(s2.get0(&format!("args:{}", e.name)), e.sig, "{}", e.name);
    }
    assert!(un2.is_empty());
    // a flag other than the list's: no list, read again
    let mut flipped = by_name.clone();
    for s in flipped.values_mut() {
        s.flags = Some(SlotFlags { resistant: true, ..flags });
    }
    let (s3, _, un3) = task::replan_start_locals(&mut b, &start, &Facts::new(), &keys, &flipped, &lists);
    for e in &in_task {
        assert_eq!(s3.get0(&format!("args:{}", e.name)), 0, "{}", e.name);
    }
    assert_eq!(un3.len(), in_task.len());
}

/// F10 under --live-locals: a dumped slot without flags stops the reading (a list's number covers them).
#[test]
#[should_panic(expected = "no flags")]
fn live_locals_dumped_slot_needs_flags() {
    use scumm_reader::builder::SlotLocals;
    let mut b = oracle::builder(game(), static_oracle(), "room33");
    b.table.live_locals = Some(std::sync::Arc::new(scumm_reader::locals::LiveLocals::of(game())));
    let start = b.table.start.clone();
    let (k, _) = start.iter().find(|(k, v)| k.starts_with("at:") && *v == 1).expect("a live slot at the start");
    let (n, w) = k[3..].rsplit_once(':').unwrap();
    b.slot_locals = vec![SlotLocals {
        name: n.to_string(),
        room: start.get0("room") as i32,
        w: w.parse().unwrap(),
        esc: None,
        locals: vec![0; 25],
        override_: 0,
        cut_data: vec![],
        owned: false,
        flags: None,
    }];
    b.dumped_events();
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
