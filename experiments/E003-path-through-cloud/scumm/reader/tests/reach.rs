//! The switch --reach (experiments/E004-fast-exact/reach.md) on room 33's scope, read with the Rust machine: the same
//! records for any number of threads; what it writes is what the full reading reaches; the order values arrive in
//! changes nothing; a start with a new value stops taskcore, and the extended reading covers it.

use scumm_reader::builder::{Builder, Switches};
use scumm_reader::domain::Label;
use scumm_reader::oracle::{self, Static};
use scumm_reader::record::Record;
use scumm_reader::{par, task, Eff, Facts};
use std::collections::BTreeSet;
use std::sync::OnceLock;

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

fn statics() -> &'static Static {
    static S: OnceLock<Static> = OnceLock::new();
    S.get_or_init(Static::load)
}

const ROOM33: &str = "room33";

fn start() -> Facts {
    Facts::from_py(statics().named("builder", ROOM33).get("start_items"))
}

fn switches(reach: bool) -> Switches {
    Switches { split_limit: true, reach, ..Default::default() }
}

/// One pass, so the domains are the same in every reading compared.
fn read(reach: bool, starts: Vec<Facts>, threads: usize, passes: usize) -> Builder<'static> {
    let mut b = oracle::builder_with(game(), statics(), ROOM33, switches(reach));
    b.reach_starts = starts;
    let scope = b.scope.clone();
    par::with_pool(game(), &scope, threads, |pool| {
        b.read_all_into(pool, true, None, passes, None);
    });
    b
}

fn lines(recs: &[Record]) -> Vec<String> {
    recs.iter().map(|r| r.py().text()).collect()
}

fn set(b: &Builder) -> BTreeSet<String> {
    lines(&b.records).into_iter().collect()
}

#[test]
fn the_same_for_any_number_of_threads() {
    let a = read(true, vec![start()], 0, 2);
    let b = read(true, vec![start()], 3, 2);
    assert!(!a.records.is_empty());
    assert_eq!(lines(&a.records), lines(&b.records));
}

#[test]
fn writes_a_part_of_the_full_reading() {
    let full = read(false, vec![], 3, 1);
    let on = read(true, vec![start()], 3, 1);
    // the set on args:X is always written under the switch (reach.md, "Lists"): compared without it, and without a write of 0
    let canon = |b: &Builder| -> BTreeSet<String> {
        b.records
            .iter()
            .filter_map(|r| {
                let eff: Vec<_> = r.effects.iter().filter(|(k, e)| !(k.starts_with("args:") && *e == Eff::Set(Label::Int(0)))).map(|(k, e)| (k.clone(), e.py().text())).collect();
                if eff.is_empty() {
                    return None;
                }
                let pre: Vec<_> = r.pre.iter().filter(|(k, s)| !(k.starts_with("args:") && **s == [Label::Int(0)])).collect();
                Some(format!("{} {:?} {:?}", r.label, pre, eff))
            })
            .collect()
    };
    let full_set = canon(&full);
    let extra: Vec<String> = canon(&on).difference(&full_set).cloned().collect();
    eprintln!("room 33: {} records full, {} with --reach, {} not in the full reading", full.records.len(), on.records.len(), extra.len());
    assert!(on.records.len() < full.records.len());
    assert!(extra.is_empty(), "{:?}", &extra[..extra.len().min(3)]);
}

/// Second starts: the first with a fact at a value some record of the full reading needs and `first` does not reach.
fn other_starts(first: &Builder) -> Vec<(Facts, String, Label)> {
    let full = read(false, vec![], 2, 1);
    let reached = first.table.reach.as_ref().unwrap();
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for r in &full.records {
        for (k, set) in &r.pre {
            if k.starts_with("pos:") || k.starts_with("escpos:") || k.starts_with("args:") {
                continue;
            }
            let have = reached.values(&first.table, k);
            for v in set {
                if let (false, Label::Int(n)) = (have.contains(v), *v) {
                    if seen.insert((k.clone(), n)) {
                        let mut s = start();
                        s.insert(k.clone(), n);
                        out.push((s, k.clone(), *v));
                    }
                }
            }
        }
    }
    out
}

#[test]
fn the_order_values_arrive_in_changes_nothing() {
    let a = read(true, vec![start()], 2, 1);
    // the first second start that frees something
    let (s2, key, val, together) = other_starts(&a)
        .into_iter()
        .take(60)
        .find_map(|(s, k, v)| {
            let t = read(true, vec![start(), s.clone()], 2, 1);
            (t.records.len() > a.records.len()).then_some((s, k, v, t))
        })
        .expect("a second start that frees a branch");
    eprintln!("second start: {key} = {val:?}");
    let extended = |first: Facts, second: Facts| {
        let mut b = read(true, vec![first], 2, 1);
        let scope = b.scope.clone();
        par::with_pool(game(), &scope, 2, |pool| b.reach_extend(pool, &second, None, None));
        b
    };
    let ab = extended(start(), s2.clone());
    let ba = extended(s2.clone(), start());
    eprintln!("records: first start {}, both {}, first then second {}, second then first {}", a.records.len(), together.records.len(), ab.records.len(), ba.records.len());
    assert!(together.records.len() > a.records.len());
    assert_eq!(set(&ab), set(&together), "the first start, then the second");
    assert_eq!(set(&ba), set(&together), "the second start, then the first");
}

#[test]
fn a_start_with_a_new_value_stops_taskcore() {
    let root = oracle::root();
    let bin = root.join("experiments/E003-path-through-cloud/taskcore/target/release/taskcore");
    if !bin.exists() {
        eprintln!("no taskcore binary at {}: skipped", bin.display());
        return;
    }
    let dir = std::env::temp_dir().join(format!("reach-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let packed = dir.join("t.tc");
    let mut b = oracle::builder_with(game(), statics(), ROOM33, switches(true));
    b.reach_starts = vec![start()];
    let scope = b.scope.clone();
    let mut stream = scumm_reader::packed::RecordStream::create(&packed).unwrap();
    par::with_pool(game(), &scope, 2, |pool| {
        b.read_all_into(pool, true, None, 1, Some(&mut stream));
    });
    let keys = stream.fact_keys();
    let s0 = task::task_start(&mut b, &start(), keys.iter());
    let lines_r = b.table.reach.as_ref().unwrap().lines(&b.table, keys.iter());
    stream.finish(&mut b, &[], &[("start".to_string(), s0.clone())]).unwrap();
    let reached = dir.join("t.reached");
    std::fs::write(&reached, lines_r.join("\n") + "\n").unwrap();
    let write_start = |path: &std::path::Path, change: Option<(&str, Label)>| {
        let mut text = String::new();
        for k in &keys {
            let v = s0.get0(k);
            let t = match change {
                Some((c, l)) if c == k => l.text(),
                _ => b.table.domain(k).and_then(|d| d.label(v)).map_or(v.to_string(), |l| l.text()),
            };
            text.push_str(&format!("{k}\t{t}\n"));
        }
        std::fs::write(path, text).unwrap();
    };
    let sas = |sf: &std::path::Path| {
        std::process::Command::new(&bin)
            .args(["sas", "--task"])
            .arg(&packed)
            .arg("--start-file")
            .arg(sf)
            .arg("--reached")
            .arg(&reached)
            .arg("--out")
            .arg(dir.join("t.sas"))
            .arg("--stats")
            .arg(dir.join("t.json"))
            .output()
            .unwrap()
    };
    let same = dir.join("same.txt");
    write_start(&same, None);
    let out = sas(&same);
    assert_eq!(out.status.code(), Some(0), "the start's own values: {}", String::from_utf8_lossy(&out.stderr));
    // a value of a fact in the file, in its domain, not reached
    let reach = b.table.reach.as_ref().unwrap();
    let (k, v) = keys
        .iter()
        .filter(|k| !k.starts_with("args:") && !k.starts_with("pos:") && !k.starts_with("escpos:"))
        .find_map(|k| {
            let d = b.table.domain(k)?;
            let have = reach.values(&b.table, k);
            d.values.iter().find(|x| !have.contains(x)).map(|x| (k.clone(), *x))
        })
        .expect("a value not reached");
    let new = dir.join("new.txt");
    write_start(&new, Some((&k, v)));
    let out = sas(&new);
    assert_eq!(out.status.code(), Some(7), "{k} = {v:?}: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains(&format!("{k}={}", v.text())));
    std::fs::remove_dir_all(&dir).ok();
}
