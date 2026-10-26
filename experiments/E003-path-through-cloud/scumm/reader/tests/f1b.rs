//! Fix round 1, F1b (fix-round1.md, "F1b"; decision D10): switch sentence_queue, on top of input_script. A click's
//! record ends before checkAndRunSentenceScript; the queued sentence is facts (`queue`, `queue:verb`, `queue:a`,
//! `queue:b`) and runs as its own event (kind `queued`).
//!
//! These tests run single jobs (the Builder's `run_job` with the Rust machine half, the reading's first-pass domains),
//! not whole readings: the kitchen's whole reading with the switch passes the size gate (notes/log). MI1's room,
//! object, script and verb numbers appear here as test data only (D1, plan.md's C2).

use scumm_reader::builder::{Job, Switches};
use scumm_reader::domain::Label;
use scumm_reader::oracle::{self, Static};
use scumm_reader::record::{Eff, Record, What};
use scumm_reader::Builder;
use std::sync::OnceLock;

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

fn static_oracle() -> &'static Static {
    static S: OnceLock<Static> = OnceLock::new();
    S.get_or_init(Static::load)
}

/// Fix round 1's switches with F1 and F1b (due_first is applied after a whole reading, so single jobs leave it out).
fn f1b() -> Switches {
    Switches::parse("value_sets,draw_clears,actor_rooms,input_script,sentence_queue").unwrap()
}

fn builder(name: &str) -> Builder<'static> {
    let mut b = oracle::builder_with(game(), static_oracle(), name, f1b());
    // what `read_all_into` sets before a reading: the machine half takes value_sets from the table
    b.table.value_sets = b.switches.value_sets;
    b
}

/// A Builder for `rooms` from seed 1's Part I start (results/start-part1.jsonl), as reader-scope makes one.
fn builder_rooms(rooms: &[i32]) -> Builder<'static> {
    let root = oracle::root();
    let start = scumm_reader::scaling::start_facts(&root, 1);
    let sw = f1b();
    let doms = scumm_reader::domains::build_with(game(), &scumm_reader::domains::meta_rooms(&root), &start, &[], sw.scope());
    let scope = scumm_reader::Scope::of_with(game(), rooms, &start, sw.scope());
    let mut b = Builder::new(game(), scope, doms);
    b.switches = sw;
    b.table.value_sets = sw.value_sets;
    b
}

/// The jobs of `b` whose label is `label`, run with the Rust machine half.
fn run(b: &mut Builder<'static>, label: &str) -> Vec<Record> {
    run_pinned(b, label, &[])
}

/// `run` with facts pinned to one value each, as an event's position is (`Builder::pin`).
fn run_pinned(b: &mut Builder<'static>, label: &str, fixed: &[(&str, i64)]) -> Vec<Record> {
    let mut jobs: Vec<Job> = b.inputs().into_iter().filter(|j| j.label == label).collect();
    assert!(!jobs.is_empty(), "no job {label}");
    for j in &mut jobs {
        for (k, v) in fixed {
            assert!(b.pin(&mut j.pins, &mut j.pre, k, &[Label::Int(*v)]), "{k} = {v} never holds");
        }
    }
    let mut m = scumm_reader::machine::Reader::new(game());
    jobs.iter().flat_map(|j| b.run_job(&mut m, j)).collect()
}

/// The cells of room `room` where object `o` is the first box (findObject tries the boxes in the list's order).
fn cells_of(room: i32, o: i32) -> Vec<usize> {
    let g = game();
    let res = g.resource(room).unwrap();
    g.cells[1][&res].iter().enumerate().filter(|(_, c)| c.boxes.first().map(|b| b.0) == Some(o)).map(|(i, _)| i).collect()
}

fn set(r: &Record, k: &str) -> Option<Label> {
    r.effects.iter().find(|e| e.0 == k).and_then(|e| match &e.1 {
        Eff::Set(l) => Some(*l),
        _ => None,
    })
}

fn pre(r: &Record, k: &str) -> Option<Vec<Label>> {
    r.pre.get(k).cloned()
}

fn checks(r: &Record) -> Vec<String> {
    r.checks.iter().map(|a| format!("{} {}", a.cond, a.side_str())).collect()
}

/// F1b rule 8: a slot verb is clicked anywhere on the verb screen, from the least SetScreen bottom under 200 (MI1:
/// SetScreen(0,144) and SetScreen(0,200), so 144) to 199. The size probe took 200 to 199.
#[test]
fn verb_screen_top() {
    let b = builder("room33");
    let inp = b.scope.inputs.as_ref().unwrap();
    assert_eq!(inp.verb_top, 144);
    assert_eq!(inp.screen, None, "MI1's SetScreen calls disagree");
}

/// F1 rule 1: keys are no input. Key 23 (global 4's "win" key, [0257]-[02A6]) gives no job, so no record.
#[test]
fn keys_give_no_record() {
    let mut b = builder("room33");
    let jobs = b.inputs();
    for j in &jobs {
        if let What::Input { area, .. } = j.what {
            assert!(area == 1 || area == 2, "{}: area {area} (kKeyClickArea is 4, verbs.h:38)", j.label);
        }
    }
    assert!(jobs.iter().any(|j| j.kind == "queued"));
    assert!(!jobs.iter().any(|j| matches!(j.what, What::Input { value: 23, area: 4, .. })));
}

/// F1b rules 1, 2 and 6 on room 33: a cell click queues the sentence as facts and runs no sentence script; every
/// record but the queued sentence's needs `queue` 0 or 2.
#[test]
fn room33_click_queues() {
    let mut b = builder("room33");
    let recs = run(&mut b, "click(33.2,1)");
    assert!(!recs.is_empty());
    let mut queued = 0;
    for r in &recs {
        assert_eq!(pre(r, "queue"), Some(vec![Label::Int(0), Label::Int(2)]), "{}", r.label);
        assert_ne!(set(r, "script:2"), Some(Label::Int(1)), "a click's record runs no sentence script");
        if set(r, "queue") == Some(Label::Int(1)) {
            queued += 1;
            assert!(r.effects.iter().any(|e| e.0 == "queue:verb"));
        }
    }
    assert!(queued > 0, "no click record queues a sentence");
    let q = run(&mut b, "queued(2,426)");
    assert!(!q.is_empty());
    for r in &q {
        assert_eq!(pre(r, "queue"), Some(vec![Label::Int(1), Label::Int(2)]));
        assert_eq!(pre(r, "queue:verb"), Some(vec![Label::Int(2)]));
        assert_eq!(pre(r, "queue:a"), Some(vec![Label::Int(426)]));
    }
    eprintln!("room 33: click(33.2,1) {} records ({queued} queue a sentence); queued(2,426) {}", recs.len(), q.len());
}

/// The bar's kitchen door (object 316, room 28) from run B's S1#7 pre state (the kitchen's start): the room's input
/// script, local 202, hands the click to local 203 ([001C]-[0023]), which tests the cook (actor 6) after its walk:
/// in room 28 at x > 310 it starts locals 215 and 11 and stops ([0025]-[002B]); in room 28 at x <= 310 it chains to
/// global 4, which queues the sentence ([0030]-[0055]); elsewhere it waits for ego ([003A]), then (an event) starts
/// the door's own entry, with no sentence script. In run B's S1#7 the cook was in the kitchen: local 203 waited, then
/// ran the door's Open entry (results/groundtruth-part1.jsonl: at:local:203:80, at:obj:316:36).
#[test]
#[ignore]
fn kitchen_door_click() {
    let mut b = builder("kitchen");
    let cells = cells_of(28, 316);
    assert!(!cells.is_empty(), "no cell of the door");
    let mut away = 0;
    let mut gt = 0;
    let mut le = 0;
    for c in cells {
        let label = format!("click(28.{c},1)");
        // the hover (global 23) and the cursor as at the start: the hover live, not frozen, at its loop's wait, the
        // cursor on (VAR_CURSORSTATE 1). Without these pins the run's first 1,024 paths take the hover's slot as not
        // running or the cursor as off (VAR_CURSORSTATE and VAR_USERPUT are separate facts), and the path limit cuts
        // the others
        let recs = run_pinned(&mut b, &label, &[("script:23", 1), ("frozen:script:23", 0), ("at:script:23:409", 1), ("var:52", 1)]);
        if std::env::var_os("F1B_DEBUG").is_some() {
            eprintln!("{label}: {} records, cell boxes {:?}", recs.len(), game().cells[1][&game().resource(28).unwrap()][c].boxes);
            for r in recs.iter() {
                let nx: Vec<String> = r.next_events.iter().map(|e| format!("{}@{}", e.name, e.w)).collect();
                let pk: Vec<String> = ["var:32", "var:108", "var:107", "actorroom:6", "var:52", "bit:563", "var:181", "var:110", "var:53"].iter().map(|k| format!("{k}={:?}", r.pre.get(*k))).collect();
                eprintln!("  end {:?} next {nx:?} queue {:?} a {:?} w108 {:?} w181 {:?} {} checks {:?}", r.end, set(r, "queue"), set(r, "queue:a"), set(r, "var:108"), set(r, "var:181"), pk.join(" "), checks(r));
            }
        }
        for r in recs {
            if !r.pre.get("var:32").is_none_or(|s| s.contains(&Label::Int(202))) {
                continue;
            }
            let room6 = pre(&r, "actorroom:6");
            let ch = checks(&r).join("; ");
            // the cook's room: a fact (F9), or not known at the start (NOT_KNOWN, -1), when getActorRoom is a replay
            // check
            let in28 = room6 == Some(vec![Label::Int(28)])
                || (room6 == Some(vec![Label::Int(-1)]) && ch.contains("VAR_RESULT == 28 True"));
            let not28 = room6.as_ref().is_some_and(|s| !s.contains(&Label::Int(28)) && !s.contains(&Label::Int(-1)))
                || (room6 == Some(vec![Label::Int(-1)]) && ch.contains("VAR_RESULT == 28 False"));
            if r.next_events.iter().any(|e| e.name == "local:203" && e.w == 0x3A) {
                assert!(not28, "{label}: waits for ego with the cook in 28: {room6:?} {ch}");
                assert_ne!(set(&r, "queue"), Some(Label::Int(1)));
                away += 1;
            } else if r.next_events.iter().any(|e| e.name == "local:215") {
                assert!(in28 && ch.contains("VAR_RESULT > 310 True"), "{label}: local 215 without its tests: {room6:?} {ch}");
                assert_ne!(set(&r, "queue"), Some(Label::Int(1)));
                gt += 1;
            } else if set(&r, "queue") == Some(Label::Int(1)) && set(&r, "queue:a") == Some(Label::Int(316)) {
                assert!(in28 && ch.contains("VAR_RESULT > 310 False"), "{label}: the door's sentence without its tests: {room6:?} {ch}");
                le += 1;
            }
        }
    }
    eprintln!("kitchen door: cook away {away}, in 28 at x > 310 {gt}, in 28 at x <= 310 {le} records");
    assert!(away > 0 && gt > 0 && le > 0, "local 203's three paths: {away} {gt} {le}");
}

/// The records of `b`'s left clicks on each cell of `room`, with VAR_VERB_SCRIPT `script` (the room's input script).
fn clicks(b: &mut Builder<'static>, room: i32, script: i64) -> Vec<(usize, Record)> {
    let res = game().resource(room).unwrap();
    let n = game().cells[1][&res].len();
    let mut out = Vec::new();
    for c in 0..n {
        for r in run(b, &format!("click({room}.{c},1)")) {
            if r.pre.get("var:32").is_none_or(|s| s == &vec![Label::Int(script)]) {
                out.push((c, r));
            }
        }
    }
    out
}

/// Room 57's input script, local 201 ([000B]-[0012]): a room click with VAR_VIRT_MOUSE_X >= 132 and
/// VAR_VIRT_MOUSE_Y <= 119 walks ego there and waits for it ([0049], a yield: the record ends, local 201 is kept),
/// any other chains to global 4 ([0052]), which queues the sentence. A cell on both sides splits, with a replay check
/// on where to click.
#[test]
#[ignore]
fn room57_inside_outside() {
    let mut b = builder_rooms(&[57]);
    let res = game().resource(57).unwrap();
    let cells = game().cells[1][&res].clone();
    let (mut inside, mut outside, mut split) = (0, 0, 0);
    for (c, r) in clicks(&mut b, 57, 201) {
        let cell = &cells[c];
        let waits = r.next_events.iter().any(|e| e.name == "local:201" && e.w == 0x49);
        if cell.x.0 >= 132 && cell.y.1 <= 119 {
            if r.pre.get("var:107").is_some_and(|v| v.iter().all(|l| ![Label::Int(10), Label::Int(4), Label::Int(7)].contains(l))) {
                assert!(waits, "cell {c} inside: no wait for ego: {:?}", r.next_events);
                inside += 1;
            }
        } else if cell.x.1 < 132 || cell.y.0 > 119 {
            assert!(!waits, "cell {c} outside waits");
            outside += 1;
        } else if checks(&r).iter().any(|ch| ch.contains("132") || ch.contains("119")) {
            split += 1;
        }
    }
    eprintln!("room 57: {inside} records inside, {outside} outside, {split} on a split cell");
    assert!(inside > 0 && outside > 0);
}

/// Room 85's input script, local 201: a click on object 914 walks ego to it, waits ([0029], a yield) and then (an
/// event) starts 914's verb 11; any other click chains to global 33 ([0035]).
#[test]
#[ignore]
fn room85_object_914() {
    let mut b = builder_rooms(&[85]);
    let on914 = cells_of(85, 914);
    assert!(!on914.is_empty());
    let res = game().resource(85).unwrap();
    let cells = game().cells[1][&res].clone();
    let (mut walk, mut other) = (0, 0);
    for (c, r) in clicks(&mut b, 85, 201) {
        let waits = r.next_events.iter().any(|e| e.name == "local:201" && e.w == 0x29);
        if on914.contains(&c) && waits {
            walk += 1;
        }
        if !cells[c].boxes.iter().any(|b| b.0 == 914) {
            assert!(!waits, "cell {c} has no box of 914 and waits");
            other += 1;
        }
    }
    eprintln!("room 85: {walk} records walk to 914 and wait, {other} on other cells");
    assert!(walk > 0 && other > 0);
}
