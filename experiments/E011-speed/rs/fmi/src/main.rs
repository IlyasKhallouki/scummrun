//! fmi: E011's one resident process (experiments/E011-speed/plan.md, "Architecture").
//!
//! usage:
//!   fmi explore run ARCHIVE --workers B [--step-cmd CMD | --fake] [--no-verify]
//!   fmi explore status ARCHIVE
//!   fmi fake-step run JOB_DIR          (fake_step.py, for the driver's tests)
//!   fmi check step FIXTURES.jsonl      (step.py's targets, reusable and idle_rule against the reference's outputs)
//!   fmi guide map --goal K=V[,K=V] | part1 --out GUIDE.json [--wide] [--seed 1]   (guide.py map)
//!   fmi guide stats GUIDE.json [--key KEY.json]                                     (guide.py stats)
//!
//! `explore run` is explore.py's `run`: the same archive files. With `--step-cmd` each job is a process as explore.py
//! runs it; with `--fake` each job is fake_step.py's port run inside this process.

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

const USAGE: &str = "usage: fmi explore run ARCHIVE --workers B [--step-cmd CMD | --fake] [--no-verify]
       fmi explore status ARCHIVE
       fmi fake-step run JOB_DIR
       fmi check step FIXTURES.jsonl
       fmi guide map --goal K=V[,K=V] | part1 --out GUIDE.json [--wide] [--seed 1]
       fmi guide stats GUIDE.json [--key KEY.json]";

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

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() {
        usage();
    }
    let code = match (argv[0].as_str(), argv.get(1).map(|s| s.as_str())) {
        ("explore", Some("run")) => cmd_run(&argv[2..]),
        ("explore", Some("status")) => cmd_status(&argv[2..]),
        ("fake-step", Some("run")) => cmd_fake(&argv[2..]),
        ("check", Some("step")) => cmd_check_step(&argv[2..]),
        ("guide", Some("map")) => cmd_guide_map(&argv[2..]),
        ("guide", Some("stats")) => cmd_guide_stats(&argv[2..]),
        _ => usage(),
    };
    std::process::exit(code)
}

fn cmd_run(argv: &[String]) -> i32 {
    let a = Args::parse(argv, &["workers", "step-cmd"]);
    let Some(root) = a.pos.first() else { usage() };
    let workers: usize = a.get("workers").and_then(|w| w.parse().ok()).unwrap_or(2);
    let mode = if let Some(cmd) = a.get("step-cmd") {
        StepMode::Command(shlex_split(cmd))
    } else if a.flag("fake") {
        StepMode::InProcess(Arc::new(FakeStep))
    } else {
        eprintln!("fmi explore run: the real try is wired at integration; give --fake or --step-cmd");
        return 2;
    };
    if !a.flag("no-verify") {
        eprintln!("fmi explore run: the verify needs the engine (integration); give --no-verify");
        return 2;
    }
    let mut stats = DriverStats::default();
    match driver::run(&PathBuf::from(root), workers, &mode, None, &mut stats) {
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

fn cmd_check_step(argv: &[String]) -> i32 {
    let Some(p) = argv.first() else { usage() };
    match fmi_explore::check::step_check(&PathBuf::from(p)) {
        Ok(r) => {
            for (k, n, bad) in &r.by_kind {
                let s = r.seconds.iter().find(|x| x.0 == *k).map(|x| x.1).unwrap_or(0.0);
                println!("{k}: {n} cases, {bad} differ; {:.3} ms a case in Rust", 1000.0 * s / (*n).max(1) as f64);
            }
            for b in &r.first_bad {
                println!("  {b}");
            }
            let bad: usize = r.by_kind.iter().map(|x| x.2).sum();
            println!("step check: {} cases, {bad} differ", r.cases);
            (bad > 0) as i32
        }
        Err(e) => {
            eprintln!("fmi check step: {e}");
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
        let start = fmi_explore::guide::start_row(&e003, seed)?;
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

#[allow(dead_code)]
fn no_cancel() -> AtomicBool {
    AtomicBool::new(false)
}
