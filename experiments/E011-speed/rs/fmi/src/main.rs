//! fmi: E011's one resident process (experiments/E011-speed/plan.md, "Architecture").
//!
//! usage:
//!   fmi explore init ARCHIVE --stage NAME --goal K=V[,K=V]|part1 --guide GUIDE.json --allowed kN|files|R,R,...
//!                    [--workers B] [--idle-frames L] [--rotate] [--evict-by-size] [--reuse-nonfacts] [--stop-jobs N]
//!                    [--ready] [--commit-as-finished]
//!   fmi explore run ARCHIVE --workers B [--step-cmd CMD | --fake] [--no-verify] [--search fd|gbfs] [--tail]
//!   fmi explore verify ARCHIVE [--entry ID]
//!   fmi explore status ARCHIVE
//!   fmi explore stalls ARCHIVE [--json OUT]   (E013: the stall table over the archive's files, stall.rs)
//!   fmi try JOB_DIR... | - | @FILE [--targets K=V,...] [--search fd|gbfs] [--tail] [--doubles RECORDED_JOB_DIR]
//!
//! `--search gbfs --tail` are the fast settings (the Rust search; at a replan the last plan's valid suffix first);
//! without them the planner is called as the reference calls it.
//!   fmi fake-step run JOB_DIR          (fake_step.py, for the driver's tests)
//!   fmi check step FIXTURES.jsonl      (step.py's targets, reusable and idle_rule against the reference's outputs)
//!   fmi guide map --goal K=V[,K=V] | part1 --out GUIDE.json [--wide] [--seed 1]   (guide.py map)
//!   fmi guide stats GUIDE.json [--key KEY.json]                                     (guide.py stats)
//!   fmi read FILE      readings one after another through the readings port, one per line of reader-scope's
//!                      arguments (E012 resident: read.rs)
//!   fmi shorten label --movie M --archive A [--seed S] [--no-twins] [--no-draws] [--check] [--tail N] OUT
//!                      E024: a movie's steps, anchors and draw keys (fmi-shorten; shorten.rs)
//!   fmi shorten run --movie M --archive A --goal end --seed S [--skips] [--workers N] [--chunk C] [--tail N]
//!                   [--no-twins] [--no-ego] [--no-fallbacks] [--rounds R] [--wall SECONDS] OUT
//!                      E024: the movie's steps found again from boot (the forward pass, its fallbacks), its route
//!                      verified, then the reductions (R rounds)
//!   fmi shorten verify ROUTE [--ends] OUT
//!                      E024: a route checked from boot from its files (item 9)
//!   fmi shorten trace --movie M [--seed S] [--from F] [--to T] [--env K=V,...] OUT
//!                      E024: a diagnostic, the movie's stops frame by frame
//!
//! `explore run` is explore.py's `run`: the same archive files. With `--step-cmd` each job is a process as explore.py
//! runs it; with `--fake` each job is fake_step.py's port run inside this process; else each job is a try (step.py's
//! run_job, fmi-explore's tryjob) inside this process, its ports warm from job to job (wire.rs).
//! `try` is step.py's `run JOB_DIR`: one try from a job.json; `--targets` forces the targets, in order (the benchmark).
//! With several JOB_DIRs (or "-": one per line on stdin) the tries run one after the other in this process, warm.

mod read;
mod shorten;
mod wire;
mod wire_real;
mod trycheck;

use fmi_explore::driver::{self, DriverStats, JobLog, StepMode};
use fmi_explore::fake::FakeStep;
use fmi_explore::files;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn usage() -> ! {
    eprintln!("{}", USAGE);
    std::process::exit(2)
}

const USAGE: &str = "usage: fmi explore init ARCHIVE --stage NAME --goal K=V[,K=V]|part1 --guide GUIDE.json --allowed kN|files|R,..
                [--workers B] [--idle-frames L] [--rotate] [--evict-by-size] [--reuse-nonfacts] [--stop-jobs N] [--ready]
                [--commit-as-finished]
       fmi explore run ARCHIVE --workers B [--step-cmd CMD | --fake] [--no-verify] [--search fd|gbfs] [--tail]
       fmi explore verify ARCHIVE [--entry ID]
       fmi explore status ARCHIVE
       fmi explore stalls ARCHIVE [--json OUT]
       fmi try JOB_DIR... | - | @FILE [--targets K=V,...] [--search fd|gbfs] [--tail] [--doubles RECORDED_JOB_DIR]
       fmi fake-step run JOB_DIR
       fmi check step|harness FIXTURES.jsonl
       fmi check replay JOB_DIR --out DIR [--domains DIR]
       fmi check reingest ARCHIVE --out DIR
       fmi check tries A_SEQ B_SEQ [--json OUT]
       fmi check verify-new SEQ --out DIR [--threads N]
       fmi check try-report SEQ...
       fmi check try-parts SEQ... [--json OUT]
       fmi check wait ARCHIVE --entries I,J,.. (--reading READING.json | --read-scope R,R,.. --read-dir DIR) --out DIR
                [--verify] [--view-check] [--due-check] [--idle-check] [--repeat N]
       fmi guide map --goal K=V[,K=V] | part1 | end=1 --out GUIDE.json [--wide] [--seed 1] [--start-boot]
       fmi guide stats GUIDE.json [--key KEY.json]
       fmi read FILE
       fmi shorten label --movie M --archive A [--seed S] [--no-twins] [--no-draws] [--check] [--tail N] OUT
       fmi shorten run --movie M --archive A --goal end --seed S [--skips] [--workers N] [--chunk C] [--tail N] [--no-twins]
                       [--no-ego] [--no-fallbacks] [--rounds R] [--wall SECONDS] OUT
       fmi shorten verify ROUTE [--ends] OUT
       fmi shorten trace --movie M [--seed S] [--from F] [--to T] [--env K=V,...] OUT";

struct Args {
    pos: Vec<String>,
    opts: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(argv: &[String], with_value: &[&str]) -> Args {
        let (mut pos, mut opts) = (Vec::new(), Vec::new());
        let mut i = 0;
        while i < argv.len() {
            let a = &argv[i];
            if let Some(name) = a.strip_prefix("--") {
                if let Some((k, v)) = name.split_once('=') {
                    opts.push((k.to_string(), Some(v.to_string())));
                } else if with_value.contains(&name) {
                    i += 1;
                    opts.push((name.to_string(), argv.get(i).cloned()));
                } else {
                    opts.push((name.to_string(), None));
                }
            } else {
                pos.push(a.clone());
            }
            i += 1;
        }
        Args { pos, opts }
    }
    fn get(&self, k: &str) -> Option<&str> {
        self.opts.iter().rev().find(|(n, _)| n == k).and_then(|(_, v)| v.as_deref())
    }
    fn flag(&self, k: &str) -> bool {
        self.opts.iter().any(|(n, _)| n == k)
    }
}

fn shlex_split(s: &str) -> Vec<String> {
    // the step commands the tests give: words separated by spaces, with simple quotes
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut q: Option<char> = None;
    let mut any = false;
    for c in s.chars() {
        match (q, c) {
            (Some(x), c) if c == x => q = None,
            (Some(_), c) => cur.push(c),
            (None, '\'' | '"') => {
                q = Some(c);
                any = true
            }
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() || any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if !cur.is_empty() || any {
        out.push(cur);
    }
    out
}

/// E022: the reader's `--slot-locals` lines from a try's state (fmi-play's `dumped_slots`), for fmi-explore's
/// `tryjob::SLOT_LOCALS`.
fn slot_locals_json(s: &fmi_core::Obs) -> Option<Vec<serde_json::Value>> {
    let (slots, _) = fmi_play::reading::dumped_slots(s)?;
    let mut v: Vec<&fmi_play::reading::DSlot> = slots.values().collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    Some(v.into_iter().map(|d| serde_json::json!({
        "name": d.name, "room": d.room, "w": d.w, "esc": d.esc, "locals": d.locals, "override": d.override_,
        "cut_data": d.cut_data, "owned": d.owned,
        "flags": d.flags.map(|(r, c, o)| serde_json::json!({"resistant": r, "recursive": c, "owned": o})),
    })).collect())
}

fn main() {
    let _ = fmi_explore::tryjob::SLOT_LOCALS.set(slot_locals_json);
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() {
        usage();
    }
    let code = match (argv[0].as_str(), argv.get(1).map(|s| s.as_str())) {
        ("explore", Some("run")) => cmd_run(&argv[2..]),
        ("explore", Some("init")) => cmd_init(&argv[2..]),
        ("explore", Some("verify")) => cmd_verify(&argv[2..]),
        ("explore", Some("status")) => cmd_status(&argv[2..]),
        ("explore", Some("stalls")) => cmd_stalls(&argv[2..]),
        ("try", Some(_)) => cmd_try(&argv[1..]),
        ("fake-step", Some("run")) => cmd_fake(&argv[2..]),
        ("check", Some("step")) => cmd_check_step(&argv[2..]),
        ("check", Some("harness")) => cmd_check_harness(&argv[2..]),
        ("check", Some("replay")) => cmd_check_replay(&argv[2..]),
        ("check", Some("reingest")) => cmd_check_reingest(&argv[2..]),
        ("check", Some("tries")) => cmd_check_tries(&argv[2..]),
        ("check", Some("verify-new")) => cmd_check_verify_new(&argv[2..]),
        ("check", Some("try-report")) => cmd_check_try_report(&argv[2..]),
        ("check", Some("try-parts")) => cmd_check_try_parts(&argv[2..]),
        ("check", Some("wait")) => cmd_check_wait(&argv[2..]),
        ("guide", Some("map")) => cmd_guide_map(&argv[2..]),
        ("guide", Some("stats")) => cmd_guide_stats(&argv[2..]),
        ("read", Some(_)) => read::cmd_read(&argv[1..]),
        ("shorten", Some("label")) => shorten::cmd_label(&argv[2..]),
        ("shorten", Some("run")) => shorten::cmd_run(&argv[2..]),
        ("shorten", Some("trace")) => shorten::cmd_trace(&argv[2..]),
        ("shorten", Some("verify")) => shorten::cmd_verify(&argv[2..]),
        _ => usage(),
    };
    std::process::exit(code)
}

fn targets_of(t: Option<&str>) -> Result<Option<Vec<(String, i64)>>, String> {
    let Some(t) = t else { return Ok(None) };
    let mut out = Vec::new();
    for part in t.split(',').filter(|p| !p.trim().is_empty()) {
        let (k, v) = part.split_once('=').ok_or(format!("--targets {t}: K=V,..."))?;
        out.push((k.trim().to_string(), v.trim().parse().map_err(|_| format!("--targets {t}: {v} is not an int"))?));
    }
    Ok(Some(out))
}

fn wire_opts(a: &Args) -> Result<wire::WireOpts, String> {
    Ok(wire::WireOpts { search: wire::search_of(a.get("search"))?, doubles: a.get("doubles").map(PathBuf::from),
                        forced_targets: targets_of(a.get("targets"))?, tail: a.flag("tail") })
}

fn cmd_try(argv: &[String]) -> i32 {
    let rc = cmd_try_in(argv);
    if std::env::var_os("FMI_FORK_COUNTS").is_some() {
        for (site, n) in fmi_core::fork_counts() {
            eprintln!("forks {site} {n}");
        }
    }
    rc
}

fn cmd_try_in(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &["targets", "search", "doubles", "repeat"]);
    if a.pos.is_empty() {
        usage()
    }
    let w = match wire_opts(&a).and_then(|o| wire::wire(&o)) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("fmi try: {e}");
            return 2;
        }
    };
    fmi_explore::machine::nice10();
    // several JOB_DIRs: the tries one after the other in this process, its ports warm from try to try (the
    // benchmark's warm regime); "-" or "@FILE": the job folders read from stdin or FILE, one per line, each answered on stdout with
    // "done JOB_DIR EXIT" once its try has ended (a driver interleaves other work between the tries)
    if a.pos.len() > 1 || a.pos[0] == "-" || a.pos[0].starts_with('@') {
        let mut code = 0;
        let mut one = |jd: &str| -> i32 {
            let c = try_one(&PathBuf::from(jd), &a, &w);
            if c != 0 {
                code = c;
            }
            c
        };
        if a.pos[0] == "-" || a.pos[0].starts_with('@') {
            use std::io::{BufRead, Write};
            // "@PATH": the job folders from that file (a FIFO a driver writes to), else stdin
            let input: Box<dyn BufRead> = match a.pos[0].strip_prefix('@') {
                Some(p) => match std::fs::File::open(p) {
                    Ok(f) => Box::new(std::io::BufReader::new(f)),
                    Err(e) => {
                        eprintln!("fmi try: {p}: {e}");
                        return 1;
                    }
                },
                None => Box::new(std::io::stdin().lock()),
            };
            for line in input.lines() {
                let Ok(line) = line else { break };
                let jd = line.trim();
                if jd.is_empty() {
                    continue;
                }
                let c = one(jd);
                println!("done {jd} {c}");
                let _ = std::io::stdout().flush();
            }
        } else {
            for jd in &a.pos {
                one(jd);
            }
        }
        return code;
    }
    try_one(&PathBuf::from(&a.pos[0]), &a, &w)
}

fn try_one(jd: &std::path::Path, a: &Args, w: &wire::Wired) -> i32 {
    let jd = jd.to_path_buf();
    let log = match JobLog::append(&jd.join("log")) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("fmi try: {e}");
            return 1;
        }
    };
    // --repeat N: the same try N times in this process (a benchmark of the try's own time on a fixed input)
    let n: usize = a.get("repeat").and_then(|x| x.parse().ok()).unwrap_or(1);
    let mut r = serde_json::Value::Null;
    let mut own = Vec::new();
    for k in 0..n {
        if k > 0 {
            for d in ["run", "new"] {
                let _ = std::fs::remove_dir_all(jd.join(d));
            }
        }
        r = fmi_explore::tryjob::run_job(&jd, &w.resident, &log, &AtomicBool::new(false));
        own.push(r["explore_s"]["own"].as_f64().unwrap_or(0.0));
    }
    if n > 1 {
        own.sort_by(|a, b| a.partial_cmp(b).unwrap());
        eprintln!("try x{n}: own time median {:.2} ms, min {:.2} ms, max {:.2} ms", own[n / 2] * 1e3, own[0] * 1e3, own[n - 1] * 1e3);
    }
    fmi_explore::step::exit_code(&r)
}

fn cmd_init(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &["stage", "goal", "guide", "allowed", "workers", "idle-frames", "stop-jobs", "search", "doubles"]);
    let Some(arch) = a.pos.first() else { usage() };
    let (Some(stage), Some(goal), Some(guide), Some(allowed)) = (a.get("stage"), a.get("goal"), a.get("guide"), a.get("allowed")) else { usage() };
    let ia = fmi_explore::explorecmd::InitArgs {
        archive: arch.clone(),
        stage: stage.into(),
        goal: goal.into(),
        guide: guide.into(),
        allowed: allowed.into(),
        workers: a.get("workers").and_then(|w| w.parse().ok()).unwrap_or(2),
        idle_frames: a.get("idle-frames").and_then(|w| w.parse().ok()).unwrap_or(0),
        rotate: a.flag("rotate"),
        evict_by_size: a.flag("evict-by-size"),
        reuse_nonfacts: a.flag("reuse-nonfacts"),
        stop_jobs: a.get("stop-jobs").and_then(|w| w.parse().ok()).unwrap_or(0),
        ready: a.flag("ready"),
        commit_as_finished: a.flag("commit-as-finished"),
    };
    let w = match wire_opts(&a).and_then(|o| wire::wire(&o)) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("fmi explore init: {e}");
            return 2;
        }
    };
    let game = if allowed.starts_with('k') { fmi_explore::guide::load_game(&fmi_explore::guide::default_root()).ok() } else { None };
    let mut ret = w.resident.returns.lock().unwrap();
    match fmi_explore::explorecmd::init(&ia, &mut **ret, game.as_ref()) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("fmi explore init: {e}");
            1
        }
    }
}

fn cmd_verify(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &["entry", "search", "doubles"]);
    let Some(arch) = a.pos.first() else { usage() };
    let host = match wire_opts(&a).and_then(|o| wire::wire(&o)).and_then(|w| w.host.ok_or_else(|| "no engine host wired".to_string())) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("fmi explore verify: {e}");
            return 2;
        }
    };
    let run = || -> Result<serde_json::Value, String> {
        let ar = fmi_explore::archive::Archive::open(&PathBuf::from(arch))?;
        let i = match a.get("entry").and_then(|e| e.parse::<usize>().ok()) {
            Some(i) => i,
            None => *ar.goal_entries().first().ok_or(format!("{arch}: no goal entry to verify"))?,
        };
        use fmi_explore::driver::Verifier;
        fmi_explore::explorecmd::HostVerifier { host, lean: true }.verify(&ar, i)
    };
    match run() {
        Ok(v) => {
            let mut m = serde_json::Map::new();
            for k in ["entry", "same", "goal", "frames", "qf", "seconds", "inputs", "segments"] {
                m.insert(k.into(), v.get(k).cloned().unwrap_or(serde_json::Value::Null));
            }
            println!("{}", fmi_explore::pyjson::dumps_default(&serde_json::Value::Object(m)));
            println!("differences: {}; final {}", v["verify"]["differ"].as_array().map(|d| d.len()).unwrap_or(0),
                     files::py_repr(&v["verify"]["final"]));
            0
        }
        Err(e) => {
            eprintln!("fmi explore verify: {e}");
            1
        }
    }
}

fn cmd_run(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &["workers", "step-cmd", "search", "doubles"]);
    let Some(root) = a.pos.first() else { usage() };
    let workers: usize = a.get("workers").and_then(|w| w.parse().ok()).unwrap_or(2);
    let mut verifier: Option<fmi_explore::explorecmd::HostVerifier> = None;
    let mode = if let Some(cmd) = a.get("step-cmd") {
        StepMode::Command(shlex_split(cmd))
    } else if a.flag("fake") {
        StepMode::InProcess(Arc::new(FakeStep))
    } else {
        match wire_opts(&a).and_then(|o| wire::wire(&o)) {
            Ok(w) => {
                verifier = w.host.clone().map(|host| fmi_explore::explorecmd::HostVerifier { host, lean: true });
                fmi_explore::machine::nice10();
                StepMode::InProcess(Arc::new(fmi_explore::tryjob::TryStep(w.resident)))
            }
            Err(e) => {
                eprintln!("fmi explore run: {e}");
                return 2;
            }
        }
    };
    if !a.flag("no-verify") && verifier.is_none() {
        eprintln!("fmi explore run: the verify needs the engine host (wire.rs); give --no-verify");
        return 2;
    }
    // E013: stall's two rules as free's port (asked only with FMI_FREE_SHIFT=1)
    fmi_explore::free::set_shift(Box::new(fmi_explore::stall::ArchivePort::default()));
    let mut stats = DriverStats::default();
    let ver: Option<&dyn fmi_explore::driver::Verifier> = if a.flag("no-verify") { None } else { verifier.as_ref().map(|v| v as &dyn fmi_explore::driver::Verifier) };
    match driver::run(&PathBuf::from(root), workers, &mode, ver, &mut stats) {
        Ok(_) => {
            if std::env::var("FMI_DRIVER_STATS").is_ok() {
                eprintln!(
                    "driver: {} jobs started, own {:.6} s; per committed job {:?}",
                    stats.jobs, stats.own_s, stats.own_by_job
                );
            }
            0
        }
        Err(e) => {
            eprintln!("fmi explore run: {e}");
            1
        }
    }
}

fn cmd_status(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &[]);
    let Some(root) = a.pos.first() else { usage() };
    match fmi_explore::archive::Archive::open(&PathBuf::from(root)) {
        Ok(ar) => {
            println!(
                "{}: {} entries, {} cells ({} selectable), {} committed jobs, {} unfinished, goal entries {:?}, stagnant {}",
                root,
                ar.rows.len(),
                ar.cells.len(),
                ar.n_selectable(),
                ar.committed_jobs().len(),
                ar.unfinished_jobs().len(),
                ar.goal_entries(),
                ar.stagnant()
            );
            if let Ok(s) = files::read_json(&PathBuf::from(root).join("summary.json")) {
                println!("stopped: {}", fmi_explore::pyjson::compact(&s));
            }
            0
        }
        Err(e) => {
            eprintln!("fmi explore status: {e}");
            1
        }
    }
}

/// E013: `fmi explore stalls ARCHIVE [--json OUT]` (explorecmd::stalls).
fn cmd_stalls(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &["json"]);
    let Some(root) = a.pos.first() else { usage() };
    match fmi_explore::explorecmd::stalls(&PathBuf::from(root), a.get("json").map(std::path::Path::new)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("fmi explore stalls: {e}");
            1
        }
    }
}

fn cmd_fake(argv: &[String]) -> i32 {
    let Some(d) = argv.first() else { usage() };
    let d = PathBuf::from(d);
    // a process of its own: its lines go to stdout, which the guard script keeps as the job's log
    let log = JobLog::append(&PathBuf::from("/dev/stdout")).unwrap_or_else(|_| JobLog::create(&d.join("fake.log")).unwrap());
    match fmi_explore::fake::fake(&d, &log) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("fake step: {e}");
            1
        }
    }
}

fn cmd_check_replay(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &["out", "domains"]);
    let (Some(job), Some(out)) = (a.pos.first(), a.get("out")) else { usage() };
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let doms = a.get("domains").map(PathBuf::from).unwrap_or(home.join(".cache/fast-mi-e011/explore/fixtures/harness/domains"));
    match fmi_explore::check::replay_check(&PathBuf::from(job), &PathBuf::from(out), &doms) {
        Ok((rows, secs)) => {
            let bad = rows.iter().filter(|r| !r.1).count();
            for (what, ok, why) in &rows {
                println!("{} {what}{}", if *ok { "same" } else { "DIFF" }, if why.is_empty() { String::new() } else { format!(": {}", &why[..why.len().min(600)]) });
            }
            println!("replay of {job}: {} checks, {bad} differ; the try took {:.3} s with the doubles", rows.len(), secs);
            (bad > 0) as i32
        }
        Err(e) => {
            eprintln!("fmi check replay: {e}");
            1
        }
    }
}

fn cmd_check_reingest(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &["out"]);
    let (Some(arch), Some(out)) = (a.pos.first(), a.get("out")) else { usage() };
    match fmi_explore::check::reingest_check(&PathBuf::from(arch), &PathBuf::from(out)) {
        Ok((rows, times)) => {
            let bad = rows.iter().filter(|r| !r.1).count();
            for (what, ok, why) in &rows {
                if !ok || what == "cpu" || what.starts_with("entries") || what.ends_with("json") || what.ends_with("jsonl") || what.contains("(") {
                    println!("{} {what}{}", if *ok { "same" } else { "DIFF" }, if why.is_empty() { String::new() } else { format!(": {why}") });
                }
            }
            let mut t = times.clone();
            t.sort_by(|x, y| x.partial_cmp(y).unwrap());
            let med = t.get(t.len() / 2).copied().unwrap_or(0.0);
            let total: f64 = t.iter().sum();
            println!("reingest of {arch}: {} jobs, {} checks differ; the archive's time per job: median {:.2} ms, mean {:.2} ms, max {:.2} ms",
                     times.len(), bad, med * 1e3, total * 1e3 / t.len().max(1) as f64, t.last().copied().unwrap_or(0.0) * 1e3);
            (bad > 0) as i32
        }
        Err(e) => {
            eprintln!("fmi check reingest: {e}");
            1
        }
    }
}

/// E012's wait gates (fmi_explore::waitcheck): the record on stdout, one summary line per entry on stderr.
fn cmd_check_wait(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &["entries", "reading", "read-scope", "read-dir", "out", "repeat", "search"]);
    let (Some(arch), Some(entries), Some(out)) = (a.pos.first(), a.get("entries"), a.get("out")) else { usage() };
    let ints = |t: &str| -> Vec<i64> { t.split(',').filter_map(|x| x.trim().parse().ok()).collect() };
    let args = fmi_explore::waitcheck::WaitCheckArgs {
        archive: PathBuf::from(arch),
        entries: ints(entries).into_iter().map(|x| x as usize).collect(),
        reading: a.get("reading").map(PathBuf::from),
        read_scope: a.get("read-scope").map(ints),
        read_dir: a.get("read-dir").map(PathBuf::from),
        out: PathBuf::from(out),
        verify: a.flag("verify"),
        view_check: a.flag("view-check"),
        due_check: a.flag("due-check"),
        idle_check: a.flag("idle-check"),
        repeat: a.get("repeat").and_then(|x| x.parse().ok()).unwrap_or(0),
    };
    let w = match wire_opts(&a).and_then(|o| wire::wire(&o)) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("fmi check wait: {e}");
            return 2;
        }
    };
    let Some(host) = w.host.clone() else {
        eprintln!("fmi check wait: no engine host wired");
        return 2;
    };
    fmi_explore::machine::nice10();
    match fmi_explore::waitcheck::wait_check(host, w.resident.readings.clone(), &args) {
        Ok(v) => {
            println!("{}", fmi_explore::pyjson::dumps_default(&v));
            0
        }
        Err(e) => {
            eprintln!("fmi check wait: {e}");
            1
        }
    }
}

fn cmd_check_harness(argv: &[String]) -> i32 {
    let Some(p) = argv.first() else { usage() };
    report_check("harness", fmi_explore::check::harness_check(&PathBuf::from(p)))
}

fn cmd_check_step(argv: &[String]) -> i32 {
    let Some(p) = argv.first() else { usage() };
    report_check("step", fmi_explore::check::step_check(&PathBuf::from(p)))
}

fn report_check(what: &str, r: Result<fmi_explore::check::CheckReport, String>) -> i32 {
    match r {
        Ok(r) => {
            for (k, n, bad) in &r.by_kind {
                let s = r.seconds.iter().find(|x| x.0 == *k).map(|x| x.1).unwrap_or(0.0);
                println!("{k}: {n} cases, {bad} differ; {:.3} ms a case in Rust", 1000.0 * s / (*n).max(1) as f64);
            }
            for b in &r.first_bad {
                println!("  {b}");
            }
            let bad: usize = r.by_kind.iter().map(|x| x.2).sum();
            println!("{what} check: {} cases, {bad} differ", r.cases);
            (bad > 0) as i32
        }
        Err(e) => {
            eprintln!("fmi check {what}: {e}");
            1
        }
    }
}

fn repo_root() -> PathBuf {
    scumm_root()
}

fn scumm_root() -> PathBuf {
    std::env::var_os("FAST_MI_ROOT").map(PathBuf::from).unwrap_or_else(scumm_ir_default_root)
}

fn scumm_ir_default_root() -> PathBuf {
    fmi_explore::guide::default_root()
}

fn cmd_guide_map(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &["goal", "out", "seed"]);
    let (Some(goal), Some(out)) = (a.get("goal"), a.get("out")) else { usage() };
    let seed: i64 = a.get("seed").and_then(|s| s.parse().ok()).unwrap_or(1);
    let t0 = std::time::Instant::now();
    let root = repo_root();
    let e003 = root.join("experiments").join("E003-path-through-cloud");
    let run = || -> Result<String, String> {
        let goal = fmi_explore::guide::parse_goal(goal, &e003)?;
        // E014: --start-boot, the start facts of the game's own boot (its first idle stop, the engine host from the
        // environment: FMI_GAME_ID's game), not MI's start row
        let start = if a.flag("start-boot") { start_of_boot(seed)? } else { fmi_explore::guide::start_row(&e003, seed)? };
        let game = fmi_explore::guide::load_game(&root)?;
        if std::env::var("FMI_GUIDE_PROFILE").is_ok() {
            eprintln!("Game::load and the start row: {:.3} s", t0.elapsed().as_secs_f64());
        }
        let g = fmi_explore::guide::build(&game, &goal, a.flag("wide"), &start, &mut || t0.elapsed().as_secs_f64())?;
        std::fs::write(out, fmi_explore::guide::dumps(&g)).map_err(|e| format!("{out}: {e}"))?;
        fmi_explore::guide::stats_text(&g, None)
    };
    match run() {
        Ok(t) => {
            print!("{t}");
            0
        }
        Err(e) => {
            eprintln!("fmi guide map: {e}");
            1
        }
    }
}

/// E014: the facts at the game's first idle stop from boot (seed `seed`, the lean switches), as the guide's start row.
fn start_of_boot(seed: i64) -> Result<std::collections::HashMap<String, i64>, String> {
    use fmi_core::{BootCfg, EngineHost, ObsReq};
    let host = fmi_engine::Host::from_env().map_err(|e| format!("the engine host: {e}"))?;
    let out = std::env::temp_dir().join(format!("fmi-guide-boot-{}", std::process::id()));
    let mut b = host
        .boot(&BootCfg { seed: seed as u32, events: vec![], frame: None, dump_locals: false, lean: true, out_dir: out.clone() })
        .map_err(|e| format!("the boot: {e}"))?;
    let o = b.obs(&ObsReq::default()).map_err(|e| format!("the boot: {e}"))?;
    b.quit();
    let _ = std::fs::remove_dir_all(&out);
    Ok(fmi_core::facts_of(&o).iter().map(|(k, v)| (k.text(), *v)).collect())
}

fn cmd_guide_stats(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &["key"]);
    let Some(p) = a.pos.first() else { usage() };
    let run = || -> Result<String, String> {
        let g = files::read_json(&PathBuf::from(p))?;
        if g.get("format").and_then(|f| f.as_str()) != Some(fmi_explore::guide::FORMAT) {
            return Err(format!("{p}: format {}, not {}", files::py_repr(g.get("format").unwrap_or(&serde_json::Value::Null)), fmi_explore::guide::FORMAT));
        }
        fmi_explore::guide::stats_text(&g, a.get("key").map(std::path::Path::new))
    };
    match run() {
        Ok(t) => {
            print!("{t}");
            0
        }
        Err(e) => {
            eprintln!("fmi guide stats: {e}");
            1
        }
    }
}



/// E012 try's G0: `fmi check tries A_SEQ B_SEQ [--json OUT]` (trycheck.rs).
fn cmd_check_tries(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &["json"]);
    if a.pos.len() != 2 {
        usage();
    }
    match trycheck::cmd_tries(&PathBuf::from(&a.pos[0]), &PathBuf::from(&a.pos[1]), a.get("json").map(std::path::Path::new)) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(e) => {
            eprintln!("fmi check tries: {e}");
            2
        }
    }
}

/// E012 try's G-on: `fmi check verify-new SEQ --out DIR [--threads N]` (trycheck.rs).
fn cmd_check_verify_new(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &["out", "threads"]);
    let (Some(seq), Some(out)) = (a.pos.first(), a.get("out")) else { usage() };
    let threads = a.get("threads").and_then(|t| t.parse().ok()).unwrap_or(2);
    match trycheck::cmd_verify_new(&PathBuf::from(seq), &PathBuf::from(out), threads) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(e) => {
            eprintln!("fmi check verify-new: {e}");
            2
        }
    }
}

/// E012 integration's measurement: `fmi check try-parts SEQ... [--json OUT]`, the wall by part (trycheck.rs).
fn cmd_check_try_parts(argv: &[String]) -> i32 {
    let mut seqs = Vec::new();
    let mut out = None;
    let mut i = 0;
    while i < argv.len() {
        if argv[i] == "--json" && i + 1 < argv.len() {
            out = Some(PathBuf::from(&argv[i + 1]));
            i += 2;
        } else {
            seqs.push(PathBuf::from(&argv[i]));
            i += 1;
        }
    }
    match trycheck::cmd_parts(&seqs, out.as_deref()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("fmi check try-parts: {e}");
            2
        }
    }
}

/// E012 try's measurements: `fmi check try-report SEQ...` (trycheck.rs).
fn cmd_check_try_report(argv: &[String]) -> i32 {
    let seqs: Vec<PathBuf> = argv.iter().map(PathBuf::from).collect();
    match trycheck::cmd_report(&seqs) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("fmi check try-report: {e}");
            2
        }
    }
}
