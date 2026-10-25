//! machine-run: golden.py's machine goldens for one run, written by the Rust machine as a candidate folder
//! that `golden.py compare` reads (PORT-NOTES.md).
//!
//! usage: machine-run RUN --out DIR [--only S1#4,S2#7] [--jobs 4] [--no-trace] [--no-prep] [--no-check] [--repeat N]
//!
//! 1. `candidate.py prep` (Python, nice 10) writes DIR/machine/RUN/jobs.json: check.py's header of every
//!    counted move (its label, move line and action) and the run's key, features and fact families.
//! 2. The machine scores every counted move with an action, as check.py's `run_model` dispatches it, and
//!    writes DIR/machine/RUN/<label>.jsonl.gz (move, run, path and predict lines) and its trace.
//! 3. `candidate.py check` (Python, nice 10) adds check.py's own line: score_move on the Rust run.
//!
//! Then: `../../.venv/bin/python golden.py compare DIR --runs RUN [--traces]` from the experiment folder.
//! DIR/machine/RUN/rust-index.json keeps the Rust times per move, the paths, the events, and peak memory.
//! `--repeat N` runs each move's machine N times (untraced but the last), for timing and profiling; the time kept
//! is the mean.

use flate2::{Compression, GzBuilder};
use scumm_machine::harness::{load_pres, move_lines, run_model, Job, Jobs};
use scumm_machine::trace::Tracer;
use scumm_machine::{FactOracle, Features, Machine, Switches};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

struct Opts {
    run: String,
    out: PathBuf,
    only: Option<String>,
    jobs: usize,
    trace: bool,
    prep: bool,
    check: bool,
    repeat: usize,
}

fn usage() -> ! {
    eprintln!("usage: machine-run RUN --out DIR [--only S1#4,...] [--jobs N] [--no-trace] [--no-prep] [--no-check] [--repeat N]");
    std::process::exit(2)
}

fn opts() -> Opts {
    let mut it = std::env::args().skip(1);
    let mut o =
        Opts { run: String::new(), out: PathBuf::new(), only: None, jobs: 4, trace: true, prep: true, check: true, repeat: 1 };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => o.out = PathBuf::from(it.next().unwrap_or_else(|| usage())),
            "--only" => o.only = Some(it.next().unwrap_or_else(|| usage())),
            "--jobs" => o.jobs = it.next().and_then(|n| n.parse().ok()).unwrap_or_else(|| usage()),
            "--no-trace" => o.trace = false,
            "--no-prep" => o.prep = false,
            "--no-check" => o.check = false,
            "--repeat" => o.repeat = it.next().and_then(|n| n.parse().ok()).filter(|&n| n > 0).unwrap_or_else(|| usage()),
            s if s.starts_with("--") => usage(),
            s => o.run = s.to_string(),
        }
    }
    if o.run.is_empty() || o.out.as_os_str().is_empty() {
        usage();
    }
    o
}

fn python(root: &Path, o: &Opts, cmd: &str) {
    let py = root.join(".venv/bin/python");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("candidate.py");
    let mut c = Command::new("nice");
    c.args(["-n", "10"]).arg(&py).arg(&script).args([cmd, &o.run, "--out"]).arg(&o.out);
    if let Some(only) = &o.only {
        c.args(["--only", only]);
    }
    let t = Instant::now();
    let st = c.status().expect("run candidate.py");
    if !st.success() {
        eprintln!("candidate.py {cmd} failed: {st}");
        std::process::exit(1);
    }
    eprintln!("[{}] candidate.py {cmd}: {:.1} s", o.run, t.elapsed().as_secs_f64());
}

/// Writes `path` as golden.py's GzWriter does (gzip, mtime 0, no name, level 6), through a temporary file.
fn write_gz(path: &Path, lines: &[String]) -> std::io::Result<()> {
    let tmp = path.with_extension("gz.tmp");
    let f = std::fs::File::create(&tmp)?;
    let mut gz = GzBuilder::new().mtime(0).write(BufWriter::new(f), Compression::new(6));
    for l in lines {
        gz.write_all(l.as_bytes())?;
    }
    gz.finish()?.flush()?;
    std::fs::rename(tmp, path)
}

struct Stat {
    label: String,
    machine_s: f64,
    total_s: f64,
    paths: usize,
    events: u64,
}

#[allow(clippy::too_many_arguments)]
fn score(
    m: &mut Machine,
    job: &Job,
    pres: &rustc_hash::FxHashMap<i64, scumm_ir::FactMap>,
    kinds: &[String],
    folder: &Path,
    trace: bool,
    repeat: usize,
) -> std::io::Result<Stat> {
    let t0 = Instant::now();
    let (mut machine_s, mut paths, mut events) = (0.0, 0, 0);
    let lines = match &job.action {
        None => move_lines(m.game, job, None),
        Some(act) => {
            let pre = pres.get(&job.i).unwrap_or_else(|| panic!("no ground-truth row {}", job.i)).clone();
            let o = FactOracle::new(pre, Some(kinds.to_vec()));
            let tpath = folder.join(format!("{}.trace.jsonl.gz", job.label));
            let ttmp = tpath.with_extension("gz.tmp");
            if trace {
                let f = std::fs::File::create(&ttmp)?;
                let gz = GzBuilder::new().mtime(0).write(BufWriter::new(f), Compression::new(6));
                m.trace = Some(Tracer::new(Box::new(gz)));
            }
            let t = Instant::now();
            if repeat > 1 {
                let tr = m.trace.take();
                for _ in 1..repeat {
                    drop(run_model(m, &o, act));
                }
                m.trace = tr;
            }
            let run = run_model(m, &o, act);
            machine_s = t.elapsed().as_secs_f64() / repeat as f64;
            if let Some(tr) = m.trace.take() {
                events = tr.finish()?;
                std::fs::rename(&ttmp, &tpath)?;
            }
            paths = run.paths.len();
            move_lines(m.game, job, Some((&run, &o)))
        }
    };
    write_gz(&folder.join(format!("{}.jsonl.gz", job.label)), &lines)?;
    Ok(Stat { label: job.label.clone(), machine_s, total_s: t0.elapsed().as_secs_f64(), paths, events })
}

fn main() {
    let o = opts();
    let root = scumm_ir::default_root();
    let folder = o.out.join("machine").join(&o.run);
    if o.prep {
        python(&root, &o, "prep");
    }
    let t0 = Instant::now();
    let jobs = Jobs::read(&folder.join("jobs.json")).expect("jobs.json (candidate.py prep)");
    let features = Features::parse(&jobs.features).unwrap_or_else(|e| {
        eprintln!("{}: {e}", o.run);
        std::process::exit(1)
    });
    let switches = Switches::parse(&jobs.switches).expect("switches");
    let tg = Instant::now();
    let game = scumm_ir::Game::load(&root).expect("the game");
    let game_s = tg.elapsed().as_secs_f64();
    let pres = load_pres(Path::new(&jobs.groundtruth)).expect("the ground truth");
    let next = AtomicUsize::new(0);
    let stats = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..o.jobs.max(1) {
            sc.spawn(|| {
                let mut m = Machine::new(&game, features, switches);
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.moves.get(k) else { break };
                    let st = score(&mut m, job, &pres, &jobs.kinds, &folder, o.trace, o.repeat).unwrap_or_else(|e| {
                        panic!("{} {}: {e}", o.run, job.label);
                    });
                    stats.lock().unwrap().push((k, st));
                }
            });
        }
    });
    let mut stats = stats.into_inner().unwrap();
    stats.sort_by_key(|s| s.0);
    let wall = t0.elapsed().as_secs_f64();
    let machine: f64 = stats.iter().map(|s| s.1.machine_s).sum();
    let total: f64 = stats.iter().map(|s| s.1.total_s).sum();
    let rss = scumm_ir::peak_rss_kb().unwrap_or(0);
    let mut idx = String::from("{\n");
    idx += &format!(
        " \"run\": \"{}\", \"jobs\": {}, \"moves\": {}, \"game_load_s\": {game_s:.3}, \"wall_s\": {wall:.3},\n \"machine_s\": {machine:.6}, \"total_s\": {total:.6}, \"peak_rss_kb\": {rss},\n \"per_move\": [\n",
        o.run,
        o.jobs,
        stats.len()
    );
    for (n, (_, s)) in stats.iter().enumerate() {
        idx += &format!(
            "  {{\"label\": \"{}\", \"machine_s\": {:.6}, \"total_s\": {:.6}, \"paths\": {}, \"events\": {}}}{}\n",
            s.label,
            s.machine_s,
            s.total_s,
            s.paths,
            s.events,
            if n + 1 < stats.len() { "," } else { "" }
        );
    }
    idx += " ]\n}\n";
    std::fs::write(folder.join("rust-index.json"), idx).expect("rust-index.json");
    eprintln!(
        "[{}] {} moves: machine {machine:.3} s, with encoding and writing {total:.3} s, wall {wall:.1} s \
         (game load {game_s:.2} s); peak RSS {:.0} MB",
        o.run,
        stats.len(),
        rss as f64 / 1024.0
    );
    if o.check {
        python(&root, &o, "check");
    }
}
