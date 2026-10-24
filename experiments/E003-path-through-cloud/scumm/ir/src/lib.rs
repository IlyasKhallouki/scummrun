//! E003's script layer in Rust: the port of ir.py, facts.py's key scheme, and everything model.py's
//! `Game` computes from the game files alone (decisions.md D7, layer 1). The Python modules are the
//! reference; `tests/oracle.rs` checks this crate against them byte for byte (scumm/README.md).
//!
//! * [`read::load`] reads data/scripts-dos-cd into a [`Script`]: every unit, its instructions with
//!   their parsed statements, control flow (`next`, `target`, `esc`), engine flags (`yields`,
//!   `ends`) and notes, as ir.py's `load`.
//! * [`Game::load`] adds model.Game's data (object rooms and parents, verb block offsets, room
//!   widths, the pseudo-room map, the F flags of each script) and every static analysis, for both
//!   settings of switch const_polls: successors, join points, loops and what their exits read, wait
//!   points, time exits, the non-fact variables.
//! * [`facts`] is facts.py: typed fact keys that print as facts.py's strings, `facts_of`, `diff`, `patch`.
//!
//! Layout for speed: strings are interned ([`Sym`]); expression nodes live in one arena ([`Ast`]) and
//! are named by `u32` ids; a statement is a `Copy` value inside its instruction; every unit's
//! instructions are one vector, a unit holds its range; per-instruction tables are flat vectors.

pub mod analysis;
pub mod ast;
pub mod dump;
pub mod facts;
pub mod game;
pub mod json;
pub mod parse;
pub mod py;
pub mod read;
pub mod show;
pub mod sym;
pub mod vars;

pub use analysis::{Analyses, Loop, LoopKind, Read, Reason, WaitKind, WaitPoint};
pub use ast::*;
pub use facts::{Fact, FactMap, Slot};
pub use game::{default_root, Game, GameData, Resist};
pub use read::Script;
pub use sym::{known, Interner, Sym};

/// Peak resident memory of this process so far, in kB (VmHWM), where /proc has it.
pub fn peak_rss_kb() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}
