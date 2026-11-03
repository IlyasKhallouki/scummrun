//! kitchen-report: E012's kitchen rerun reported from its archive's files (experiments/E012-game-mechanics/plan.md,
//! "Runs" and "Reported per run"; amendment "the kitchen rerun: how it runs"). It reads files only: the archive's
//! stage.json, entries.jsonl, log.jsonl, summary.json and verify-*.json, and each committed try's
//! jobs/J/result.json and jobs/J/run/a*/run.json.
//!
//!   kitchen-report ARCHIVE [--json OUT]
//!
//! Per try: the wall by part (E011's report.py rules with T, the wait and the affordance step as parts of their own,
//! as `fmi check try-parts`), the played attempt's stop kind, the targets attempted, and E012's records (the wait, the
//! goal again, the delay fork, the affordance steps, the dead moves). Then the sums: the wall by part and by stop kind,
//! each mechanism's counts, the planner's limits, the targets, the calls per frame by room, the rooms, T, and the
//! best chain with its verify records.

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

fn u(v: &Value) -> usize {
    v.as_u64().unwrap_or(0) as usize
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

/// The played attempt's stop, by kind: the harness's stop texts up to their detail.
fn stop_kind(s: &Value) -> String {
    let Some(t) = s.as_str() else { return "none played".into() };
    for (p, k) in [
        ("goal, not from the plan", "goal, not from the plan"),
        ("refinement: Lemma R's premise R0", "Lemma R's R0"),
        ("the same empty window", "the same empty window 2 times"),
        ("planner timeout", "planner timeout"),
        ("delay fork", "delay fork"),
        ("engine: engine ended", "engine ended"),
    ] {
        if t.starts_with(p) {
            return k.into();
        }
    }
    if t == "goal" {
        return "goal".into();
    }
    t.split(':').next().unwrap_or("").to_string()
}

pub const PARTS: [&str; 8] = ["return", "read", "T", "idle/wait", "plan", "play", "afford", "rest"];

/// The wall of one try by part (`fmi check try-parts`'s rules, trycheck.rs `wall_parts`).
fn wall_parts(d: &Path, r: &Value) -> [f64; 9] {
    let a = &r["attempt"];
    let rd = &r["reading"];
    let sub: f64 = ["refinements", "extensions"]
        .iter()
        .flat_map(|k| a[*k].as_array().cloned().unwrap_or_default())
        .map(|x| f(&x["seconds"]))
        .sum();
    let mut play = 0.0;
    if let Ok(rs) = std::fs::read_dir(d.join("run")) {
        let mut ps: Vec<PathBuf> = rs.flatten().map(|e| e.path().join("run.json")).filter(|p| p.exists()).collect();
        ps.sort();
        for p in ps {
            let v = read_json(&p);
            play += v["windows"].as_array().map(|w| w.iter().map(|x| f(&x["wall_s"])).sum::<f64>()).unwrap_or(0.0);
        }
    }
    let probes: f64 = r["probes"].as_array().map(|p| p.iter().map(|x| f(&x["seconds"])).sum()).unwrap_or(0.0);
    let read = if rd["fresh"].as_bool().unwrap_or(false) { f(&rd["wall_s"]) } else { f(&rd["load_s"]) } + sub;
    let e = &r["e012"];
    let idle = if e["wait"].is_object() { f(&e["wait"]["seconds"]) } else { f(&r["idle"]["seconds"]) };
    let afford: f64 = e["afford"].as_array().map(|a| a.iter().map(|x| f(&x["step_seconds"])).sum()).unwrap_or(0.0);
    let t = f(&e["t"]["seconds"]);
    let plan = (probes - play - sub).max(0.0);
    let ret = f(&r["return"]["boot_s"]);
    let wall = f(&r["wall_s"]);
    let rest = (wall - (ret + read + t + idle + plan + play + afford)).max(0.0);
    [ret, read, t, idle, plan, play, afford, rest, wall]
}

fn target_text(t: &Value) -> String {
    match (t.get(0), t.get(1)) {
        (Some(k), Some(v)) => format!("{}={}", k.as_str().unwrap_or("?"), v),
        _ => t.to_string(),
    }
}

fn inc(m: &mut BTreeMap<String, usize>, k: impl Into<String>, n: usize) {
    *m.entry(k.into()).or_default() += n;
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(arch) = args.get(1).map(PathBuf::from) else {
        eprintln!("usage: kitchen-report ARCHIVE [--json OUT]");
        std::process::exit(2);
    };
    let json_out = args.iter().position(|a| a == "--json").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let stage = read_json(&arch.join("stage.json"));
    let summary = read_json(&arch.join("summary.json"));
    let rows = read_jsonl(&arch.join("entries.jsonl"));
    let log = read_jsonl(&arch.join("log.jsonl"));
    let root_frame = rows.first().map(|r| r["frame"].as_i64().unwrap_or(0)).unwrap_or(0);
    let room_of_entry = |i: usize| rows.get(i).and_then(|r| r["room"].as_i64()).unwrap_or(-1);

    println!("# The kitchen rerun: {}\n", arch.display());
    println!("stop: {} {}", summary["stop"], summary["detail"]);
    println!("committed tries {}, entries {}, cells {}, goal entries {}\n", summary["jobs"], summary["entries"], summary["cells"], summary["goal_entries"]);

    // the tries, in commit order
    let mut tries: Vec<Value> = Vec::new();
    let mut part_sum = [0.0f64; 9];
    let mut walls: Vec<f64> = Vec::new();
    let mut by_kind: BTreeMap<String, (usize, f64, Vec<f64>)> = BTreeMap::new();
    let mut outcomes: BTreeMap<String, usize> = BTreeMap::new();
    // the mechanisms
    let mut wait_end: BTreeMap<String, usize> = BTreeMap::new();
    let (mut wait_n, mut wait_stops, mut wait_kept, mut wait_dues) = (0usize, 0usize, 0usize, 0usize);
    let (mut wait_secs, mut wait_frames): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    let mut goal_again: Vec<Value> = Vec::new();
    let (mut d_windows, mut d_trig, mut d_ref, mut d_out, mut d_cand, mut d_kept, mut d_unb) = (0, 0, 0, 0, 0, 0, 0);
    let mut d_chosen: Vec<Value> = Vec::new();
    let mut af_by: BTreeMap<String, (usize, usize, usize, usize, usize, usize, f64)> = BTreeMap::new();
    let mut af_secs: Vec<f64> = Vec::new();
    let mut af_rooms: BTreeMap<String, usize> = BTreeMap::new();
    let mut dead: Vec<Value> = Vec::new();
    let mut kept: BTreeMap<String, usize> = BTreeMap::new();
    let mut new_how: BTreeMap<String, usize> = BTreeMap::new();
    // the planner and the targets
    let mut probe_status: BTreeMap<String, usize> = BTreeMap::new();
    let mut limits: Vec<Value> = Vec::new();
    let mut last_keys: BTreeMap<String, usize> = BTreeMap::new();
    let (mut last_list, mut last_v103, mut max_v103) = (0usize, 0usize, 0usize);
    let mut deferred_n = 0usize;
    // the calls per frame, by the try's entry room
    let mut rng: BTreeMap<i64, (u64, u64, u64, u64, BTreeMap<u64, u64>)> = BTreeMap::new();
    // R0 and empty windows without the affordance step
    let (mut r0, mut r0_afford, mut ew, mut ew_afford) = (0, 0, 0, 0);
    println!("| try | entry | room | scope | wall s | {} | stop | target | probes | new (how) | wait end/stops/kept | delay trig/cand/kept | afford why:kind trials/kept | new_t | rooms | T |", PARTS.join(" | "));
    println!("|---|---:|---:|---:|---:|{}---|---|---:|---|---|---|---|---:|---|---:|", "---:|".repeat(PARTS.len()));
    for l in log.iter().filter(|l| l.get("outcome").is_some()) {
        let job = l["job"].as_str().unwrap_or("").to_string();
        let outcome = l["outcome"].as_str().unwrap_or("").to_string();
        inc(&mut outcomes, outcome.clone(), 1);
        if outcome == "guard-killed" {
            continue;
        }
        let d = arch.join("jobs").join(&job);
        let r = read_json(&d.join("result.json"));
        if r.is_null() {
            continue;
        }
        let p = wall_parts(&d, &r);
        for (s, x) in part_sum.iter_mut().zip(p.iter()) {
            *s += x;
        }
        walls.push(p[8]);
        let kind = stop_kind(&r["attempt"]["stop"]);
        let e = by_kind.entry(kind.clone()).or_insert((0, 0.0, Vec::new()));
        e.0 += 1;
        e.1 += p[8];
        e.2.push(p[8]);
        let e12 = &r["e012"];
        // the wait
        let w = &e12["wait"];
        let wtxt = if w.is_object() {
            wait_n += 1;
            inc(&mut wait_end, w["end"].as_str().unwrap_or("?"), 1);
            wait_stops += len(&w["stops"]);
            wait_kept += u(&w["kept"]);
            if len(&w["dues0"]) > 0 {
                wait_dues += 1;
            }
            wait_secs.push(f(&w["seconds"]));
            wait_frames.push(f(&w["frames"]));
            format!("{}/{}/{}", w["end"].as_str().unwrap_or("?"), len(&w["stops"]), u(&w["kept"]))
        } else {
            "-".into()
        };
        if let Some(g) = e12["goal_again"].as_array() {
            for x in g {
                let mut x = x.clone();
                x["job"] = json!(job);
                goal_again.push(x);
            }
        }
        // the delay fork
        let dl = &e12["delay"];
        d_windows += u(&dl["windows"]);
        d_trig += u(&dl["triggered"]);
        d_ref += u(&dl["refuted"]);
        d_out += u(&dl["outcomes"]);
        d_cand += u(&dl["candidates"]);
        d_kept += u(&dl["kept"]);
        d_unb += u(&dl["summary"]["unbounded"]);
        for k in dl["chosen"].as_array().cloned().unwrap_or_default() {
            d_chosen.push(k);
        }
        let room = room_of_entry(u(&r["entry"]));
        let rr = &dl["summary"]["rng"];
        if rr.is_object() && u(&rr["frames"]) > 0 {
            let x = rng.entry(room).or_insert((0, 0, u64::MAX, 0, BTreeMap::new()));
            x.0 += rr["frames"].as_u64().unwrap_or(0);
            x.1 += rr["calls"].as_u64().unwrap_or(0);
            x.2 = x.2.min(rr["least"].as_u64().unwrap_or(u64::MAX));
            x.3 = x.3.max(rr["most"].as_u64().unwrap_or(0));
            for h in rr["hist"].as_array().cloned().unwrap_or_default() {
                *x.4.entry(h[0].as_u64().unwrap_or(0)).or_default() += h[1].as_u64().unwrap_or(0);
            }
        }
        // the affordance steps
        let mut atxt = Vec::new();
        let afs = e12["afford"].as_array().cloned().unwrap_or_default();
        for a in &afs {
            let why = a["why"].as_str().unwrap_or("?").to_string();
            let idle = a["offer"]["idle"].as_str().unwrap_or("?").to_string();
            let x = af_by.entry(format!("{why}, {idle}")).or_insert((0, 0, 0, 0, 0, 0, 0.0));
            x.0 += 1;
            x.1 += u(&a["trials"]);
            x.2 += u(&a["kept"]);
            x.3 += u(&a["third"]);
            x.4 += u(&a["offer"]["walks"]);
            x.5 += u(&a["ended"]);
            x.6 += f(&a["step_seconds"]);
            af_secs.push(f(&a["step_seconds"]));
            for rm in a["rooms"].as_array().cloned().unwrap_or_default() {
                inc(&mut af_rooms, rm[0].to_string(), rm[1].as_u64().unwrap_or(0) as usize);
            }
            atxt.push(format!("{why}:{idle} {}/{}", u(&a["trials"]), u(&a["kept"])));
        }
        if kind == "Lemma R's R0" {
            r0 += 1;
            if !afs.is_empty() {
                r0_afford += 1;
            }
        }
        if kind == "the same empty window 2 times" {
            ew += 1;
            if !afs.is_empty() {
                ew_afford += 1;
            }
        }
        for x in r["dead"].as_array().cloned().unwrap_or_default() {
            let mut x = x.clone();
            x["job"] = json!(job);
            dead.push(x);
        }
        for (k, v) in e12["kept"].as_object().cloned().unwrap_or_default() {
            inc(&mut kept, k, v.as_u64().unwrap_or(0) as usize);
        }
        let mut hows: BTreeMap<String, usize> = BTreeMap::new();
        for n in r["new"].as_array().cloned().unwrap_or_default() {
            inc(&mut hows, n["how"].as_str().unwrap_or("?"), 1);
            inc(&mut new_how, n["how"].as_str().unwrap_or("?"), 1);
        }
        // the planner's requests and the targets
        let probes = r["probes"].as_array().cloned().unwrap_or_default();
        let mut v103 = 0;
        for pr in &probes {
            let st = pr["status"].as_str().unwrap_or("null").to_string();
            inc(&mut probe_status, st.clone(), 1);
            if st == "time" {
                limits.push(json!({"job": job, "target": pr["target"], "limit_s": pr["limit_s"], "seconds": pr["seconds"], "kind": "first plan at deepening's limit"}));
            }
            if pr["stop"].as_str().map(|s| s.starts_with("planner timeout")).unwrap_or(false) {
                limits.push(json!({"job": job, "target": pr["target"], "seconds": pr["seconds"], "kind": "a replan at the planner's limit"}));
            }
            if pr["target"][0].as_str() == Some("var:103") {
                v103 += 1;
            }
        }
        max_v103 = max_v103.max(v103);
        deferred_n += len(&r["deferred"]);
        if let Some(last) = probes.last() {
            let k = last["target"][0].as_str().unwrap_or("?").to_string();
            if k == "var:103" {
                last_v103 += 1;
            }
            if last["target"][1].is_array() {
                last_list += 1;
            }
            inc(&mut last_keys, k, 1);
        }
        let target = r["attempt"]["target"].clone();
        println!(
            "| {job} | {} | {room} | {} | {:.1} | {} | {kind} | {} | {} | {} {} | {wtxt} | {}/{}/{} | {} | {} | {} | {} |",
            r["entry"], len(&r["reading"]["scope"]), p[8],
            p[..8].iter().map(|x| format!("{x:.1}")).collect::<Vec<_>>().join(" | "),
            if target.is_null() { "-".into() } else { target_text(&target) },
            probes.len(), len(&r["new"]), json!(hows), u(&dl["triggered"]), u(&dl["candidates"]), u(&dl["kept"]),
            if atxt.is_empty() { "-".into() } else { atxt.join(", ") }, len(&l["new_t"]), l["new_rooms"], l["t_size"]
        );
        tries.push(json!({"job": job, "entry": r["entry"], "room": room, "scope": r["reading"]["scope"], "wall": p[8],
            "parts": PARTS.iter().zip(p.iter()).map(|(k, v)| (k.to_string(), json!(v))).collect::<Map<_, _>>(),
            "stop_kind": kind, "stop": r["attempt"]["stop"], "target": target, "probes": probes.len(), "new": len(&r["new"]),
            "new_how": hows, "wait": wtxt, "afford": atxt, "new_t": l["new_t"], "new_rooms": l["new_rooms"], "t_size": l["t_size"],
            "selectable": l["selectable"]}));
    }
    let total = part_sum[8];
    println!("\n## The wall\n");
    println!("{} tries, {:.1} s ({:.2} h) in all; median {:.1} s, max {:.1} s\n", walls.len(), total, total / 3600.0, median(&walls), max(&walls));
    println!("| part | {} |\n|---|{}", PARTS.join(" | "), "---:|".repeat(PARTS.len()));
    println!("| sum, s | {} |", part_sum[..8].iter().map(|x| format!("{x:.1}")).collect::<Vec<_>>().join(" | "));
    println!("| share | {} |", part_sum[..8].iter().map(|x| format!("{:.1}%", 100.0 * x / total.max(1e-9))).collect::<Vec<_>>().join(" | "));
    println!("\n| the played attempt's stop | tries | wall s | share | median s | max s |\n|---|---:|---:|---:|---:|---:|");
    for (k, (n, w, ws)) in &by_kind {
        println!("| {k} | {n} | {w:.1} | {:.1}% | {:.1} | {:.1} |", 100.0 * w / total.max(1e-9), median(ws), max(ws));
    }
    println!("\noutcomes: {}", json!(outcomes));
    println!("R0 stops {r0}, of them with the affordance step {r0_afford}; the same empty window {ew}, with the step {ew_afford}");
    println!("\n## The mechanisms\n");
    println!("- wait: {wait_n} waits; ends {}; {wait_stops} stops, {wait_kept} kept; {wait_dues} with a paused slot's due frame at the entry; seconds median {:.3}, max {:.3}; frames median {:.0}, max {:.0}",
             json!(wait_end), median(&wait_secs), max(&wait_secs), median(&wait_frames), max(&wait_frames));
    println!("- goal again: {} requests from wait stops: {}", goal_again.len(), json!(goal_again));
    println!("- delay fork: {d_windows} windows, {d_trig} triggered, chosen k {}, {d_ref} refuted, {d_unb} with no K, {d_out} outcomes, {d_cand} candidates, {d_kept} kept", json!(d_chosen));
    println!("- affordance steps: {} steps, seconds median {:.2}, max {:.2}", af_secs.len(), median(&af_secs), max(&af_secs));
    for (k, x) in &af_by {
        println!("  - {k}: {} steps, {} trials, {} kept, {} third clicks, {} walk clicks, {} trials ended the run, {:.1} s", x.0, x.1, x.2, x.3, x.4, x.5, x.6);
    }
    println!("  - trials by room: {}", json!(af_rooms));
    println!("- dead moves: {} {}", dead.len(), json!(dead));
    println!("- entries kept by the layer: {}; every new entry by how: {}", json!(kept), json!(new_how));
    println!("\n## The planner and the targets\n");
    println!("- requests (probes) by first-plan status: {}", json!(probe_status));
    println!("- requests at a limit: {} {}", limits.len(), json!(limits));
    println!("- targets deferred: {deferred_n}");
    println!("- the last target's key: {}; var:103 last in {last_v103} tries; a value list last in {last_list} tries; var:103 attempted at most {max_v103} times in a try", json!(last_keys));
    println!("\n## Calls per frame (the window searches' parent frames, by the try's entry room)\n");
    println!("| room | frames | calls | least | most | histogram |\n|---:|---:|---:|---:|---:|---|");
    for (room, x) in &rng {
        println!("| {room} | {} | {} | {} | {} | {} |", x.0, x.1, x.2, x.3, json!(x.4.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>()));
    }
    // the rooms the archive's entries hold, each with its first entry
    let mut rooms: BTreeMap<i64, Value> = BTreeMap::new();
    for r in &rows {
        let rm = r["room"].as_i64().unwrap_or(-1);
        let fr = r["frame"].as_i64().unwrap_or(0);
        let better = rooms.get(&rm).map(|x| x["frame"].as_i64().unwrap_or(i64::MAX) > fr).unwrap_or(true);
        if better {
            rooms.insert(rm, json!({"entry": r["id"], "frame": fr, "how": r["how"], "job": r["job"]}));
        }
    }
    let mut hows_all: BTreeMap<String, usize> = BTreeMap::new();
    for r in &rows {
        inc(&mut hows_all, r["how"].as_str().unwrap_or("?"), 1);
    }
    println!("\n## The archive\n");
    println!("- {} entries by how: {}", rows.len(), json!(hows_all));
    println!("- rooms (the earliest entry in each): {}", json!(rooms));
    let t_sizes: Vec<i64> = log.iter().filter_map(|l| l["t_size"].as_i64()).collect();
    println!("- T: {} atoms after the first try, {} at the end", t_sizes.first().unwrap_or(&0), t_sizes.last().unwrap_or(&0));
    let last_sel = log.iter().rev().find_map(|l| l["selectable"].as_i64());
    println!("- selectable cells after the last commit: {:?}", last_sel);
    // the best chain
    let mut chain_out = Value::Null;
    if let Some(g) = summary["goal_entries"].as_array().and_then(|a| a.first()).and_then(|x| x.as_u64()) {
        let mut ch = Vec::new();
        let mut i = Some(g as usize);
        while let Some(k) = i {
            let r = &rows[k];
            ch.push(json!({"entry": k, "frame": r["frame"], "room": r["room"], "how": r["how"], "job": r["job"]}));
            i = r["parent"].as_u64().map(|p| p as usize);
        }
        ch.reverse();
        let frame = rows[g as usize]["frame"].as_i64().unwrap_or(0);
        println!("\n## The goal entry\n");
        println!("- entry {g}, frame {frame}: {} frames from the root ({root_frame})", frame - root_frame);
        println!("- chain: {}", json!(ch));
        let mut vers = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&arch) {
            let mut ps: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| {
                p.file_name().map(|n| n.to_string_lossy().starts_with("verify-") && n.to_string_lossy().ends_with(".json")).unwrap_or(false)
            }).collect();
            ps.sort();
            for p in ps {
                let v = read_json(&p);
                let s = json!({"file": p.file_name().unwrap().to_string_lossy(), "entry": v["entry"], "same": v["same"], "goal": v["goal"],
                               "frames": v["frames"], "qf": v["qf"], "seconds": v["seconds"], "inputs": v["inputs"],
                               "differences": len(&v["verify"]["differ"]), "final": v["verify"]["final"]});
                println!("- verify: {s}");
                vers.push(s);
            }
        }
        println!("- the driver's verify (summary.json): {}", summary["verify"]);
        chain_out = json!({"entry": g, "frame": frame, "frames": frame - root_frame, "chain": ch, "verify": vers, "driver_verify": summary["verify"]});
    }
    if let Some(p) = json_out {
        let out = json!({"archive": arch.display().to_string(), "stage": {"goal": stage["goal"], "allowed_rooms": stage["allowed_rooms"], "e009": stage["e009"], "e010": stage["e010"], "e012": stage["e012"]},
            "summary": summary, "tries": tries, "wall": {"sum": total, "median": median(&walls), "max": max(&walls),
            "parts": PARTS.iter().zip(part_sum.iter()).map(|(k, v)| (k.to_string(), json!(v))).collect::<Map<_, _>>(),
            "by_stop": by_kind.iter().map(|(k, (n, w, ws))| (k.clone(), json!({"tries": n, "wall": w, "median": median(ws), "max": max(ws)}))).collect::<Map<_, _>>()},
            "outcomes": outcomes, "r0": {"tries": r0, "with_afford": r0_afford}, "empty_window": {"tries": ew, "with_afford": ew_afford},
            "wait": {"n": wait_n, "ends": wait_end, "stops": wait_stops, "kept": wait_kept, "with_dues": wait_dues, "median_s": median(&wait_secs), "max_s": max(&wait_secs)},
            "goal_again": goal_again,
            "delay": {"windows": d_windows, "triggered": d_trig, "chosen": d_chosen, "refuted": d_ref, "no_k": d_unb, "outcomes": d_out, "candidates": d_cand, "kept": d_kept},
            "afford": af_by.iter().map(|(k, x)| (k.clone(), json!({"steps": x.0, "trials": x.1, "kept": x.2, "third": x.3, "walks": x.4, "ended": x.5, "seconds": x.6}))).collect::<Map<_, _>>(),
            "dead": dead, "kept": kept, "new_how": new_how, "probe_status": probe_status, "limits": limits, "deferred": deferred_n,
            "last_target_keys": last_keys, "last_var103": last_v103, "last_list": last_list, "max_var103": max_v103,
            "rng": rng.iter().map(|(k, x)| (k.to_string(), json!({"frames": x.0, "calls": x.1, "least": x.2, "most": x.3, "hist": x.4.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>()}))).collect::<Map<_, _>>(),
            "rooms": rooms.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<Map<_, _>>(), "entries_by_how": hows_all,
            "t_first": t_sizes.first(), "t_last": t_sizes.last(), "goal": chain_out});
        std::fs::write(&p, serde_json::to_string_pretty(&out).unwrap()).expect("write the JSON");
    }
}
