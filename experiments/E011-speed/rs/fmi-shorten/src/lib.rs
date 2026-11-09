//! fmi-shorten: E024's stage that shortens the system's own route (experiments/E024-shorten/plan.md, Part 2), run as
//! `fmi shorten`. Its inputs are a movie from boot, the run's archive folder (the stage's guide atoms, its idle keys,
//! its readings) and a goal; it names no room, script, variable or object of any game (G7).
//!
//! * `chain`: the movie's steps (item 1): one replay from boot, the progress at each control stop, keep(k).
//! * `label`: each input's anchor and timing class (item 2), the steps' draw keys from a twin fork (item 3), and the
//!   anchors re-placed at their own frames (G2).

pub mod chain;
pub mod label;

pub type Res<T> = Result<T, String>;

pub(crate) fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
