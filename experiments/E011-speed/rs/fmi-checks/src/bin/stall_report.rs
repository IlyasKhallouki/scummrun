//! stall-report: E013's gates C1 and C3 (experiments/E013-robust/plan.md, section C and the amendment "component
//! `stall`") over an archive's files, read only.
//!
//!   stall-report ARCHIVE [--report REPORT.json] [--live STALLS.json] [--json OUT] [--md OUT] [--project]
//!
//! C1: every committed try's rows sum to its wall; with `--report` (kitchen-report's report.json) each try's wall is
//! the report's and the rows' total is the report's; the rest row is at most 1% of the seconds, signed and absolute;
//! the novelty check holds on every line that has E012's records. C3: with `--live`, the driver's stalls.json is the
//! text this report's table writes. Then the table (fmi explore stalls), the shift's changes and the cells the cell
//! rule names after each commit (with the cells as they are now). Exit 0 when every check holds, 1 otherwise.
//! `--project` (E026 plan.md I5, G10): the line 14 x T_K1, T_K1 the wall from the first try's start to the commit of
//! the first goal entry (the end of the first log line with goal entries): Part I's 14 milestones each costing what
//! this goal from boot cost, an estimate; within 4 h when T_K1 <= 17.1 min. Not a check: the exit code is unchanged.

use fmi_explore::files;
use fmi_explore::stall::{self, Class, Stalls};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

fn arg(args: &[String], k: &str) -> Option<PathBuf> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).map(PathBuf::from)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(arch) = args.get(1).filter(|a| !a.starts_with("--")).map(PathBuf::from) else {
        eprintln!("usage: stall-report ARCHIVE [--report REPORT.json] [--live STALLS.json] [--json OUT] [--md OUT] [--project]");
        std::process::exit(2);
    };
    let t0 = Instant::now();
    let s = match Stalls::of_archive(&arch) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("stall-report: {e}");
            std::process::exit(2);
        }
    };
    let build_s = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let table = s.json();
    let json_s = t1.elapsed().as_secs_f64();
    let mut md = stall::report_text(&s);
    let mut checks: Vec<(String, bool, String)> = Vec::new();

    // C1: each try's rows sum to its wall
    let worst = s.tries.iter().map(|t| (t.rows.iter().map(|x| x.2).sum::<f64>() - t.wall).abs()).fold(0.0f64, f64::max);
    checks.push(("every try's rows sum to its wall".into(), worst < 1e-6, format!("{} tries, largest difference {worst:.2e} s", s.tries.len())));
    let (wall, rows) = s.totals();
    checks.push(("the rows' total is the tries' wall".into(), (wall - rows).abs() < 1e-6, format!("rows {rows:.3} s, wall {wall:.3} s")));
    if let Some(p) = arg(&args, "--report") {
        let rep = files::read_json(&p).unwrap_or(Value::Null);
        let by_job: BTreeMap<String, f64> = rep["tries"].as_array().map(|a| a.iter().map(|t| (t["job"].as_str().unwrap_or("").to_string(), t["wall"].as_f64().unwrap_or(f64::NAN))).collect()).unwrap_or_default();
        let mut bad = Vec::new();
        for t in &s.tries {
            let r = by_job.get(&t.job).copied().unwrap_or(f64::NAN);
            let mine: f64 = t.rows.iter().map(|x| x.2).sum();
            if !((mine - r).abs() < 1e-6) {
                bad.push(format!("{}: rows {mine} report {r}", t.job));
            }
        }
        let rsum = rep["wall"]["sum"].as_f64().unwrap_or(f64::NAN);
        checks.push(("each try's rows sum to its wall in report.json".into(), bad.is_empty() && by_job.len() == s.tries.len(),
                     format!("{} of {} tries; report {} tries; {}", s.tries.len() - bad.len(), s.tries.len(), by_job.len(), bad.join("; "))));
        checks.push(("the rows' total is report.json's".into(), (rows - rsum).abs() < 1e-6, format!("rows {rows:.3} s, report {rsum:.3} s")));
    }
    let run = &table["run"];
    let rest = run["rest"]["share"].as_f64().unwrap_or(1.0);
    let rest_abs = run["rest"]["abs_share"].as_f64().unwrap_or(1.0);
    checks.push(("the rest row is at most 1% (signed and absolute)".into(), rest.abs() <= 0.01 && rest_abs <= 0.01,
                 format!("rest {:.3} s ({:.3}%), absolute {:.3} s ({:.3}%)", run["rest"]["seconds"].as_f64().unwrap_or(0.0), 100.0 * rest,
                         run["rest"]["abs"].as_f64().unwrap_or(0.0), 100.0 * rest_abs)));
    let nov: Vec<&str> = s.tries.iter().filter(|t| t.novelty == Some(false)).map(|t| t.job.as_str()).collect();
    let with = s.tries.iter().filter(|t| t.novelty.is_some()).count();
    checks.push(("the new entries give back each line's new_t and new_rooms".into(), nov.is_empty(),
                 format!("{} of {with} lines with E012's records; failing: {nov:?}", with - nov.len())));
    // C3
    if let Some(p) = arg(&args, "--live") {
        let live = std::fs::read_to_string(&p).unwrap_or_default();
        let tmp = std::env::temp_dir().join(format!("stall-report-{}.json", std::process::id()));
        files::write_json(&tmp, &table, Some(1)).unwrap();
        let off = std::fs::read_to_string(&tmp).unwrap_or_default();
        let _ = std::fs::remove_file(&tmp);
        checks.push(("the live stalls.json equals this report's table".into(), live == off && !live.is_empty(),
                     format!("{} bytes live, {} bytes offline", live.len(), off.len())));
    }

    // the replay: the shift's changes, and the tries the cell rule would have made free (the try's entry's cell, as the
    // cells are now, named by the table of the commits before it)
    let cells = s.cells_now();
    let mut named: Vec<Value> = Vec::new();
    for k in 0..s.tries.len() {
        if let Some(c) = cells.get(&s.tries[k].entry) {
            if s.upto(k).free_cell(c) {
                named.push(json!([k, s.tries[k].job, s.tries[k].class.name()]));
            }
        }
    }
    let named_total = named.len();
    let trace = s.shift_trace();
    let on_commits = trace.iter().filter(|x| x.1.on).count();
    let mut by_kind: BTreeMap<String, (usize, usize, f64, f64)> = BTreeMap::new();
    for t in &s.tries {
        let e = by_kind.entry(format!("{} | {}", t.stop, if t.gave { "progress" } else { "none" })).or_insert((0, 0, 0.0, 0.0));
        e.0 += 1;
        e.2 += t.wall;
        e.3 += t.rows.iter().filter(|x| x.1 == Class::Productive).map(|x| x.2).sum::<f64>();
        if t.gave {
            e.1 += 1;
        }
    }
    md += "\n## Tries by stop and progress (a new atom of T or a new room)\n\n| stop | progress | tries | wall s | productive s |\n|---|---|---:|---:|---:|\n";
    for (k, (n, _, w, p)) in &by_kind {
        let (st, pr) = k.split_once(" | ").unwrap();
        md += &format!("| {st} | {pr} | {n} | {w:.1} | {p:.1} |\n");
    }
    md += &format!("\n## The shift and the cell rule replayed over the commits\n\n- the shift on after {on_commits} of {} commits; changes {}\n", s.tries.len(), table["shift"]["changes"]);
    // each non-productive class's largest share of the last 20 tries' seconds, and the commit it came at
    let mut peaks: Vec<Value> = Vec::new();
    for (k, c) in Class::ALL.iter().enumerate().filter(|(_, c)| **c != Class::Productive) {
        let best = trace.iter().enumerate().fold((0usize, 0.0f64), |b, (i, x)| if x.0[k] > b.1 { (i, x.0[k]) } else { b });
        if best.1 > 0.0 {
            peaks.push(json!([c.name(), best.1, best.0, s.tries[best.0].job]));
        }
    }
    md += "- the largest last-20 share of each non-productive class (the shift's threshold is 50%): ";
    md += &peaks.iter().map(|p| format!("{} {:.1}% after commit {} ({})", p[0].as_str().unwrap(), 100.0 * p[1].as_f64().unwrap(), p[2], p[3].as_str().unwrap())).collect::<Vec<_>>().join("; ");
    md += "\n";
    md += &format!("- tries the cell rule would have made free ({named_total} of {}; the cells as they are now; [commit, job, the try's class]): {}\n",
                   s.tries.len(), json!(named));
    md += &format!("- cost: the table from the files {build_s:.3} s ({:.2} ms per try), its JSON {:.1} ms\n", 1e3 * build_s / s.tries.len().max(1) as f64, 1e3 * json_s);
    let projection = if args.iter().any(|a| a == "--project") {
        let p = project(&arch);
        md += &format!("\n## Projection (E026 G10)\n\n- {}\n", p["line"].as_str().unwrap_or(""));
        p
    } else {
        Value::Null
    };
    md += "\n## Checks\n\n";
    let mut ok = true;
    for (name, pass, detail) in &checks {
        ok &= *pass;
        md += &format!("- {} {name}: {detail}\n", if *pass { "PASS" } else { "FAIL" });
    }
    print!("{md}");
    if let Some(p) = arg(&args, "--md") {
        std::fs::write(&p, &md).expect("write the text");
    }
    if let Some(p) = arg(&args, "--json") {
        let mut out = json!({"archive": arch.display().to_string(), "table": table,
            "checks": checks.iter().map(|(n, p, d)| json!({"check": n, "pass": p, "detail": d})).collect::<Vec<_>>(),
            "replay": {"shift_on_commits": on_commits, "cell_rule_tries": named, "last20_peaks": peaks},
            "by_stop": by_kind.iter().map(|(k, (n, g, w, p))| json!({"stop": k, "tries": n, "gave": g, "wall": w, "productive": p})).collect::<Vec<_>>(),
            "cost": {"table_s": build_s, "json_s": json_s}});
        if !projection.is_null() {
            out["projection"] = projection;
        }
        files::write_json(&p, &out, Some(1)).expect("write the JSON");
    }
    std::process::exit(if ok { 0 } else { 1 });
}

/// The milestones of Part I's ladder (rungs-part1.json's P1-P14) and the run's wall (E026 plan.md G10).
const MILESTONES: f64 = 14.0;
const RUN_WALL_S: f64 = 4.0 * 3600.0;

/// G10's projection from the archive's log: MILESTONES x T_K1 against RUN_WALL_S.
fn project(arch: &std::path::Path) -> Value {
    let lines = files::read_jsonl(&arch.join("log.jsonl")).map(|x| x.0).unwrap_or_default();
    let t0 = lines.iter().filter_map(|l| l["started"].as_f64()).fold(f64::INFINITY, f64::min);
    let goal = lines.iter().find(|l| l["goal_entries"].as_array().is_some_and(|g| !g.is_empty()));
    match goal.and_then(|l| Some((l, l["started"].as_f64()? + l["wall_s"].as_f64()?))) {
        Some((l, t1)) if t0.is_finite() => {
            let t = t1 - t0;
            let x = MILESTONES * t;
            let entry = l["goal_entries"].as_array().and_then(|g| g.iter().filter_map(|e| e.as_i64()).min());
            json!({"t_k1_s": t, "goal_entry": entry, "job": l["job"], "projected_s": x, "bound_s": RUN_WALL_S, "within": x <= RUN_WALL_S,
                   "line": format!("T_K1 {t:.1} s ({:.1} min): goal entry {} ({}), from the first try's start to its commit; {MILESTONES} x T_K1 = {:.2} h, {} the {:.0} h bound (T_K1 <= {:.1} min)",
                                   t / 60.0, entry.map(|e| e.to_string()).unwrap_or("?".into()), l["job"].as_str().unwrap_or("?"), x / 3600.0,
                                   if x <= RUN_WALL_S { "within" } else { "over" }, RUN_WALL_S / 3600.0, RUN_WALL_S / MILESTONES / 60.0)})
        }
        _ => json!({"t_k1_s": null, "within": false, "line": format!("no goal entry in {} log lines: no projection", lines.len())}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_projection_is_14_times_the_wall_to_the_first_goal_commit() {
        let d = std::env::temp_dir().join(format!("stall-report-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let rows = [json!({"job": "j00001", "started": 1000.0, "wall_s": 50.0, "goal_entries": []}),
                    json!({"job": "j00000", "started": 990.0, "wall_s": 5.0}),
                    json!({"job": "j00002", "started": 1500.0, "wall_s": 100.0, "goal_entries": [9, 7]}),
                    json!({"job": "j00003", "started": 1550.0, "wall_s": 10.0, "goal_entries": [11]})];
        std::fs::write(d.join("log.jsonl"), rows.iter().map(|r| format!("{r}\n")).collect::<String>()).unwrap();
        let p = project(&d);
        // from 990 (the first try's start) to 1600 (j00002's end): 610 s; 14 x 610 s within 4 h
        assert_eq!(p["t_k1_s"], json!(610.0));
        assert_eq!(p["goal_entry"], json!(7));
        assert_eq!(p["projected_s"], json!(8540.0));
        assert_eq!(p["within"], json!(true));
        std::fs::write(d.join("log.jsonl"), format!("{}\n", rows[0])).unwrap();
        assert_eq!(project(&d)["within"], json!(false));
        let _ = std::fs::remove_dir_all(&d);
    }
}
