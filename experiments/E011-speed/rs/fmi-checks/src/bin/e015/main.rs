//! e015: E015's gates in the engine (experiments/E015-engine-gates/plan.md, "Tool"). A measurement tool outside the
//! system, like `probe` and `afford-check`: the numbers a gate needs (rooms, objects, scripts, entries, the steps, the
//! target room) are its arguments, and the routines it calls name none. Counts, frames and ids only; no game text.
//!
//!   e015 item OUT ...   the item gate (plan.md, "The item gate"; amendment (1)); see item.rs
//!
//!   e015 realize OUT ARCHIVE IDX --steps S1,S2,.. [--room R] [--run-on N] [--verify]
//!       G1: from the archive's entry IDX (its movie from boot; a no-input fork to the first idle stop when the entry
//!       is not idle), each step realized by fmi-play's realize (FMI_REALIZE=1 is required) in order; between steps
//!       the accepted fork runs to the bridge's next idle. A step is "s<v>:<a>[:<b>]" (a sentence) or "d<v>" (a
//!       dialog verb). With --room, after the last step the engine runs one frame at a time until the room is R or N
//!       frames pass (default 600). With --verify, the entry's movie and the realized inputs are replayed from boot
//!       in a fresh engine to the last frame and compared there: every fact, the room and the state digest.
//!       Writes OUT/realize.json (the record) and OUT/events.jsonl (the movie's events after the entry's).

mod item;
mod realize_cmd;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let r = match argv.first().map(String::as_str) {
        Some("item") => item::main(&argv[1..]),
        Some("realize") => realize_cmd::run(&argv[1..]),
        _ => Err("usage: e015 item OUT ... | e015 realize OUT ARCHIVE IDX --steps S1,S2,.. [--room R] [--run-on N] \
                  [--verify]"
            .to_string()),
    };
    if let Err(e) = r {
        eprintln!("e015: {e}");
        std::process::exit(1);
    }
}
