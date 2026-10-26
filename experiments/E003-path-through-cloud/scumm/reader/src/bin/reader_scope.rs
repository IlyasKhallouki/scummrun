//! reader-scope: the reading of a scope at Part I's scale (size_scaling.py's start, goal and scopes), on N threads
//! (par.rs), its records streamed into taskcore's packed file as made (packed.rs `RecordStream`), measured. It
//! reads the game files, results/start-part1.jsonl and results/end-part1.json (with --part1 also
//! results/groundtruth-part1.jsonl) only, no oracle file.
//!
//! usage: reader-scope (--k K | --part1 | --rooms R,R,...) [--name NAME] [--threads N] [--out DIR] [--packed FILE]
//!                     [--cap-min M] [--max-rss-gb G] [--min-free-gb G] [--hash] [--keep] [--lines FILE]
//!
//! --part1       the Part I scope: the rooms of results/groundtruth-part1.jsonl's moves (scaling.rs `part1_rooms`)
//! --threads N   worker threads (0: the reading's own thread alone, as `machine::Reader` alone reads)
//! --out DIR     the report, DIR/NAME.json (and the packed file DIR/NAME.tc unless --packed)
//! --cap-min M   stop the reading after M minutes (default 60); --max-rss-gb G (default 8) and --min-free-gb G
//!               (MemAvailable, default 3) stop it too; the report then says where it stopped (exit code 3)
//! --max-waiting N  records done and waiting for an earlier job past which workers start no new job but that
//!               one (default 100,000: memory stays bounded when one job runs long)
//! --hash        sha1 of the records' lines (golden.py's "record" lines, as made, before `_read_once`'s rewrite of
//!               folded lists) and of the counters' line, to compare readings across thread counts
//! --keep        keep the records in memory (the Builder's `records`); --lines FILE writes golden.py's reader file
//!               (with --keep)
//! --start-seed N  the start: results/start-part1.jsonl's seed N (default 1)
//! --domain-goals part1|none
//!               the goal domains.build counts as compared constants: results/end-part1.json (default), or none
//!               (golden.read_scope's and replay_part1.py export's readings)
//! --goal part1|FACT=V,...
//!               the packed file's goal (default results/end-part1.json's)
//! --slot-locals FILE
//!               fix round 1, F10: slots live in the engine with the locals the bridge dumped, one JSON object per
//!               line ({"name", "room", "w", "esc", "locals", "override", "cut_data", "owned"}); their events are
//!               read with the start's (`Builder.slot_locals`). Without it the reading is as before.
//! --sample N --seed S --dump FILE
//!               the spot-check's jobs: of the last pass, N per stratum (the room a job's unit or object belongs
//!               to, or "global"), drawn by reservoir sampling with seed S; each written to FILE with what its batch
//!               started with (the `args:` values, the folded lists) and its records (golden.py's encoding), for
//!               reader_job_dump.py to make again in Python

use scumm_reader::builder::{Builder, RecordSink, SlotLocals};
use scumm_reader::domain::Label;
use scumm_reader::packed::RecordStream;
use scumm_reader::py::Py;
use scumm_reader::builder::Job;
use scumm_reader::record::{Record, Val, What};
use scumm_reader::table::FactTable;
use scumm_reader::{domains, par, peak_rss_kb, scaling, task, Scope};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// mimalloc: the workers allocate and free many small objects at once, and the sink thread frees what they made
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).map(|i| args[i + 1].clone())
}

use scumm_reader::cpu_secs;

fn status_kb(field: &str) -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    s.lines().find(|l| l.starts_with(field)).and_then(|l| l.split_whitespace().nth(1)).and_then(|x| x.parse().ok()).unwrap_or(0)
}

fn mem_available_kb() -> u64 {
    let s = std::fs::read_to_string("/proc/meminfo").unwrap();
    s.lines().find(|l| l.starts_with("MemAvailable:")).and_then(|l| l.split_whitespace().nth(1)).and_then(|x| x.parse().ok()).unwrap_or(0)
}

fn loadavg() -> String {
    std::fs::read_to_string("/proc/loadavg").unwrap_or_default().split_whitespace().take(3).collect::<Vec<_>>().join(" ")
}

/// What the reading has done so far (the progress callback and the sink write it; the watchdog reads it).
#[derive(Default)]
struct Prog {
    /// the highest RSS the watchdog saw (MB), and where the reading was then (the pass, the batch it was in)
    rss_peak_mb: u64,
    rss_peak_at: String,
    /// the batch running now
    current: String,
    records: usize,
    pass: usize,
    batches: Vec<(String, usize, usize, f64)>,
    jobs: usize,
}

/// The sink: the packed file, and optionally the records' lines hashed (and written).
struct Tee {
    busy: f64,
    stream: Option<RecordStream>,
    hash: Option<sha1_smol::Sha1>,
    kinds: BTreeMap<String, usize>,
    prog: Arc<Mutex<Prog>>,
}

impl RecordSink for Tee {
    fn batch(&mut self, first: usize, recs: &[Record], table: &FactTable) {
        let t = Instant::now();
        self.batch_timed(first, recs, table);
        self.busy += t.elapsed().as_secs_f64();
    }
    fn reset(&mut self) {
        if let Some(s) = &mut self.stream {
            s.reset().expect("the packed file");
        }
        if self.hash.is_some() {
            self.hash = Some(sha1_smol::Sha1::new());
        }
        self.kinds.clear();
        let mut p = self.prog.lock().unwrap();
        p.records = 0;
        p.pass += 1;
    }
}

impl Tee {
    fn batch_timed(&mut self, first: usize, recs: &[Record], table: &FactTable) {
        if let Some(s) = &mut self.stream {
            s.records(first, recs, table).expect("the packed file");
        }
        for (j, r) in recs.iter().enumerate() {
            if let Some(h) = &mut self.hash {
                let line = Py::dict(vec![("kind", Py::str("record")), ("index", Py::Int((first + j) as i64)), ("record", r.py())]).text();
                h.update(line.as_bytes());
                h.update(b"\n");
            }
            *self.kinds.entry(r.kind.clone()).or_default() += 1;
        }
        self.prog.lock().unwrap().records = first + recs.len();
    }
}

/// splitmix64
fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The spot-check's sample: per stratum a reservoir of jobs (the last pass's), each with its batch's start and
/// its records, as golden.py-encoded lines.
#[derive(Default)]
struct Sample {
    per: usize,
    seed: u64,
    pass: usize,
    /// stratum -> (jobs seen, picked: (the job's place in the pass, its line))
    strata: BTreeMap<String, (u64, Vec<(u64, String)>)>,
    seen: u64,
}

impl Sample {
    fn offer(&mut self, pass: usize, stratum: String, line: impl FnOnce(u64) -> String) {
        if pass != self.pass {
            self.pass = pass;
            self.strata.clear();
            self.seen = 0;
        }
        let g = self.seen;
        self.seen += 1;
        let h = stratum.bytes().fold(self.seed, |a, c| mix(a ^ c as u64));
        let (n, picked) = self.strata.entry(stratum).or_default();
        *n += 1;
        if picked.len() < self.per {
            picked.push((g, line(g)));
        } else {
            let j = (mix(h ^ *n) % *n) as usize;
            if j < self.per {
                picked[j] = (g, line(g));
            }
        }
    }
}

/// The room a job belongs to: its event's unit's room, its object's room, the room it is pinned to, or "global".
fn stratum(job: &Job, object_room: &rustc_hash::FxHashMap<i32, i32>) -> String {
    use scumm_ir::UnitKey;
    let room = match &job.what {
        What::Event(ev) => match ev.unit {
            UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => Some(r),
            UnitKey::Obj(o) => object_room.get(&o).copied(),
            UnitKey::Global(_) => return "global".into(),
        },
        What::Sentence { a, .. } => object_room.get(&(*a as i32)).copied(),
        What::Verb(_) | What::Input { .. } => match job.pins.get("room") {
            Some(Val::Int(r)) => Some(*r as i32),
            _ => None,
        },
    };
    room.map_or_else(|| "none".to_string(), |r| format!("room {r}"))
}

fn batches_json(batches: &[(String, usize, usize, f64)], per_batch: &[(usize, f64, f64, f64)]) -> Value {
    // the inputs and the first five rounds of each pass, the rest summed; with the pool's numbers when it ran them
    // (the longest job's seconds, the jobs' seconds summed)
    let mut out = Vec::new();
    let mut rest = (0usize, 0usize, 0usize, 0.0f64, 0.0f64, 0.0f64);
    let mut pass = 0;
    let r1 = |x: f64| (x * 10.0).round() / 10.0;
    for (i, (what, jobs, recs, secs)) in batches.iter().enumerate() {
        let (longest, job_s) = per_batch.get(i).map_or((0.0, 0.0), |p| (p.2, p.3));
        if what == "inputs" {
            if rest.0 > 0 {
                out.push(json!({"what": format!("pass {pass}: later rounds ({})", rest.0), "jobs": rest.1, "records": rest.2, "s": r1(rest.3), "longest_job_s": r1(rest.4), "job_s": r1(rest.5)}));
            }
            rest = (0, 0, 0, 0.0, 0.0, 0.0);
            pass += 1;
        }
        let round: usize = what.strip_prefix("event round ").and_then(|r| r.parse().ok()).unwrap_or(0);
        if round <= 5 {
            out.push(json!({"what": format!("pass {pass}: {what}"), "jobs": jobs, "records": recs, "s": r1(*secs), "longest_job_s": r1(longest), "job_s": r1(job_s)}));
        } else {
            rest.0 += 1;
            rest.1 += jobs;
            rest.2 += recs;
            rest.3 += secs;
            rest.4 = rest.4.max(longest);
            rest.5 += job_s;
        }
    }
    if rest.0 > 0 {
        out.push(json!({"what": format!("pass {pass}: later rounds ({})", rest.0), "jobs": rest.1, "records": rest.2, "s": r1(rest.3), "longest_job_s": r1(rest.4), "job_s": r1(rest.5)}));
    }
    Value::Array(out)
}

/// `--goal`: "part1" (results/end-part1.json) or "FACT=V,FACT=V".
fn goal_arg(text: &str, root: &Path) -> Vec<(String, Vec<i64>)> {
    if text == "part1" {
        return scaling::part1_goal(root);
    }
    text.split(',')
        .map(|p| {
            let (k, v) = p.split_once('=').expect("FACT=V");
            (k.trim().to_string(), vec![v.trim().parse().expect("an int goal value")])
        })
        .collect()
}

/// `--slot-locals`: one slot per line (module docstring).
fn slot_locals(path: &str) -> Vec<SlotLocals> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let ints = |v: &Value| -> Vec<i64> { v.as_array().expect("a list").iter().map(|x| x.as_i64().expect("an int")).collect() };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).expect("a JSON line");
            SlotLocals {
                name: v["name"].as_str().expect("name").to_string(),
                room: v["room"].as_i64().expect("room") as i32,
                w: v["w"].as_i64().expect("w"),
                esc: v["esc"].as_i64(),
                locals: ints(&v["locals"]),
                override_: v["override"].as_i64().unwrap_or(0),
                cut_data: v.get("cut_data").map_or(vec![], ints),
                owned: v["owned"].as_bool().unwrap_or(false),
            }
        })
        .collect()
}

fn write_json(path: &Path, v: &Value) {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(v).unwrap() + "\n").unwrap();
    std::fs::rename(&tmp, path).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let t_all = Instant::now();
    let root = scumm_ir::default_root();
    let game = scumm_ir::Game::load(&root).expect("the game");
    let start_seed: i64 = arg(&args, "--start-seed").map_or(scaling::SEED, |n| n.parse().unwrap());
    let start = scaling::start_facts(&root, start_seed);
    let goal = goal_arg(&arg(&args, "--goal").unwrap_or_else(|| "part1".into()), &root);
    let domain_goals = arg(&args, "--domain-goals").unwrap_or_else(|| "part1".into());
    let dom_goals = match domain_goals.as_str() {
        "part1" => scaling::part1_goal(&root),
        "none" => vec![],
        x => panic!("--domain-goals part1|none, not {x}"),
    };
    let slots = arg(&args, "--slot-locals").map(|p| slot_locals(&p)).unwrap_or_default();
    let k: Option<usize> = arg(&args, "--k").map(|k| k.parse().unwrap());
    let part1 = args.iter().any(|a| a == "--part1");
    let rooms: Vec<i32> = match (&k, arg(&args, "--rooms")) {
        (Some(k), _) => scaling::scopes(&game, start.get0("room") as i32)[*k].clone(),
        (None, Some(r)) => r.split(',').map(|x| x.parse().unwrap()).collect(),
        _ if part1 => scaling::part1_rooms(&game, &root),
        _ => panic!("--k K, --part1 or --rooms R,R,..."),
    };
    let name = arg(&args, "--name").unwrap_or_else(|| match k {
        Some(k) => format!("k{k}"),
        None if part1 => "part1".to_string(),
        None => "rooms".to_string(),
    });
    let threads: usize = arg(&args, "--threads").map_or(6, |n| n.parse().unwrap());
    let out = PathBuf::from(arg(&args, "--out").unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&out).unwrap();
    let packed = arg(&args, "--packed").map_or_else(|| out.join(format!("{name}.tc")), PathBuf::from);
    let cap_s = arg(&args, "--cap-min").map_or(60.0, |m| m.parse::<f64>().unwrap()) * 60.0;
    let max_rss_kb = (arg(&args, "--max-rss-gb").map_or(8.0, |g| g.parse::<f64>().unwrap()) * 1024.0 * 1024.0) as u64;
    let min_free_kb = (arg(&args, "--min-free-gb").map_or(3.0, |g| g.parse::<f64>().unwrap()) * 1024.0 * 1024.0) as u64;
    let hash = args.iter().any(|a| a == "--hash");
    let keep = args.iter().any(|a| a == "--keep");
    let report_path = out.join(format!("{name}.json"));
    let sample_n: usize = arg(&args, "--sample").map_or(0, |n| n.parse().unwrap());
    let dump = arg(&args, "--dump");

    let t_setup = Instant::now();
    let doms = domains::build(&game, &domains::meta_rooms(&root), &start, &dom_goals);
    let scope = Scope::of(&game, &rooms, &start);
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for &u in &scope.units {
        let kind = match game.unit(u).key {
            scumm_ir::UnitKey::Entry(_) => "entry",
            scumm_ir::UnitKey::Exit(_) => "exit",
            scumm_ir::UnitKey::Local(..) => "local",
            scumm_ir::UnitKey::Obj(_) => "obj",
            scumm_ir::UnitKey::Global(_) => "global",
        };
        *kinds.entry(kind).or_default() += 1;
    }
    let mut base = json!({
        "name": name,
        "k": k,
        "rooms": rooms,
        "n_rooms": scope.rooms.len(),
        "units": kinds,
        "n_units": scope.units.len(),
        "objects": scope.objects.len(),
        "slot_facts": scope.universe.len(),
        "start": format!("results/start-part1.jsonl seed {start_seed}"),
        "goal": goal.iter().map(|g| g.0.clone()).collect::<Vec<_>>(),
        "domain_goals": domain_goals,
        "slot_locals": slots.len(),
        "threads": threads,
        "cap_min": cap_s / 60.0,
        "load_at_start": loadavg(),
        "setup_s": 0.0,
    });
    base["setup_s"] = json!((t_setup.elapsed().as_secs_f64() * 100.0).round() / 100.0);
    eprintln!(
        "[{name}] {} rooms {:?}, {} units, {} slot facts; {threads} threads; load {}",
        scope.rooms.len(),
        rooms,
        scope.units.len(),
        scope.universe.len(),
        loadavg()
    );

    let mut b = Builder::new(&game, scope.clone(), doms);
    b.keep_records = keep;
    b.slot_locals = slots;
    let prog = Arc::new(Mutex::new(Prog { pass: 1, ..Default::default() }));
    {
        let prog = prog.clone();
        let t0 = Instant::now();
        let name = name.clone();
        b.progress = Some(Box::new(move |what: &str, jobs: usize, recs: usize, secs: f64| {
            let mut p = prog.lock().unwrap();
            p.batches.push((what.to_string(), jobs, recs, secs));
            p.jobs += jobs;
            let round: usize = what.strip_prefix("event round ").and_then(|r| r.parse().ok()).unwrap_or(0);
            if jobs >= 200 || secs >= 2.0 || round <= 3 || round % 100 == 0 {
                eprintln!(
                    "[{name}] pass {} {what}: {jobs} runs, {recs} records, {secs:.1} s; {} records in the pass, {:.0} s, RSS {} MB",
                    p.pass,
                    p.records,
                    t0.elapsed().as_secs_f64(),
                    status_kb("VmRSS:") / 1024
                );
            }
        }));
    }
    let sample = Arc::new(Mutex::new(Sample {
        per: sample_n,
        seed: arg(&args, "--seed").map_or(1, |n| n.parse().unwrap()),
        ..Default::default()
    }));
    if sample_n > 0 {
        let sample = sample.clone();
        let prog = prog.clone();
        let object_room = game.object_room.clone();
        b.on_job = Some(Box::new(move |job: &Job, recs: &[Record], args_vals, folded| {
            let pass = prog.lock().unwrap().pass;
            let st = stratum(job, &object_room);
            sample.lock().unwrap().offer(pass, st.clone(), |g| {
                Py::dict(vec![
                    ("kind", Py::str("job")),
                    ("pass", Py::Int(pass as i64)),
                    ("g", Py::Int(g as i64)),
                    ("stratum", Py::str(&st)),
                    ("job", job.py()),
                    (
                        "args_vals",
                        Py::Dict(args_vals.iter().map(|(k, v)| (k.clone(), Py::Set(v.iter().map(|&x| Py::Int(x)).collect()))).collect()),
                    ),
                    ("folded", Py::Set(folded.iter().map(|&x| Py::Int(x)).collect())),
                    ("records", Py::List(recs.iter().map(|r| r.py()).collect())),
                ])
                .text()
            });
        }));
    }
    {
        let prog = prog.clone();
        let name = name.clone();
        b.on_batch_start = Some(Box::new(move |what: &str, jobs: usize, events: usize| {
            let mut p = prog.lock().unwrap();
            p.current = format!("{what} ({jobs} jobs)");
            if jobs >= 1000 {
                eprintln!(
                    "[{name}] pass {} {what}: {jobs} runs from {events} events start; RSS {} MB",
                    p.pass,
                    status_kb("VmRSS:") / 1024
                );
            }
        }));
    }
    let mut tee = Tee {
        busy: 0.0,
        stream: Some(RecordStream::create(&packed).expect("the packed file")),
        hash: hash.then(sha1_smol::Sha1::new),
        kinds: BTreeMap::new(),
        prog: prog.clone(),
    };

    // the watchdog: the cap, the process's memory, the machine's free memory
    let t_read = Instant::now();
    let cpu0 = cpu_secs();
    {
        let prog = prog.clone();
        let base = base.clone();
        let report_path = report_path.clone();
        let packed = packed.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let wall = t_read.elapsed().as_secs_f64();
            let rss = status_kb("VmRSS:");
            let free = mem_available_kb();
            {
                let mut p = prog.lock().unwrap();
                if rss / 1024 > p.rss_peak_mb {
                    p.rss_peak_mb = rss / 1024;
                    p.rss_peak_at = format!("pass {}, {}, {:.0} s", p.pass, p.current, wall);
                }
            }
            let why = if wall > cap_s {
                Some(format!("the reading passed {:.0} minutes", cap_s / 60.0))
            } else if rss > max_rss_kb {
                Some(format!("the process's RSS passed {:.1} GB", max_rss_kb as f64 / 1048576.0))
            } else if free < min_free_kb {
                Some(format!("MemAvailable fell under {:.1} GB", min_free_kb as f64 / 1048576.0))
            } else {
                None
            };
            if let Some(why) = why {
                let p = prog.lock().unwrap();
                let cpu = cpu_secs() - cpu0;
                let mut r = base.clone();
                r["status"] = json!(format!("stopped: {why}"));
                r["read_wall_s"] = json!((wall * 10.0).round() / 10.0);
                r["read_cpu_s"] = json!((cpu * 10.0).round() / 10.0);
                r["cpu_use"] = json!((cpu / wall * 100.0).round() / 100.0);
                r["peak_rss_mb"] = json!(status_kb("VmHWM:") / 1024);
                r["rss_peak_seen"] = json!({"mb": p.rss_peak_mb, "at": p.rss_peak_at});
                r["pass"] = json!(p.pass);
                r["records_in_pass"] = json!(p.records);
                r["runs_done_in_finished_batches"] = json!(p.jobs);
                r["batches"] = batches_json(&p.batches, &[]);
                r["packed_bytes_so_far"] = json!(std::fs::metadata(&packed).map(|m| m.len()).unwrap_or(0));
                r["load_at_end"] = json!(loadavg());
                write_json(&report_path, &r);
                eprintln!("STOPPED: {why}; {} records in pass {}; report {}", p.records, p.pass, report_path.display());
                std::process::exit(3);
            }
        });
    }

    // the records are written on their own thread (par.rs `SinkThread`), unless they are kept or the reading has
    // no workers (one thread in all)
    let max_waiting: u64 = arg(&args, "--max-waiting").map_or(par::DEFAULT_MAX_WAITING, |n| n.parse().unwrap());
    let stats = if keep || threads == 0 {
        par::with_pool(&game, &scope, threads, |pool| {
            pool.set_max_waiting(max_waiting);
            b.read_all_into(pool, true, None, 2, Some(&mut tee));
            pool.stats.clone()
        })
    } else {
        let mut sink = par::SinkThread::new(tee, 256);
        let stats = par::with_pool(&game, &scope, threads, |pool| {
            pool.set_max_waiting(max_waiting);
            b.read_all_into(pool, true, None, 2, Some(&mut sink));
            pool.stats.clone()
        });
        tee = sink.finish();
        stats
    };
    let read_wall = t_read.elapsed().as_secs_f64();

    let read_cpu = cpu_secs() - cpu0;
    let read_rss = peak_rss_kb().unwrap_or(0);
    b.progress = None;
    eprintln!("[{name}] read: {} records, {read_wall:.1} s wall, {read_cpu:.1} CPU s, peak RSS {} MB", b.n_records, read_rss / 1024);

    // the packed file's tables and footer, with the start
    let t_fin = Instant::now();
    let stream = tee.stream.take().unwrap();
    let keys = stream.fact_keys();
    let goal_labels: Vec<(String, Vec<Label>)> = goal.iter().map(|(k, v)| (k.clone(), v.iter().map(|&x| Label::Int(x)).collect())).collect();
    let s0 = task::task_start(&mut b, &start, keys.iter().chain(goal.iter().map(|g| &g.0)));
    let (info, folded) = stream.finish(&mut b, &goal_labels, &[("start".to_string(), s0)]).expect("the packed file");
    let fin_s = t_fin.elapsed().as_secs_f64();
    let packed_sha1 = {
        let mut h = sha1_smol::Sha1::new();
        let mut f = std::fs::File::open(&packed).unwrap();
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let n = std::io::Read::read(&mut f, &mut buf).unwrap();
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        h.digest().to_string()
    };
    let counters = b.counters_py().text();
    let mut r = base;
    r["status"] = json!("done");
    r["records"] = json!(b.n_records);
    r["records_by_kind"] = json!(tee.kinds);
    let pass_counts: BTreeMap<String, i64> =
        b.counts.0.iter().filter(|(k, _)| k.starts_with("pass ") || k.as_str() == "event rounds" || k.as_str() == "records").map(|(k, v)| (k.clone(), *v)).collect();
    r["reader_counts"] = json!(pass_counts);
    r["out_of_domain_keys"] = json!(b.out_of_domain.len());
    r["read_wall_s"] = json!((read_wall * 10.0).round() / 10.0);
    r["read_cpu_s"] = json!((read_cpu * 10.0).round() / 10.0);
    r["cpu_use"] = json!((read_cpu / read_wall * 100.0).round() / 100.0);
    r["read_peak_rss_mb"] = json!(read_rss / 1024);
    r["packed"] = json!({
        "file": packed.display().to_string(),
        "bytes": info.bytes,
        "actions": info.actions,
        "facts": info.facts,
        "strings": info.strings,
        "folded": folded,
        "args_values_zeroed": info.zeroed,
        "finish_s": (fin_s * 100.0).round() / 100.0,
        "sha1": packed_sha1,
    });
    if let Some(h) = &tee.hash {
        r["records_sha1"] = json!(h.digest().to_string());
        r["counters_sha1"] = json!(sha1_smol::Sha1::from(counters.as_bytes()).digest().to_string());
    }
    r["pool"] = json!({
        "batches": stats.batches,
        "jobs": stats.jobs,
        "job_s": (stats.job_secs * 10.0).round() / 10.0,
        "split_parts_queued": stats.parts_made,
        "split_parts_run_elsewhere": stats.parts_elsewhere,
        "snapshots_s": (stats.snap_secs * 100.0).round() / 100.0,
        "workers_sync_s": (stats.sync_secs * 100.0).round() / 100.0,
        "merge_and_stream_s": (stats.merge_secs * 100.0).round() / 100.0,
        "merge_counters_s": (stats.merge_job_secs * 100.0).round() / 100.0,
        "stream_records_s": (stats.each_secs * 100.0).round() / 100.0,
        "max_records_waiting": stats.max_buffered,
        "longest_jobs": stats.longest.iter().map(|(s, l, b)| json!([(s * 100.0).round() / 100.0, l, b])).collect::<Vec<_>>(),
    });
    r["sink_thread_busy_s"] = json!((tee.busy * 10.0).round() / 10.0);
    r["phases_s"] = json!(b.phase_secs.iter().map(|(k, v)| (k.to_string(), (v * 100.0).round() / 100.0)).collect::<BTreeMap<_, _>>());
    {
        let p = prog.lock().unwrap();
        r["batches"] = batches_json(&p.batches, &stats.per_batch);
        r["rss_peak_seen"] = json!({"mb": p.rss_peak_mb, "at": p.rss_peak_at});
    }
    // F10: the lists the dumped slots' events were read with (the harness matches the slots against them after a
    // reading made again), each as packed_reading's list lines
    r["slot_events"] = json!(task::slot_lists(&b)
        .into_iter()
        .flat_map(|((name, w), ls)| ls.into_iter().map(move |l| (name.clone(), w, l)))
        .map(|(name, w, l)| json!({
            "name": name,
            "w": w,
            "sig": l.sig,
            "locals": l.locals.iter().map(|(n, v)| json!([n, v.int().expect("a dumped local is a number")])).collect::<Vec<_>>(),
            "override": l.override_,
            "cut_data": l.cut_data,
        }))
        .collect::<Vec<_>>());
    r["peak_rss_mb"] = json!(peak_rss_kb().unwrap_or(0) / 1024);
    r["total_s"] = json!((t_all.elapsed().as_secs_f64() * 10.0).round() / 10.0);
    r["load_at_end"] = json!(loadavg());
    write_json(&report_path, &r);
    if let Some(dump) = &dump {
        let smp = sample.lock().unwrap();
        let mut f = std::io::BufWriter::new(std::fs::File::create(dump).unwrap());
        let widened = Py::List(
            b.widened
                .iter()
                .map(|m| Py::Dict(m.iter().map(|(k, v)| (k.clone(), Py::Set(v.iter().map(|&x| Py::Int(x)).collect()))).collect()))
                .collect(),
        );
        let head = Py::dict(vec![
            ("kind", Py::str("head")),
            ("name", Py::str(&name)),
            ("rooms", Py::List(rooms.iter().map(|&r| Py::Int(r as i64)).collect())),
            ("seed", Py::Int(start_seed)),
            ("pass", Py::Int(smp.pass as i64)),
            ("widened", widened),
            ("per_stratum", Py::Int(smp.per as i64)),
            ("rng_seed", Py::Int(smp.seed as i64)),
            ("strata", Py::Dict(smp.strata.iter().map(|(k, v)| (k.clone(), Py::Int(v.0 as i64))).collect())),
        ]);
        writeln!(f, "{}", head.text()).unwrap();
        let mut picked: Vec<&(u64, String)> = smp.strata.values().flat_map(|v| v.1.iter()).collect();
        picked.sort_by_key(|p| p.0);
        for (_, line) in picked {
            writeln!(f, "{line}").unwrap();
        }
        eprintln!("[{name}] dumped {} jobs to {dump}", smp.strata.values().map(|v| v.1.len()).sum::<usize>());
    }
    if let Some(lines) = arg(&args, "--lines") {
        assert!(keep, "--lines needs --keep");
        let f = std::fs::File::create(&lines).unwrap();
        let mut gz = flate2::write::GzEncoder::new(std::io::BufWriter::new(f), flate2::Compression::new(6));
        for l in b.reading_lines(&[("name".to_string(), Py::str(&name))]) {
            gz.write_all(l.as_bytes()).unwrap();
            gz.write_all(b"\n").unwrap();
        }
        gz.finish().unwrap();
    }
    eprintln!(
        "[{name}] done: {} records, read {read_wall:.1} s wall {read_cpu:.1} CPU s ({:.2} CPUs), peak RSS {} MB; packed {} bytes, {} actions ({folded} folded); report {}",
        b.n_records,
        read_cpu / read_wall,
        read_rss / 1024,
        info.bytes,
        info.actions,
        report_path.display()
    );
}
