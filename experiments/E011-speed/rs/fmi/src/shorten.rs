//! E024: `fmi shorten label --movie M --archive A [--seed S] [--no-twins] [--no-draws] [--check] [--tail N] OUT`
//! (fmi-shorten, experiments/E024-shorten/plan.md Part 2, items 1-3). The movie replayed from boot with the engine host
//! of this process's environment: OUT/steps.jsonl (the steps, each input's anchor, timing and identity, keep(k), the
//! twin's draw keys and the readings' draws that mark it) and OUT/labels.json (the summary); with --check, G2's
//! re-placement in OUT/g2.json. The archive folder is read only (its readings are linked into OUT/readings).

use fmi_explore::events::event_of;
use fmi_shorten::chain::{self, Held, Opts};
use fmi_shorten::{draws, label};
use std::io::Write;
use std::path::{Path, PathBuf};

fn read_movie(p: &Path) -> Result<Vec<fmi_core::Event>, String> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).ok().and_then(|v| event_of(&v)).ok_or(format!("{}: not an event: {l}", p.display())))
        .collect()
}

fn label_in(a: &super::Args) -> Result<(), String> {
    let out = PathBuf::from(a.pos.first().ok_or("OUT")?);
    let movie = read_movie(Path::new(a.get("movie").ok_or("--movie")?))?;
    let mut held = Held::load(Path::new(a.get("archive").ok_or("--archive")?))?;
    let num = |k: &str, d: u64| a.get(k).map(|v| v.parse::<u64>().map_err(|e| format!("--{k}: {e}"))).unwrap_or(Ok(d));
    let o = Opts { seed: num("seed", 1)? as u32, out: out.join("replay"), twins: !a.flag("no-twins"), tail: num("tail", 3000)? };
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let host = fmi_engine::Host::from_env().map_err(|e| format!("the engine host: {e}"))?;
    let t0 = std::time::Instant::now();
    let ch = chain::replay(&host, &movie, &mut held, &o)?;
    let replay_s = t0.elapsed().as_secs_f64();
    let l = label::labels(&ch);
    let rds = if a.flag("no-draws") {
        Vec::new()
    } else {
        let game = fmi_reading::draws::DrawGame::load(&fmi_explore::guide::default_root()).ok();
        draws::load(Path::new(a.get("archive").unwrap_or_default()), &out.join("readings"), game.as_ref())?
    };
    let mut f = std::fs::File::create(out.join("steps.jsonl")).map_err(|e| e.to_string())?;
    let mut marked = Vec::new();
    for (s, mut v) in ch.steps.iter().zip(label::steps_json(&ch, &l)) {
        let m = draws::mark(s, &rds);
        if !m.is_empty() {
            marked.push(s.k);
        }
        v["reader_draws"] = m.into();
        writeln!(f, "{v}").map_err(|e| e.to_string())?;
    }
    let mut sum = label::summary(&ch, &l);
    sum["replay_s"] = replay_s.into();
    sum["reader_draw_steps"] = marked.into();
    sum["readings"] = draws::summary(&rds);
    if a.flag("check") {
        let t1 = std::time::Instant::now();
        let mut g2 = label::check(&host, &movie, &ch, &l, &Opts { out: out.join("check"), ..o.clone() })?;
        g2["seconds"] = t1.elapsed().as_secs_f64().into();
        std::fs::write(out.join("g2.json"), serde_json::to_string_pretty(&g2).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        for k in ["inputs", "placed", "same_hit", "same_identity", "moved", "steps_checked", "keep_at_completion", "keep_before_completion", "same_end"] {
            sum["g2"][k] = g2[k].clone();
        }
    }
    std::fs::write(out.join("labels.json"), serde_json::to_string_pretty(&sum).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("{sum}");
    Ok(())
}

pub fn cmd_label(argv: &[String]) -> i32 {
    let a = super::Args::parse(argv, &["movie", "archive", "seed", "tail"]);
    match label_in(&a) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("fmi shorten label: {e}");
            1
        }
    }
}
