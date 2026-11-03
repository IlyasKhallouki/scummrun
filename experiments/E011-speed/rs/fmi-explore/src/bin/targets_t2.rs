//! E012's gate T2 (experiments/E012-game-mechanics/plan.md, component `targets`): the kitchen check's logged tries'
//! targets recomputed from their job files, with FMI_TARGET_PER_KEY off (E008's and E009's targets, as the try made
//! them) and on (one target per key), on the readings made again from their start files.
//!
//! usage: targets_t2 ARCHIVE READINGS_TSV OUT.jsonl
//!   ARCHIVE: the kitchen archive (jobs/jNNNNN/{job,result}.json, entries.jsonl, entries/);
//!   READINGS_TSV: lines "the reading's path as result.json names it<TAB>the folder made again (reading.reached,
//!   reading.json)".
//! Prints the report (markdown) and writes one JSON line per try.

use fmi_explore::archive::fact_map;
use fmi_explore::files::{read_json, read_text};
use fmi_explore::step::{self, Target, TV};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};

fn pairs(ts: &[Target]) -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    for x in ts {
        match &x.v {
            TV::Text(s) => {
                out.insert((x.k.clone(), s.clone()));
            }
            v => {
                for n in v.ints() {
                    out.insert((x.k.clone(), n.to_string()));
                }
            }
        }
    }
    out
}

fn by_tier(ts: &[Target]) -> Value {
    let n = |t: i64| ts.iter().filter(|x| x.tier == t).count();
    json!({"0": n(0), "1": n(1), "2": n(2)})
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 4 {
        eprintln!("usage: targets_t2 ARCHIVE READINGS_TSV OUT.jsonl");
        std::process::exit(2);
    }
    let arch = PathBuf::from(&a[1]);
    let mut made: HashMap<String, PathBuf> = HashMap::new();
    for ln in read_text(Path::new(&a[2])).unwrap().lines() {
        if let Some((p, d)) = ln.split_once('\t') {
            made.insert(p.to_string(), PathBuf::from(d));
        }
    }
    let mut out = std::fs::File::create(&a[3]).unwrap();
    let mut jobs: Vec<PathBuf> = std::fs::read_dir(arch.join("jobs")).unwrap().flatten().map(|e| e.path())
        .filter(|p| p.join("result.json").is_file()).collect();
    jobs.sort();
    let mut guides: HashMap<String, step::Atoms> = HashMap::new();
    let (mut n_tries, mut n_logged_eq, mut n_pairs_eq, mut n_records_eq) = (0, 0, 0, 0);
    let (mut sum_off, mut sum_on, mut first_changed, mut max_103_on, mut sum_103_off, mut tries_103) = (0, 0, 0, 0, 0, 0);
    let mut max_per_key_lists = 0;
    let mut n_mixed = 0;
    let mut collapsed_keys: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut rows = Vec::new();
    for jd in &jobs {
        let job = read_json(&jd.join("job.json")).unwrap();
        let res = read_json(&jd.join("result.json")).unwrap();
        let name = jd.file_name().unwrap().to_string_lossy().into_owned();
        let rpath = res["reading"]["path"].as_str().unwrap_or("").to_string();
        let Some(rd) = made.get(&rpath) else {
            println!("{name}: no reading made again for {rpath}");
            continue;
        };
        n_tries += 1;
        let head = read_json(&rd.join("reading.json")).unwrap();
        let records_eq = head["n_records"] == res["reading"]["records"] && head["n_actions"] == res["reading"]["actions"];
        n_records_eq += records_eq as usize;
        let reached = step::read_reached(&rd.join("reading.reached")).unwrap();
        let gpath = job["guide"].as_str().unwrap().to_string();
        let atoms = guides.entry(gpath.clone()).or_insert_with(|| step::guide_atoms(&read_json(Path::new(&gpath)).unwrap()));
        let facts = fact_map(&job["entry"]["checkpoint"][1]);
        let root = step::root_facts_of(&job, jd);
        let empty = vec![];
        let held = job.get("held").and_then(|h| h.as_array()).unwrap_or(&empty);
        let tried = job.get("tried").and_then(|h| h.as_array()).unwrap_or(&empty);
        let rot = job.get("rotation").filter(|x| !x.is_null());
        let goal = step::goal_of(&job);
        let off = step::targets_with(&reached, atoms, &facts, root.as_ref(), held, tried, &goal, rot).unwrap();
        let on = step::targets_per_key(&reached, atoms, &facts, root.as_ref(), held, tried, &goal, rot, None).unwrap();
        // the log's targets: by tier, how many, the first 20
        let first: Vec<Value> = off.iter().take(20).map(|x| x.json()).collect();
        let logged_eq = res["targets"]["by_tier"] == by_tier(&off) && res["targets"]["n"] == json!(off.len())
            && res["targets"]["first"] == json!(first);
        n_logged_eq += logged_eq as usize;
        let pairs_eq = pairs(&off) == pairs(&on);
        n_pairs_eq += pairs_eq as usize;
        let lists: Vec<(String, usize)> = on.iter().filter_map(|x| match &x.v {
            TV::Values(vs) => Some((x.k.clone(), vs.len())),
            _ => None,
        }).collect();
        // value lists whose values are partly held by the archive (tier 1 with held values in it)
        let held_pairs: BTreeSet<String> = held.iter().map(|h| h.to_string()).collect();
        let mixed: Vec<(String, usize, usize)> = on.iter().filter_map(|x| match &x.v {
            TV::Values(vs) => {
                let h = vs.iter().filter(|v| held_pairs.contains(&json!([x.k, v]).to_string())).count();
                (h > 0 && h < vs.len()).then(|| (x.k.clone(), h, vs.len()))
            }
            _ => None,
        }).collect();
        n_mixed += mixed.len();
        let mut per_key: HashMap<&str, usize> = HashMap::new();
        for (k, _) in &lists {
            *per_key.entry(k.as_str()).or_default() += 1;
        }
        max_per_key_lists = max_per_key_lists.max(per_key.values().copied().max().unwrap_or(0));
        for (k, n) in &lists {
            let e = collapsed_keys.entry(k.clone()).or_default();
            e.0 += 1;
            e.1 = e.1.max(*n);
        }
        let n103_off = off.iter().filter(|x| x.k == "var:103").count();
        let n103_on = on.iter().filter(|x| x.k == "var:103").count();
        if n103_off > 0 {
            tries_103 += 1;
        }
        sum_103_off += n103_off;
        max_103_on = max_103_on.max(n103_on);
        sum_off += off.len();
        sum_on += on.len();
        let fc = off.first().map(|x| x.json()) != on.first().map(|x| x.json());
        first_changed += fc as usize;
        // the try's attempts as logged, and how many went for var:103
        let probes: Vec<Value> = res["probes"].as_array().cloned().unwrap_or_default();
        let p103 = probes.iter().filter(|p| p["target"][0] == "var:103").count();
        let row = json!({"job": name, "entry": job["entry"]["id"], "reading": rpath, "records_eq": records_eq,
            "logged_eq": logged_eq, "pairs_eq": pairs_eq, "off": off.len(), "on": on.len(), "by_tier_off": by_tier(&off),
            "by_tier_on": by_tier(&on), "lists": lists, "var103_off": n103_off, "var103_on": n103_on,
            "first_off": off.first().map(|x| x.json()), "first_on": on.first().map(|x| x.json()), "probes": probes.len(),
            "probes_var103": p103, "mixed": mixed});
        writeln!(out, "{}", row).unwrap();
        rows.push(row);
    }
    println!("| try | entry | reading counts | logged = off | pairs off = on | targets off | on | var:103 off | on | value lists (key: values) | attempts (var:103) |");
    println!("|---|---:|---|---|---|---:|---:|---:|---:|---|---:|");
    for r in &rows {
        let lists = r["lists"].as_array().unwrap().iter().map(|l| format!("{}: {}", l[0].as_str().unwrap(), l[1])).collect::<Vec<_>>().join(", ");
        let yn = |b: &Value| if b.as_bool() == Some(true) { "yes" } else { "NO" };
        println!("| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} ({}) |", r["job"].as_str().unwrap(), r["entry"],
                 yn(&r["records_eq"]), yn(&r["logged_eq"]), yn(&r["pairs_eq"]), r["off"], r["on"], r["var103_off"], r["var103_on"],
                 lists, r["probes"], r["probes_var103"]);
    }
    println!();
    println!("tries {n_tries}; reading counts equal to the log's {n_records_eq}; off targets equal to the log's (by tier, n, first 20) {n_logged_eq}; \
              the same (key, value) pairs off and on {n_pairs_eq}");
    println!("targets in all: off {sum_off}, on {sum_on}; tries whose first target changes {first_changed}; \
              at most {max_per_key_lists} value list per key per try");
    println!("var:103: in {tries_103} tries, {sum_103_off} targets off; at most {max_103_on} per try on");
    println!("value lists with held and unheld values (tier 1, some values held by the archive): {n_mixed}");
    println!("collapsed keys (tries with a list, largest list): {}",
             collapsed_keys.iter().map(|(k, (n, m))| format!("{k} ({n}, {m})")).collect::<Vec<_>>().join(", "));
}
