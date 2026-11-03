//! E012 component `archive`, gate S2 (experiments/E012-game-mechanics/plan.md, the amendment "component `archive`",
//! "S2, how"): the E011 kitchen archive's entries recounted under the cells over T.
//!
//!   archive-s2 tested NAME=DIR ...     one JSON line per reading: its T as the archive's atoms (`progress::t_atoms`
//!                                       of `Tested::of_reading` under the reading's domains), with counts and seconds
//!   archive-s2 recount COPY T.jsonl --json OUT.json --md OUT.md
//!                                       COPY is a copy of the archive (it is opened, so its files may be cleaned as
//!                                       any open does); T is the goal's atoms and the union of T.jsonl's atoms, in
//!                                       its order. E011's selection (the switch off) against the switch's.

use fmi_core::ReadingSource;
use fmi_explore::archive::{committed, Archive};
use fmi_explore::progress::{self, TAtom};
use fmi_reading::tested::Tested;
use fmi_reading::Reading;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::time::Instant;

fn r3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

fn tested(args: &[String]) {
    for a in args {
        let (name, dir) = a.split_once('=').expect("NAME=DIR");
        let t0 = Instant::now();
        let rd = Reading::open(Path::new(dir)).unwrap_or_else(|e| panic!("{dir}: {e}"));
        let open_s = t0.elapsed().as_secs_f64();
        let t1 = Instant::now();
        let t = Tested::of_reading(&rd);
        let atoms = progress::t_atoms(&t.flat(), rd.domains());
        let of_s = t1.elapsed().as_secs_f64();
        let control = atoms.iter().filter(|x| x.control()).count();
        println!(
            "{}",
            json!({"name": name, "dir": dir, "actions": rd.n_actions(), "keys": t.counts.keys, "tested_atoms": t.counts.atoms,
                   "atoms": progress::atoms_json(&atoms), "n_atoms": atoms.len(), "control_atoms": control,
                   "open_s": r3(open_s), "of_s": r3(of_s)})
        );
    }
}

/// Entries per cell: (cells, max, median, cells of one entry).
fn per_cell(a: &Archive) -> Value {
    let mut n: Vec<usize> = a.cells.iter().map(|(_, c)| c.entries.len()).collect();
    n.sort();
    json!({"cells": n.len(), "max": n.last(), "median": n.get(n.len() / 2), "singletons": n.iter().filter(|&&k| k == 1).count()})
}

fn order(a: &Archive, k: usize) -> Vec<Value> {
    a.select(k)
        .into_iter()
        .map(|i| {
            let r = &a.rows[i];
            json!({"entry": i, "room": r.room, "frame": r.frame, "key": a.sel_key_json(i)})
        })
        .collect()
}

fn gave(x: &Value) -> bool {
    let ne = |k: &str| x.get(k).and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(false);
    ne("new_t") || ne("new_rooms")
}

/// Plan.md 6's stagnation replayed over the log under the switch: after each committed line, the selectable cells
/// that the rows made so far give (with the exhausted marks so far, under the one T of S2), and the lines at which
/// the rule would fire.
fn stagnation(b: &Archive) -> Value {
    let done: Vec<&Value> = b.logs.iter().filter(|x| committed(x)).collect();
    let mut rows_after: Vec<usize> = Vec::new();
    let mut n = 1usize;
    for x in &done {
        if let Some(a) = x.get("new_entries").and_then(|v| v.as_array()) {
            for i in a.iter().filter_map(|v| v.as_u64()) {
                n = n.max(i as usize + 1);
            }
        }
        rows_after.push(n);
    }
    let selectable_after = |k: usize| -> usize {
        let mut exh: BTreeSet<usize> = BTreeSet::new();
        for x in &done[..=k] {
            if x["outcome"].as_str() != Some("return differs") && x["exhausted"] == json!(true) {
                exh.insert(x["entry"].as_u64().unwrap() as usize);
            }
        }
        let mut cells: BTreeSet<&str> = BTreeSet::new();
        for r in &b.rows[..rows_after[k]] {
            if r.dead.is_none() && !exh.contains(&r.id) && r.room.map(|x| b.allowed.contains(&x)).unwrap_or(false) {
                cells.insert(&r.cell);
            }
        }
        cells.len()
    };
    let (mut last, mut frontier, mut since) = (None::<usize>, 1usize, 0usize);
    let mut fires: Vec<Value> = Vec::new();
    let mut gave_lines = 0;
    for (k, x) in done.iter().enumerate() {
        if gave(x) {
            last = Some(k);
            frontier = selectable_after(k).max(1);
            since = 0;
            gave_lines += 1;
        } else {
            since += 1;
            if since == frontier {
                fires.push(json!({"line": k, "job": x["job"], "after": last.map(|l| done[l]["job"].clone()), "frontier": frontier}));
            }
        }
    }
    json!({"lines": done.len(), "lines_with_new": gave_lines, "fires": fires,
           "last_new": last.map(|l| done[l]["job"].clone()), "frontier_at_the_end": frontier, "since_at_the_end": since})
}

fn recount(args: &[String]) {
    let copy = Path::new(&args[0]);
    let tfile = Path::new(&args[1]);
    let flag = |k: &str| args.iter().position(|a| a == k).map(|i| args[i + 1].clone());
    let (out_json, out_md) = (flag("--json").expect("--json"), flag("--md").expect("--md"));

    // E011's selection, the switch off
    std::env::remove_var(progress::SELECT_ENV);
    let t0 = Instant::now();
    let a = Archive::open(copy).expect("open (switch off)");
    let open_off_s = t0.elapsed().as_secs_f64();
    assert!(a.progress.is_none());
    let e011 = json!({"open_s": r3(open_off_s), "rows": a.rows.len(), "per_cell": per_cell(&a), "first20": order(&a, 20),
                      "stagnant": a.stagnant(), "selectable": a.n_selectable(), "idle": a.idle.len()});
    let e011_cell: Vec<String> = a.rows.iter().map(|r| r.cell.clone()).collect();
    drop(a);

    // T: the goal's atoms, then each reading's in the file's order
    let mut readings: Vec<Value> = Vec::new();
    let mut all: Vec<TAtom> = Vec::new();
    let mut seen: std::collections::HashSet<TAtom> = std::collections::HashSet::new();
    for line in std::fs::read_to_string(tfile).expect("T.jsonl").lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(line).expect("a T line");
        let atoms = progress::atoms_of_json(v.get("atoms"));
        let new = atoms.iter().filter(|x| seen.insert((*x).clone())).count();
        all.extend(atoms.iter().cloned());
        readings.push(json!({"name": v["name"], "actions": v["actions"], "atoms": atoms.len(), "control": v["control_atoms"],
                             "new_to_the_union": new, "of_s": v["of_s"]}));
    }

    // the switch on: the rows keyed from the files, T merged, the rows keyed again, the log's novelty filled in
    std::env::set_var(progress::SELECT_ENV, progress::PROGRESS);
    let t1 = Instant::now();
    let mut b = Archive::open(copy).expect("open (switch on)");
    let open_on_s = t1.elapsed().as_secs_f64();
    let t_goal = b.progress.as_ref().unwrap().t.len();
    let t2 = Instant::now();
    let merged = b.progress.as_mut().unwrap().merge(&all).len();
    b.rekey_t();
    let rekey_t_s = t2.elapsed().as_secs_f64();
    let t3 = Instant::now();
    let filled = b.backfill_novelty();
    let backfill_s = t3.elapsed().as_secs_f64();
    let p = b.progress.as_ref().unwrap();
    let t = p.t.clone();
    let live = b.live_rows();
    let t4 = Instant::now();
    let nv = progress::novelty_json(p, &live);
    let novelty_s = t4.elapsed().as_secs_f64();
    let novelty_bytes = serde_json::to_string(&nv).unwrap().len();
    let t5 = Instant::now();
    let first = b.select(1);
    let select_s = t5.elapsed().as_secs_f64();
    let t6 = Instant::now();
    let nsel = b.n_selectable();
    let nsel_s = t6.elapsed().as_secs_f64();

    // T held
    let held_atoms: BTreeSet<u32> = p.first.keys().copied().collect();
    let control_t = t.iter().filter(|x| x.control()).count();
    let held_control = held_atoms.iter().filter(|&&j| t[j as usize].control()).count();
    // novel atoms per room (the room of each atom's least live holder) and the rooms first entered
    let mut per_room: BTreeMap<i64, (usize, usize)> = BTreeMap::new();
    for (&j, &(_, i)) in &p.first {
        let r = b.rows[i].room.unwrap_or(-1);
        let e = per_room.entry(r).or_insert((0, 0));
        if t[j as usize].control() {
            e.1 += 1;
        } else {
            e.0 += 1;
        }
    }
    let rooms_first: BTreeMap<i64, usize> = {
        let mut m = BTreeMap::new();
        for r in &b.rows {
            if let Some(room) = r.room {
                m.entry(room).or_insert(r.id);
            }
        }
        m
    };
    // cells against E011's
    let mut by_new: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    let mut by_old: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    for (i, r) in b.rows.iter().enumerate() {
        by_new.entry(&r.cell).or_default().insert(&e011_cell[i]);
        by_old.entry(&e011_cell[i]).or_default().insert(&r.cell);
    }
    let merges = by_new.values().filter(|s| s.len() > 1).count();
    let splits = by_old.values().filter(|s| s.len() > 1).count();
    let one_to_one = by_new.values().filter(|s| s.len() == 1).filter(|s| by_old[s.iter().next().unwrap()].len() == 1).count();
    // the counts per cell
    let mut counts: Vec<(i64, i64, String, usize, Value)> = b
        .cells
        .iter()
        .map(|(id, c)| (c.chosen.unwrap_or(0), c.n, id.clone(), c.entries.len(), c.key.clone().unwrap_or(Value::Null)))
        .collect();
    counts.sort_by(|x, y| y.0.cmp(&x.0).then(y.1.cmp(&x.1)).then(x.2.cmp(&y.2)));
    let chosen_cells = counts.iter().filter(|c| c.0 > 0).count();
    // a cell's atoms as their difference from the root's cell (+ held here and not at the root, - the other way)
    let root_atoms: BTreeSet<String> = b.cell(&b.rows[0].cell).key.as_ref().map(|k| atoms_of_key(k)).unwrap_or_default();
    let diff = |key: &Value| -> Value {
        let mine = atoms_of_key(key);
        let plus: Vec<String> = mine.difference(&root_atoms).map(|a| format!("+{a}")).collect();
        let minus: Vec<String> = root_atoms.difference(&mine).map(|a| format!("-{a}")).collect();
        json!({"held": mine.len(), "against_the_root": plus.into_iter().chain(minus).collect::<Vec<_>>()})
    };
    let top: Vec<Value> = counts
        .iter()
        .take(12)
        .map(|(ch, n, id, k, key)| json!({"cell": &id[..8], "chosen": ch, "n": n, "entries": k, "room": key[0],
                                          "owned": key[1], "atoms": diff(key)}))
        .collect();
    let nov = b.novel_counts();
    let first20: Vec<Value> = b
        .select(20)
        .into_iter()
        .map(|i| {
            let r = &b.rows[i];
            let c = b.cell(&r.cell);
            json!({"entry": i, "room": r.room, "frame": r.frame, "key": b.sel_key_json(i), "n": c.n, "chosen": c.chosen,
                   "chain_rooms": b.chain_rooms(i).iter().collect::<Vec<_>>(), "novel": nov.get(&i).copied().unwrap_or(0),
                   "atoms": c.key.as_ref().map(|k| diff(k))})
        })
        .collect();
    let lines_new: Vec<Value> = b
        .logs
        .iter()
        .filter(|x| committed(x) && gave(x))
        .map(|x| json!({"job": x["job"], "entry": x["entry"], "new_t": x["new_t"], "new_rooms": x["new_rooms"]}))
        .collect();
    let report = json!({
        "copy": copy, "t_file": tfile, "readings": readings,
        "t": {"goal": t_goal, "merged": merged, "size": t.len(), "control": control_t, "held": held_atoms.len(),
              "held_control": held_control, "never_held": t.len() - held_atoms.len()},
        "e011": e011,
        "progress": {"open_s": r3(open_on_s), "rekey_t_s": r3(rekey_t_s), "backfill_s": r3(backfill_s), "lines_filled": filled,
                     "rows": b.rows.len(), "live": live.len(), "per_cell": per_cell(&b), "selectable": nsel,
                     "cells_chosen": chosen_cells, "top_by_chosen": top, "first20": first20, "first": first,
                     "stagnation": stagnation(&b), "stagnant_now": b.stagnant(),
                     "novel_per_room": per_room.iter().map(|(r, (w, c))| json!([r, w, c])).collect::<Vec<_>>(),
                     "rooms_first_entry": rooms_first, "lines_with_new": lines_new,
                     "store_cells": p.store.cells(),
                     "cost": {"novelty_json_s": r3(novelty_s), "novelty_bytes": novelty_bytes, "select1_s": r3(select_s),
                              "n_selectable_s": r3(nsel_s)}},
        "against_e011": {"progress_cells_merging_e011_cells": merges, "e011_cells_split": splits, "one_to_one": one_to_one},
    });
    std::fs::write(&out_json, serde_json::to_string_pretty(&report).unwrap() + "\n").expect("write the report");
    std::fs::write(&out_md, md(&report)).expect("write the md");
}

/// The report's tables (the numbers only; the reading of them is the lab note's).
fn md(r: &Value) -> String {
    let mut s = String::new();
    let p = &r["progress"];
    let e = &r["e011"];
    s.push_str("# E012 archive, gate S2: the E011 kitchen archive recounted under the cells over T\n\n");
    s.push_str("Made by `archive-s2 recount` (experiments/E012-game-mechanics/archive/s2); the numbers are its JSON report's.\n\n");
    s.push_str("## T\n\n| reading | actions | atoms | control-key atoms | new to the union |\n|---|---:|---:|---:|---:|\n");
    for x in r["readings"].as_array().unwrap() {
        s.push_str(&format!("| {} | {} | {} | {} | {} |\n", x["name"].as_str().unwrap_or(""), x["actions"], x["atoms"], x["control"],
                            x["new_to_the_union"]));
    }
    let t = &r["t"];
    s.push_str(&format!("\nT: {} atoms ({} the goal's), {} on control keys; held by some live entry: {} ({} on control keys); \
                         never held: {}.\n\n", t["size"], t["goal"], t["control"], t["held"], t["held_control"], t["never_held"]));
    s.push_str("## Cells\n\n| | E011 (switch off) | progress |\n|---|---:|---:|\n");
    s.push_str(&format!("| rows | {} | {} |\n", e["rows"], p["rows"]));
    for k in ["cells", "max", "median", "singletons"] {
        s.push_str(&format!("| {} | {} | {} |\n", match k { "max" => "entries in the largest cell", "median" => "median entries per cell",
                                                            "singletons" => "cells of one entry", _ => "cells" },
                            e["per_cell"][k], p["per_cell"][k]));
    }
    s.push_str(&format!("| selectable cells | {} | {} |\n", e["selectable"], p["selectable"]));
    s.push_str(&format!("| stagnant at the end | {} | {} |\n", e["stagnant"], p["stagnant_now"]));
    let ag = &r["against_e011"];
    s.push_str(&format!("\nAgainst E011's cells: {} progress cells hold entries of more than one E011 cell; {} E011 cells are split; \
                         {} cells are the same.\n\n", ag["progress_cells_merging_e011_cells"], ag["e011_cells_split"], ag["one_to_one"]));
    s.push_str(&format!("## Counts per cell\n\n{} of {} cells were chosen at least once. The most chosen:\n\n\
                         | cell | room | chosen | n | entries | atoms of T held; against the root's cell (owned objects) |\n|---|---:|---:|---:|---:|---|\n",
                        p["cells_chosen"], p["per_cell"]["cells"]));
    for x in p["top_by_chosen"].as_array().unwrap() {
        s.push_str(&format!("| {} | {} | {} | {} | {} | {} held; {} ({}) |\n", x["cell"].as_str().unwrap_or(""), x["room"], x["chosen"], x["n"],
                            x["entries"], x["atoms"]["held"], pyjson_list(&x["atoms"]["against_the_root"]), pyjson_list(&x["owned"])));
    }
    s.push_str("\n## The order's first 20\n\n| # | progress: entry | room | frame | (n, chosen, -rooms, -novel, frame, id) | E011: entry | room | frame |\n|---:|---:|---:|---:|---|---:|---:|---:|\n");
    let pf = p["first20"].as_array().unwrap();
    let ef = e["first20"].as_array().unwrap();
    for k in 0..pf.len().max(ef.len()) {
        let (a, b) = (pf.get(k), ef.get(k));
        s.push_str(&format!("| {} | {} | {} | {} | {} | {} | {} | {} |\n", k + 1,
                            a.map(|x| x["entry"].to_string()).unwrap_or_default(), a.map(|x| x["room"].to_string()).unwrap_or_default(),
                            a.map(|x| x["frame"].to_string()).unwrap_or_default(), a.map(|x| compact(&x["key"])).unwrap_or_default(),
                            b.map(|x| x["entry"].to_string()).unwrap_or_default(), b.map(|x| x["room"].to_string()).unwrap_or_default(),
                            b.map(|x| x["frame"].to_string()).unwrap_or_default()));
    }
    let st = &p["stagnation"];
    s.push_str(&format!("\n## Stagnation over T\n\n{} committed lines, {} with a new atom of T or a new room; the last: {}. \
                         The rule fires at {} line(s): {}. At the end: {} lines since the last new atom, frontier {}.\n\n",
                        st["lines"], st["lines_with_new"], compact(&st["last_new"]), st["fires"].as_array().map(|a| a.len()).unwrap_or(0),
                        compact(&st["fires"]), st["since_at_the_end"], st["frontier_at_the_end"]));
    s.push_str("## Novel atoms per room (the room of each atom's least live holder)\n\n| room | atoms | control-key atoms | first entry |\n|---:|---:|---:|---:|\n");
    for x in p["novel_per_room"].as_array().unwrap() {
        let room = x[0].to_string();
        s.push_str(&format!("| {} | {} | {} | {} |\n", room, x[1], x[2], p["rooms_first_entry"].get(&room).map(|v| v.to_string()).unwrap_or_default()));
    }
    let c = &p["cost"];
    s.push_str(&format!("\n## Cost\n\nOpen with the switch off {} s, on {} s (both key every row from the entry files); T merged and \
                         the rows keyed again from the store {} s; the log's novelty filled in and counted again {} s; job.json's \
                         \"novelty\" {} s, {} bytes; select(1) {} s; the selectable cells {} s. The store keeps {} values.\n",
                        e["open_s"], p["open_s"], p["rekey_t_s"], p["backfill_s"], c["novelty_json_s"], c["novelty_bytes"],
                        c["select1_s"], c["n_selectable_s"], p["store_cells"]));
    s
}

fn atoms_of_key(k: &Value) -> BTreeSet<String> {
    k[2].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default()
}

fn compact(v: &Value) -> String {
    serde_json::to_string(v).unwrap()
}

/// A list's items joined; an atom's text of more than 4 alternatives is shortened to `k=(N values)` (the JSON report
/// has them in full).
fn pyjson_list(v: &Value) -> String {
    let short = |t: String| -> String {
        match t.split_once('=') {
            Some((k, vs)) if vs.split('|').count() > 4 => format!("{k}=({} values)", vs.split('|').count()),
            _ => t,
        }
    };
    v.as_array()
        .map(|a| a.iter().map(|x| short(x.as_str().map(|s| s.to_string()).unwrap_or_else(|| x.to_string()))).collect::<Vec<_>>().join(", "))
        .unwrap_or_default()
}

/// E011's `reingest_check` with the switch off: the recorded archive's jobs ingested again into a fresh archive, its
/// files compared byte for byte (the switch off changes nothing of E011's archive).
fn reingest_off(args: &[String]) {
    std::env::remove_var(progress::SELECT_ENV);
    let (rows, times) = fmi_explore::check::reingest_check(Path::new(&args[0]), Path::new(&args[1])).expect("reingest_check");
    let mut bad = 0;
    for (what, ok, why) in &rows {
        println!("{} {what} {why}", if *ok { "ok  " } else { "DIFF" });
        bad += !ok as usize;
    }
    println!("{} jobs, {:.2} s in the archive", times.len(), times.iter().sum::<f64>());
    if bad > 0 {
        std::process::exit(1);
    }
}

/// The recorded archive's jobs ingested again with the switch on, each result given the T of the reading its try used
/// (MAP: reading path -> name in T.jsonl): what the archive would have logged, cell for cell, with its own time.
fn replay(args: &[String]) {
    let (rec, out) = (Path::new(&args[0]), Path::new(&args[1]));
    let tfile = Path::new(&args[2]);
    let map_file = Path::new(&args[3]);
    let flag = |k: &str| args.iter().position(|a| a == k).map(|i| args[i + 1].clone());
    let (out_json, out_md) = (flag("--json").expect("--json"), flag("--md").expect("--md"));
    std::env::set_var(progress::SELECT_ENV, progress::PROGRESS);
    let mut t_of: HashMap<String, Value> = HashMap::new();
    for line in std::fs::read_to_string(tfile).expect("T.jsonl").lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(line).expect("a T line");
        t_of.insert(v["name"].as_str().unwrap().to_string(), v["atoms"].clone());
    }
    let mut name_of: HashMap<String, String> = HashMap::new();
    for l in std::fs::read_to_string(map_file).expect("MAP").lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let (p, n) = l.split_once('\t').expect("path<TAB>name");
        name_of.insert(p.to_string(), n.to_string());
    }
    let _ = std::fs::remove_dir_all(out);
    std::fs::create_dir_all(out.join("jobs")).unwrap();
    let rd = |p: &Path| -> Value { serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap() };
    let stage = rd(&rec.join("stage.json"));
    let root0 = fmi_explore::files::read_gz(&rec.join("entries").join("0.json.gz")).unwrap();
    let allowed: Vec<i64> = stage["allowed_rooms"].as_array().unwrap().iter().filter_map(|x| x.as_i64()).collect();
    let mut a = Archive::create(out, stage["stage"].as_str().unwrap_or(""), &stage["goal"], Path::new(stage["guide"].as_str().unwrap()),
                                &allowed, stage["workers"].as_i64().unwrap_or(2), stage.get("root").cloned(),
                                stage.get("e009").cloned(), stage.get("e010").cloned()).unwrap();
    let cp = root0["checkpoints"].as_array().cloned().unwrap();
    a.add(None, root0["frame"].as_i64().unwrap(), root0["tick"].as_i64(), &[], &cp, None, "boot").unwrap();
    a.write_cells().unwrap();
    let (logs, _) = fmi_explore::files::read_jsonl(&rec.join("log.jsonl")).unwrap();
    let mut per_job: Vec<Value> = Vec::new();
    let (mut ingest_s, mut doc_s, mut doc_bytes, mut sel_s) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut no_t = 0;
    for x in &logs {
        let job = x["job"].as_str().unwrap_or("").to_string();
        if !committed(x) {
            a.log_event(x.clone()).unwrap();
            continue;
        }
        let (src, dst) = (rec.join("jobs").join(&job), out.join("jobs").join(&job));
        std::fs::create_dir_all(dst.join("new")).unwrap();
        for f in ["job.json", "select.json"] {
            if src.join(f).exists() {
                std::fs::copy(src.join(f), dst.join(f)).unwrap();
            }
        }
        if let Ok(d) = std::fs::read_dir(src.join("new")) {
            for e in d.flatten() {
                std::fs::copy(e.path(), dst.join("new").join(e.file_name())).unwrap();
            }
        }
        let mut res = rd(&src.join("result.json"));
        let path = x["reading"]["path"].as_str().unwrap_or("").to_string();
        let name = name_of.get(&path).cloned();
        match name.as_ref().and_then(|n| t_of.get(n)) {
            Some(t) => res["tested"] = t.clone(),
            None => no_t += 1,
        }
        std::fs::write(dst.join("result.json"), res.to_string()).unwrap();
        let e = rd(&dst.join("job.json"))["entry"]["id"].as_u64().unwrap() as usize;
        let t0 = Instant::now();
        let doc = a.job_doc(&job, e).unwrap();
        doc_s.push(t0.elapsed().as_secs_f64());
        doc_bytes.push(serde_json::to_string(&doc["novelty"]).unwrap().len());
        let t1 = Instant::now();
        let _ = a.select(1);
        let _ = a.n_selectable();
        sel_s.push(t1.elapsed().as_secs_f64());
        let mut extra = serde_json::Map::new();
        for k in ["started", "ended", "driver_wall_s", "rc", "rusage", "guard"] {
            if let Some(v) = x.get(k) {
                extra.insert(k.into(), v.clone());
            }
        }
        let t2 = Instant::now();
        let line = a.ingest(&dst, Some(&extra)).unwrap();
        ingest_s.push(t2.elapsed().as_secs_f64());
        per_job.push(json!({"job": job, "entry": e, "reading": name, "t_new": line["t_new"].as_array().map(|v| v.len()),
                            "t_size": line["t_size"], "new_t": line["new_t"], "new_rooms": line["new_rooms"], "n": line["n"],
                            "chosen": line["chosen"], "selectable": line["selectable"], "stagnant_after": a.stagnant(),
                            "cells": a.cells.len(), "ingest_s": r3(*ingest_s.last().unwrap())}));
    }
    let stats = |v: &mut Vec<f64>| -> Value {
        v.sort_by(|x, y| x.partial_cmp(y).unwrap());
        json!({"median_ms": r3(v.get(v.len() / 2).copied().unwrap_or(0.0) * 1e3), "max_ms": r3(v.last().copied().unwrap_or(0.0) * 1e3),
               "sum_s": r3(v.iter().sum::<f64>())})
    };
    let mut db: Vec<f64> = doc_bytes.iter().map(|&b| b as f64).collect();
    db.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let fires: Vec<Value> = per_job.iter().filter(|j| j["stagnant_after"] == json!(true)).map(|j| j["job"].clone()).collect();
    let report = json!({
        "rec": rec, "out": out, "jobs": per_job.len(), "jobs_without_t": no_t, "t_size": a.progress.as_ref().unwrap().t.len(),
        "rows": a.rows.len(), "per_cell": per_cell(&a), "selectable": a.n_selectable(), "stagnant": a.stagnant(),
        "stagnant_after": fires, "first20": order(&a, 20),
        "cost": {"ingest": stats(&mut ingest_s), "job_doc": stats(&mut doc_s), "select_and_selectable": stats(&mut sel_s),
                 "novelty_bytes_median": db.get(db.len() / 2), "novelty_bytes_max": db.last()},
        "per_job": per_job,
    });
    std::fs::write(&out_json, serde_json::to_string_pretty(&report).unwrap() + "\n").unwrap();
    let mut s = String::new();
    s.push_str("# E012 archive, S2's replay: the kitchen check's committed tries ingested again with the switch on\n\n");
    s.push_str("Made by `archive-s2 replay`; each try's result carries the T of the reading it used (made again from its start files).\n\n");
    s.push_str(&format!("{} tries ({} without a reading's T); T at the end {} atoms; {} rows in {} cells (largest {}, median {}, {} of one entry); \
                         {} selectable at the end; stagnant after: {}.\n\n",
                        report["jobs"], no_t, report["t_size"], report["rows"], report["per_cell"]["cells"], report["per_cell"]["max"],
                        report["per_cell"]["median"], report["per_cell"]["singletons"], report["selectable"], compact(&report["stagnant_after"])));
    s.push_str("| try | entry | reading | atoms new to T | T | new atoms of T held | new rooms | n | chosen | selectable | cells | ingest s |\n");
    s.push_str("|---|---:|---|---:|---:|---|---|---:|---:|---:|---:|---:|\n");
    for j in report["per_job"].as_array().unwrap() {
        s.push_str(&format!("| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n", j["job"].as_str().unwrap(), j["entry"],
                            j["reading"].as_str().unwrap_or("-"), j["t_new"], j["t_size"], pyjson_list(&j["new_t"]), pyjson_list(&j["new_rooms"]),
                            j["n"], j["chosen"], j["selectable"], j["cells"], j["ingest_s"]));
    }
    let c = &report["cost"];
    s.push_str(&format!("\nCost per try in the archive: ingest median {} ms, max {} ms, {} s in all; job.json median {} ms, max {} ms \
                         (\"novelty\" median {} bytes, max {}); select(1) and the selectable cells median {} ms, max {} ms.\n",
                        c["ingest"]["median_ms"], c["ingest"]["max_ms"], c["ingest"]["sum_s"], c["job_doc"]["median_ms"], c["job_doc"]["max_ms"],
                        c["novelty_bytes_median"], c["novelty_bytes_max"], c["select_and_selectable"]["median_ms"], c["select_and_selectable"]["max_ms"]));
    std::fs::write(&out_md, s).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("tested") => tested(&args[1..]),
        Some("recount") => recount(&args[1..]),
        Some("reingest-off") => reingest_off(&args[1..]),
        Some("replay") => replay(&args[1..]),
        _ => {
            eprintln!("usage: archive-s2 tested NAME=DIR ... | recount COPY T.jsonl --json OUT --md OUT | reingest-off REC OUT | \
                       replay REC OUT T.jsonl MAP.tsv --json OUT --md OUT");
            std::process::exit(2);
        }
    }
}
