//! e024: E024's gates (experiments/E024-shorten/plan.md, "Gates"). A measurement tool outside the system, like `e016`:
//! the settings a gate compares are its arguments, and the routines it calls name no game number. Counts, frames and
//! ids only; no game text.
//!
//!   e024 g0 OUT [--seed S] [--max-frame M]
//!       G0, the settings (plan.md Part 2, item 8), one boot each to its first control (the bridge's first idle stop):
//!       `std`, the run's boot (the segment at the first sentence idle); (a) SPEEDRUN_START_FRAME=1; (b) the same with
//!       Skips{text: 1, cutscenes: 1} at frame 1; (c) (b)'s var 19 against (a)'s. Then the var-19 rule: (d) (b) with
//!       SPEEDRUN_TIMER_NEXT set to (a)'s var 19; when that start fails the check, (e) the start moved to the first
//!       frame of the standard boot whose var 19 equals it, skips from there; and the pass-through itself, any other
//!       TIMER_NEXT ends the boot at the segment start. Per boot: the first control's frame, tick, room, var 19, idle
//!       kind and digest, or the end; the bridge's skip records by kind. Writes OUT/g0.json.

mod g0;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let r = match argv.first().map(String::as_str) {
        Some("g0") => g0::run(&argv[1..]),
        _ => Err("usage: e024 g0 OUT [--seed S] [--max-frame M]".to_string()),
    };
    if let Err(e) = r {
        eprintln!("e024: {e}");
        std::process::exit(1);
    }
}
