//! fmi-reading: the readio component's checks and measures (experiments/E011-speed/plan.md, G8 and the targets).
//!
//! usage: fmi-reading dump DIR [--every N] [--out FILE]   action(i) and record(i) as canonical JSON lines, every Nth
//!                                                         action (ref/dump_reading.py writes the Python reference's)
//!        fmi-reading names DIR [--out FILE]              per action its name and by_name(name), then names no action
//!                                                         has with by_name's answer
//!        fmi-reading reuse-keys DIR                      E009's K of the reading (reuse_keys_of on its scope and
//!                                                         start), as JSON, with the seconds
//!        fmi-reading reusable DIR ROWS [--e009]          step.py's reusable and reusable_detail of the reading for
//!                                                         the start row of seed 1 in ROWS (with E009's K: --e009)
//!        fmi-reading idx DIR                             reading.idx built from the files, against the one there
//!        fmi-reading bench DIR [--every N] [--reps R]    open, action, record, by_name: seconds and microseconds
//!        fmi-reading key ROWS ROOMS GOAL [--reader BIN] [--fact-vars N,..] [--reach-also F ...]
//!                                                         the cache key of E009's reading of that input
//!        fmi-reading cache ROWS ROOMS GOAL DIR --reader BIN [--fact-vars ..] [--reach-also ..] [--threads N]
//!                                                         the reading through the cache under DIR (a miss runs the
//!                                                         reader), with the seconds of the lookup and the open

use fmi_core::{val_text, EffVal, ReadingSource, RecEff, Val, ABOVE, BELOW};
use fmi_reading::cache::{file_sha1, game_index, ReadSpec, ReadingCache};
use fmi_reading::pyjson::PJ;
use fmi_reading::reuse::ReuseGame;
use fmi_reading::{idx, Reading};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// CPU seconds of this thread (CLOCK_THREAD_CPUTIME_ID): the measures, less the machine's load.
fn cpu() -> f64 {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: clock_gettime fills the timespec it is given
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    t.tv_sec as f64 + t.tv_nsec as f64 / 1e9
}

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).map(|i| args[i + 1].clone())
}

fn val(v: Val) -> PJ {
    match v {
        BELOW | ABOVE => PJ::str(val_text(v)),
        fmi_core::NONE => PJ::str("None"),
        n => PJ::Int(n),
    }
}

fn vals(v: &[Val]) -> PJ {
    let mut v = v.to_vec();
    v.sort_unstable();
    PJ::List(v.into_iter().map(val).collect())
}

fn sets(s: &[(fmi_core::Key, Vec<Val>)]) -> PJ {
    PJ::List(s.iter().map(|(k, v)| PJ::List(vec![PJ::str(k.text()), vals(v)])).collect())
}

/// The canonical JSON of action i and its record (ref/dump_reading.py writes the same for PackedTask._action).
fn canonical(rd: &Reading, i: u32) -> PJ {
    let a = rd.action(i);
    let r = rd.record(i);
    let effects = a
        .effects
        .iter()
        .map(|e| {
            let (kind, p) = match &e.v {
                EffVal::Value(v) => ("value", val(*v)),
                EffVal::Values(vs) => ("values", vals(vs)),
                EffVal::Add(d) => ("add", PJ::Int(*d)),
            };
            PJ::List(vec![PJ::str(e.key.text()), PJ::str(kind), p, sets(&e.when)])
        })
        .collect();
    let reffects = r
        .effects
        .iter()
        .map(|(k, e)| {
            let mut x = vec![PJ::str(k.text())];
            match e {
                RecEff::Set(v) => x.extend([PJ::str("set"), val(*v)]),
                RecEff::Guess(vs) => x.extend([PJ::str("guess"), vals(vs)]),
                RecEff::Add(d) => x.extend([PJ::str("add"), PJ::Int(*d)]),
                RecEff::Copy(k2) => x.extend([PJ::str("copy"), PJ::str(k2.text())]),
                RecEff::When(k2, v) => x.extend([PJ::str("when"), PJ::str(k2.text()), val(*v)]),
            }
            PJ::List(x)
        })
        .collect();
    let mut ep: Vec<String> = r.engine_pre.iter().map(|k| k.text()).collect();
    ep.sort();
    PJ::obj(vec![
        ("i", PJ::Int(i as i64)),
        ("name", PJ::str(a.name.clone())),
        ("cost", PJ::Int(a.cost)),
        ("what", PJ::Int(a.what as i64)),
        ("pre", sets(&a.pre)),
        ("effects", PJ::List(effects)),
        (
            "record",
            PJ::obj(vec![
                ("kind", PJ::str(r.kind.clone())),
                ("label", PJ::str(r.label.clone())),
                ("pre", sets(&r.pre)),
                ("effects", PJ::List(reffects)),
                ("checks", PJ::List(r.checks.iter().map(|c| PJ::str(c.clone())).collect())),
                ("end", PJ::str(r.end.clone())),
                ("source", PJ::str(r.source.clone())),
                ("engine_pre", PJ::List(ep.into_iter().map(PJ::str).collect())),
            ]),
        ),
    ])
}

fn out_file(args: &[String]) -> Box<dyn Write> {
    match arg(args, "--out") {
        Some(p) => Box::new(std::io::BufWriter::new(std::fs::File::create(p).expect("the output file"))),
        None => Box::new(std::io::BufWriter::new(std::io::stdout())),
    }
}

/// Names no action has: each action's name with a character changed, a rec past the last, and a few others.
fn absent_names(rd: &Reading) -> Vec<String> {
    let n = rd.n_actions();
    let mut out = vec![String::new(), "#".into(), "no such label#0".into()];
    for i in (0..n).step_by((n as usize / 50).max(1)) {
        let name = rd.action(i).name;
        out.push(format!("{name}x"));
        out.push(format!("{name}#{}", u32::MAX));
        if let Some((l, _)) = name.rsplit_once('#') {
            out.push(format!("{l}#{}", 4_000_000_000u32));
            out.push(l.to_string());
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("");
    let every: u32 = arg(&args, "--every").map_or(1, |n| n.parse().unwrap());
    match cmd {
        "dump" => {
            let rd = Reading::open(Path::new(&args[1])).expect("the reading");
            let mut f = out_file(&args);
            for i in (0..rd.n_actions()).step_by(every as usize) {
                writeln!(f, "{}", canonical(&rd, i).compact_text()).unwrap();
            }
        }
        "names" => {
            let rd = Reading::open(Path::new(&args[1])).expect("the reading");
            let mut f = out_file(&args);
            for i in 0..rd.n_actions() {
                let name = rd.action(i).name;
                let b = rd.by_name(&name).map_or(PJ::Null, |x| PJ::Int(x as i64));
                writeln!(f, "{}", PJ::List(vec![PJ::str(name), b]).compact_text()).unwrap();
            }
            for name in absent_names(&rd) {
                let b = rd.by_name(&name).map_or(PJ::Null, |x| PJ::Int(x as i64));
                writeln!(f, "{}", PJ::List(vec![PJ::str(name), b]).compact_text()).unwrap();
            }
        }
        "reuse-keys" => {
            let t = Instant::now();
            let g = ReuseGame::load(&scumm_ir::default_root()).expect("the game");
            let load_s = t.elapsed().as_secs_f64();
            let rd = Reading::open(Path::new(&args[1])).expect("the reading");
            let rooms: Vec<i32> = rd.doc()["scope"]["rooms"].as_array().unwrap().iter().map(|r| r.as_i64().unwrap() as i32).collect();
            let t = Instant::now();
            let k = g.keys_of(&rooms, rd.start());
            let s = t.elapsed().as_secs_f64();
            let out = PJ::obj(vec![
                ("keys", PJ::List(k.keys.iter().map(|x| PJ::str(x.clone())).collect())),
                ("named", PJ::ints(k.named.iter().map(|&n| n as i64))),
                ("guards", PJ::ints(k.guards.iter().map(|&n| n as i64))),
                ("game_load_s", PJ::Float((load_s * 1000.0).round() / 1000.0)),
                ("keys_of_s", PJ::Float((s * 1000.0).round() / 1000.0)),
            ]);
            println!("{}", out.compact_text());
        }
        "reusable" => {
            let rd = Reading::open(Path::new(&args[1])).expect("the reading");
            let text = std::fs::read_to_string(&args[2]).expect("the rows");
            let row: serde_json::Value =
                text.lines().map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()).find(|v| v["seed"] == 1).expect("seed 1");
            let entry = fmi_core::Facts::from_pairs(
                row["facts"].as_object().unwrap().iter().map(|(k, v)| (fmi_core::Key::parse(k), v.as_i64().unwrap())).collect(),
            );
            let t = Instant::now();
            let extra: std::collections::BTreeSet<fmi_core::Key> = if args.iter().any(|a| a == "--e009") {
                let g = ReuseGame::load(&scumm_ir::default_root()).expect("the game");
                g.reuse_keys(&rd).expect("reuse keys").iter().map(|k| fmi_core::Key::parse(k)).collect()
            } else {
                Default::default()
            };
            let keys = fmi_reading::reuse::task_keys(&rd, &extra);
            let (ok, n) = fmi_reading::reuse::reusable(rd.start(), &keys, &entry);
            let d = fmi_reading::reuse::reusable_detail(rd.start(), &keys, &entry);
            let s = t.elapsed().as_secs_f64();
            println!(
                "{}",
                PJ::obj(vec![
                    ("ok", PJ::Bool(ok)),
                    ("n", PJ::Int(n as i64)),
                    ("differ", PJ::Int(d.differ as i64)),
                    ("task", PJ::Int(d.task as i64)),
                    ("slots", PJ::Int(d.slots as i64)),
                    ("other", PJ::Int(d.other as i64)),
                    ("first", PJ::List(d.first.iter().map(|(k, a, b)| PJ::List(vec![PJ::str(k.clone()), val(*a), val(*b)])).collect())),
                    ("reuse_keys", PJ::Int(extra.len() as i64)),
                    ("s", PJ::Float((s * 1000.0).round() / 1000.0)),
                ])
                .compact_text()
            );
        }
        "idx" => {
            let dir = PathBuf::from(&args[1]);
            let there = std::fs::read(dir.join("reading.idx")).expect("reading.idx");
            let t = Instant::now();
            let packed = fmi_reading::tc::Packed::open(&dir.join("reading.tc")).unwrap();
            let a = std::fs::read(dir.join("reading.actions.jsonl")).unwrap();
            let l = std::fs::read(dir.join("reading.lists.jsonl")).unwrap();
            let built = idx::build(&packed, &a, &l).unwrap();
            println!(
                "{}",
                PJ::obj(vec![
                    ("same", PJ::Bool(built == there)),
                    ("bytes", PJ::Int(built.len() as i64)),
                    ("build_s", PJ::Float((t.elapsed().as_secs_f64() * 1000.0).round() / 1000.0)),
                ])
                .compact_text()
            );
        }
        "bench" => {
            let dir = PathBuf::from(&args[1]);
            let reps: usize = arg(&args, "--reps").map_or(5, |n| n.parse().unwrap());
            let mut opens = Vec::new();
            let mut opens_cpu = Vec::new();
            for _ in 0..reps {
                let (t, c) = (Instant::now(), cpu());
                let rd = Reading::open(&dir).expect("the reading");
                opens.push(t.elapsed().as_secs_f64());
                opens_cpu.push(cpu() - c);
                drop(rd);
            }
            let rd = Reading::open(&dir).unwrap();
            let idxs: Vec<u32> = (0..rd.n_actions()).step_by(every as usize).collect();
            let names: Vec<String> = idxs.iter().map(|&i| rd.action(i).name).collect();
            // (wall us, CPU us) per call, over the sample
            let time = |f: &dyn Fn(u32)| {
                let (t, c) = (Instant::now(), cpu());
                for &i in &idxs {
                    f(i);
                }
                let n = idxs.len() as f64;
                (t.elapsed().as_secs_f64() / n * 1e6, (cpu() - c) / n * 1e6)
            };
            let action_us = time(&|i| {
                std::hint::black_box(rd.action(i));
            });
            let record_us = time(&|i| {
                std::hint::black_box(rd.record(i));
            });
            let line_us = time(&|i| {
                std::hint::black_box(rd.action_line(i));
            });
            let raw_us = time(&|i| {
                std::hint::black_box(rd.tc().raw(rd.index().tc_word(i)));
            });
            let parse_us = time(&|i| {
                std::hint::black_box(fmi_reading::line::parse(rd.action_line(i)).unwrap());
            });
            let (t, c) = (Instant::now(), cpu());
            for n in &names {
                std::hint::black_box(rd.by_name(n));
            }
            let nn = names.len() as f64;
            let by_name_us = (t.elapsed().as_secs_f64() / nn * 1e6, (cpu() - c) / nn * 1e6);
            let mut action_max = 0f64;
            for &i in &idxs {
                let t = Instant::now();
                std::hint::black_box(rd.action(i));
                action_max = action_max.max(t.elapsed().as_secs_f64() * 1e6);
            }
            opens.sort_by(|a, b| a.partial_cmp(b).unwrap());
            opens_cpu.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let r3 = |x: f64| PJ::Float((x * 1000.0).round() / 1000.0);
            let pair = |x: (f64, f64)| PJ::List(vec![r3(x.0), r3(x.1)]);
            println!(
                "{}",
                PJ::obj(vec![
                    ("actions", PJ::Int(rd.n_actions() as i64)),
                    ("sampled", PJ::Int(idxs.len() as i64)),
                    ("open_ms", PJ::List(opens.iter().map(|x| r3(x * 1000.0)).collect())),
                    ("open_cpu_ms", PJ::List(opens_cpu.iter().map(|x| r3(x * 1000.0)).collect())),
                    ("action_us_wall_cpu", pair(action_us)),
                    ("action_us_max_wall", r3(action_max)),
                    ("record_us_wall_cpu", pair(record_us)),
                    ("action_line_us_wall_cpu", pair(line_us)),
                    ("raw_us_wall_cpu", pair(raw_us)),
                    ("line_parse_us_wall_cpu", pair(parse_us)),
                    ("by_name_us_wall_cpu", pair(by_name_us)),
                ])
                .compact_text()
            );
        }
        "key" | "cache" => {
            let (rows, rooms, goal) = (PathBuf::from(&args[1]), &args[2], &args[3]);
            let rooms: Vec<i32> = rooms.split(',').map(|r| r.parse().unwrap()).collect();
            let mut spec = ReadSpec::e009(&rooms, goal, &rows);
            spec.fact_vars = arg(&args, "--fact-vars").map_or(vec![], |s| s.split(',').map(|n| n.parse().unwrap()).collect());
            spec.reach_also = args.windows(2).filter(|w| w[0] == "--reach-also").map(|w| PathBuf::from(&w[1])).collect();
            let root = scumm_ir::default_root();
            let reader = PathBuf::from(arg(&args, "--reader").expect("--reader BIN"));
            let t = Instant::now();
            let rsha = file_sha1(&reader).unwrap();
            let gsha = file_sha1(&game_index(&root)).unwrap();
            let sha_s = t.elapsed().as_secs_f64();
            let t = Instant::now();
            let key = spec.key(&rsha, &gsha, &root).unwrap();
            let key_s = t.elapsed().as_secs_f64();
            if cmd == "key" {
                println!("{key}");
                return;
            }
            let cache = ReadingCache::new(Path::new(&args[4])).unwrap();
            let threads: usize = arg(&args, "--threads").map_or(3, |n| n.parse().unwrap());
            let t = Instant::now();
            let (rd, hit) = cache
                .read(&spec, &key, &reader, "reading-cache", threads, &mut |mut c| {
                    let st = c.stderr(std::process::Stdio::null()).status();
                    st
                })
                .expect("the reading");
            let read_s = t.elapsed().as_secs_f64();
            let r3 = |x: f64| PJ::Float((x * 1000.0).round() / 1000.0);
            println!(
                "{}",
                PJ::obj(vec![
                    ("key", PJ::str(key)),
                    ("hit", PJ::Bool(hit)),
                    ("dir", PJ::str(rd.dir().display().to_string())),
                    ("actions", PJ::Int(rd.n_actions() as i64)),
                    ("shas_ms", r3(sha_s * 1000.0)),
                    ("key_ms", r3(key_s * 1000.0)),
                    ("read_ms", r3(read_s * 1000.0)),
                ])
                .compact_text()
            );
        }
        _ => {
            eprintln!("usage: fmi-reading dump|names|reuse-keys|idx|bench|key|cache ... (the module docstring)");
            std::process::exit(2);
        }
    }
}
