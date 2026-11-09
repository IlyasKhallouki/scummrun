//! fmi-shorten: E024's stage that shortens the system's own route (experiments/E024-shorten/plan.md, Part 2), run as
//! `fmi shorten`. Its inputs are a movie from boot, the run's archive folder (the stage's guide atoms, its idle keys,
//! its readings) and a goal; it names no room, script, variable or object of any game (G7).
//!
//! * `chain`: the movie's steps (item 1): one replay from boot, the progress at each control stop, keep(k).
//! * `label`: each input's anchor and timing class (item 2), the steps' draw keys from a twin fork (item 3), and the
//!   anchors re-placed at their own frames (G2).
//! * `draws`: item 3's cross-check, the archive's readings' draws (fmi-reading draws.rs) against the steps.
//! * `verdict`: item 4, a click's and a step's verdicts.
//! * `scan`: item 5, a step found again at every frame from the steps before it.
//! * `pass`: the forward pass, every step found again in order from boot.
//! * `refind`: item 6, the fallbacks for a step the scan does not complete.
//! * `reduce`: item 7, drops from the verified route, kept for an earlier verified end.
//! * `skips`: item 8, the bridge's skips from boot.
//! * `movie`: item 9, a route's files and its check from boot.

pub mod chain;
pub mod draws;
pub mod label;
pub mod movie;
pub mod pass;
pub mod reduce;
pub mod refind;
pub mod scan;
pub mod skips;
pub mod verdict;

pub type Res<T> = Result<T, String>;

pub(crate) fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
