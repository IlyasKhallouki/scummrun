//! reader-seq: readings one after another in one process, as fmi makes them (E011 round 2): the game loaded once, the
//! job memo kept in memory from reading to reading (memo.rs `MemStore`), each reading by reader-scope's command line
//! (src/run/mod.rs), its progress in its folder's reader-scope.err.
//!
//! usage: reader-seq [--mem-memo-gb G] FILE
//!   FILE         one reading per line: reader-scope's arguments, split at spaces (no quoting); empty lines and lines
//!                that start with # are left out
//!   --mem-memo-gb G  the store in memory holds at most G GB (default 1.5; 0: no store in memory, --memo DIR alone)
//! Prints one line per reading: its name, its wall seconds in the call, the store's bytes in memory after it.

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use scumm_reader::run::{run, Ctl, Log, Resident};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let gb: f64 = args.iter().position(|a| a == "--mem-memo-gb").map_or(1.5, |i| args[i + 1].parse().expect("--mem-memo-gb G"));
    let file = args.last().expect("usage: reader-seq [--mem-memo-gb G] FILE");
    let t0 = Instant::now();
    let mut res = Resident::load(&scumm_ir::default_root()).expect("the game");
    let load_s = t0.elapsed().as_secs_f64();
    let text = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{file}: {e}"));
    let lines: Vec<Vec<String>> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.split_whitespace().map(String::from).collect())
        .collect();
    if gb > 0.0 {
        let spill = lines.iter().find_map(|a| a.iter().position(|x| x == "--memo").map(|i| PathBuf::from(&a[i + 1])));
        res.memo = Some(Arc::new(scumm_reader::memo::MemStore::new((gb * (1u64 << 30) as f64) as u64, spill)));
    }
    println!("game loaded in {load_s:.2} s");
    for a in lines {
        let get = |f: &str| a.iter().position(|x| x == f).map(|i| a[i + 1].clone());
        let dir = PathBuf::from(get("--out-dir").or_else(|| get("--out")).unwrap_or_else(|| ".".into()));
        std::fs::create_dir_all(&dir).expect("the reading's folder");
        let ctl = Ctl::new(Log::file(&dir.join("reader-scope.err")).expect("the log"), Arc::new(AtomicBool::new(false)));
        let epoch = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
        // PERF_ONLY=NAME with PERF_CTL=CTL_FIFO,ACK_FIFO: perf record --control (started with -D -1) records that reading
        // alone
        let perf = std::env::var("PERF_ONLY").ok().filter(|n| Some(n.clone()) == get("--name")).and_then(|_| std::env::var("PERF_CTL").ok());
        let ctl_perf = |cmd: &str| {
            if let Some(p) = &perf {
                let (ctl, ack) = p.split_once(',').expect("PERF_CTL=CTL,ACK");
                std::fs::write(ctl, format!("{cmd}\n")).expect("perf's control fifo");
                // perf answers "ack\n" and keeps its end open: one line, not to the end
                let mut line = String::new();
                let _ = std::io::BufRead::read_line(&mut std::io::BufReader::new(std::fs::File::open(ack).expect("perf's ack fifo")), &mut line);
            }
        };
        ctl_perf("enable");
        println!("start {} at {:.3}", get("--name").unwrap_or_default(), epoch());
        let t = Instant::now();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&a, &res, Some(&ctl), t)));
        let wall = t.elapsed().as_secs_f64();
        ctl_perf("disable");
        println!("end {} at {:.3}", get("--name").unwrap_or_default(), epoch());
        let mem = res.memo.as_ref().map_or(0, |m| m.bytes());
        match r {
            Ok(r) => println!("{}: {wall:.2} s, status {}, memo in memory {} MB", get("--name").unwrap_or_default(), r["status"], mem >> 20),
            Err(_) => println!("{}: FAILED after {wall:.2} s ({:?})", get("--name").unwrap_or_default(), ctl.stopped.lock().unwrap()),
        }
    }
}
