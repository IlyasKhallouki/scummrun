//! E013 gate A3 on the handler rule (experiments/E013-robust/plan.md, gate A3 and amendment (2) (a)): the bridge's old
//! decision rule (SPEEDRUN_IDLE unset: the verb ids' kinds) and E014's handler rule (SPEEDRUN_IDLE=handler) read on the
//! same states. Two engine hosts from the environment (`HostCfg::from_env`, E014's game settings unset: Monkey Island),
//! one per rule; the process's own SPEEDRUN_IDLE must be unset. The rule only reads, so the two engines' tick, rng, room
//! and qf must be equal at every state read; a difference stops the tool.
//!
//!   a3 chain ARCH ENTRY OUT.jsonl
//!       the entry's movie from boot in both engines, from the root's frame to the entry's, one frame at a time: one
//!       line per frame where the two rules' (idle kind, blocker) pair changes, and a summary line
//!   a3 entries ARCH OUT.jsonl
//!       every entry's movie from boot to its frame in both engines: one line per entry
//!   a3 offers ARCH OUT.jsonl
//!       E012's affordance offers (each committed try's result.json e012.afford): the offer's state is the entry the try
//!       committed at the offer's frame (its attempt's last checkpoint), or the try's own entry; one line per offer
//!
//! Every line holds the old rule's (idle, blk) and the handler rule's; the summary counts how often both are idle, both
//! blocked, or only one is idle, by the old kind and the blocker. Frames, rooms and kinds only: no game text.

use fmi_core::*;
use fmi_engine::{Host, HostCfg};
use fmi_explore::events::event_of;
use fmi_explore::waitcheck::ArchiveRo;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

struct Pair {
    old: Host,
    new: Host,
    out: PathBuf,
}

/// A fresh engine with the movie's events up to `movie_to`, run to the decision point of frame `run_to` (spawn stops at
/// the first decision point).
fn boot(h: &Host, out: &Path, events: &[Event], movie_to: u64, run_to: u64) -> EResult<fmi_engine::BinEngine> {
    let mut e = h.spawn(&BootCfg { seed: 1, events: events.to_vec(), frame: Some(movie_to), dump_locals: false, lean: true, out_dir: out.to_path_buf() })?;
    if e.status().frame < run_to {
        e.run(&RunSpec { frame: Some(run_to), ..Default::default() })?;
    }
    Ok(e)
}

fn same_state(a: &Status, b: &Status) -> bool {
    a.frame == b.frame && a.tick == b.tick && a.rng == b.rng && a.room == b.room && a.qf == b.qf && a.ended == b.ended
}

fn read(s: &Status) -> Value {
    json!({"idle": s.idle, "blk": s.blk})
}

/// The class of a pair of reads: "both idle", "both blocked", "old only", "new only".
fn class(o: &Status, n: &Status) -> &'static str {
    match (o.idle.is_some(), n.idle.is_some()) {
        (true, true) => "both idle",
        (false, false) => "both blocked",
        (true, false) => "old only",
        (false, true) => "new only",
    }
}

/// The table's row: the old rule's kind ("dialog", "sentence") or "blocked: BLK", then the handler rule's
/// ("idle" or "blocked: BLK").
fn row(o: &Status, n: &Status) -> (String, String) {
    let ok = match &o.idle {
        Some(k) => k.clone(),
        None => format!("blocked: {}", o.blk.clone().unwrap_or_default()),
    };
    let nk = match &n.idle {
        Some(_) => "idle".to_string(),
        None => format!("blocked: {}", n.blk.clone().unwrap_or_default()),
    };
    (ok, nk)
}

#[derive(Default)]
struct Tally {
    n: u64,
    by_class: BTreeMap<&'static str, u64>,
    table: BTreeMap<(String, String), u64>,
}

impl Tally {
    fn add(&mut self, o: &Status, n: &Status, w: u64) {
        self.n += w;
        *self.by_class.entry(class(o, n)).or_default() += w;
        *self.table.entry(row(o, n)).or_default() += w;
    }
    fn json(&self) -> Value {
        let agree = self.by_class.get("both idle").copied().unwrap_or(0) + self.by_class.get("both blocked").copied().unwrap_or(0);
        json!({"n": self.n, "agree": agree, "agree_share": if self.n > 0 { agree as f64 / self.n as f64 } else { 0.0 },
               "by_class": self.by_class,
               "table": self.table.iter().map(|((o, n), c)| json!({"old": o, "handler": n, "count": c})).collect::<Vec<_>>()})
    }
}

fn events_of(movie: &[Value]) -> Vec<Event> {
    movie.iter().filter_map(event_of).collect()
}

impl Pair {
    fn both(&self, events: &[Event], frame: u64, tag: &str) -> EResult<(Status, Status)> {
        let eo = boot(&self.old, &self.out.join(format!("{tag}-old")), events, frame, frame)?;
        let en = boot(&self.new, &self.out.join(format!("{tag}-new")), events, frame, frame)?;
        let (o, n) = (eo.status().clone(), en.status().clone());
        drop(eo);
        drop(en);
        Ok((o, n))
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 3 {
        eprintln!("usage: a3 chain ARCH ENTRY OUT.jsonl | a3 entries ARCH OUT.jsonl | a3 offers ARCH OUT.jsonl");
        std::process::exit(2);
    }
    if std::env::var("SPEEDRUN_IDLE").is_ok_and(|v| !v.is_empty()) {
        eprintln!("a3: SPEEDRUN_IDLE must be unset in this process (the tool sets it for the handler rule's engines)");
        std::process::exit(2);
    }
    let base = HostCfg::from_env();
    let mut hcfg = base.clone();
    hcfg.extra_env.push(("SPEEDRUN_IDLE".into(), "handler".into()));
    let tmp = std::env::var("TMPDIR").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/var/tmp")).join(format!("a3-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("tmp");
    let p = Pair { old: Host::new(base).expect("host"), new: Host::new(hcfg).expect("host"), out: tmp.clone() };
    let arch = PathBuf::from(&a[1]);
    let ro = ArchiveRo::open(&arch).expect("the archive");
    let out_path = PathBuf::from(a.last().unwrap());
    let mut out = std::io::BufWriter::new(std::fs::File::create(&out_path).expect("OUT"));
    let mut tally = Tally::default();
    let t0 = std::time::Instant::now();
    let rc = match a[0].as_str() {
        "chain" => {
            let id: usize = a[2].parse().expect("ENTRY");
            let (movie, _) = ro.movie_cps(id).expect("the movie");
            let ev = events_of(&movie);
            let start = ro.frame[ro.chain(id)[0]];
            let end = ro.frame[id];
            let mut eo = boot(&p.old, &tmp.join("chain-old"), &ev, end, start).expect("boot old");
            let mut en = boot(&p.new, &tmp.join("chain-new"), &ev, end, start).expect("boot new");
            let mut last: Option<(Value, Value)> = None;
            let mut differ = None;
            let (mut o, mut n) = (eo.status().clone(), en.status().clone());
            loop {
                if !same_state(&o, &n) {
                    differ = Some(json!({"frame": o.frame, "old": [o.tick, o.rng, o.room, o.qf], "handler": [n.tick, n.rng, n.room, n.qf]}));
                    break;
                }
                tally.add(&o, &n, 1);
                let key = (read(&o), read(&n));
                if last.as_ref() != Some(&key) {
                    writeln!(out, "{}", json!({"frame": o.frame, "room": o.room, "old": key.0, "handler": key.1})).unwrap();
                    last = Some(key);
                }
                if o.frame >= end || o.ended {
                    break;
                }
                o = eo.step(1, false, false).expect("step old").status;
                n = en.step(1, false, false).expect("step new").status;
            }
            drop(eo);
            drop(en);
            let s = json!({"summary": "chain", "entry": id, "from": start, "to": end, "frames": tally.json(), "state_differs": differ,
                           "seconds": t0.elapsed().as_secs_f64()});
            writeln!(out, "{s}").unwrap();
            println!("{s}");
            i32::from(differ.is_some())
        }
        "entries" => {
            let mut differ = 0;
            for id in 0..ro.frame.len() {
                let (movie, _) = ro.movie_cps(id).expect("the movie");
                let (o, n) = p.both(&events_of(&movie), ro.frame[id], &format!("e{id}")).expect("boot");
                let same = same_state(&o, &n);
                if !same {
                    differ += 1;
                }
                tally.add(&o, &n, 1);
                writeln!(out, "{}", json!({"entry": id, "frame": ro.frame[id], "room": o.room, "old": read(&o), "handler": read(&n),
                                            "class": class(&o, &n), "same_state": same})).unwrap();
            }
            let s = json!({"summary": "entries", "archive": arch, "entries": tally.json(), "state_differs": differ,
                           "seconds": t0.elapsed().as_secs_f64()});
            writeln!(out, "{s}").unwrap();
            println!("{s}");
            i32::from(differ > 0)
        }
        "offers" => {
            // the log's lines give each committed try's new entry ids in result.json "new" order
            let log: Vec<Value> = std::fs::read_to_string(arch.join("log.jsonl")).expect("log.jsonl").lines()
                .filter_map(|l| serde_json::from_str(l).ok()).collect();
            let mut differ = 0;
            let mut gate = (0u64, 0u64); // (offers with kept outcomes where the old rule was idle, of them idle in the handler rule)
            let mut kept_tally = Tally::default();
            for line in &log {
                let Some(job) = line["job"].as_str() else { continue };
                let r: Value = match std::fs::read_to_string(arch.join("jobs").join(job).join("result.json")) {
                    Ok(t) => serde_json::from_str(&t).unwrap_or(Value::Null),
                    Err(_) => continue,
                };
                let steps = r["e012"]["afford"].as_array().cloned().unwrap_or_default();
                if steps.is_empty() {
                    continue;
                }
                let new_ids: Vec<usize> = line["new_entries"].as_array().map(|v| v.iter().filter_map(|x| x.as_u64().map(|x| x as usize)).collect()).unwrap_or_default();
                let entry = line["entry"].as_u64().map(|x| x as usize);
                for (k, s) in steps.iter().enumerate() {
                    let f = s["offer_frame"].as_u64().or(s["offer"]["frame"].as_u64()).unwrap_or(0);
                    // the state: an entry the try made at the offer's frame, else the try's own entry at that frame
                    let at = new_ids.iter().copied().find(|&i| i < ro.frame.len() && ro.frame[i] == f && !r["new"].as_array().map(|n| n.iter().any(|x| x["how"] == "afford" && x["frame"].as_u64() == Some(f))).unwrap_or(false))
                        .or_else(|| new_ids.iter().copied().find(|&i| i < ro.frame.len() && ro.frame[i] == f))
                        .or(entry.filter(|&i| ro.frame[i] == f));
                    let Some(at) = at else {
                        writeln!(out, "{}", json!({"job": job, "step": k, "offer_frame": f, "error": "no entry at the offer's frame"})).unwrap();
                        continue;
                    };
                    let (movie, _) = ro.movie_cps(at).expect("the movie");
                    let (o, n) = p.both(&events_of(&movie), f, &format!("{job}-{k}")).expect("boot");
                    let same = same_state(&o, &n);
                    if !same {
                        differ += 1;
                    }
                    let kept = s["kept"].as_u64().unwrap_or(0);
                    tally.add(&o, &n, 1);
                    if kept > 0 && o.idle.is_some() {
                        gate.0 += 1;
                        if n.idle.is_some() {
                            gate.1 += 1;
                        }
                        kept_tally.add(&o, &n, 1);
                    }
                    writeln!(out, "{}", json!({"job": job, "step": k, "why": s["why"], "kind": s["offer"]["idle"], "offer_frame": f, "entry": at,
                                                "trials": s["trials"], "kept": kept, "room": o.room, "old": read(&o), "handler": read(&n),
                                                "class": class(&o, &n), "same_state": same})).unwrap();
                }
            }
            let s = json!({"summary": "offers", "archive": arch, "offers": tally.json(), "kept_offers": kept_tally.json(),
                           "gate": {"old_idle_with_kept": gate.0, "handler_idle_there": gate.1, "pass": gate.0 == gate.1},
                           "state_differs": differ, "seconds": t0.elapsed().as_secs_f64()});
            writeln!(out, "{s}").unwrap();
            println!("{s}");
            i32::from(differ > 0 || gate.0 != gate.1)
        }
        m => {
            eprintln!("a3: unknown mode {m}");
            2
        }
    };
    out.flush().expect("write OUT");
    drop(out);
    let _ = std::fs::remove_dir_all(&tmp);
    std::process::exit(rc);
}
