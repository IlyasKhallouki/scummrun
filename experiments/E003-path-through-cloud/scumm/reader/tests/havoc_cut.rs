//! The switch havoc_cut (experiments/E004-fast-exact/havoc-cut.md): each path the step limit ended becomes one havoc
//! record, with its precondition, a guess or a constant-or-nothing case for each fact the code it can still run may
//! write, and an event with unknown locals at each wait point of that code; off, the reading stops. The ignored test
//! runs the Part I job whose paths ended at the step limit, with the Rust machine.

use scumm_ir::UnitKey;
use scumm_reader::builder::{Job, Machine, RunOut, Switches};
use scumm_reader::domain::Label;
use scumm_reader::oracle::{self, Static};
use scumm_reader::record::{Cut, PathOut, Pins, Pre, Record, Val, What};
use scumm_reader::{havoc, Builder, Ctx, Eff};
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

fn static_oracle() -> &'static Static {
    static S: OnceLock<Static> = OnceLock::new();
    S.get_or_init(Static::load)
}

/// A machine whose run has one path that ends and `cuts` paths the step limit ended at `cut`.
struct Fake {
    cut: Cut,
    cuts: usize,
    write: (String, Val),
}

impl Machine for Fake {
    fn run(&mut self, _cx: &Ctx, _label: &str, _pins: &Pins, pre: &Pre, _what: &What) -> RunOut {
        let writes = BTreeMap::from([self.write.clone()]);
        let mut paths = vec![PathOut { end: Some("end".into()), sym: true, pre: pre.clone(), writes, ..Default::default() }];
        for _ in 0..self.cuts {
            paths.push(PathOut { end: Some("limit".into()), sym: true, pre: pre.clone(), cut: Some(self.cut.clone()), ..Default::default() });
        }
        RunOut { paths, ..Default::default() }
    }
}

fn job() -> Job {
    Job { kind: "dialog_line".into(), label: "fake".into(), pins: Pins::new(), pre: Pre::new(), what: What::Verb(1), checks: vec![] }
}

/// Room 33's builder, and a unit of its scope whose code writes a fact by a constant, cut at its first instruction.
fn setup(on: bool) -> (Builder<'static>, Fake, havoc::HavocWrites, UnitKey) {
    let sw = Switches { split_limit: true, havoc_cut: on, ..Default::default() };
    let mut b = oracle::builder_with(game(), static_oracle(), "room33", sw);
    b.table.split_limit = true;
    b.table.havoc_cut = on;
    let mut found = None;
    for &u in &b.scope.units {
        let k = game().unit(u).key;
        let cut = Cut { frames: vec![(k, Some(0))], steps: vec![] };
        let hw = havoc::writes(game(), &b.scope, &b.table, &cut, None);
        if hw.facts.values().any(|v| v.is_some()) && !hw.slots.is_empty() {
            found = Some((cut, hw, k));
            break;
        }
    }
    let (cut, hw, k) = found.expect("a unit of room 33's scope that writes a fact by a constant");
    let key = b.table.doms.doms.keys().find(|k| k.starts_with("var:") && b.kind(k) == scumm_reader::table::Kind::Fact).unwrap().clone();
    let v = b.domain(&key).unwrap().values.iter().find_map(|l| l.int()).unwrap();
    (b, Fake { cut, cuts: 2, write: (key, Val::Int(v)) }, hw, k)
}

fn havocs(recs: &[Record]) -> Vec<&Record> {
    recs.iter().filter(|r| r.end.as_deref() == Some("havoc")).collect()
}

#[test]
fn each_cut_path_is_one_havoc_record() {
    let (mut b, mut m, hw, unit) = setup(true);
    let recs = b.run_job(&mut m, &job());
    let h = havocs(&recs);
    assert_eq!(h.len(), 2, "one havoc record per path the step limit ended");
    assert_eq!(recs.len(), 3, "and the path that ended, as before");
    assert_eq!(b.counts.get("havoc_cut: paths the step limit ended, made havoc records: dialog_line"), 2);
    let r = h[0];
    // every fact the cut code may write is written: a guess, or cases on its current value
    for (k, vals) in &hw.facts {
        let Some(e) = r.effects.iter().find(|e| &e.0 == k) else {
            assert!(b.domain(k).is_none_or(|d| d.values.len() < 2), "{k} not written");
            continue;
        };
        match (vals, &e.1) {
            (None, Eff::Guess(g)) => assert_eq!(g, &b.domain(k).unwrap().values),
            (Some(vs), Eff::Cases(cs)) => {
                for (when, out) in cs {
                    assert_eq!(when.len(), 1);
                    assert!(out.contains(&when[0].1[0]), "{k}: no change is a choice");
                    for v in vs {
                        assert!(out.contains(&Label::Int(*v)), "{k}: {v} is a choice");
                    }
                }
            }
            (Some(_), Eff::Guess(_) | Eff::Set(_)) => {}
            (v, e) => panic!("{k}: {v:?} as {e:?}"),
        }
    }
    // the cut unit's slot: anywhere or nowhere, its list unknown, an event with unknown locals at each wait point
    let name = scumm_ir::Game::unit_name(unit).to_string();
    assert!(r.effects.iter().any(|e| e.0 == format!("args:{name}") && e.1 == Eff::Set(Label::Int(0))));
    let u = game().id(unit).unwrap();
    for w in scumm_reader::scope::stop_points(game(), u) {
        if b.table.is_point("at", &name, w as i64) {
            assert!(r.next_events.iter().any(|e| e.name == name && e.w == w as i64 && e.sig == 0 && !e.abort), "{name}@{w}");
        }
    }
    assert_eq!(r.pre, recs[0].pre, "the path's precondition");
}

#[test]
#[should_panic(expected = "paths the step limit ended; the reading stops")]
fn off_the_reading_stops() {
    let (mut b, mut m, _, _) = setup(false);
    b.run_job(&mut m, &job());
}

#[test]
fn off_the_machine_hands_over_no_cut() {
    let (b, _, _, _) = setup(false);
    assert!(!b.table.havoc_cut);
    let (b, _, _, _) = setup(true);
    assert!(b.table.havoc_cut);
}

/// The machine half, keeping where each cut path stood.
struct Keep<'g> {
    rd: scumm_reader::machine::Reader<'g>,
    cuts: Vec<Cut>,
}

impl Machine for Keep<'_> {
    fn run(&mut self, cx: &Ctx, label: &str, pins: &Pins, pre: &Pre, what: &What) -> RunOut {
        let out = self.rd.run(cx, label, pins, pre, what);
        self.cuts.extend(out.paths.iter().filter_map(|p| p.cut.clone()));
        out
    }
    fn set_path_limit(&mut self, n: usize) {
        self.rd.set_path_limit(n);
    }
}

/// Part I's scope as reader-scope --part1 reads it, with run D's switches (E004's lab note of 2026-10-31).
fn part1(havoc: bool) -> Builder<'static> {
    let g = game();
    let root = scumm_ir::default_root();
    let start = scumm_reader::scaling::start_facts(&root, 1);
    let goal = scumm_reader::scaling::part1_goal(&root);
    let rooms = scumm_reader::scaling::part1_rooms(g, &root);
    let sw = Switches::parse("value_sets,draw_clears,actor_rooms,split_limit,grow_limit").unwrap();
    let sw = Switches { havoc_cut: havoc, ..sw };
    let doms = scumm_reader::domains::build_with(g, &scumm_reader::domains::meta_rooms(&root), &start, &goal, sw.scope());
    let scope = scumm_reader::Scope::of_with(g, &rooms, &start, sw.scope());
    let mut b = Builder::new(g, scope, doms);
    b.switches = sw;
    b.table.elsewhere = true;
    b.table.live_locals = Some(std::sync::Arc::new(scumm_reader::locals::LiveLocals::of(g)));
    b.table.value_sets = true;
    b.table.split_limit = true;
    b.table.grow_limit = true;
    b.table.havoc_cut = havoc;
    b
}

const STOP: &str = "dialog_line(120)[204@28]";

#[test]
#[ignore]
fn part1_the_job_that_stopped_the_reading() {
    let mut b = part1(true);
    let jobs: Vec<Job> = b.inputs().into_iter().filter(|j| j.label == STOP).collect();
    assert_eq!(jobs.len(), 1, "{STOP} among Part I's inputs");
    let mut m = Keep { rd: scumm_reader::machine::Reader::new(game()), cuts: vec![] };
    let t = std::time::Instant::now();
    let recs = b.run_job(&mut m, &jobs[0]);
    let h = havocs(&recs);
    eprintln!("{STOP}: {} records, {} havoc records, {:.1} s", recs.len(), h.len(), t.elapsed().as_secs_f64());
    assert!(!m.cuts.is_empty(), "paths the step limit ended");
    assert_eq!(h.len(), m.cuts.len(), "one havoc record per path the step limit ended");
    for (c, r) in m.cuts.iter().zip(&h) {
        let hw = havoc::writes(game(), &b.scope, &b.table, c, Some(28));
        let facts: Vec<String> = hw.facts.iter().map(|(k, v)| format!("{k}{}", if v.is_some() { "=c" } else { "" })).collect();
        eprintln!(
            "  cut at {:?}, pending {:?}; closure {} units; {} facts ({} with constants), {} slots; record: {} effects, {} events",
            c.frames,
            c.steps,
            hw.closure_units,
            hw.facts.len(),
            hw.facts.values().filter(|v| v.is_some()).count(),
            hw.slots.len(),
            r.effects.len(),
            r.next_events.len()
        );
        eprintln!("  facts: {}", facts.iter().take(40).cloned().collect::<Vec<_>>().join(" "));
        let top = c.frames.last().unwrap();
        assert_eq!(top.0, UnitKey::Local(28, 204), "the cut path runs room 28's local 204");
    }
    // off, the same job stops the reading
    let mut b = part1(false);
    let job = jobs[0].clone();
    let mut m = scumm_reader::machine::Reader::new(game());
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| b.run_job(&mut m, &job)));
    let msg = r.err().and_then(|e| e.downcast_ref::<String>().cloned()).unwrap_or_default();
    assert!(msg.contains("paths the step limit ended; the reading stops"), "{msg}");
}
