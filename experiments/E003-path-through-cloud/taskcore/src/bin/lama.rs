//! E011 route: the Rust lama-first-costs on a SAS file, and its checks (experiments/E011-speed/route/).
//!
//! usage:
//!   lama search --sas FILE [--plan FILE] [--limit S] [--trace]   the search (lama.rs) on a SAS file: the plan file and
//!                                                                 one JSON line of counts and seconds
//!   lama graph --sas FILE                                         the landmark graph as Fast Downward's debug dump
//!                                                                 prints it (node lines and orderings)
//!   lama stdhash FILE                                             a stdhash_ref.cc script on the emulated containers

use std::collections::BTreeMap;
use std::io::Write;
use taskcore::stdhash::{Fact, StdMap, StdSet};

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1).cloned())
}

fn stdhash(path: &str) {
    let text = std::fs::read_to_string(path).expect("the script");
    let mut fs: BTreeMap<i64, StdSet<Fact>> = BTreeMap::new();
    let mut is: BTreeMap<i64, StdSet<i32>> = BTreeMap::new();
    let mut ms: BTreeMap<i64, StdMap<i32, Vec<Fact>>> = BTreeMap::new();
    let out = std::io::stdout();
    let mut o = std::io::BufWriter::new(out.lock());
    writeln!(o, "hash00 {}", taskcore::stdhash::fd_hash64_2(0, 0)).unwrap();
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.is_empty() {
            continue;
        }
        let n = |i: usize| -> i64 { w[i].parse().unwrap() };
        match w[0] {
            "F" => {
                fs.insert(n(1), StdSet::new());
            }
            "f" => {
                fs.entry(n(1)).or_default().insert((n(2) as u32, n(3) as u32));
            }
            "e" => {
                fs.entry(n(1)).or_default().erase(&(n(2) as u32, n(3) as u32));
            }
            "c" => {
                let s = fs.entry(n(2)).or_default().clone();
                fs.insert(n(1), s);
            }
            "x" => {
                let a = fs.entry(n(2)).or_default().clone();
                let b = fs.entry(n(3)).or_default().clone();
                fs.insert(n(1), a.intersection(&b));
            }
            "r" => {
                let c = n(2) as usize;
                let v: Vec<Fact> = (0..c).map(|k| (n(3 + 2 * k) as u32, n(4 + 2 * k) as u32)).collect();
                fs.insert(n(1), StdSet::from_iter_std(v));
            }
            "p" => {
                let s = fs.entry(n(1)).or_default();
                write!(o, "F{}:", n(1)).unwrap();
                for (a, b) in s.iter() {
                    write!(o, " {}={}", a, b).unwrap();
                }
                writeln!(o, " |{}", s.bucket_count()).unwrap();
            }
            "I" => {
                is.insert(n(1), StdSet::new());
            }
            "i" => {
                is.entry(n(1)).or_default().insert(n(2) as i32);
            }
            "j" => {
                is.entry(n(1)).or_default().erase(&(n(2) as i32));
            }
            "q" => {
                let s = is.entry(n(1)).or_default();
                write!(o, "I{}:", n(1)).unwrap();
                for x in s.iter() {
                    write!(o, " {}", x).unwrap();
                }
                writeln!(o, " |{}", s.bucket_count()).unwrap();
            }
            "M" => {
                ms.insert(n(1), StdMap::new());
            }
            "m" => {
                ms.entry(n(1)).or_default().entry(n(2) as i32).push((n(3) as u32, n(4) as u32));
            }
            "o" => {
                let m = ms.entry(n(1)).or_default();
                write!(o, "M{}:", n(1)).unwrap();
                for (k, v) in m.iter() {
                    write!(o, " {}/{}", k, v.len()).unwrap();
                }
                writeln!(o, " |{}", m.set_bucket_count()).unwrap();
            }
            x => panic!("script op {:?}", x),
        }
    }
}

fn read_sas(args: &[String]) -> taskcore::lama::Parsed {
    let path = arg(args, "--sas").expect("--sas FILE");
    let text = std::fs::read_to_string(&path).expect("the SAS file");
    taskcore::lama::parse_with_predicates(&text).expect("a SAS file")
}

fn graph(args: &[String]) {
    let (t, pred, names) = read_sas(args);
    let adj = taskcore::search::plusone(&t);
    let t0 = std::time::Instant::now();
    let g = taskcore::landmarks::build(&t, &adj, &pred, None).expect("the landmark graph");
    let s = t0.elapsed().as_secs_f64();
    print!("{}", g.dump(&t.init, &|v, x| names[v as usize][x as usize].clone()));
    eprintln!(
        "landmarks {} disjunctive {} orderings {} explorations {} seconds {:.3} (setup {:.3} rhw {:.3} achievers {:.3} hps {:.3})",
        g.len(),
        g.n_disjunctive,
        g.n_orderings,
        g.stats.explorations,
        s,
        g.stats.setup_s,
        g.stats.rhw_s,
        g.stats.achievers_s,
        g.stats.hps_s
    );
    if args.iter().any(|a| a == "--costs") {
        for i in 0..g.len() {
            eprintln!("lm{} first {} possible {}", i, g.min_first[i], g.min_possible[i]);
        }
    }
}

fn search(args: &[String]) {
    let t0 = std::time::Instant::now();
    let c0 = taskcore::cli::cpu_s();
    let (t, pred, _) = read_sas(args);
    let read_s = t0.elapsed().as_secs_f64();
    let limit: Option<u64> = arg(args, "--limit").map(|s| s.parse().expect("--limit S"));
    let deadline = limit.map(|s| std::time::Instant::now() + std::time::Duration::from_secs(s));
    let c1 = taskcore::cli::cpu_s();
    let o = taskcore::lama::lama_first_costs(&t, &pred, deadline);
    let cpu = taskcore::cli::cpu_s() - c1;
    if o.status == "plan" {
        taskcore::search::check_plan(&t, &o.plan).expect("the plan checks");
        if let Some(p) = arg(args, "--plan") {
            std::fs::write(p, taskcore::search::plan_text(&t, &o.plan)).expect("the plan file");
        }
    }
    println!(
        "{{\"status\": \"{}\", \"steps\": {}, \"cost\": {}, \"expanded\": {}, \"evaluated\": {}, \"generated\": {}, \"dead_ends\": {}, \"h0\": {:?}, \"landmarks\": {}, \"disjunctive\": {}, \"orderings\": {}, \"explorations\": {}, \"read_s\": {:.3}, \"graph_s\": {:.3}, \"relax_s\": {:.3}, \"search_s\": {:.3}, \"eval_s\": {:.3}, \"lm_s\": {:.3}, \"cpu_s\": {:.3}, \"read_cpu_s\": {:.3}, \"peak_rss_mb\": {}}}",
        o.status,
        o.plan.len(),
        o.cost,
        o.expanded,
        o.evaluated,
        o.generated,
        o.dead_ends,
        o.h0.map(|(a, b)| vec![a, b]).unwrap_or_default(),
        o.landmarks,
        o.disjunctive,
        o.orderings,
        o.explorations,
        read_s,
        o.graph_s,
        o.relax_s,
        o.search_s,
        o.eval_s,
        o.lm_s,
        cpu,
        c1 - c0,
        taskcore::model::peak_rss() / (1 << 20)
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("stdhash") => stdhash(&args[1]),
        Some("graph") => graph(&args),
        Some("search") => search(&args),
        _ => {
            eprintln!("usage: lama search --sas FILE [--plan FILE] [--limit S] | lama graph --sas FILE | lama stdhash FILE");
            std::process::exit(2);
        }
    }
}
