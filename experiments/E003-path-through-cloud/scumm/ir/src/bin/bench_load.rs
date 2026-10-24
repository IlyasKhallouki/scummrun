//! Times loading the whole game and reports peak memory: the units (ir.load), then model.Game's data and
//! every static analysis for both const_polls settings (Game.load plus what Python computes lazily).
//!
//! usage: cargo run --release --bin bench_load [-- REPEATS]   (peak RSS is the process's over all repeats)

use scumm_ir::{default_root, peak_rss_kb, read, Game};
use std::time::Instant;

fn main() {
    let root = default_root();
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(5);
    let scripts = root.join("data").join("scripts-dos-cd");
    let (mut units, mut game, mut total) = (Vec::new(), Vec::new(), Vec::new());
    let mut count = (0, 0);
    for _ in 0..n {
        let t = Instant::now();
        let meta: serde_json::Value = serde_json::from_slice(&std::fs::read(scripts.join("index.json")).unwrap()).unwrap();
        let sc = read::load_with(&scripts, Some(&root), &meta).expect("units");
        let t1 = t.elapsed();
        let g = Game::from_script(sc, &root, &meta).expect("game");
        let t2 = t.elapsed();
        units.push(t1.as_secs_f64() * 1e3);
        game.push((t2 - t1).as_secs_f64() * 1e3);
        total.push(t2.as_secs_f64() * 1e3);
        count = (g.units.len(), g.ins.len());
        std::hint::black_box(&g);
    }
    let med = |v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };
    println!(
        "{} units, {} instructions; median over {n}: units {:.1} ms, game data and analyses {:.1} ms, total {:.1} ms; \
         peak RSS {:.1} MB",
        count.0,
        count.1,
        med(&mut units),
        med(&mut game),
        med(&mut total),
        peak_rss_kb().unwrap_or(0) as f64 / 1024.0
    );
}
