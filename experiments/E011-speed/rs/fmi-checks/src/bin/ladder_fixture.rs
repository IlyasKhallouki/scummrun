//! ladder-fixture: an archive-format fixture for E016's ladder (`ladder`) from a verified chain of clicks: the chain's
//! movie replayed from boot in the engine (seed 1, as `fmi explore verify` boots), one entry per stop frame given, each
//! the child of the one before, its events the movie's up to its frame and its one checkpoint the engine's state there.
//! Each checkpoint's facts are cut to the keys the ladder reads (a key absent from the state stays absent), so the
//! fixture is small; its rows' `how` is "fixture" and their `job` "fixture".
//!
//!   ladder-fixture OUT --chain CHAIN.json --frames F0,F1,...
//!
//! CHAIN.json: an E015 item check file (`results/item/check/carry-*.json`), its "movie" a list of [frame, x, y] clicks.
//! The engine and the game come from the environment, as for `e015chk` (E015's results/item/env.sh).

use fmi_core::*;
use fmi_engine::Host;
use fmi_explore::files;
use fmi_explore::harness::checkpoint_json;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

type Res<T> = Result<T, String>;

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// The keys the ladder reads.
fn kept(k: &str) -> bool {
    matches!(k, "room" | "ego" | "var:32" | "owner:946" | "class:491:12") || k == "end" || k.starts_with("end:")
}

fn opt<'a>(argv: &'a [String], k: &str) -> Res<&'a str> {
    let i = argv.iter().position(|a| a == &format!("--{k}")).ok_or(format!("--{k}"))?;
    argv.get(i + 1).map(String::as_str).ok_or(format!("--{k} needs a value"))
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&argv) {
        eprintln!("ladder-fixture: {e}");
        std::process::exit(1);
    }
}

fn run(argv: &[String]) -> Res<()> {
    let out = PathBuf::from(argv.first().filter(|a| !a.starts_with("--")).ok_or("OUT")?);
    if out.join("entries.jsonl").exists() {
        return Err(format!("{}: an archive is there", out.display()));
    }
    std::fs::create_dir_all(out.join("entries")).map_err(e2s)?;
    let chain = files::read_json(Path::new(opt(argv, "chain")?))?;
    let movie: Vec<Event> = chain["movie"]
        .as_array()
        .ok_or("CHAIN.json: no movie")?
        .iter()
        .map(|c| {
            let a = c.as_array()?;
            Some(Event { frame: a.first()?.as_u64()?, what: Input::Click(a.get(1)?.as_i64()? as i32, a.get(2)?.as_i64()? as i32) })
        })
        .collect::<Option<_>>()
        .ok_or("CHAIN.json: a click is not [frame, x, y]")?;
    let frames: Vec<u64> = opt(argv, "frames")?.split(',').map(|f| f.trim().parse::<u64>().map_err(e2s)).collect::<Res<_>>()?;
    let h = Host::from_env().map_err(e2s)?;
    let mut rows = Vec::new();
    for (id, &frame) in frames.iter().enumerate() {
        let evs: Vec<Event> = movie.iter().filter(|e| e.frame <= frame).cloned().collect();
        let cfg = BootCfg { seed: 1, events: evs.clone(), frame: Some(frame), dump_locals: false, lean: true,
                            out_dir: out.join("boot").join(id.to_string()) };
        let mut e: Box<dyn Engine> = Box::new(h.boot_bin(&cfg).map_err(e2s)?);
        let st = e.status().clone();
        let o = e.obs(&ObsReq::default()).map_err(e2s)?;
        e.quit();
        if st.frame != frame {
            return Err(format!("the boot to frame {frame} stopped at {} ({})", st.frame, st.reason));
        }
        let facts = facts_of(&o);
        let mut actors: Vec<(i32, i32, i32)> = o.actors.iter().map(|a| (a.id, a.x, a.y)).collect();
        actors.sort();
        let cp = Checkpoint { frame, facts, tick: st.tick, rng: st.rng, actors };
        let mut cj = checkpoint_json(&cp);
        let full: Map<String, Value> = cj[1].as_object().cloned().unwrap_or_default();
        let n_full = full.len();
        cj[1] = Value::Object(full.into_iter().filter(|(k, _)| kept(k)).collect());
        let doc = json!({"id": id, "parent": if id == 0 { Value::Null } else { json!(id - 1) }, "frame": frame, "tick": st.tick,
                         "events": evs.iter().map(fmi_explore::events::event_json).collect::<Vec<_>>(), "checkpoints": [cj]});
        files::write_gz_default(&out.join("entries").join(format!("{id}.json.gz")), &doc)?;
        rows.push(json!({"id": id, "parent": if id == 0 { Value::Null } else { json!(id - 1) }, "frame": frame, "tick": st.tick,
                         "room": st.room, "job": "fixture", "how": "fixture", "goal": false}));
        eprintln!("frame {frame}: room {}, {n_full} facts, kept {}", st.room, doc["checkpoints"][0][1].as_object().map(|m| m.len()).unwrap_or(0));
    }
    let _ = std::fs::remove_dir_all(out.join("boot"));
    let text: String = rows.iter().map(|r| r.to_string() + "\n").collect();
    std::fs::write(out.join("entries.jsonl"), text).map_err(e2s)?;
    Ok(())
}
