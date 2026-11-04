//! e016: E016's gates for A, realization on the run path (experiments/E016-realize-run/plan.md, "Gates": G3 and G4;
//! amendment (1)). A measurement tool outside the system, like `e015` and `probe`: the numbers a gate needs (the
//! movie, the steps, the script, the class, the archive and its entries) are its arguments, and the routines it calls
//! name none. Counts, frames and ids only; no game text.
//!
//!   e016 inlog OUT --events FILE --from F --to T
//!       amendment (1)'s probe: the movie of FILE (one event per line, as SPEEDRUN_INPUTS; its events after F are
//!       scheduled too) from boot to frame F, the input log taken there, the run on to frame T, and patch 0007's
//!       records of (F, T]: per record its frame, inputs, verdict, the handler's area, value and mode, the verb slot,
//!       the zone and the writes by kind (Sentence, Walk, Started, Var, ...). Writes OUT/inlog.json.

mod common;
mod inlog;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let r = match argv.first().map(String::as_str) {
        Some("inlog") => inlog::run(&argv[1..]),
        _ => Err("usage: e016 inlog OUT ...".to_string()),
    };
    if let Err(e) = r {
        eprintln!("e016: {e}");
        std::process::exit(1);
    }
}
