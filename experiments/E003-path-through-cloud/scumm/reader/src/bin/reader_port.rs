//! The port's comparisons and timings (PORT-NOTES.md).
//!
//! reader-port static                  domains, scopes, hook summaries, the Builder's machine-free parts
//! reader-port records NAME            golden records parsed and written back
//! reader-port replay NAME [--out F]   the reading by the Rust Builder over the captured runs, against golden.py's
//! reader-port read NAME [--threads N] [--switches S,...] [--out F]
//!                                     the reading by the Rust machine half (fix round 1's switches on), against
//!                                     golden.py's
//! reader-port task --out DIR          the kitchen's packed files (build_task, prune) from the replayed reading
//! reader-port stream --out FILE       the kitchen reading with its records written to a packed file as made
//! reader-port bench-task --out DIR    build_task, relevance, prune and the packed files on the kitchen, timed
//! reader-port bench                   time and memory of domains.build and Scope.of

use scumm_reader::oracle::{self, Static};
use scumm_reader::{domains, packed, peak_rss_kb, task, Facts, Scope};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

/// mimalloc, as reader-scope: the reading on threads allocates and frees many small objects at once
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn report(what: &str, (diffs, n): (Vec<String>, usize)) -> bool {
    if diffs.is_empty() {
        println!("{what}: {n} compared, 0 differences");
        true
    } else {
        println!("{what}: {n} compared, {} differing; first:", diffs.len());
        for d in diffs.iter().take(5) {
            println!("  {d}");
        }
        false
    }
}

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).map(|i| args[i + 1].clone())
}

/// The pool's batches by size: wall seconds, the longest jobs' seconds, the jobs' seconds.
fn print_batches(stats: &scumm_reader::par::PoolStats) {
    println!(
        "  snapshots {:.2} s, workers taking them {:.2} s, merging and streaming {:.2} s",
        stats.snap_secs, stats.sync_secs, stats.merge_secs
    );
    let mut rows: Vec<(String, usize, f64, f64, f64)> = Vec::new();
    for (lo, hi) in [(0, 0), (1, 2), (3, 11), (12, 99), (100, 999), (1000, usize::MAX)] {
        let bs: Vec<_> = stats.per_batch.iter().filter(|b| b.0 >= lo && b.0 <= hi).collect();
        if bs.is_empty() {
            continue;
        }
        rows.push((
            format!("{lo}-{}", if hi == usize::MAX { "".to_string() } else { hi.to_string() }),
            bs.len(),
            bs.iter().map(|b| b.1).sum(),
            bs.iter().map(|b| b.2).sum(),
            bs.iter().map(|b| b.3).sum(),
        ));
    }
    for (what, n, wall, longest, sum) in rows {
        println!("  batches of {what} jobs: {n}, wall {wall:.2} s, longest jobs {longest:.2} s, jobs {sum:.2} s");
    }
    for (s, l, b) in stats.longest.iter().take(8) {
        println!("  longest: {s:.2} s {l} (batch {b})");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("");
    let t0 = Instant::now();
    let game = oracle::load_game();
    eprintln!("game loaded in {:.2} s", t0.elapsed().as_secs_f64());
    let st = Static::load();
    let mut ok = true;
    match cmd {
        "static" => {
            ok &= report("domains.build", oracle::compare_domains(&game, &st));
            ok &= report("Scope.of", oracle::compare_scopes(&game, &st));
            ok &= report("hook and entry writes", oracle::compare_hooks(&game, &st));
            for name in ["room33", "kitchen"] {
                ok &= report(&format!("builder {name}"), oracle::compare_builder(&game, &st, name));
            }
        }
        "records" => {
            let name = &args[1];
            let (recs, bad) = oracle::golden_records(&oracle::golden_dir().join(format!("{name}.jsonl.gz")));
            ok &= report(&format!("{name} records written back"), (bad, recs.len()));
        }
        "replay" => {
            let name = &args[1];
            let t = Instant::now();
            let (b, runs) = oracle::replay(&game, &st, name);
            let secs = t.elapsed().as_secs_f64();
            println!("{name}: {runs} runs replayed, {} records, {secs:.2} s", b.records.len());
            if let Some(out) = arg(&args, "--out") {
                let f = std::fs::File::create(&out).unwrap();
                let mut gz = flate2::write::GzEncoder::new(std::io::BufWriter::new(f), flate2::Compression::new(6));
                for l in b.reading_lines(&oracle::golden_info(&oracle::golden_dir().join(format!("{name}.jsonl.gz")))) {
                    gz.write_all(l.as_bytes()).unwrap();
                    gz.write_all(b"\n").unwrap();
                }
                gz.finish().unwrap();
            }
            ok &= report(
                &format!("{name} reading against golden.py's"),
                oracle::compare_reading(&b, &oracle::golden_dir().join(format!("{name}.jsonl.gz")), 5),
            );
        }
        "runs" => {
            // every captured machine run against the Rust machine half's
            let name = &args[1];
            let max: usize = arg(&args, "--max").map_or(20, |m| m.parse().unwrap());
            let t = Instant::now();
            let (diffs, runs, differing, secs) = oracle::compare_runs(&game, &st, name, max);
            println!("{name}: {runs} runs compared, {differing} differ; Rust machine half {secs:.2} s, all {:.2} s", t.elapsed().as_secs_f64());
            for d in &diffs {
                println!("  {d}");
            }
            ok &= differing == 0;
        }
        "dump-runs" => {
            // chosen captured runs made again by the Rust machine half, whole (reader_run_dump.py's lines)
            let name = &args[1];
            let want: Vec<usize> = arg(&args, "--runs").expect("--runs N,...").split(',').map(|n| n.parse().unwrap()).collect();
            let b = oracle::builder(&game, &st, name);
            let mut rd = scumm_reader::machine::Reader::new(&game);
            let mut out: Box<dyn Write> = match arg(&args, "--out") {
                Some(f) => Box::new(std::fs::File::create(f).unwrap()),
                None => Box::new(std::io::stdout()),
            };
            let cap = oracle::oracle_dir().join(format!("capture-{name}.jsonl.gz"));
            let last = *want.iter().max().unwrap();
            for (i, l) in scumm_reader::gz_lines(&cap).enumerate().take(last + 1) {
                if !want.contains(&i) {
                    continue;
                }
                let c = scumm_reader::Py::from_text(&l);
                let line = scumm_reader::machine::dump_capture_run(&mut rd, &b.ctx(), i, &c);
                writeln!(out, "{}", line.text()).unwrap();
            }
        }
        "read" => {
            // the reading with the Rust machine half, against golden.py's file
            let name = &args[1];
            let cpu0 = scumm_reader::cpu_secs();
            // fix round 1's switches (fix-round1.md); with any on, the golden comparison reports the differences
            let names: Vec<String> = arg(&args, "--switches").map_or(Vec::new(), |s| s.split(',').map(String::from).collect());
            let sw = scumm_machine::Switches::parse(&names).unwrap_or_else(|e| panic!("{e}"));
            let (b, secs) = match arg(&args, "--threads") {
                None => oracle::read_with(&game, &st, name, sw),
                Some(n) => {
                    let (b, secs, stats) = oracle::read_par_with(&game, &st, name, n.parse().unwrap(), sw);
                    println!(
                        "pool: {} batches, {} jobs, {:.1} job s, {} split parts made, {} of them on another worker",
                        stats.batches, stats.jobs, stats.job_secs, stats.parts_made, stats.parts_elsewhere
                    );
                    print_batches(&stats);
                    (b, secs)
                }
            };
            println!(
                "{name}: {} records, reading {secs:.2} s, {:.1} CPU s, peak RSS {} MB",
                b.records.len(),
                scumm_reader::cpu_secs() - cpu0,
                peak_rss_kb().unwrap_or(0) / 1024
            );
            let golden = oracle::golden_dir().join(format!("{name}.jsonl.gz"));
            if let Some(out) = arg(&args, "--out") {
                let f = std::fs::File::create(&out).unwrap();
                let mut gz = flate2::write::GzEncoder::new(std::io::BufWriter::new(f), flate2::Compression::new(6));
                for l in b.reading_lines(&oracle::golden_info(&golden)) {
                    gz.write_all(l.as_bytes()).unwrap();
                    gz.write_all(b"\n").unwrap();
                }
                gz.finish().unwrap();
            }
            ok &= report(&format!("{name} reading against golden.py's"), oracle::compare_reading(&b, &golden, 5));
        }
        "task" => {
            let out = PathBuf::from(arg(&args, "--out").expect("--out DIR"));
            std::fs::create_dir_all(&out).unwrap();
            let kt = oracle::kitchen_task(&st);
            let t = Instant::now();
            let (mut b, _) = oracle::replay(&game, &st, "kitchen");
            let t_read = t.elapsed().as_secs_f64();
            let recs = std::mem::take(&mut b.records);
            let keys = task::record_keys(&recs, &kt.goal);
            let s17 = task::task_start(&mut b, &kt.s17, &keys);
            let replan = task::replan_start(&mut b, &kt.replan, &Facts::new(), &keys).0;
            let starts = vec![("s17".to_string(), s17.clone()), ("replan".to_string(), replan.clone())];
            let t = Instant::now();
            let (info, folded) = packed::export_records(&out.join("kitchen.tc"), &mut b, &recs, &kt.goal, &starts).unwrap();
            println!("kitchen.tc: {} actions ({folded} folded), {} facts, {} bytes, {:.2} s", info.actions, info.facts, info.bytes, t.elapsed().as_secs_f64());
            let kept: Vec<_> = recs.iter().filter(|r| r.label != kt.no211).cloned().collect();
            let (info, folded) =
                packed::export_records(&out.join("kitchen-no211.tc"), &mut b, &kept, &kt.goal, &starts[..1]).unwrap();
            println!("kitchen-no211.tc: {} actions ({folded} folded), {} bytes", info.actions, info.bytes);
            for (which, rs) in [("all", &recs), ("no211", &kept)] {
                let t = Instant::now();
                let full = task::build_task(&mut b, rs, &kt.goal);
                let rel = task::relevant(&full);
                let small = task::prune(&full, &rel);
                let s = vec![
                    ("s17".to_string(), task::task_start(&mut b, &kt.s17, small.facts.iter().map(|f| &f.0))),
                    (
                        "replan".to_string(),
                        task::replan_start(&mut b, &kt.replan, &Facts::new(), small.facts.iter().map(|f| &f.0)).0,
                    ),
                ];
                let meta = |i: usize| (rs[i].kind.clone(), rs[i].end_text().to_string(), packed::cont(&rs[i]));
                let info = packed::export_task(&out.join(format!("kitchen-pruned-{which}.tc")), &small, &s, &meta).unwrap();
                println!(
                    "kitchen-pruned-{which}.tc: build_task {} actions ({} folded), pruned {} actions, {} bytes, {:.2} s",
                    full.actions.len(),
                    full.folded,
                    info.actions,
                    info.bytes,
                    t.elapsed().as_secs_f64()
                );
                if which == "all" {
                    let s = vec![("s17".to_string(), s17.clone()), ("replan".to_string(), replan.clone())];
                    let meta = |i: usize| (rs[i].kind.clone(), rs[i].end_text().to_string(), packed::cont(&rs[i]));
                    let info = packed::export_task(&out.join("kitchen-task.tc"), &full, &s, &meta).unwrap();
                    println!("kitchen-task.tc (export_task of build_task): {} actions, {} bytes", info.actions, info.bytes);
                }
            }
            println!("replayed reading {t_read:.2} s; peak RSS {} kB", peak_rss_kb().unwrap_or(0));
        }
        "stream" => {
            // the reading with its records written to the packed file as made (RecordStream), tables at the end
            let out = PathBuf::from(arg(&args, "--out").expect("--out FILE"));
            let kt = oracle::kitchen_task(&st);
            let mut b = oracle::builder(&game, &st, "kitchen");
            let mut m = scumm_reader::replay::Replay::open(&oracle::oracle_dir().join("capture-kitchen.jsonl.gz"));
            let mut stream = packed::RecordStream::create(&out).unwrap();
            b.read_all_into(&mut m, true, None, 2, Some(&mut stream));
            let keys = task::record_keys(&b.records, &kt.goal);
            let s17 = task::task_start(&mut b, &kt.s17, &keys);
            let at_write = stream.folded_when_written();
            let (info, folded) = stream.finish(&mut b, &kt.goal, &[("s17".to_string(), s17)]).unwrap();
            println!(
                "{}: {} actions ({folded} folded, {at_write} when written), {} args values of folded lists set to 0, {} bytes",
                out.display(),
                info.actions,
                info.zeroed,
                info.bytes
            );
        }
        "bench-task" => {
            // build_task and the packed files from the replayed kitchen reading, each step timed, its peak memory
            // measured from the reading's (VmHWM reset through /proc/self/clear_refs)
            let out = PathBuf::from(arg(&args, "--out").expect("--out DIR"));
            std::fs::create_dir_all(&out).unwrap();
            let kt = oracle::kitchen_task(&st);
            let (mut b, _) = oracle::replay(&game, &st, "kitchen");
            let recs = std::mem::take(&mut b.records);
            let rss = || -> u64 {
                let s = std::fs::read_to_string("/proc/self/status").unwrap();
                s.lines().find(|l| l.starts_with("VmRSS:")).unwrap().split_whitespace().nth(1).unwrap().parse().unwrap()
            };
            let reset = || std::fs::write("/proc/self/clear_refs", "5").expect("clear_refs");
            let base = rss();
            println!("after the reading: RSS {} MB (the Builder and its 72,256 records)", base / 1024);
            let step = |what: &str, t: Instant| {
                println!(
                    "{what}: {:.2} s, peak RSS {} MB ({} MB above the reading's)",
                    t.elapsed().as_secs_f64(),
                    peak_rss_kb().unwrap() / 1024,
                    peak_rss_kb().unwrap().saturating_sub(base) / 1024
                )
            };
            let keys = task::record_keys(&recs, &kt.goal);
            let s17 = task::task_start(&mut b, &kt.s17, &keys);
            let replan = task::replan_start(&mut b, &kt.replan, &Facts::new(), &keys).0;
            let starts = vec![("s17".to_string(), s17), ("replan".to_string(), replan)];
            reset();
            let t = Instant::now();
            packed::export_records(&out.join("kitchen.tc"), &mut b, &recs, &kt.goal, &starts).unwrap();
            step("export_records (build_task's rules, streamed, no Task) -> kitchen.tc", t);
            reset();
            let t = Instant::now();
            let full = task::build_task(&mut b, &recs, &kt.goal);
            step("build_task (a Task in memory)", t);
            let t = Instant::now();
            let meta = |i: usize| (recs[i].kind.clone(), recs[i].end_text().to_string(), packed::cont(&recs[i]));
            packed::export_task(&out.join("kitchen-task.tc"), &full, &starts, &meta).unwrap();
            step("export_task of it -> kitchen-task.tc", t);
            let t = Instant::now();
            let rel = task::relevant(&full);
            step("relevance", t);
            let t = Instant::now();
            let small = task::prune(&full, &rel);
            let s = vec![
                ("s17".to_string(), task::task_start(&mut b, &kt.s17, small.facts.iter().map(|f| &f.0))),
                ("replan".to_string(), task::replan_start(&mut b, &kt.replan, &Facts::new(), small.facts.iter().map(|f| &f.0)).0),
            ];
            packed::export_task(&out.join("kitchen-pruned-all.tc"), &small, &s, &meta).unwrap();
            step("prune + export_task -> kitchen-pruned-all.tc", t);
        }
        "bench" => {
            // the best of three runs each, as reader_port_oracle.py bench times Python
            let rooms = domains::meta_rooms(&oracle::root());
            let best = |f: &mut dyn FnMut()| {
                (0..3)
                    .map(|_| {
                        let t = Instant::now();
                        f();
                        t.elapsed().as_secs_f64()
                    })
                    .fold(f64::MAX, f64::min)
            };
            for l in st.of_kind("domains") {
                let start = Facts::from_py(l.get("start_items"));
                let goals: Vec<(String, Vec<i64>)> = if l.get("goals").is_none() {
                    vec![]
                } else {
                    l.get("goals").dict_items().iter().map(|(k, v)| (k.clone(), v.items().iter().map(|x| x.int()).collect())).collect()
                };
                let s = best(&mut || {
                    domains::build(&game, &rooms, &start, &goals);
                });
                println!("domains.build {}: {s:.4} s", l.get("name").as_str());
            }
            for l in st.of_kind("scope") {
                let rooms: Vec<i32> = l.get("rooms").items().iter().map(|r| r.int() as i32).collect();
                let start = Facts::from_py(l.get("start_items"));
                let s = best(&mut || {
                    Scope::of(&game, &rooms, &start);
                });
                println!("Scope.of {}: {s:.4} s", l.get("name").as_str());
            }
            let t = Instant::now();
            let _g = oracle::load_game();
            println!("Game::load: {:.3} s", t.elapsed().as_secs_f64());
            println!("peak RSS {} MB", peak_rss_kb().unwrap_or(0) / 1024);
        }
        _ => {
            eprintln!("usage: reader-port static | records NAME | replay NAME [--out F] | runs NAME [--max N] | dump-runs NAME --runs N,.. [--out F] | read NAME [--out F] | task --out DIR | stream --out F | bench-task --out DIR | bench");
            std::process::exit(2);
        }
    }
    eprintln!("total {:.2} s, peak RSS {} kB", t0.elapsed().as_secs_f64(), peak_rss_kb().unwrap_or(0));
    if !ok {
        std::process::exit(1);
    }
}
