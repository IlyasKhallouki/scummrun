//! Fix round 1, F1c (fix-round1.md, "F1c"): switch input_settled. A sentence or a walk whose VAR_VERB_SCRIPT is not the
//! plain input script starts at that script, with the sentence variables as constants.
//!
//! These tests run single jobs (the Builder's `run_job` with the Rust machine half) and the scope's first-pass domains.
//! MI1's room, object, script and verb numbers appear here as test data only (D1, plan.md's C2).

use scumm_reader::builder::{Job, Switches};
use scumm_reader::domain::Label;
use scumm_reader::inputs::SETTLED_PIN;
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

/// Fix round 1's reader switches with F1c (due_first is applied after a whole reading, so single jobs leave it out).
fn switches() -> Switches {
    Switches::parse("value_sets,draw_clears,actor_rooms,split_limit,input_settled").unwrap()
}

fn builder(name: &str) -> Builder<'static> {
    let mut b = oracle::builder_with(game(), static_oracle(), name, switches());
    b.table.value_sets = true;
    b
}

/// A Builder for `rooms` from seed 1's Part I start (results/start-part1.jsonl), as reader-scope makes one.
fn builder_rooms(rooms: &[i32]) -> Builder<'static> {
    let root = oracle::root();
    let start = scumm_reader::scaling::start_facts(&root, 1);
    let sw = switches();
    let doms = scumm_reader::domains::build_with(game(), &scumm_reader::domains::meta_rooms(&root), &start, &[], sw.scope());
    let scope = scumm_reader::Scope::of_with(game(), rooms, &start, sw.scope());
    let mut b = Builder::new(game(), scope, doms);
    b.switches = sw;
    b.table.value_sets = true;
    b
}

fn jobs(b: &mut Builder<'static>, label: &str) -> Vec<Job> {
    let js: Vec<Job> = b.inputs().into_iter().filter(|j| j.label == label).collect();
    assert!(!js.is_empty(), "no job {label}");
    js
}

/// The records of the jobs labelled `label`, with the Rust machine half.
fn run(b: &mut Builder<'static>, label: &str) -> Vec<Record> {
    let js = jobs(b, label);
    let mut m = scumm_reader::machine::Reader::new(game());
    js.iter().flat_map(|j| b.run_job(&mut m, j)).collect()
}

fn set(r: &Record, k: &str) -> Option<Label> {
    r.effects.iter().find(|e| e.0 == k).and_then(|e| match &e.1 {
        Eff::Set(l) => Some(*l),
        _ => None,
    })
}

fn checks(r: &Record) -> String {
    r.checks.iter().map(|a| format!("{} {}", a.cond, a.side_str())).collect::<Vec<_>>().join("; ")
}

/// The roles, as inputs.rs reads them from the scripts. MI1's numbers are the test's: global 4's doSentence
/// ([0407]), global 23's tests ([0000], [00F9], [0105]) and its one wait.
#[test]
fn roles_read_from_the_scripts() {
    let mut b = builder("room33");
    let r = b.settled_rule().expect("a plain script and a hover");
    assert_eq!(r.plain.iter().copied().collect::<Vec<_>>(), vec![4]);
    assert_eq!((r.v, r.a, r.b, r.p, r.last), (107, 108, 109, 110, 181));
    assert_eq!(r.hover, scumm_ir::UnitKey::Global(23));
    assert_eq!(r.wait, 409);
    assert_eq!(r.guards.len(), 1);
    assert_eq!(r.guards[0].key, "var:52");
    assert!(r.guards[0].holds(1) && !r.guards[0].holds(0));
}

/// F1's rule 1: keys are no input. Key 23 (global 4's game-end key, [0257]-[02A6]) gives no job, so no record: every job
/// is a today's input, and no input is a click with kKeyClickArea.
#[test]
fn keys_give_no_record() {
    let mut b = builder("kitchen");
    for j in b.inputs() {
        assert!(matches!(j.kind.as_str(), "sentence" | "walk_to" | "dialog_line"), "{}: {}", j.label, j.kind);
        match j.what {
            What::Sentence { a, b, .. } => assert!(a != 23 && b != Some(23), "{}", j.label),
            What::Verb(v) => assert_ne!(v, 23, "{}", j.label),
            What::Event(_) => panic!("an event among the inputs"),
        }
    }
}

/// F1c rule 2 and 3: a plain value of var:32 is today's job with var:32 pinned, with nothing else; another value needs
/// the hover live, not frozen and at its wait, and the cursor on; an inventory click is as today.
#[test]
fn jobs_split_on_the_input_script() {
    let mut b = builder("kitchen");
    let inputs = b.inputs();
    let mut off = oracle::builder_with(game(), static_oracle(), "kitchen", Switches::parse("value_sets,draw_clears,actor_rooms,split_limit").unwrap());
    let today = off.inputs();
    let label = "walk_to(316)";
    let plain = inputs.iter().find(|j| j.label == label).expect("the plain job");
    let was = today.iter().find(|j| j.label == label).unwrap();
    assert_eq!(plain.pre.get("var:32"), Some(&vec![Label::Int(4)]));
    let mut pre = plain.pre.clone();
    pre.remove("var:32");
    assert_eq!(pre, was.pre, "only var:32's pin differs");
    assert!(!plain.pins.contains_key(SETTLED_PIN));
    let j = inputs.iter().find(|j| j.label == "walk_to(316)[202]").expect("the bar's input script");
    for (k, v) in [("var:32", vec![202]), ("script:23", vec![1]), ("frozen:script:23", vec![0]), ("at:script:23:409", vec![1]), ("var:52", vec![1, 2])] {
        let want: Vec<Label> = v.into_iter().map(Label::Int).collect();
        let have = j.pre.get(k).unwrap_or_else(|| panic!("{k} not in the job's precondition"));
        assert!(want.iter().all(|l| have.contains(l)) && !have.contains(&Label::Int(0)) || k == "frozen:script:23", "{k}: {have:?}");
    }
    assert!(j.pre["var:53"].iter().all(|l| *l != Label::Int(0)));
    // the bar's local input script does not run in the kitchen's room
    assert!(!inputs.iter().any(|j| j.label.starts_with("walk_to(566)[202]")));
    let inv: Vec<&Job> = inputs.iter().filter(|j| j.label.contains("[inv]")).collect();
    assert!(!inv.is_empty());
    for j in inv {
        assert!(!j.pins.contains_key(SETTLED_PIN) && !j.pre.contains_key("var:32"), "{}", j.label);
    }
}

/// The bar's kitchen door (object 316, room 28) from run B's S1#7 pre state (the kitchen's start): local 202 hands the
/// click to local 203 ([001C]-[0023]), which tests the cook (actor 6) after its walk. In room 28 at x > 310 it starts
/// locals 215 and 11 and stops ([0025]-[002B]); in room 28 at x <= 310 it chains to global 4, which queues the
/// sentence, which runs ([0030]-[0055]); elsewhere it waits for ego ([003A]), and its later turn starts the door's entry
/// for the input's verb, with no sentence script ([003E]-[0053]).
#[test]
#[ignore]
fn kitchen_door_click() {
    let mut b = builder("kitchen");
    for (label, verb) in [("sentence(2,316)[202]", 2), ("walk_to(316)[202]", 11)] {
        let recs = run(&mut b, label);
        eprintln!("{label}: {} records", recs.len());
        let (mut away, mut gt, mut le) = (0, 0, 0);
        for r in &recs {
            assert_eq!(r.pre.get("var:32"), Some(&vec![Label::Int(202)]));
            let room6 = r.pre.get("actorroom:6").cloned();
            let ch = checks(r);
            let in28 = room6 == Some(vec![Label::Int(28)]) || (room6 == Some(vec![Label::Int(-1)]) && ch.contains("VAR_RESULT == 28 True"));
            let not28 = room6.as_ref().is_some_and(|s| !s.contains(&Label::Int(28)) && !s.contains(&Label::Int(-1)))
                || (room6 == Some(vec![Label::Int(-1)]) && ch.contains("VAR_RESULT == 28 False"));
            let waits = r.next_events.iter().find(|e| e.name == "local:203" && e.w == 0x3A);
            let cut = r.next_events.iter().any(|e| e.name == "local:215");
            let sentence = r.source == (What::Sentence { verb, a: 316, b: None }) && set(r, "script:2").is_some();
            eprintln!(
                "  end {:?} next {:?} room6 {room6:?} checks {ch} sentence {sentence}",
                r.end,
                r.next_events.iter().map(|e| format!("{}@{}", e.name, e.w)).collect::<Vec<_>>()
            );
            if let Some(ev) = waits {
                assert!(not28, "{label}: waits for ego with the cook in 28: {room6:?} {ch}");
                // the input's verb, carried on the slot's list (rule 8)
                assert!(ev.locals.contains(&(1107, scumm_reader::record::Val::Int(verb))), "{:?}", ev.locals);
                away += 1;
            } else if cut {
                assert!(in28 && ch.contains("VAR_RESULT > 310 True"), "{label}: local 215 without its tests: {room6:?} {ch}");
                gt += 1;
            } else if in28 && ch.contains("VAR_RESULT > 310 False") {
                le += 1;
            }
        }
        eprintln!("{label}: cook away {away}, in 28 at x > 310 {gt}, in 28 at x <= 310 {le}");
        assert!(away > 0 && gt > 0 && le > 0, "{label}: local 203's three paths: {away} {gt} {le}");
    }
}

/// The door's later turn: local 203 at its wait, with the verb carried, starts the door's Open entry (obj-0316 verb 2:
/// startScript(25,[316,570]) or startScript(214)) and script 11, with no sentence script.
#[test]
#[ignore]
fn door_later_turn_knows_the_verb() {
    let mut b = builder("kitchen");
    let recs = run(&mut b, "sentence(2,316)[202]");
    let ev = recs.iter().flat_map(|r| r.next_events.iter()).find(|e| e.name == "local:203" && e.w == 0x3A).expect("local 203 waits").clone();
    let (ok, pins, pre) = b.event_pins(&ev);
    assert!(ok);
    let job = Job { kind: "event".into(), label: ev.label(), pins, pre, what: What::Event(ev), checks: vec![] };
    let mut m = scumm_reader::machine::Reader::new(game());
    let later = b.run_job(&mut m, &job);
    assert!(!later.is_empty());
    for r in &later {
        eprintln!("later turn: end {:?} next {:?} effects {:?}", r.end, r.next_events.iter().map(|e| format!("{}@{}", e.name, e.w)).collect::<Vec<_>>(), r.effects.iter().map(|e| e.0.clone()).collect::<Vec<_>>());
    }
    // the Open entry's two paths: script 25 ran (a write of its slot), or the entry waits (groundtruth: at:obj:316:36)
    assert!(later.iter().any(|r| r.effects.iter().any(|e| e.0 == "script:25")), "the door's Open entry starts script 25");
    assert!(later.iter().any(|r| r.next_events.iter().any(|e| e.name == "obj:316" && e.w == 36)), "the door's Open entry waits at its point");
}

/// Room 57's input script, local 201 ([000B]-[0012]): a room click with VAR_VIRT_MOUSE_X >= 132 and
/// VAR_VIRT_MOUSE_Y <= 119 walks ego there and waits for it ([0049], a yield: the record ends, local 201 is kept), any
/// other chains to global 4 ([0052]), which queues the sentence. A box across the bound splits, with a replay check on
/// where to click.
#[test]
#[ignore]
fn room57_inside_outside() {
    let mut b = builder_rooms(&[57]);
    let objects = game().room_objects[&game().resource(57).unwrap()].clone();
    let labels: Vec<(i32, String)> = b
        .inputs()
        .into_iter()
        .filter(|j| j.pins.get(SETTLED_PIN).is_some() && j.pre.get("var:32") == Some(&vec![Label::Int(201)]) && j.kind == "walk_to" && !j.label.ends_with("~frozen"))
        .filter_map(|j| match j.what {
            What::Sentence { a, .. } if objects.contains(&(a as i32)) => Some((a as i32, j.label)),
            _ => None,
        })
        .collect();
    let (mut inside, mut outside, mut split) = (0, 0, 0);
    for (o, label) in labels {
        let bx = game().object_box[&o];
        let (x0, x1, y0, y1) = (bx.x, bx.x + bx.w - 1, bx.y, bx.y + bx.h - 1);
        for r in run(&mut b, &label) {
            let waits = r.next_events.iter().any(|e| e.name == "local:201" && e.w == 0x49);
            if x0 >= 132 && y1 <= 119 {
                assert!(waits, "object {o} inside: no wait for ego: {:?}", r.next_events);
                inside += 1;
            } else if x1 < 132 || y0 > 119 {
                assert!(!waits, "object {o} outside waits");
                outside += 1;
            } else {
                assert!(checks(&r).contains("VAR_VIRT_MOUSE"), "object {o} across the bound with no check: {}", checks(&r));
                split += 1;
            }
        }
    }
    eprintln!("room 57: {inside} records inside, {outside} outside, {split} on a box across the bound");
    assert!(inside > 0 && outside > 0);
}

/// Room 85's input script, local 201: a click on object 914 walks ego to it, waits ([0029], a yield) and then (an event)
/// starts 914's verb 11; any other click chains to global 33 ([0035]), which takes the object under the mouse.
#[test]
#[ignore]
fn room85_object_914() {
    let mut b = builder_rooms(&[85]);
    let recs = run(&mut b, "walk_to(914)[201]");
    assert!(recs.iter().any(|r| r.next_events.iter().any(|e| e.name == "local:201" && e.w == 0x29)), "914: no wait");
    let objects = game().room_objects[&game().resource(85).unwrap()].clone();
    let other = objects.iter().find(|&&o| o != 914).unwrap();
    let recs = run(&mut b, &format!("walk_to({other})[201]"));
    assert!(!recs.iter().any(|r| r.next_events.iter().any(|e| e.name == "local:201" && e.w == 0x29)), "{other} waits");
    eprintln!("room 85: {} records on {other}", recs.len());
}
