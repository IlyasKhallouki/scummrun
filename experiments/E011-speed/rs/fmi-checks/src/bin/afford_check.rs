//! afford-check: E012's affordance step (fmi_explore::afford) on archive entries, for its gates A1-A3
//! (experiments/E012-game-mechanics/plan.md, "Components", `afford`). Counts and timings only; no game text.
//!
//!   afford-check m4 OUT ARCHIVE IDX --tested FILE --horizon H [--probe AFFORD.TSV]
//!       M4's form: the offer at the entry, each input in a fork run to the entry's frame + H, its outcome against a
//!       no-input fork there. Writes OUT/afford.tsv (name, outcome) as M4's probe wrote it; with --probe the trials and
//!       their outcomes are compared name by name with the probe's file.
//!   afford-check run OUT ARCHIVE IDX --tested FILE [--horizon H] [--no-third] [--verify] [--repeat N]
//!       the step as a try runs it: each trial to the bridge's idle stop (or the horizon, default W), the third click
//!       when a verb-then-target trial left the game composing, the stops kept by novelty over the archive's entries
//!       (the tested keys' values and the room: the stand-in for the archive's novelty over T). With --verify every
//!       kept entry is verified from boot in a fresh engine (its movie: the entry's, then the trial's clicks).
//!       --repeat N runs the step N times on the same engine (the timing's spread).
//!
//! ARCHIVE is read only: the entry's chain of segments from the root (entries/K.json.gz) and every entry's last
//! checkpoint.

use fmi_core::*;
use fmi_engine::Host;
use fmi_explore::afford::{self, AffordCfg, Inputs, Novelty, PairNovelty};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

type Res<T> = Result<T, String>;

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

struct Args {
    pos: Vec<String>,
    opts: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(argv: &[String]) -> Args {
        let with_value = ["tested", "horizon", "probe", "repeat"];
        let (mut pos, mut opts) = (Vec::new(), Vec::new());
        let mut i = 0;
        while i < argv.len() {
            if let Some(n) = argv[i].strip_prefix("--") {
                if with_value.contains(&n) {
                    i += 1;
                    opts.push((n.to_string(), argv.get(i).cloned()));
                } else {
                    opts.push((n.to_string(), None));
                }
            } else {
                pos.push(argv[i].clone());
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

/// An archive's entry file, read only.
fn entry(arch: &Path, i: u64) -> Res<Value> {
    fmi_explore::files::read_gz(&arch.join("entries").join(format!("{i}.json.gz")))
}

/// The entry's movie from boot (its chain's segments, events up to its frame) as the archive's JSON events, and its
/// frame.
fn movie(arch: &Path, idx: u64) -> Res<(Vec<Value>, u64)> {
    let e = entry(arch, idx)?;
    let frame = e["frame"].as_u64().ok_or("entry without a frame")?;
    let mut chain = vec![e];
    while let Some(p) = chain.last().unwrap().get("parent").and_then(|p| p.as_u64()) {
        chain.push(entry(arch, p)?);
    }
    chain.reverse();
    let evs: Vec<Value> = chain
        .iter()
        .flat_map(|c| c["events"].as_array().cloned().unwrap_or_default())
        .filter(|v| v["frame"].as_u64().is_some_and(|f| f <= frame))
        .collect();
    Ok((evs, frame))
}

/// The archive's states for the novelty stand-in: every entry's last checkpoint's facts.
fn archive_states(arch: &Path) -> Res<Vec<Facts>> {
    let mut out = Vec::new();
    for i in 0.. {
        let p = arch.join("entries").join(format!("{i}.json.gz"));
        if !p.exists() {
            break;
        }
        let e = entry(arch, i)?;
        let cps = e["checkpoints"].as_array().ok_or("entry without checkpoints")?;
        let last = cps.last().ok_or("entry with no checkpoint")?;
        let mut pairs = Vec::new();
        for (k, v) in last[1].as_object().ok_or("checkpoint facts")? {
            if let Some(n) = v.as_i64() {
                pairs.push((Key::parse(k), n));
            }
        }
        out.push(Facts::from_pairs(pairs));
    }
    Ok(out)
}

fn tested_keys(p: &str) -> Res<Vec<Key>> {
    Ok(std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?.lines().filter(|l| !l.is_empty()).map(Key::parse).collect())
}

fn sig(o: &[(Key, Val)]) -> String {
    o.iter().map(|(k, v)| format!("{}={v}", k.text())).collect::<Vec<_>>().join(",")
}

fn boot(h: &Host, out: &Path, evs: &[Value], frame: u64) -> Res<Box<dyn Engine>> {
    let events: Vec<Event> = evs.iter().filter_map(fmi_explore::events::event_of).collect();
    let cfg = BootCfg { seed: 1, events, frame: Some(frame), dump_locals: false, lean: true, out_dir: out.join("boot") };
    Ok(Box::new(h.boot_bin(&cfg).map_err(e2s)?))
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let r = match argv.first().map(|s| s.as_str()) {
        Some("m4") => m4(&Args::parse(&argv[1..])),
        Some("run") => run(&Args::parse(&argv[1..])),
        _ => Err("usage: afford-check m4|run OUT ARCHIVE IDX --tested FILE ...".into()),
    };
    if let Err(e) = r {
        eprintln!("afford-check: {e}");
        std::process::exit(1);
    }
}

/// The offer and the inputs at the entry (the engine at its frame, made ready when it is not idle there).
fn setup(a: &Args) -> Res<(PathBuf, PathBuf, u64, Host, Vec<Value>, Box<dyn Engine>, f64)> {
    if a.pos.len() < 3 {
        return Err("OUT ARCHIVE IDX".into());
    }
    let out = PathBuf::from(&a.pos[0]);
    let arch = PathBuf::from(&a.pos[1]);
    let idx: u64 = a.pos[2].parse().map_err(e2s)?;
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let h = Host::from_env().map_err(e2s)?;
    let (evs, frame) = movie(&arch, idx)?;
    let t = Instant::now();
    let e = boot(&h, &out, &evs, frame)?;
    Ok((out, arch, idx, h, evs, e, t.elapsed().as_secs_f64()))
}

fn m4(a: &Args) -> Res<()> {
    let (out, _arch, idx, _h, _evs, mut e, boot_s) = setup(a)?;
    let tested = tested_keys(a.get("tested").ok_or("--tested FILE")?)?;
    let horizon: u64 = a.get("horizon").ok_or("--horizon H")?.parse().map_err(e2s)?;
    let o = e.obs(&ObsReq::default()).map_err(e2s)?;
    let t = Instant::now();
    let of = afford::offer(&mut e, &o).map_err(e2s)?;
    let offer_s = t.elapsed().as_secs_f64();
    let inputs = afford::inputs_of(&of);
    let all_seen = NoNovelty;
    let cfg = AffordCfg { horizon, fixed: true, third: false };
    let res = afford::try_all(&mut e, &of, &inputs, &tested, &all_seen, &cfg, None).map_err(e2s)?;
    e.quit();
    let mut tsv = String::new();
    for t in &res.trials {
        tsv.push_str(&format!("{}\t{}\n", t.inputs.name(), sig(&t.outcome)));
    }
    std::fs::write(out.join("afford.tsv"), &tsv).map_err(e2s)?;
    let mut summary = res.json();
    summary["entry"] = json!(idx);
    summary["boot_s"] = json!(boot_s);
    summary["offer_s"] = json!(offer_s);
    summary["form"] = json!("m4");
    summary["horizon"] = json!(horizon);
    let backs: Vec<Value> = res.trials.iter().map(|t| json!([t.inputs.name(), t.back, t.idle])).collect();
    summary["backs"] = json!(backs.into_iter().take(12).collect::<Vec<_>>());
    if let Some(p) = a.get("probe") {
        summary["probe"] = compare_probe(p, &res.trials.iter().map(|t| (t.inputs.name(), sig(&t.outcome))).collect::<Vec<_>>())?;
    }
    println!("{}", summary);
    Ok(())
}

/// Verify from boot (replay.verify with one checkpoint): a fresh engine plays the movie from boot to the checkpoint's
/// frame; its facts, tick, random state and actors are compared with the checkpoint's. Returns the differences.
fn verify(h: &Host, out: &Path, movie: &[Value], cp: &Checkpoint) -> Res<Vec<Value>> {
    let want = fmi_explore::pyjson::loads(&fmi_explore::archive::checkpoint_text(cp)).map_err(e2s)?;
    let mut e = match boot(h, out, movie, cp.frame) {
        Ok(e) => e,
        Err(x) => return Ok(vec![json!({"frame": cp.frame, "error": x})]),
    };
    let o = e.obs(&ObsReq::default()).map_err(e2s)?;
    let st = e.status().clone();
    e.quit();
    let d = fmi_explore::tryjob::compare_checkpoint(&want, &o, &st);
    Ok(if d.is_empty() { vec![] } else { vec![Value::Object(d)] })
}

/// M4's probe file (name, all, outcome) against this run's (name, outcome), name by name.
fn compare_probe(p: &str, mine: &[(String, String)]) -> Res<Value> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
    let theirs: Vec<(String, String)> = text
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f[0].to_string(), f.get(2).copied().unwrap_or("").to_string())
        })
        .collect();
    let tm: BTreeMap<&str, &str> = theirs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let mm: BTreeMap<&str, &str> = mine.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let only_probe: Vec<&str> = tm.keys().filter(|k| !mm.contains_key(*k)).copied().collect();
    let only_mine: Vec<&str> = mm.keys().filter(|k| !tm.contains_key(*k)).copied().collect();
    let differ: Vec<&str> = mm.iter().filter(|(k, v)| tm.get(*k).is_some_and(|w| w != *v)).map(|(k, _)| *k).collect();
    let order_same = theirs.iter().map(|x| &x.0).eq(mine.iter().map(|x| &x.0));
    let mut out_t: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, s) in &theirs {
        *out_t.entry(s.as_str()).or_default() += 1;
    }
    Ok(json!({"probe_trials": theirs.len(), "trials": mine.len(), "only_probe": only_probe, "only_mine": only_mine,
              "outcome_differs": differ, "order_same": order_same, "probe_outcomes": out_t.len(),
              "same": only_probe.is_empty() && only_mine.is_empty() && differ.is_empty() && order_same}))
}

/// No state is novel (M4's form keeps nothing).
struct NoNovelty;

impl Novelty for NoNovelty {
    fn novel(&self, _f: &Facts) -> Vec<String> {
        Vec::new()
    }
}

fn run(a: &Args) -> Res<()> {
    let (out, arch, idx, h, evs, mut e, boot_s) = setup(a)?;
    let tested = tested_keys(a.get("tested").ok_or("--tested FILE")?)?;
    let horizon: u64 = a.get("horizon").map(|h| h.parse().map_err(e2s)).transpose()?.unwrap_or(afford::W);
    let repeat: usize = a.get("repeat").map(|h| h.parse().map_err(e2s)).transpose()?.unwrap_or(1);
    let mut nov = PairNovelty::new(tested.iter().copied());
    for f in archive_states(&arch)? {
        nov.add(&f);
    }
    let cfg = AffordCfg { horizon, fixed: false, third: !a.flag("no-third") };
    // the interface is read where the bridge is idle (a fork run to the first idle stop when the entry is not)
    let mut readied = match afford::ready(&mut e, horizon).map_err(e2s)? {
        None => None,
        Some(Some(b)) => Some(b),
        Some(None) => return Err(format!("entry {idx}: no idle stop within {horizon} frames")),
    };
    let base: &mut Box<dyn Engine> = readied.as_mut().unwrap_or(&mut e);
    let mut runs = Vec::new();
    let mut last = None;
    for _ in 0..repeat {
        let t = Instant::now();
        let o = base.obs(&ObsReq::default()).map_err(e2s)?;
        let of = afford::offer(base, &o).map_err(e2s)?;
        let inputs: Vec<Inputs> = afford::inputs_of(&of);
        let offer_s = t.elapsed().as_secs_f64();
        let res = afford::try_all(base, &of, &inputs, &tested, &nov, &cfg, None).map_err(e2s)?;
        let mut j = res.json();
        j["offer_s"] = json!(offer_s);
        j["step_s"] = json!(t.elapsed().as_secs_f64());
        runs.push(j);
        last = Some(res);
    }
    let res = last.ok_or("--repeat 0")?;
    let f0 = base.status().frame;
    if let Some(b) = readied {
        b.quit();
    }
    e.quit();
    let mut tsv = String::new();
    for t in &res.trials {
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            t.inputs.name(),
            t.frame,
            t.reason,
            t.idle.as_deref().unwrap_or("-"),
            t.back.map(|b| b.to_string()).unwrap_or("-".into()),
            t.composing as u8,
            sig(&t.outcome)
        ));
    }
    std::fs::write(out.join("trials.tsv"), &tsv).map_err(e2s)?;
    let mut summary = res.json();
    summary["entry"] = json!(idx);
    summary["frame"] = json!(f0);
    summary["boot_s"] = json!(boot_s);
    summary["horizon"] = json!(horizon);
    summary["third_on"] = json!(cfg.third);
    summary["runs"] = json!(runs.iter().map(|r| json!({"ms_per_trial": r["ms_per_trial"], "step_s": r["step_s"], "offer_s": r["offer_s"]})).collect::<Vec<_>>());
    summary["kept_list"] = json!(res.kept.iter().map(|k| json!({"name": k.name, "frame": k.checkpoint.frame, "room": k.room, "novel": k.novel.len()})).collect::<Vec<_>>());
    if a.flag("verify") {
        let mut rows = Vec::new();
        let mut differ_total = 0;
        for (i, k) in res.kept.iter().enumerate() {
            let mut m = evs.clone();
            m.extend(k.events_json());
            let t = Instant::now();
            let d = verify(&h, &out.join(format!("verify-{i}")), &m, &k.checkpoint)?;
            differ_total += d.len();
            rows.push(json!({"kept": i, "name": k.name, "frame": k.checkpoint.frame, "same": d.is_empty(), "differ": d.len(),
                             "first": d.first(), "seconds": t.elapsed().as_secs_f64()}));
        }
        summary["verify"] = json!({"kept": res.kept.len(), "same": rows.iter().filter(|r| r["same"] == json!(true)).count(),
                                   "differences": differ_total, "rows": rows});
    }
    println!("{}", summary);
    Ok(())
}
