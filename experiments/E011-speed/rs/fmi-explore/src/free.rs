//! E013's model-free share of tries (experiments/E013-robust/plan.md, section D, and the amendment "component
//! `free`"; switch FMI_FREE=ρ0, off by default).
//!
//! * **The kind** (`pick`): job n is free when floor((n + 1)·ρ) > floor(n·ρ), ρ being FMI_FREE's ρ0 or, with
//!   FMI_FREE_SHIFT=1, stall's share; else the model's candidate, which stall's cell rule may make free. Stall's two
//!   rules come through the `Shift` port. A free job's entry is chosen in the free order (`Archive::select_free`: E012's
//!   order with the cell's free counts).
//! * **A free try** (`explore`, called by the try after its return, reading, T and wait): at the entry and at each of
//!   the wait's stops, the interface's inputs (E012's affordance step, with A's enumeration and B's verdicts and dedupe
//!   when those are on), each trial followed by the wait's stops; the inputs that consumed more random draws than the
//!   no-input twin tried again 1, 2, 3, 5, 8, 13 and 21 frames later; one more level of offers from each outcome kept.
//!   Kept by the try's novelty (IW(1) within the try), committed as entries with "how": "free".
//!
//! Every kept entry is the entry's movie and the trial's clicks, a real run: it verifies from boot like any entry (A0).
//! Nothing is pruned: what is not kept can be made again from the same entry.

use crate::archive::Archive;
use crate::files::Res;
use serde_json::Value;
use std::sync::OnceLock;

/// The switch: FMI_FREE=ρ0, the standing share of free tries.
pub const SWITCH: &str = "FMI_FREE";
/// Stall's switch for its two rules (the share's shift and the cell rule), read here where the kind is decided.
pub const SHIFT_SWITCH: &str = "FMI_FREE_SHIFT";

fn rho0_text() -> Option<String> {
    std::env::var(SWITCH).ok().filter(|v| !v.trim().is_empty())
}

/// ρ0 when FMI_FREE holds a share in (0, 1]; None when it is unset, empty or not such a share.
pub fn rho0() -> Option<f64> {
    let x: f64 = rho0_text()?.trim().parse().ok()?;
    (x > 0.0 && x <= 1.0).then_some(x)
}

/// FMI_FREE_SHIFT=1 (stall's rules decide with the share).
pub fn shift_on() -> bool {
    std::env::var(SHIFT_SWITCH).is_ok_and(|v| v == "1")
}

/// The driver's check at its start: with FMI_FREE set, a share in (0, 1], the progress order (FMI_SELECT=progress),
/// and stall's port when FMI_FREE_SHIFT=1. With FMI_FREE unset there is nothing to check.
pub fn check_env(a: &Archive) -> Res<()> {
    let Some(t) = rho0_text() else { return Ok(()) };
    if rho0().is_none() {
        return Err(format!("{SWITCH}={t}: not a share in (0, 1]"));
    }
    if a.progress.is_none() {
        return Err(format!("{SWITCH}={t}: the free order needs the progress order (FMI_SELECT=progress)"));
    }
    if shift_on() && shift().is_none() {
        return Err(format!("{SHIFT_SWITCH}=1: stall's rules are not wired (free::set_shift)"));
    }
    Ok(())
}

/// A job's kind as the driver decides it, and why: "share" (the share's sequence) or "cell" (stall's cell rule).
#[derive(Clone, Debug, PartialEq)]
pub struct Kind {
    pub free: bool,
    pub why: &'static str,
    pub rho: f64,
}

impl Kind {
    pub fn text(&self) -> &'static str {
        if self.free {
            "free"
        } else {
            "model"
        }
    }
}

/// Job n is free under share ρ when floor((n + 1)·ρ) > floor(n·ρ) (a tolerance of 10⁻⁹ keeps shares like 0.3 exact).
pub fn is_free_job(n: u64, rho: f64) -> bool {
    let fl = |x: u64| ((x as f64) * rho + 1e-9).floor() as i64;
    fl(n + 1) > fl(n)
}

/// A job's number: j00003 is 3.
pub fn job_number(name: &str) -> Option<u64> {
    name.strip_prefix('j')?.parse().ok()
}

/// job.json says the job is free.
pub fn job_is_free(job: &Value) -> bool {
    job.get("kind").and_then(|k| k.as_str()) == Some("free")
}

/// A committed log line is a free try's.
pub fn line_is_free(x: &Value) -> bool {
    x.get("kind").and_then(|k| k.as_str()) == Some("free")
}

/// Stall's two rules (component `stall`: `share(&Stalls, ρ0)` and `free_cell(&Stalls, cell)` over the archive's files),
/// as the kind reads them. Wired by the integration; with FMI_FREE_SHIFT unset they are never asked.
pub trait Shift: Send + Sync {
    /// The current share of free tries.
    fn share(&self, a: &Archive, rho0: f64) -> Result<f64, String>;
    /// The cell's last two model tries both ended non-productive in the same class: its next try is free.
    fn free_cell(&self, a: &Archive, cell: &str) -> Result<bool, String>;
}

static SHIFT: OnceLock<Box<dyn Shift>> = OnceLock::new();

/// Wires stall's rules (once per process). False when they were wired already.
pub fn set_shift(s: Box<dyn Shift>) -> bool {
    SHIFT.set(s).is_ok()
}

pub fn shift() -> Option<&'static dyn Shift> {
    SHIFT.get().map(|b| &**b)
}

/// The driver's next job: its entry and kind (None: no cell is selectable). FMI_FREE unset: E012's selection and no
/// kind.
pub fn pick(a: &Archive) -> Res<Option<(usize, Option<Kind>)>> {
    pick_with(a, rho0(), if shift_on() { shift() } else { None })
}

/// `pick` with ρ0 and stall's rules given.
pub fn pick_with(a: &Archive, rho0: Option<f64>, sh: Option<&dyn Shift>) -> Res<Option<(usize, Option<Kind>)>> {
    let Some(r0) = rho0 else { return Ok(a.select(1).first().map(|&i| (i, None))) };
    let n = job_number(&a.next_job()).unwrap_or(0);
    let rho = match sh {
        Some(s) => s.share(a, r0)?,
        None => r0,
    };
    if is_free_job(n, rho) {
        return Ok(a.select_free(1).first().map(|&i| (i, Some(Kind { free: true, why: "share", rho }))));
    }
    let Some(&i) = a.select(1).first() else { return Ok(None) };
    if let Some(s) = sh {
        if s.free_cell(a, &a.rows[i].cell)? {
            return Ok(Some((i, Some(Kind { free: true, why: "cell", rho }))));
        }
    }
    Ok(Some((i, Some(Kind { free: false, why: "share", rho }))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_kind_sequence() {
        let free: Vec<u64> = (0..16).filter(|&n| is_free_job(n, 0.25)).collect();
        assert_eq!(free, vec![3, 7, 11, 15]);
        assert!((0..80).all(|n| is_free_job(n, 1.0)));
        let half: Vec<u64> = (0..8).filter(|&n| is_free_job(n, 0.5)).collect();
        assert_eq!(half, vec![1, 3, 5, 7]);
        // a share whose products are not exact in binary: 3 in every 10
        let n3 = (0..100).filter(|&n| is_free_job(n, 0.3)).count();
        assert_eq!(n3, 30);
        assert_eq!((0..10).filter(|&n| is_free_job(n, 0.3)).collect::<Vec<_>>(), vec![3, 6, 9]);
        // over any run the count of free jobs is floor(N·ρ)
        for rho in [0.1, 0.2, 0.25, 1.0 / 3.0, 0.4, 0.5, 0.75] {
            for big in [7u64, 20, 80, 600] {
                let n = (0..big).filter(|&k| is_free_job(k, rho)).count() as i64;
                assert_eq!(n, ((big as f64) * rho + 1e-9).floor() as i64, "rho {rho} N {big}");
            }
        }
    }

    #[test]
    fn job_numbers_and_kinds() {
        assert_eq!(job_number("j00003"), Some(3));
        assert_eq!(job_number("j01234"), Some(1234));
        assert_eq!(job_number("x"), None);
        assert!(job_is_free(&json!({"kind": "free"})));
        assert!(!job_is_free(&json!({"kind": "model"})));
        assert!(!job_is_free(&json!({})));
        assert!(line_is_free(&json!({"kind": "free", "outcome": "ok"})));
        assert_eq!(Kind { free: true, why: "share", rho: 0.25 }.text(), "free");
        assert_eq!(Kind { free: false, why: "share", rho: 0.25 }.text(), "model");
    }
}
