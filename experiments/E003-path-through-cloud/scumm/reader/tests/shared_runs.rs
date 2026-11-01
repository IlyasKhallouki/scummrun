//! The switch shared_runs (experiments/E004-fast-exact/shared-runs.md): the dialog line jobs that differ only in their
//! verb, run with one shared run, give byte for byte the records and counters of each run alone. On k = 2 (fast) and,
//! ignored, on Part I, the five `dialog_line(120..124)[202@85]` jobs.

use scumm_reader::builder::{Job, Switches};
use scumm_reader::oracle;
use scumm_reader::shared::SharedRuns;
use scumm_reader::Builder;
use std::sync::{Arc, OnceLock};

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

/// A scope as reader-scope reads it, with run D's switches (E004's lab note of 2026-10-31), no --reach.
fn builder(rooms: Vec<i32>) -> Builder<'static> {
    let g = game();
    let root = scumm_ir::default_root();
    let start = scumm_reader::scaling::start_facts(&root, 1);
    let goal = scumm_reader::scaling::part1_goal(&root);
    let sw = Switches::parse("value_sets,draw_clears,actor_rooms,split_limit,grow_limit").unwrap();
    let doms = scumm_reader::domains::build_with(g, &scumm_reader::domains::meta_rooms(&root), &start, &goal, sw.scope());
    let scope = scumm_reader::Scope::of_with(g, &rooms, &start, sw.scope());
    let mut b = Builder::new(g, scope, doms);
    b.switches = sw;
    b.table.elsewhere = true;
    b.table.live_locals = Some(Arc::new(scumm_reader::locals::LiveLocals::of(g)));
    b.table.value_sets = true;
    b.table.split_limit = true;
    b.table.grow_limit = true;
    b
}

fn k2_rooms() -> Vec<i32> {
    let root = scumm_ir::default_root();
    let start = scumm_reader::scaling::start_facts(&root, 1);
    scumm_reader::scaling::scopes(game(), start.get0("room") as i32)[2].clone()
}

fn part1_rooms() -> Vec<i32> {
    scumm_reader::scaling::part1_rooms(game(), &scumm_ir::default_root())
}

fn jobs(b: &mut Builder, at: &str) -> Vec<Job> {
    b.inputs().into_iter().filter(|j| j.kind == "dialog_line" && j.label.ends_with(at)).collect()
}

/// Each job's records (golden.py's text) and the counters after all of them, with the switch on or off; and the
/// shared runs' counts.
fn read(rooms: Vec<i32>, at: &str, on: bool, only: usize) -> (Vec<Vec<String>>, String, Option<(u64, u64, u64)>) {
    let mut b = builder(rooms);
    let js: Vec<Job> = jobs(&mut b, at).into_iter().take(only).collect();
    assert!(js.len() >= 2, "jobs at {at}: {}", js.len());
    let sh = on.then(|| Arc::new(SharedRuns::default()));
    if let Some(sh) = &sh {
        sh.next_batch();
    }
    b.table.shared = sh.clone();
    let mut m = scumm_reader::machine::Reader::new(game());
    let mut out = Vec::new();
    for j in &js {
        let t = std::time::Instant::now();
        let recs = b.run_job(&mut m, j);
        eprintln!("{} ({}): {} records, {:.1} s", j.label, if on { "on" } else { "off" }, recs.len(), t.elapsed().as_secs_f64());
        out.push(recs.iter().map(|r| r.py().text()).collect());
    }
    let counts = format!("{}\n{}", b.counters_py().text(), b.counts.py().text());
    let n = |a: &std::sync::atomic::AtomicU64| a.load(std::sync::atomic::Ordering::Relaxed);
    (out, counts, sh.map(|s| (n(&s.made), n(&s.taken), n(&s.not_kept))))
}

fn same(rooms: Vec<i32>, at: &str, only: usize, shared: bool) {
    let (off, coff, _) = read(rooms.clone(), at, false, only);
    let (on, con, st) = read(rooms, at, true, only);
    let (made, taken, refused) = st.unwrap();
    eprintln!("{at}: {} jobs, made {made}, taken {taken}, not kept {refused}", off.len());
    assert_eq!(off.len(), on.len());
    for (i, (a, b)) in off.iter().zip(&on).enumerate() {
        assert_eq!(a, b, "job {i}'s records");
    }
    assert_eq!(coff, con, "the counters");
    if shared {
        assert!(taken >= 1, "at least one job took another's run");
    }
}

#[test]
fn k2_dialog_jobs_of_202_at_85() {
    same(k2_rooms(), "[202@85]", 5, true);
}

/// In Part I's scope the run reads the dialog verbs (the room change it makes reaches code that does), so no job
/// takes another's run; the records are the same either way.
#[test]
#[ignore]
fn part1_dialog_jobs_of_202_at_85() {
    same(part1_rooms(), "[202@85]", 5, false);
}
