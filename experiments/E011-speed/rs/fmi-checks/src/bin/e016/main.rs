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
//!
//!   e016 g3 OUT --movie FILE --clicks N --frame F --steps S1,S2 --local L --class-obj C --class K [--object O]
//!          [--run-on R]
//!       G3: the first N clicks of FILE's "movie" ([frame, x, y] from boot) to frame F, a no-input fork to the first
//!       idle stop when F is not one; each step ("s<v>:<a>[:<b>]" or "d<v>") realized by fmi-play's realize
//!       (FMI_REALIZE=1) and its events played in place as the hook plays them, the chain run to the next idle between
//!       steps; then on, frame by frame up to R frames (default 600), until the room's local L has run and class:C:K
//!       is cleared. Pass: both, within 15 candidates over the steps, and the chain replayed from boot equal (every
//!       fact, the room, the digest). A second pass plays the steps through `executor::play_input` (the hook) from
//!       the same start; its events and digest are compared. owner:O is reported. Writes OUT/g3.json and
//!       OUT/events.jsonl (the realized inputs).
//!
//!   e016 cp OUT --movie FILE --clicks N --frame F
//!       the first N clicks of FILE's "movie" to frame F, and the engine's checkpoint there as an archive entry keeps
//!       it. Writes OUT/cp.json.
//!
//!   e016 g4 OUT ARCHIVE GOAL_ENTRY --goal KEY=V [--run-on R] [--mode jobs,chain]
//!       G4: the goal entry's chain in ARCHIVE (read only), each sentence, walk and dialog step it took re-realized
//!       (FMI_REALIZE=1): `jobs`, each chain entry's steps from the entry its job started at; `chain`, every step from
//!       the boot's entry, then on to KEY = V (up to R frames, default 600) and the movie verified from boot. See
//!       g4.rs. Writes OUT/g4.json and OUT/chain-events.jsonl.

mod asap;
mod common;
mod cp;
mod g3;
mod g4;
mod hits;
mod inlog;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let r = match argv.first().map(String::as_str) {
        Some("inlog") => inlog::run(&argv[1..]),
        Some("g3") => g3::run(&argv[1..]),
        Some("g4") => g4::run(&argv[1..]),
        Some("hits") => hits::run(&argv[1..]),
        Some("cp") => cp::run(&argv[1..]),
        Some("asap") => asap::run(&argv[1..]),
        _ => Err("usage: e016 inlog OUT ... | e016 g3 OUT ... | e016 g4 OUT ...".to_string()),
    };
    if let Err(e) = r {
        eprintln!("e016: {e}");
        std::process::exit(1);
    }
}
