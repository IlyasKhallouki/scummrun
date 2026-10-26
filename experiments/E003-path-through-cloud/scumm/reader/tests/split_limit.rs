//! The switch split_limit (PORT-NOTES.md, "The path limit"): a run past the path limit is split on its first fork on
//! a read until no part passes the limit, and a run that passes it with no fact left to split on, or a path the step
//! limit ended, stops the reading.
//!
//! Fast: the rule on a machine that passes the limit while given facts are unpinned (deeper than SPLIT_DEPTH, more
//! forks on non-facts than on facts: reader.py's rule leaves such a run unsplit), and room 33 read with the path limit
//! lowered to 32: with the switch, its relaxed reach (no deletes, from the start) is the uncut reading's; without it,
//! the reading drops paths and reaches less. (At 24 and below a part of room 33 passes the limit on non-fact forks
//! alone, and the switch stops the reading, as it should.)
//!
//! Slow (`-- --ignored`): rooms 33, 28 and 35 from results/start-part1.jsonl's seed 1 with `--elsewhere
//! --live-locals`, as reader-scope reads them: without the switch the walk through the kitchen door is cut and the
//! relaxed reach from the start holds neither room 28 nor the open door (state:316 = 1); with it, both. MI1's fact names appear here as test data only (D1, plan.md's C2).

use scumm_reader::builder::{Job, Machine, RdCounts, RunOut, Switches, FORKS_ON_NONFACTS, FORKS_ON_READ};
use scumm_reader::domain::Label;
use scumm_reader::oracle::{self, Static};
use scumm_reader::record::{PathOut, Pins, Pre, Val, What};
use scumm_reader::table::Kind;
use scumm_reader::{task, Builder, Ctx, Facts};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

fn static_oracle() -> &'static Static {
    static S: OnceLock<Static> = OnceLock::new();
    S.get_or_init(Static::load)
}

const SPLIT: &str = "runs past the path limit split on their first fork on a read";
const CUT: &str = "paths ended by the path limit";
const NOT_SPLIT: &str = "runs past the path limit not split (more forks on non-facts than on facts)";

/// A machine whose run passes the path limit (3 paths cut) while one of `keys` is unpinned, its first fork the first
/// such key, with more forks on non-facts than on facts. With every key pinned it ends one path that writes `target`.
struct Fake {
    keys: Vec<String>,
    target: (String, i64),
    /// the keys' pins of each run that had them all (as text)
    leaves: Vec<Vec<String>>,
    /// a run with this many keys pinned has no fact to split on (first fork None)
    no_fact_at: Option<usize>,
    /// the leaves' paths end "limit" (the step limit)
    step_limit: bool,
}

impl Machine for Fake {
    fn run(&mut self, _cx: &Ctx, _label: &str, pins: &Pins, pre: &Pre, _what: &What) -> RunOut {
        let mut rd = RdCounts::default();
        rd.counts.add(FORKS_ON_READ, 1);
        rd.counts.add(FORKS_ON_NONFACTS, 10);
        let pinned = self.keys.iter().filter(|k| pins.contains_key(*k)).count();
        if let Some(k) = self.keys.iter().find(|k| !pins.contains_key(*k)) {
            let first_fork = (self.no_fact_at != Some(pinned)).then(|| k.clone());
            let cut = PathOut { end: Some("limit".into()), sym: true, pre: pre.clone(), ..Default::default() };
            return RunOut { paths: vec![cut], limit: 3, first_fork, rd, ..Default::default() };
        }
        self.leaves.push(self.keys.iter().map(|k| format!("{:?}", pins[k])).collect());
        let writes = BTreeMap::from([(self.target.0.clone(), Val::Int(self.target.1))]);
        let end = if self.step_limit { "limit" } else { "end" };
        RunOut { paths: vec![PathOut { end: Some(end.into()), sym: true, pre: pre.clone(), writes, ..Default::default() }], rd, ..Default::default() }
    }
}

/// Room 33's Builder with the switch on or off, four facts of 2 or 3 values for the fake's forks, a fifth to write.
fn fake_setup(on: bool) -> (Builder<'static>, Fake) {
    let mut b = oracle::builder_with(game(), static_oracle(), "room33", Switches { split_limit: on, ..Default::default() });
    b.table.split_limit = on;
    let mut keys: Vec<String> = b
        .table
        .doms
        .doms
        .keys()
        .filter(|k| k.starts_with("var:") || k.starts_with("state:"))
        .filter(|k| b.kind(k) == Kind::Fact && b.domain(k).is_some_and(|d| (2..=3).contains(&d.values.len())))
        .cloned()
        .collect();
    keys.sort();
    let target = keys.pop().unwrap();
    let value = b.domain(&target).unwrap().values.iter().find_map(|l| if let Label::Int(n) = l { Some(*n) } else { None }).unwrap();
    keys.truncate(4);
    assert_eq!(keys.len(), 4);
    (b, Fake { keys, target: (target, value), leaves: Vec::new(), no_fact_at: None, step_limit: false })
}

fn fake_job() -> Job {
    Job { kind: "sentence".into(), label: "fake".into(), pins: Pins::new(), pre: Pre::new(), what: What::Verb(1), checks: vec![] }
}

#[test]
fn split_limit_splits_until_no_part_passes() {
    let (mut b, mut m) = fake_setup(true);
    let recs = b.run_job(&mut m, &fake_job());
    let sizes: Vec<usize> = m.keys.iter().map(|k| b.domain(k).unwrap().values.len()).collect();
    // every combination of the four facts' values once: the parts do not overlap and cover the run's states
    let mut want: BTreeSet<Vec<String>> = BTreeSet::from([vec![]]);
    for k in &m.keys {
        let vals: Vec<String> = b.domain(k).unwrap().values.iter().map(|&l| format!("{:?}", b.table.value(k, l))).collect();
        want = want.iter().flat_map(|w| vals.iter().map(move |v| [w.clone(), vec![v.clone()]].concat())).collect();
    }
    let got: BTreeSet<Vec<String>> = m.leaves.iter().cloned().collect();
    assert_eq!(m.leaves.len(), want.len(), "a combination run twice");
    assert_eq!(got, want);
    assert_eq!(recs.len(), want.len());
    for r in &recs {
        for k in &m.keys {
            assert_eq!(r.pre.get(k).map(|s| s.len()), Some(1), "{k} pinned in each record");
        }
    }
    // splits at four levels (reader.py's rule stops at SPLIT_DEPTH 2), none refused, no path dropped
    let inner: i64 = (0..4).map(|i| sizes[..i].iter().product::<usize>() as i64).sum();
    assert_eq!(b.counts.get(SPLIT), inner);
    assert_eq!(b.counts.get("split_limit: runs split at depth 4"), sizes[..3].iter().product::<usize>() as i64);
    assert_eq!(b.counts.get(CUT), 0);
    assert_eq!(b.counts.get(NOT_SPLIT), 0);
    assert!(b.drops.0.is_empty(), "no path dropped");
}

#[test]
fn split_limit_off_is_readerpy_rule() {
    let (mut b, mut m) = fake_setup(false);
    let recs = b.run_job(&mut m, &fake_job());
    assert!(recs.is_empty());
    assert!(m.leaves.is_empty());
    assert_eq!(b.counts.get(NOT_SPLIT), 1);
    assert_eq!(b.counts.get(CUT), 3);
}

#[test]
#[should_panic(expected = "fake passed the path limit (3 paths cut) with no fact left to split on")]
fn split_limit_stops_with_no_fact_left() {
    let (mut b, mut m) = fake_setup(true);
    m.no_fact_at = Some(2);
    b.run_job(&mut m, &fake_job());
}

#[test]
#[should_panic(expected = "fake has 1 paths the step limit ended")]
fn split_limit_stops_at_the_step_limit() {
    let (mut b, mut m) = fake_setup(true);
    m.step_limit = true;
    b.run_job(&mut m, &fake_job());
}

/// Room 33 read on one thread with the switch on or off and the Reader's path limit at `limit`.
fn room33(on: bool, limit: usize) -> Builder<'static> {
    let mut b = oracle::builder_with(game(), static_oracle(), "room33", Switches { split_limit: on, ..Default::default() });
    let mut rd = scumm_reader::machine::Reader::new(game());
    rd.m.path_limit = limit;
    b.read_all_into(&mut rd, true, None, 2, None);
    b
}

/// The (fact, value) pairs the reading's task reaches from room 33's start ignoring deletes.
fn reach(b: &mut Builder) -> BTreeSet<(String, Label)> {
    let start = Facts::from_py(static_oracle().named("builder", "room33").get("start_items"));
    reach_from(b, &start)
}

/// `reach` from `start` (task_start's facts of it).
fn reach_from(b: &mut Builder, start: &Facts) -> BTreeSet<(String, Label)> {
    let recs = b.records.clone();
    let t = task::build_task(b, &recs, &[]);
    let s = task::task_start(b, start, t.facts.iter().map(|f| &f.0));
    let mut have: Vec<BTreeSet<Label>> =
        t.facts.iter().map(|(k, d)| d.label(s.get(k).unwrap_or(0)).into_iter().collect()).collect();
    let holds = |have: &Vec<BTreeSet<Label>>, conds: &[(String, Vec<Label>)]| {
        conds.iter().all(|(k, ls)| ls.iter().any(|l| have[t.index[k]].contains(l)))
    };
    loop {
        let mut changed = false;
        for a in &t.actions {
            if !holds(&have, &a.pre) {
                continue;
            }
            for e in &a.effects {
                if !holds(&have, &e.when) {
                    continue;
                }
                let i = t.index[&e.key];
                let dom = &t.facts[i].1;
                let vals: Vec<Label> = match (&e.value, &e.values, e.add) {
                    (Some(v), _, _) => vec![*v],
                    (None, Some(vs), _) => vs.clone(),
                    // a step from each value reached (a collapsed value may step anywhere)
                    (None, None, Some(n)) => have[i]
                        .iter()
                        .flat_map(|l| match l {
                            Label::Int(v) => dom.label(v + n).into_iter().collect::<Vec<_>>(),
                            _ => dom.values.clone(),
                        })
                        .collect(),
                    (None, None, None) => dom.values.clone(),
                };
                for v in vals {
                    changed |= have[i].insert(v);
                }
            }
        }
        if !changed {
            break;
        }
    }
    t.facts.iter().zip(have).flat_map(|((k, _), vs)| vs.into_iter().map(move |v| (k.clone(), v))).collect()
}

#[test]
fn room33_split_limit_lowered() {
    let mut full = room33(false, 1024);
    assert_eq!(full.counts.get(CUT), 0, "room 33 at the reader's limit is read whole");
    let mut cut = room33(false, 32);
    let mut split = room33(true, 32);
    assert!(cut.counts.get(CUT) > 0);
    assert_eq!(split.counts.get(CUT), 0);
    assert!(split.counts.get(SPLIT) > cut.counts.get(SPLIT));
    let (r_full, r_cut, r_split) = (reach(&mut full), reach(&mut cut), reach(&mut split));
    eprintln!(
        "room 33: records {} / {} cut / {} split; reached pairs {} / {} / {}; splits {} (limit 32)",
        full.records.len(),
        cut.records.len(),
        split.records.len(),
        r_full.len(),
        r_cut.len(),
        r_split.len(),
        split.counts.get(SPLIT)
    );
    assert_eq!(r_split, r_full, "the split reading reaches what the uncut one reaches");
    assert!(r_cut.len() < r_full.len(), "the cut reading reaches less");
}


/// Rooms 33, 28 and 35 as reader-scope reads them from seed 1 with `--elsewhere --live-locals` (3 threads), with the
/// switch on or off; the reach from the start.
fn bar35(on: bool) -> (Builder<'static>, BTreeSet<(String, Label)>) {
    let root = oracle::root();
    let start = scumm_reader::scaling::start_facts(&root, 1);
    let doms = scumm_reader::domains::build(game(), &scumm_reader::domains::meta_rooms(&root), &start, &scumm_reader::scaling::part1_goal(&root));
    let scope = scumm_reader::Scope::of(game(), &[33, 28, 35], &start);
    let mut b = Builder::new(game(), scope.clone(), doms);
    b.table.elsewhere = true;
    b.table.live_locals = Some(std::sync::Arc::new(scumm_reader::locals::LiveLocals::of(game())));
    b.switches.split_limit = on;
    scumm_reader::par::with_pool(game(), &scope, 3, |pool| {
        b.read_all_into(pool, true, None, 2, None);
    });
    let r = reach_from(&mut b, &start);
    (b, r)
}

#[test]
#[ignore]
fn bar35_split_limit_reaches_the_kitchen() {
    let room28 = ("room".to_string(), Label::Int(28));
    let open = ("state:316".to_string(), Label::Int(1));
    let (off, r_off) = bar35(false);
    assert!(off.counts.get(CUT) > 0);
    assert!(!r_off.contains(&room28) && !r_off.contains(&open), "without the switch the cut paths hide the kitchen");
    let (on, r_on) = bar35(true);
    eprintln!(
        "rooms 33, 28, 35: records {} off / {} on; paths cut {} / {}; splits {}; reached pairs {} / {}",
        off.records.len(),
        on.records.len(),
        off.counts.get(CUT),
        on.counts.get(CUT),
        on.counts.get(SPLIT),
        r_off.len(),
        r_on.len()
    );
    assert_eq!(on.counts.get(CUT), 0);
    assert!(r_on.contains(&room28) && r_on.contains(&open), "with the switch the walk through the door is read");
}
