//! e015: E015's gates in the engine (experiments/E015-engine-gates/plan.md). A measurement tool outside the system,
//! like `probe` and `afford-check`: the numbers a gate needs (rooms, objects, scripts, entries) are its arguments, and
//! the routines it calls name none.
//!
//!   e015 item OUT ...   the item gate (plan.md, "The item gate"; amendment (1)); see item.rs

mod item;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let r = match argv.first().map(String::as_str) {
        Some("item") => item::main(&argv[1..]),
        _ => Err("usage: e015 item OUT ...".to_string()),
    };
    if let Err(e) = r {
        eprintln!("e015: {e}");
        std::process::exit(1);
    }
}
