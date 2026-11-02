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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("stdhash") => stdhash(&args[1]),
        _ => {
            eprintln!("usage: lama search --sas FILE [--plan FILE] [--limit S] | lama graph --sas FILE | lama stdhash FILE");
            std::process::exit(2);
        }
    }
    let _ = arg;
}
