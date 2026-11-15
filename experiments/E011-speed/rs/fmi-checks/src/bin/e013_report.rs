//! e013-report: E013's measures of a run from its archive's files (experiments/E013-robust/plan.md, "Runs", "Reported
//! per run", and the predictions' rows). It reads files only: stage.json, entries.jsonl, log.jsonl, summary.json,
//! stalls.json, and each committed try's job.json, select.json, result.json and run/a*/run.json. The stall table itself
//! is stall-report's; the wall by part is kitchen-report's.
//!
//!   e013-report ARCHIVE [--json OUT]
//!
//! Reported: the tries by kind (model, free) and why (the share, the cell rule); the free tries' offers, trials, delays
//! and kept, and how many kept a novel entry; the shift's changes (stalls.json); B's windows (ended by B, causes, frames
//! skipped, windows at their bound); the trials by verdict and the shared waits; the affordance steps' and the free
//! offer points' seconds and the outcome reading per trial; the longest stretch of committed tries with no new atom of
//! T and no new room (tries and seconds); the goal chain's frames, its entries by how, and the windows on it with their
//! end reasons; E014's end entries. Numbers, frames and ids only: no game text.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn read_json(p: &Path) -> Value {
    std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null)
}

fn read_jsonl(p: &Path) -> Vec<Value> {
    std::fs::read_to_string(p)
        .map(|t| t.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect())
        .unwrap_or_default()
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

fn u(v: &Value) -> u64 {
    v.as_u64().unwrap_or(0)
}

fn len(v: &Value) -> usize {
    v.as_array().map(|a| a.len()).unwrap_or(0)
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    match s.len() {
        0 => f64::NAN,
        n if n % 2 == 1 => s[n / 2],
        n => (s[n / 2 - 1] + s[n / 2]) / 2.0,
    }
}

fn max(v: &[f64]) -> f64 {
    v.iter().cloned().fold(f64::NAN, f64::max)
}

fn inc(m: &mut BTreeMap<String, u64>, k: impl Into<String>, n: u64) {
    *m.entry(k.into()).or_default() += n;
}

/// The windows of a try's attempts, in attempt order (run/a*/run.json "windows").
fn windows(d: &Path) -> Vec<Value> {
    let mut out = Vec::new();
    if let Ok(rs) = std::fs::read_dir(d.join("run")) {
        let mut ps: Vec<PathBuf> = rs.flatten().map(|e| e.path().join("run.json")).filter(|p| p.exists()).collect();
        ps.sort();
        for p in ps {
            out.extend(read_json(&p)["windows"].as_array().cloned().unwrap_or_default());
        }
    }
    out
}

/// A window's end reason as a kind: "refused input: CAUSE", "N frames" (the bound), "ready: ...", the reason's head.
fn end_kind(w: &Value) -> String {
    let t = w["end_reason"].as_str().unwrap_or("?");
    if t.starts_with("refused input") {
        return t.to_string();
    }
    if t.split(' ').nth(1) == Some("frames") || t.split(';').next().map(|h| h.ends_with(" frames")).unwrap_or(false) {
        return "bound".into();
    }
    t.split(':').next().unwrap_or(t).to_string()
}

/// The affordance record's verdicts and refuse counts, added to the sums.
fn add_record(rec: &Value, verdicts: &mut BTreeMap<String, u64>, verdict_s: &mut BTreeMap<String, f64>, refuse: &mut BTreeMap<String, u64>) {
    for (k, v) in rec["verdicts"].as_object().cloned().unwrap_or_default() {
        inc(verdicts, k, u(&v));
    }
    for (k, v) in rec["verdict_s"].as_object().cloned().unwrap_or_default() {
        *verdict_s.entry(k).or_default() += f(&v);
    }
    let r = &rec["refuse"];
    for k in ["ended_at_once", "digests", "shared_trial", "shared_twin", "twin_forks", "waits_run", "pairs", "pairs_same"] {
        inc(refuse, k, u(&r[k]));
    }
    inc(refuse, "pairs_differ", len(&r["pairs_differ"]) as u64);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(arch) = args.get(1).map(PathBuf::from) else {
        eprintln!("usage: e013-report ARCHIVE [--json OUT]");
        std::process::exit(2);
    };
    let json_out = args.iter().position(|a| a == "--json").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let summary = read_json(&arch.join("summary.json"));
    let rows = read_jsonl(&arch.join("entries.jsonl"));
    let log: Vec<Value> = read_jsonl(&arch.join("log.jsonl")).into_iter().filter(|l| l.get("outcome").is_some()).collect();
    let stalls = read_json(&arch.join("stalls.json"));
    let frame_of = |i: usize| rows.get(i).and_then(|r| r["frame"].as_i64()).unwrap_or(0);

    println!("# E013's measures: {}\n", arch.display());
    println!("stop: {} {}", summary["stop"], summary["detail"]);
    println!("committed tries {}, entries {}, cells {}, goal entries {}\n", summary["jobs"], summary["entries"], summary["cells"], summary["goal_entries"]);

    let mut tries = Vec::new();
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    let (mut free_n, mut free_novel, mut free_wall, mut free_entries) = (0u64, 0u64, 0.0f64, 0u64);
    let mut free_steps: Vec<f64> = Vec::new();
    let mut free_points_s: Vec<f64> = Vec::new();
    let mut free_kept: BTreeMap<String, u64> = BTreeMap::new();
    let mut free_sum: BTreeMap<String, u64> = BTreeMap::new();
    let mut free_delay_s = 0.0;
    let mut model_n = 0u64;
    let mut model_novel = 0u64;
    // B
    let (mut win_n, mut win_b, mut skipped) = (0u64, 0u64, 0u64);
    let mut win_end: BTreeMap<String, u64> = BTreeMap::new();
    let mut b_causes: BTreeMap<String, u64> = BTreeMap::new();
    let mut b_try_causes: BTreeMap<String, u64> = BTreeMap::new();
    let mut verdicts: BTreeMap<String, u64> = BTreeMap::new();
    let mut verdict_s: BTreeMap<String, f64> = BTreeMap::new();
    let mut refuse: BTreeMap<String, u64> = BTreeMap::new();
    // the affordance steps
    let mut af_secs: Vec<f64> = Vec::new();
    let (mut af_trials, mut af_read_s, mut af_kept) = (0u64, 0.0f64, 0u64);
    let mut af_by: BTreeMap<String, (u64, u64, u64, f64)> = BTreeMap::new();
    let (mut fp_trials, mut fp_read_s) = (0u64, 0.0f64);
    // the stretch
    let (mut run_n, mut run_s, mut best_n, mut best_s) = (0u64, 0.0f64, 0u64, 0.0f64);
    let (mut best_s_n, mut best_s_s) = (0u64, 0.0f64);
    let mut stretch_at = String::new();
    // E014's end entries
    let mut end_entries = 0u64;
    let mut dead_game_end = 0u64;
    let mut new_how: BTreeMap<String, u64> = BTreeMap::new();
    let mut walls: Vec<f64> = Vec::new();
    for l in &log {
        let job = l["job"].as_str().unwrap_or("").to_string();
        let d = arch.join("jobs").join(&job);
        let r = read_json(&d.join("result.json"));
        let sel = read_json(&d.join("select.json"));
        let kind = l["kind"].as_str().unwrap_or("model").to_string();
        let why = sel["why"].as_str().unwrap_or("").to_string();
        inc(&mut kinds, format!("{kind}{}", if why.is_empty() { String::new() } else { format!(" ({why})") }), 1);
        let wall = f(&r["wall_s"]);
        walls.push(wall);
        let progress = len(&l["new_t"]) > 0 || len(&l["new_rooms"]) > 0;
        // the stretch with no new atom of T and no new room
        if progress {
            run_n = 0;
            run_s = 0.0;
        } else {
            run_n += 1;
            run_s += wall;
            if run_n > best_n {
                best_n = run_n;
                best_s = run_s;
                stretch_at = job.clone();
            }
            if run_s > best_s_s {
                best_s_s = run_s;
                best_s_n = run_n;
            }
        }
        for n in r["new"].as_array().cloned().unwrap_or_default() {
            inc(&mut new_how, n["how"].as_str().unwrap_or("?"), 1);
            if n["how"] == "end" && n.get("novel").is_some() {
                end_entries += 1;
            }
        }
        for x in r["dead"].as_array().cloned().unwrap_or_default() {
            if x.get("end").is_some() || x["reason"].as_str().map(|s| s.contains("restart") || s.contains("quit")).unwrap_or(false) {
                dead_game_end += 1;
            }
        }
        // the free try
        let fr = &r["free"];
        let mut ftxt = String::new();
        if kind == "free" {
            free_n += 1;
            free_wall += wall;
            if progress {
                free_novel += 1;
            }
            free_steps.push(f(&fr["seconds"]));
            free_delay_s += f(&fr["delays"]["seconds"]);
            for (k, v) in fr["kept"].as_object().cloned().unwrap_or_default() {
                inc(&mut free_kept, k, u(&v));
            }
            for k in ["offers", "n_points", "trials", "kept_total", "waits_run", "waits_reused"] {
                inc(&mut free_sum, k, u(&fr[k]));
            }
            let dl = &fr["delays"];
            for k in ["counted", "delayed_inputs", "unknown", "missed", "trials", "kept"] {
                inc(&mut free_sum, format!("delays.{k}"), u(&dl[k]));
            }
            for p in fr["points"].as_array().cloned().unwrap_or_default() {
                free_points_s.push(f(&p["seconds"]));
                let rec = &p["record"];
                fp_trials += u(&rec["trials"]);
                fp_read_s += f(&rec["phases_s"]["read"]);
                add_record(rec, &mut verdicts, &mut verdict_s, &mut refuse);
            }
            free_entries += r["new"].as_array().map(|a| a.iter().filter(|n| n["how"] == "free").count() as u64).unwrap_or(0);
            ftxt = format!("points {}, trials {}, kept {}", u(&fr["n_points"]), u(&fr["trials"]), u(&fr["kept_total"]));
        } else {
            model_n += 1;
            if progress {
                model_novel += 1;
            }
        }
        // B's windows
        let ws = windows(&d);
        let mut wtxt = Vec::new();
        for w in &ws {
            win_n += 1;
            let k = end_kind(w);
            inc(&mut win_end, k.clone(), 1);
            let rf = &w["refuse"];
            skipped += u(&rf["skipped_frames"]);
            for t in rf["tries"].as_array().cloned().unwrap_or_default() {
                if let Some(c) = t["cause"].as_str() {
                    inc(&mut b_try_causes, c, 1);
                }
            }
            if k.starts_with("refused input") {
                win_b += 1;
                inc(&mut b_causes, k.trim_start_matches("refused input: ").to_string(), 1);
                wtxt.push(format!("{}-{}: {}", u(&w["start"]), u(&w["end"]), k));
            } else if k == "bound" {
                wtxt.push(format!("{}-{}: bound", u(&w["start"]), u(&w["end"])));
            }
        }
        // the affordance steps
        for a in r["e012"]["afford"].as_array().cloned().unwrap_or_default() {
            let s = f(&a["step_seconds"]);
            af_secs.push(s);
            af_trials += u(&a["trials"]);
            af_kept += u(&a["kept"]);
            af_read_s += f(&a["phases_s"]["read"]);
            let x = af_by.entry(format!("{}, {}", a["why"].as_str().unwrap_or("?"), a["offer"]["idle"].as_str().unwrap_or("?"))).or_insert((0, 0, 0, 0.0));
            x.0 += 1;
            x.1 += u(&a["trials"]);
            x.2 += u(&a["kept"]);
            x.3 += s;
            add_record(&a, &mut verdicts, &mut verdict_s, &mut refuse);
        }
        tries.push(json!({"job": job, "kind": kind, "why": why, "entry": r["entry"], "wall": wall, "progress": progress,
                          "stop": r["attempt"]["stop"], "new": len(&r["new"]), "new_t": len(&l["new_t"]), "new_rooms": l["new_rooms"],
                          "windows": ws.len(), "windows_marked": wtxt, "free": ftxt}));
    }
    let n = log.len() as u64;
    let total: f64 = walls.iter().sum();
    println!("## The tries\n");
    println!("{n} committed tries, {:.1} s ({:.2} h) of try wall; median {:.1} s, max {:.1} s", total, total / 3600.0, median(&walls), max(&walls));
    println!("by kind (why): {}", json!(kinds));
    println!("model tries {model_n}, with a new atom of T or a new room {model_novel}");
    println!("\n| try | kind | why | entry | wall s | progress | new | windows | marked windows | free |\n|---|---|---|---:|---:|---|---:|---:|---|---|");
    for t in &tries {
        println!("| {} | {} | {} | {} | {:.1} | {} | {} | {} | {} | {} |", t["job"].as_str().unwrap_or(""), t["kind"].as_str().unwrap_or(""),
                 t["why"].as_str().unwrap_or(""), t["entry"], f(&t["wall"]), t["progress"], t["new"], t["windows"],
                 t["windows_marked"].as_array().map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect::<Vec<_>>().join("; ")).unwrap_or_default(),
                 t["free"].as_str().unwrap_or(""));
    }
    println!("\n## D, the free tries\n");
    println!("- {free_n} free tries of {n} ({:.1}%), {free_novel} with a new atom of T or a new room ({:.1}% of them); {:.1} s of try wall",
             100.0 * free_n as f64 / n.max(1) as f64, 100.0 * free_novel as f64 / free_n.max(1) as f64, free_wall);
    println!("- sums: {}; kept by level: {}; free entries {free_entries}", json!(free_sum), json!(free_kept));
    println!("- the free step's seconds: median {:.1}, max {:.1}; its offer points' seconds: median {:.1}, max {:.1}; delays {:.1} s",
             median(&free_steps), max(&free_steps), median(&free_points_s), max(&free_points_s), free_delay_s);
    println!("- the outcome reading per trial at the offer points: {:.1} ms ({} trials)", 1e3 * fp_read_s / fp_trials.max(1) as f64, fp_trials);
    println!("\n## The shift (stalls.json)\n");
    println!("- changes: {}", stalls["shift"]["changes"]);
    let cell_free = tries.iter().filter(|t| t["why"] == "cell" && t["kind"] == "free").count();
    let ons = stalls["shift"]["changes"].as_array().map(|a| a.iter().filter(|c| c[2] == "on").count()).unwrap_or(0);
    let trace = stalls["trace"].as_array().cloned().unwrap_or_default();
    let on_commits = trace.iter().filter(|x| x[3] == true).count();
    println!("- the global shift turned on {ons} times; on after {on_commits} of {} commits; tries made free by the cell rule: {cell_free}", trace.len());
    println!("\n## B, refused inputs\n");
    println!("- {win_n} windows; by end: {}; ended by B {win_b} ({:.1}%), frames skipped {skipped}", json!(win_end), 100.0 * win_b as f64 / win_n.max(1) as f64);
    println!("- B's causes of the windows it ended: {}; of the refused tries: {}", json!(b_causes), json!(b_try_causes));
    println!("- trials by verdict (affordance steps and free offer points): {}; seconds: {}", json!(verdicts),
             json!(verdict_s.iter().map(|(k, v)| (k.clone(), json!((v * 10.0).round() / 10.0))).collect::<Map<_, _>>()));
    println!("- shared waits and digests: {}", json!(refuse));
    println!("\n## The affordance steps (model tries)\n");
    println!("- {} steps, {af_trials} trials, {af_kept} kept; seconds median {:.1}, max {:.1}; the outcome reading per trial {:.1} ms",
             af_secs.len(), median(&af_secs), max(&af_secs), 1e3 * af_read_s / af_trials.max(1) as f64);
    for (k, x) in &af_by {
        println!("  - {k}: {} steps, {} trials, {} kept, {:.1} s", x.0, x.1, x.2, x.3);
    }
    let all_steps: Vec<f64> = af_secs.iter().chain(free_points_s.iter()).cloned().collect();
    println!("- every step and free offer point: {} of them, median {:.1} s; the outcome reading per trial {:.1} ms",
             all_steps.len(), median(&all_steps), 1e3 * (af_read_s + fp_read_s) / (af_trials + fp_trials).max(1) as f64);
    println!("\n## The longest stretch with no new atom of T and no new room\n");
    println!("- by tries: {best_n} committed tries, {best_s:.1} s (ending at {stretch_at}); by seconds: {best_s_s:.1} s over {best_s_n} tries");
    println!("\n## E014's end entries\n\n- end entries (how end with novel): {end_entries}; dead moves at a game end: {dead_game_end}; new entries by how: {}", json!(new_how));
    // the goal chain
    let mut chain_out = Value::Null;
    if let Some(g) = summary["goal_entries"].as_array().and_then(|a| a.first()).and_then(|x| x.as_u64()) {
        let mut ch = Vec::new();
        let mut i = Some(g as usize);
        while let Some(k) = i {
            ch.push(k);
            i = rows[k]["parent"].as_u64().map(|p| p as usize);
        }
        ch.reverse();
        let root_frame = frame_of(ch[0]);
        let mut on_chain = Vec::new();
        let mut by_how: BTreeMap<String, u64> = BTreeMap::new();
        for w in ch.windows(2) {
            let (p, c) = (w[0], w[1]);
            let how = rows[c]["how"].as_str().unwrap_or("?").to_string();
            inc(&mut by_how, how.clone(), 1);
            let Some(job) = rows[c]["job"].as_str() else { continue };
            for x in windows(&arch.join("jobs").join(job)) {
                let (s, e) = (u(&x["start"]) as i64, u(&x["end"]) as i64);
                if s >= frame_of(p) && e <= frame_of(c) {
                    on_chain.push(json!({"to_entry": c, "job": job, "start": s, "end": e, "frames": e - s, "end_reason": end_kind(&x)}));
                }
            }
        }
        let frames = frame_of(g as usize) - root_frame;
        let bound_on = on_chain.iter().filter(|x| x["end_reason"] == "bound").count();
        let refused_on = on_chain.iter().filter(|x| x["end_reason"].as_str().map(|s| s.starts_with("refused")).unwrap_or(false)).count();
        println!("\n## The goal chain\n");
        println!("- entry {g}: {frames} frames from the root ({root_frame}); {} entries; by how: {}", ch.len(), json!(by_how));
        println!("- chain: {}", json!(ch.iter().map(|&k| json!([k, rows[k]["frame"], rows[k]["room"], rows[k]["how"], rows[k]["job"]])).collect::<Vec<_>>()));
        println!("- windows on it: {}; at their bound {bound_on}; ended by B {refused_on}", on_chain.len());
        for x in &on_chain {
            println!("  - {}", x);
        }
        chain_out = json!({"entry": g, "frames": frames, "root": root_frame, "chain": ch, "by_how": by_how, "windows": on_chain,
                           "bound_windows": bound_on, "refused_windows": refused_on});
    }
    if let Some(p) = json_out {
        let out = json!({"archive": arch.display().to_string(), "summary": summary, "tries": tries, "kinds": kinds,
            "wall": {"sum": total, "median": median(&walls), "max": max(&walls)},
            "model": {"tries": model_n, "progress": model_novel},
            "free": {"tries": free_n, "progress": free_novel, "wall": free_wall, "sums": free_sum, "kept": free_kept, "entries": free_entries,
                     "step_median_s": median(&free_steps), "step_max_s": max(&free_steps), "point_median_s": median(&free_points_s),
                     "delays_s": free_delay_s, "read_ms_per_trial": 1e3 * fp_read_s / fp_trials.max(1) as f64},
            "shift": {"changes": stalls["shift"]["changes"], "turned_on": ons, "on_commits": on_commits, "cell_free": cell_free},
            "refuse": {"windows": win_n, "ended_by_b": win_b, "by_end": win_end, "causes": b_causes, "try_causes": b_try_causes,
                       "skipped_frames": skipped, "verdicts": verdicts, "verdict_s": verdict_s, "waits": refuse},
            "afford": {"steps": af_secs.len(), "trials": af_trials, "kept": af_kept, "median_s": median(&af_secs), "max_s": max(&af_secs),
                       "read_ms_per_trial": 1e3 * af_read_s / af_trials.max(1) as f64,
                       "by": af_by.iter().map(|(k, x)| (k.clone(), json!({"steps": x.0, "trials": x.1, "kept": x.2, "seconds": x.3}))).collect::<Map<_, _>>(),
                       "all_median_s": median(&all_steps)},
            "stretch": {"tries": best_n, "seconds": best_s, "ending_at": stretch_at, "by_seconds": {"seconds": best_s_s, "tries": best_s_n}},
            "end_entries": end_entries, "dead_game_end": dead_game_end, "new_how": new_how, "goal": chain_out});
        std::fs::write(&p, serde_json::to_string_pretty(&out).unwrap()).expect("write the JSON");
    }
}
