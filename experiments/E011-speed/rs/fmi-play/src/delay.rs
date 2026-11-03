//! E012's delay fork (experiments/E012-game-mechanics/plan.md, method (2), and its amendment "component delay").
//!
//! In E010's play when ready, a window whose records check a random draw takes, among its tries in fork order, the
//! first one that matches the plan's branch (success, or the goal from the plan's records). A try's delay k is its
//! first input frame minus the first issuing try's. Deviations are not taken while k < K(p); when no try with k < K
//! can still match, the branch is refuted at this entry: the first deviation is taken (E010's choice), or the window
//! ends empty. The other tries' end states are reported as candidate entries, by a digest of their facts.
//!
//! Also: the random calls per frame, counted exactly from one Status `rng` to the next (ScummVM's RandomSource:
//! xorshift32 13/21/11 per call, `common/random.cpp`).

use crate::pyj::J;
use crate::reading::Check;
use crate::recs::MRec;
use crate::replay;
use fmi_core::*;
use rustc_hash::FxHashSet;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The switch: `FMI_DELAY_FORK=1` (off by default).
pub fn on() -> bool {
    std::env::var("FMI_DELAY_FORK").map(|v| v == "1").unwrap_or(false)
}

/// The draws of a reading's action (its index, `Action.what`), as `tested`'s `draws` gives them.
pub type DrawsOf = Arc<dyn Fn(&dyn ReadingSource, u32) -> Vec<Draw> + Send + Sync>;
/// Archive novelty of an end state's facts (over T and rooms), when the try knows it.
pub type NovelOf = Arc<dyn Fn(&Facts) -> Option<bool> + Send + Sync>;

// --- the bound ------------------------------------------------------------------------------------------------------

/// The chance the bound leaves of missing the plan's branch when it is there.
pub const MISS: f64 = 0.01;

/// K(p) = ceil(ln MISS / ln(1 − p)): the delays to try so that a branch of share p is missed with chance ≤ MISS when
/// the successive draws are uniform. None for p ≤ 0 (or not a number); 1 for p ≥ 1.
pub fn k_max(p: f64) -> Option<u64> {
    if p.is_nan() || p <= 0.0 {
        return None;
    }
    if p >= 1.0 {
        return Some(1);
    }
    let x = MISS.ln() / (1.0 - p).ln();
    Some(((x - 1e-9).ceil() as u64).max(1))
}

// --- the random source ----------------------------------------------------------------------------------------------

/// The random state after the boot with seed 1 (the root's, plan.md R2).
pub const RNG_ROOT: u32 = 3_182_679_328;

/// RandomSource::scrambleSeed: one call's step.
pub fn rng_step(s: u32) -> u32 {
    let mut s = s;
    s ^= s >> 13;
    s ^= s << 21;
    s ^= s >> 11;
    s
}

/// RandomSource::getRandomNumber(max)'s value from the state after its step.
pub fn rng_out(s: u32, max: u32) -> u32 {
    let x = s.wrapping_mul(0xDEAD_BF03);
    if max == u32::MAX {
        x
    } else {
        x % (max + 1)
    }
}

/// The calls between two states: the n ≤ cap with n steps from a giving b.
pub fn rng_calls(a: u32, b: u32, cap: u32) -> Option<u32> {
    let mut s = a;
    for n in 0..=cap {
        if s == b {
            return Some(n);
        }
        s = rng_step(s);
    }
    None
}

/// The most calls looked for between two frames (M1: at most 10 measured; a room's start draws more).
pub const RNG_CAP: u32 = 100_000;

/// Random calls per frame over a run of frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RngCount {
    pub frames: u64,
    pub calls: u64,
    pub least: Option<u32>,
    pub most: Option<u32>,
    /// frames whose calls were not found within RNG_CAP steps (a reseed)
    pub unknown: u64,
    pub hist: BTreeMap<u32, u64>,
}

impl RngCount {
    /// One frame from state `a` to state `b` (the Status `rng` fields).
    pub fn add(&mut self, a: u64, b: u64) {
        self.frames += 1;
        match rng_calls(a as u32, b as u32, RNG_CAP) {
            Some(n) => {
                self.calls += n as u64;
                self.least = Some(self.least.map_or(n, |x| x.min(n)));
                self.most = Some(self.most.map_or(n, |x| x.max(n)));
                *self.hist.entry(n).or_default() += 1;
            }
            None => self.unknown += 1,
        }
    }
    pub fn merge(&mut self, o: &RngCount) {
        self.frames += o.frames;
        self.calls += o.calls;
        self.unknown += o.unknown;
        for x in [o.least, o.most].into_iter().flatten() {
            self.least = Some(self.least.map_or(x, |y| y.min(x)));
            self.most = Some(self.most.map_or(x, |y| y.max(x)));
        }
        for (k, v) in &o.hist {
            *self.hist.entry(*k).or_default() += v;
        }
    }
    pub fn j(&self) -> J {
        crate::jobj! {
            "frames" => self.frames, "calls" => self.calls, "least" => self.least, "most" => self.most,
            "unknown" => self.unknown,
            "hist" => J::Arr(self.hist.iter().map(|(k, v)| J::Arr(vec![J::Int(*k as i64), J::Int(*v as i64)])).collect()),
        }
    }
}

// --- the window's draws ---------------------------------------------------------------------------------------------

/// A check whose reason is a random draw (the reader's Reason::Random).
pub fn is_draw(c: &Check) -> bool {
    replay::why_reason(&c.why) == "random draw"
}

/// The draws of a record's checks with no range and no share (when no `DrawsOf` is given).
pub fn draws_in_checks(checks: &[Check]) -> Vec<Draw> {
    checks
        .iter()
        .enumerate()
        .filter(|(_, c)| is_draw(c))
        .map(|(i, _)| Draw {
            check: i,
            range: None,
            share: None,
        })
        .collect()
}

/// A draw's p: its share, else one value of its range, else unknown.
pub fn draw_p(d: &Draw) -> Option<f64> {
    if let Some(s) = d.share {
        return Some(s);
    }
    d.range.and_then(|(lo, hi)| {
        let n = hi as i64 - lo as i64 + 1;
        (n >= 1).then(|| 1.0 / n as f64)
    })
}

/// A window's draws: their rows for the record, the window's p and K.
#[derive(Clone, Debug, PartialEq)]
pub struct WinDraws {
    pub rows: Vec<J>,
    pub p: Option<f64>,
    pub k_max: Option<u64>,
    pub forced: bool,
}

/// The draws of the window's members (each one's paths): None when no member checks a draw. p: per path the product
/// of its draws' p; per record the largest over its paths (1 for a path with no draw); the product over the records.
pub fn window_draws(
    recs: &[MRec],
    members: &[usize],
    src: &dyn ReadingSource,
    draws_of: Option<&DrawsOf>,
) -> Option<WinDraws> {
    let mut rows = Vec::new();
    let mut p_win: Option<f64> = Some(1.0);
    for &j in members {
        let r = &recs[j];
        let mut p_rec: Option<f64> = None;
        let mut free = false;
        let mut unknown = false;
        for (path, rec) in r.paths.iter().zip(&r.whats) {
            let ds = match draws_of {
                Some(f) => f(src, path.what),
                None => draws_in_checks(&rec.checks),
            };
            if ds.is_empty() {
                free = true;
                continue;
            }
            let mut p_path = Some(1.0);
            for d in &ds {
                let c = rec.checks.get(d.check);
                rows.push(J::Arr(vec![
                    J::Str(r.name.clone()),
                    J::Int(d.check as i64),
                    c.map(|c| J::Str(c.cite.clone())).unwrap_or(J::Null),
                    c.and_then(|c| c.side).map(J::Bool).unwrap_or(J::Null),
                    d.range
                        .map(|(a, b)| J::Arr(vec![J::Int(a as i64), J::Int(b as i64)]))
                        .unwrap_or(J::Null),
                    d.share.map(J::Float).unwrap_or(J::Null),
                ]));
                p_path = match (p_path, draw_p(d)) {
                    (Some(a), Some(b)) => Some(a * b),
                    _ => None,
                };
            }
            match p_path {
                Some(x) => p_rec = Some(p_rec.map_or(x, |y: f64| y.max(x))),
                None => unknown = true,
            }
        }
        let p_rec = if free {
            Some(1.0)
        } else if unknown {
            None
        } else {
            p_rec
        };
        p_win = match (p_win, p_rec) {
            (Some(a), Some(b)) => Some(a * b),
            _ => None,
        };
    }
    if rows.is_empty() {
        return None;
    }
    Some(WinDraws {
        rows,
        p: p_win,
        k_max: p_win.and_then(k_max),
        forced: false,
    })
}

/// The test setting `Search.delay_force_p`: every input window is a fork with this p (no run sets it).
pub fn forced(p: f64) -> WinDraws {
    WinDraws {
        rows: vec![],
        p: Some(p),
        k_max: k_max(p),
        forced: true,
    }
}

// --- the fork -------------------------------------------------------------------------------------------------------

/// A try that got a k.
#[derive(Clone, Debug, PartialEq)]
pub struct Tried {
    /// the child's index in the window's fork order
    pub c: usize,
    pub k: u64,
    pub outcome: String,
    pub digest: Option<String>,
    pub novel: Option<bool>,
}

/// What the ready rule does with a resolved try.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Take {
    /// it matches the plan's branch: chosen
    Choose,
    /// not chosen; the next try is forked as E010 forks after it
    Next,
    /// its k is past the bound: not counted
    Beyond,
}

/// One window's delay fork.
#[derive(Clone, Debug, PartialEq)]
pub struct Fork {
    pub draws: WinDraws,
    /// the first issuing try's first input frame (k = 0)
    pub first: Option<u64>,
    pub tries: Vec<Tried>,
    /// the chosen try's k
    pub chosen: Option<u64>,
    /// the first deviation (child index): E010's choice when the branch is refuted
    pub fallback: Option<usize>,
    pub refuted: bool,
    pub beyond: u32,
    pub candidates: Vec<J>,
}

impl Fork {
    pub fn new(draws: WinDraws) -> Fork {
        Fork {
            draws,
            first: None,
            tries: vec![],
            chosen: None,
            fallback: None,
            refuted: false,
            beyond: 0,
            candidates: vec![],
        }
    }

    pub fn k_max(&self) -> Option<u64> {
        self.draws.k_max
    }

    /// Whether a try forked at frame g can still have k < K (its first input comes at g + 1 or later).
    pub fn open_at(&self, g: u64) -> bool {
        match (self.first, self.k_max()) {
            (Some(f0), Some(kk)) => g + 1 < f0 + kk,
            _ => true,
        }
    }

    /// A try resolved in fork order: child `c`, its first input frame, its outcome, its end state's digest and novelty.
    pub fn take(&mut self, c: usize, first: Option<u64>, outcome: &str, digest: Option<String>, novel: Option<bool>) -> Take {
        let Some(f1) = first else { return Take::Next };
        if outcome == "duplicate" || outcome == "cancelled" {
            return Take::Next;
        }
        let f0 = *self.first.get_or_insert(f1);
        let k = f1.saturating_sub(f0);
        if self.k_max().is_some_and(|kk| k >= kk) {
            self.beyond += 1;
            return Take::Beyond;
        }
        // a try whose input landed where an earlier one's did is the same run (Lemma 5), resolved as that one was
        if self.tries.iter().any(|t| t.k == k) {
            return Take::Next;
        }
        self.tries.push(Tried {
            c,
            k,
            outcome: outcome.to_string(),
            digest,
            novel,
        });
        if outcome == "success" || outcome == "goal" {
            self.chosen = Some(k);
            return Take::Choose;
        }
        if outcome == "deviation" && self.fallback.is_none() {
            self.fallback = Some(c);
        }
        Take::Next
    }

    /// No try left can have k < K: the branch is refuted. Returns the try to choose (the first deviation), if any.
    pub fn refute(&mut self) -> Option<usize> {
        self.refuted = true;
        self.fallback
    }

    pub fn j(&self) -> J {
        crate::jobj! {
            "forced" => self.draws.forced,
            "draws" => J::Arr(self.draws.rows.clone()),
            "p" => self.draws.p.map(J::Float).unwrap_or(J::Null),
            "k_max" => self.draws.k_max,
            "first" => self.first,
            "chosen" => self.chosen,
            "outcomes" => J::Arr(self.tries.iter().map(|t| J::Arr(vec![
                J::Int(t.k as i64),
                t.digest.clone().map(J::Str).unwrap_or(J::Null),
                t.novel.map(J::Bool).unwrap_or(J::Null),
                J::Str(t.outcome.clone()),
            ])).collect()),
            "beyond" => self.beyond,
            "refuted" => self.refuted,
            "candidates" => J::Arr(self.candidates.clone()),
        }
    }
}

// --- end states -----------------------------------------------------------------------------------------------------

/// The digest of an end state: sha1 of its facts on `keys`, in key order (12 hex digits).
pub fn digest(facts: &Facts, keys: &FxHashSet<Key>) -> String {
    let mut kv: Vec<(Key, Val)> = facts.iter().filter(|(k, _)| keys.contains(k)).copied().collect();
    kv.sort();
    let mut h = sha1_smol::Sha1::new();
    for (k, v) in kv {
        h.update(k.text().as_bytes());
        h.update(b"=");
        h.update(&v.to_le_bytes());
    }
    h.digest().to_string()[..12].to_string()
}

/// A fact's value as the archive's files write it (start.val_json).
pub fn val_j(v: Val) -> J {
    match v {
        BELOW => J::Str("below".into()),
        ABOVE => J::Str("above".into()),
        NONE => J::Null,
        n => J::Int(n),
    }
}

/// A checkpoint as the archive's entries hold it: [frame, facts (a map by key text), {"tick", "rng", "actors"}].
pub fn checkpoint_j(c: &Checkpoint) -> J {
    let mut kv: Vec<(String, Val)> = c.facts.iter().map(|(k, v)| (k.text(), *v)).collect();
    kv.sort();
    J::Arr(vec![
        J::Int(c.frame as i64),
        J::Obj(kv.into_iter().map(|(k, v)| (k, val_j(v))).collect()),
        crate::jobj! {
            "tick" => c.tick, "rng" => c.rng,
            "actors" => J::Arr(c.actors.iter().map(|a| J::Arr(vec![J::Int(a.0 as i64), J::Int(a.1 as i64), J::Int(a.2 as i64)])).collect()),
        },
    ])
}

/// A candidate entry: a try of a delay fork other than the chosen one, with a new outcome.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub plan: u32,
    pub window: usize,
    pub k: u64,
    pub outcome: String,
    pub digest: String,
    pub novel: Option<bool>,
    /// the attempt's inputs up to the window, then the try's own (absolute frames)
    pub inputs: Vec<Event>,
    pub checkpoint: Checkpoint,
}

impl Candidate {
    pub fn j(&self) -> J {
        crate::jobj! {
            "k" => self.k, "outcome" => self.outcome.clone(), "frame" => self.checkpoint.frame,
            "digest" => self.digest.clone(), "novel" => self.novel,
            "inputs" => J::Arr(self.inputs.iter().map(replay::event_j).collect()),
            "checkpoint" => checkpoint_j(&self.checkpoint),
        }
    }
}

/// Per try (the search's summary): the windows' forks and the calls per frame.
#[derive(Clone, Debug, Default)]
pub struct Log {
    pub windows: u64,
    pub triggered: u64,
    pub tries: u64,
    pub chosen: Vec<u64>,
    pub refuted: u64,
    pub unbounded: u64,
    pub candidates: u64,
    pub rng: RngCount,
}

impl Log {
    pub fn j(&self) -> J {
        crate::jobj! {
            "windows" => self.windows, "triggered" => self.triggered, "tries" => self.tries,
            "chosen" => J::Arr(self.chosen.iter().map(|&k| J::Int(k as i64)).collect()),
            "refuted" => self.refuted, "unbounded" => self.unbounded, "candidates" => self.candidates,
            "rng" => self.rng.j(),
        }
    }
}

// --- gate R2: the successive outputs are uniform ---------------------------------------------------------------------

/// ln Γ(x) for x > 0 (Lanczos, g = 7, 9 terms).
fn ln_gamma(x: f64) -> f64 {
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = C[0];
    let t = x + 7.5;
    for (i, c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// The regularized upper incomplete gamma Q(a, x) (series below a + 1, continued fraction above).
fn gamma_q(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    let gln = ln_gamma(a);
    if x < a + 1.0 {
        let (mut ap, mut sum) = (a, 1.0 / a);
        let mut del = sum;
        for _ in 0..10_000 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-15 {
                break;
            }
        }
        1.0 - sum * (-x + a * x.ln() - gln).exp()
    } else {
        let tiny = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / tiny;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..10_000 {
            let an = -(i as f64) * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < tiny {
                d = tiny;
            }
            c = b + an / c;
            if c.abs() < tiny {
                c = tiny;
            }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < 1e-15 {
                break;
            }
        }
        (-x + a * x.ln() - gln).exp() * h
    }
}

/// The p-value of Pearson's χ² of `counts` against the uniform distribution over them.
pub fn chi2_uniform_p(counts: &[u64]) -> (f64, f64) {
    let n: u64 = counts.iter().sum();
    let k = counts.len();
    if k < 2 || n == 0 {
        return (0.0, 1.0);
    }
    let e = n as f64 / k as f64;
    let chi2: f64 = counts.iter().map(|&c| (c as f64 - e).powi(2) / e).sum();
    (chi2, gamma_q((k - 1) as f64 / 2.0, chi2 / 2.0))
}

/// Gate R2 at one call stride: from `s0`, `samples` outputs taken every `stride` calls; for each n in 1..=n_max the
/// χ² p-value of the outputs `(s · 0xDEADBF03) mod n` (getRandomNumber(n − 1)) against the uniform.
pub fn r2(s0: u32, samples: usize, stride: u32, n_max: u32) -> Vec<(u32, f64, f64)> {
    let mut outs: Vec<u32> = Vec::with_capacity(samples);
    let mut s = s0;
    for _ in 0..samples {
        for _ in 0..stride {
            s = rng_step(s);
        }
        outs.push(s.wrapping_mul(0xDEAD_BF03));
    }
    (1..=n_max)
        .map(|n| {
            let mut counts = vec![0u64; n as usize];
            for &x in &outs {
                counts[(x % n) as usize] += 1;
            }
            let (c, p) = chi2_uniform_p(&counts);
            (n, c, p)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn k_examples() {
        // plan.md: K = 7 for p = 1/2, 17 for 1/4, 95 for 1/21
        assert_eq!(k_max(0.5), Some(7));
        assert_eq!(k_max(0.25), Some(17));
        assert_eq!(k_max(1.0 / 21.0), Some(95));
        assert_eq!(k_max(0.9), Some(2));
        assert_eq!(k_max(1.0), Some(1));
        assert_eq!(k_max(0.0), None);
        assert_eq!(k_max(f64::NAN), None);
        for p in [0.5, 0.25, 1.0 / 21.0, 0.1, 0.9, 0.99, 1.0 / 233.0] {
            let k = k_max(p).unwrap() as i32;
            assert!((1.0 - p).powi(k) <= MISS + 1e-12, "{p}");
            assert!(k == 1 || (1.0 - p).powi(k - 1) > MISS, "{p}");
        }
    }

    #[test]
    fn rng_steps() {
        // xorshift32 13/21/11 never reaches 0 from a nonzero state, and the count is the steps
        let mut s = RNG_ROOT;
        for n in 0..50u32 {
            assert_eq!(rng_calls(RNG_ROOT, s, 100), Some(n));
            s = rng_step(s);
            assert_ne!(s, 0);
        }
        assert_eq!(rng_calls(RNG_ROOT, rng_step(RNG_ROOT), 0), None);
        let mut c = RngCount::default();
        let (a, b, d) = (RNG_ROOT, rng_step(RNG_ROOT), rng_step(rng_step(rng_step(RNG_ROOT))));
        c.add(a as u64, b as u64);
        c.add(b as u64, d as u64);
        c.add(d as u64, d as u64);
        assert_eq!((c.frames, c.calls, c.least, c.most), (3, 3, Some(0), Some(2)));
        assert_eq!(c.hist.into_iter().collect::<Vec<_>>(), vec![(0, 1), (1, 1), (2, 1)]);
    }

    #[test]
    fn p_of_draws() {
        assert_eq!(draw_p(&Draw { check: 0, range: Some((0, 3)), share: None }), Some(0.25));
        assert_eq!(draw_p(&Draw { check: 0, range: Some((0, 3)), share: Some(0.75) }), Some(0.75));
        assert_eq!(draw_p(&Draw { check: 0, range: None, share: None }), None);
    }

    #[test]
    fn chi2_tails() {
        // the χ² distribution's 5% and 1% points (tables): (df, x, upper tail)
        for (df, x, q) in [
            (1usize, 3.841_459, 0.05),
            (1, 6.634_897, 0.01),
            (2, 5.991_465, 0.05),
            (10, 23.209_25, 0.01),
            (30, 50.892_2, 0.01),
            (50, 67.504_8, 0.05),
            (100, 124.342_1, 0.05),
        ] {
            let p = gamma_q(df as f64 / 2.0, x / 2.0);
            assert!((p - q).abs() < 2e-4, "df {df}: {p} for {q}");
        }
        assert!((ln_gamma(5.0) - 24f64.ln()).abs() < 1e-12);
        assert_eq!(chi2_uniform_p(&[10, 10, 10]).0, 0.0);
    }

    /// Gate R2 (computational): the outputs at successive calls from the root's state are uniform for every range.
    #[test]
    fn r2_uniform_stride_1() {
        let rows = r2(RNG_ROOT, 100_000, 1, 233);
        let fam = 0.01 / 232.0;
        let worst = rows.iter().skip(1).map(|r| r.2).fold(1.0, f64::min);
        assert!(worst >= fam, "least p {worst} under the family's {fam}");
    }

    /// Gate R2 in full: strides 1-10 (M1's calls per frame), printed (cargo test --release -- --ignored r2_table
    /// --nocapture).
    #[test]
    #[ignore]
    fn r2_table() {
        let fam = 0.01 / 232.0;
        let mut all_ok = true;
        for stride in 1..=10u32 {
            let rows = r2(RNG_ROOT, 100_000, stride, 233);
            let nt = &rows[1..];
            let below = nt.iter().filter(|r| r.2 < 0.01).count();
            let (wn, _, wp) = nt.iter().copied().fold((0, 0.0, 1.0), |a, r| if r.2 < a.2 { r } else { a });
            let ok = wp >= fam;
            all_ok &= ok;
            println!(
                "R2 stride {stride}: n = 2..233, {below} of 232 under 1% (expected 2.3), least p {wp:.5} at n = {wn}; family (Bonferroni 1%): {}",
                if ok { "pass" } else { "FAIL" }
            );
        }
        println!("R2: {}", if all_ok { "PASS" } else { "FAIL" });
        assert!(all_ok);
    }

    #[test]
    fn fork_rules() {
        let mut f = Fork::new(forced(0.5)); // K = 7
        assert!(f.open_at(1000));
        assert_eq!(f.take(0, None, "interface", None, None), Take::Next);
        assert_eq!(f.take(1, Some(101), "mismatch", None, None), Take::Next);
        assert_eq!(f.first, Some(101));
        assert!(f.open_at(106) && !f.open_at(107));
        assert_eq!(f.take(2, Some(101), "duplicate", None, None), Take::Next);
        assert_eq!(f.take(3, Some(102), "deviation", None, None), Take::Next);
        assert_eq!(f.fallback, Some(3));
        assert_eq!(f.take(4, Some(108), "success", None, None), Take::Beyond);
        assert_eq!(f.take(5, Some(104), "success", None, None), Take::Choose);
        assert_eq!(f.chosen, Some(3));
        assert_eq!(f.tries.iter().map(|t| t.k).collect::<Vec<_>>(), vec![0, 1, 3]);
    }
}
