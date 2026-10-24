//! Writes the Rust side of the oracle: the same canonical lines as dump_ir.py.
//!
//! usage: cargo run --release --bin dump_ir [-- OUT_DIR]
//!
//! Writes OUT_DIR/ir-rust.jsonl.gz, ir-rust-noblocks.jsonl.gz and ir-rust-synthetic.jsonl.gz (default
//! ROOT/out/scumm-ir).

use flate2::write::GzEncoder;
use flate2::Compression;
use scumm_ir::{default_root, dump, read, Game};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

fn write_gz(path: &PathBuf, lines: &[String]) -> std::io::Result<()> {
    let mut gz = GzEncoder::new(std::fs::File::create(path)?, Compression::default());
    for l in lines {
        gz.write_all(l.as_bytes())?;
    }
    gz.finish()?;
    Ok(())
}

fn main() -> std::io::Result<()> {
    let root = default_root();
    let out = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| root.join("out").join("scumm-ir"));
    std::fs::create_dir_all(&out)?;
    let t = Instant::now();
    let g = Game::load(&root)?;
    let load = t.elapsed();
    let scripts = root.join("data").join("scripts-dos-cd");
    let meta: serde_json::Value = serde_json::from_slice(&std::fs::read(scripts.join("index.json"))?)?;
    let states_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fact_states.json");
    let states: serde_json::Value = serde_json::from_slice(&std::fs::read(states_path)?)?;
    let t = Instant::now();
    let lines = dump::lines(&g, &meta, &states);
    write_gz(&out.join("ir-rust.jsonl.gz"), &lines)?;
    let nb = read::load_with(&scripts, None, &meta)?;
    write_gz(&out.join("ir-rust-noblocks.jsonl.gz"), &dump::noblocks_lines(&nb))?;
    let synth_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("synthetic.json");
    let synth: serde_json::Value = serde_json::from_slice(&std::fs::read(synth_path)?)?;
    write_gz(&out.join("ir-rust-synthetic.jsonl.gz"), &dump::synthetic_lines(&synth))?;
    eprintln!(
        "wrote {}: {} units, {} instructions; load {:.1} ms, dump {:.1} ms",
        out.display(),
        g.units.len(),
        g.ins.len(),
        load.as_secs_f64() * 1e3,
        t.elapsed().as_secs_f64() * 1e3
    );
    Ok(())
}
