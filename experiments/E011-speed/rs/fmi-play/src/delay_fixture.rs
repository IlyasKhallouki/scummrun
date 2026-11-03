//! Gate R3 (experiments/E012-game-mechanics/plan.md, amendment "component delay"): the window record's k equals the
//! least k < K whose try matches the plan's branch, on a fixture of forced branches.
//!
//! The fixture is the ready window's parent loop with the engine replaced by a table: a try forked at frame g lands
//! its first input at issue(g), the first issuable frame after g (the input code waits until it can issue), and its
//! outcome is the table's at that frame (Lemma 5: the run is fixed by where the input lands). The tries run "in
//! parallel": each ends after a random number of the parent's frames, in any order. The decisions are the window
//! search's own: `Ready`, `resolve_tries`, `Ready::delay_refute` and resolve_final's fallback, as window_strict calls
//! them.

use super::*;

/// A small deterministic generator (xorshift64*), so that the tables are the same on every run.
struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

/// One forced window: from g0, `frames` parent frames; gated parent frames; issuable input frames; the outcome at each
/// issuable frame; p (K).
struct Table {
    g0: u64,
    frames: u64,
    gated: Vec<bool>,
    issuable: Vec<bool>,
    outcome: Vec<&'static str>,
    p: f64,
    workers: usize,
}

impl Table {
    fn at(&self, f: u64) -> usize {
        (f - self.g0) as usize
    }
    /// The first issuable frame after g within the table (None: the input code never issues).
    fn issue(&self, g: u64) -> Option<u64> {
        (g + 1..self.g0 + self.frames + 64).find(|&f| self.issuable.get(self.at(f)).copied().unwrap_or(false))
    }
    /// The expected choice: over the issue frames E010's rule can reach (issue(g) for a gated g; skipping the frames
    /// between a failed try and its first input never loses a landing frame), the least k < K that matches.
    fn expected(&self) -> (Option<u64>, Option<u64>) {
        let gs: Vec<u64> = (self.g0..self.g0 + self.frames).filter(|&g| self.gated[self.at(g)]).collect();
        let Some(f0) = gs.iter().find_map(|&g| self.issue(g)) else { return (None, None) };
        let kk = delay::k_max(self.p).unwrap_or(u64::MAX);
        let mut lands: Vec<u64> = gs.iter().filter_map(|&g| self.issue(g)).collect();
        lands.sort();
        lands.dedup();
        let k = lands
            .iter()
            .filter(|&&f| f - f0 < kk)
            .find(|&&f| matches!(self.outcome[self.at(f)], "success" | "goal"))
            .map(|&f| f - f0);
        (Some(f0), k)
    }
}

struct Run {
    record: J,
    chosen_child: Option<usize>,
    children: Vec<Child>,
    end: &'static str,
}

/// The ready window's parent loop over a table (window_strict's ready section, the engine replaced by the table).
fn play(t: &Table, gen: &mut Gen) -> Run {
    let board = Board::new();
    let ff: Mutex<FxHashSet<u64>> = Mutex::new(FxHashSet::default());
    let dw = DelayW {
        keys: Arc::new([Key::var(1)].into_iter().collect()),
        novel_of: None,
    };
    let mut rd = Ready {
        workers: t.workers,
        max_failed: 294,
        limit: 1,
        cursor: 0,
        next: None,
        failed: 0,
        chosen: None,
        stopped: false,
        order: vec![],
        cancelled: 0,
        parallel_from: None,
        delay: Some(delay::Fork::new(delay::forced(t.p))),
    };
    let mut children: Vec<Option<Child>> = vec![];
    let mut child_g: Vec<u64> = vec![];
    // the tries in flight: (child, the parent frame it ends at)
    let mut flight: Vec<(usize, u64)> = vec![];
    let finish = |c: usize, children: &mut Vec<Option<Child>>, child_g: &[u64], board: &Board, ff: &Mutex<FxHashSet<u64>>| {
        let g = child_g[c];
        let mut ch = Child::new(g, 0, false, vec![], Arc::new(View::new()));
        if board.cancelled(c) {
            ch.outcome = "cancelled".into();
        } else {
            match t.issue(g) {
                None => {
                    ch.outcome = "interface".into();
                    ch.reached = Some(t.g0 + t.frames);
                }
                Some(f) => {
                    ch.first = Some(f);
                    // child_strict_: a try whose first input lands on a failed try's is the same run
                    if ff.lock().unwrap().contains(&f) {
                        ch.outcome = "duplicate".into();
                    } else {
                        ch.outcome = t.outcome[t.at(f)].to_string();
                        let cp = Checkpoint {
                            frame: f + 5,
                            facts: Facts::from_pairs(vec![(Key::var(1), f as i64)]),
                            tick: 0,
                            rng: 0,
                            actors: vec![],
                        };
                        if ch.is_end_point() {
                            ch.checkpoints = vec![cp];
                        } else {
                            ch.end_cp = Some(cp);
                        }
                    }
                }
            }
        }
        children[c] = Some(ch);
    };
    let mut end = "frames";
    let mut g = t.g0;
    loop {
        // the tries that end at this frame, in a random order (the threads)
        let mut due: Vec<usize> = flight.iter().filter(|x| x.1 <= g).map(|x| x.0).collect();
        flight.retain(|x| x.1 > g);
        while !due.is_empty() {
            let c = due.swap_remove(gen.below(due.len() as u64) as usize);
            finish(c, &mut children, &child_g, &board, &ff);
        }
        if g - t.g0 >= t.frames {
            break;
        }
        let gate = t.gated[t.at(g)];
        resolve_tries(&mut rd, &board, &ff, Some(&dw), &children, &child_g);
        while gate && rd.may_fork(g) && (child_g.len() - rd.cursor >= rd.limit || flight.len() >= rd.workers) {
            // wait: one try in flight ends
            assert!(!flight.is_empty(), "waiting with no try in flight");
            let i = gen.below(flight.len() as u64) as usize;
            let (c, _) = flight.swap_remove(i);
            finish(c, &mut children, &child_g, &board, &ff);
            resolve_tries(&mut rd, &board, &ff, Some(&dw), &children, &child_g);
        }
        if rd
            .delay
            .as_ref()
            .is_some_and(|d| d.first.is_some() && !d.open_at(g.max(rd.next.unwrap_or(0))))
        {
            while rd.chosen.is_none() && !rd.stopped && rd.cursor < child_g.len() {
                if flight.is_empty() {
                    let before = rd.cursor;
                    resolve_tries(&mut rd, &board, &ff, Some(&dw), &children, &child_g);
                    assert!(rd.cursor > before || rd.chosen.is_some() || rd.stopped, "a try neither in flight nor ended");
                    continue;
                }
                let i = gen.below(flight.len() as u64) as usize;
                let (c, _) = flight.swap_remove(i);
                finish(c, &mut children, &child_g, &board, &ff);
                resolve_tries(&mut rd, &board, &ff, Some(&dw), &children, &child_g);
            }
            if rd.delay_refute(g, child_g.len()) && rd.chosen.is_none() {
                end = "refuted";
                break;
            }
        }
        if rd.chosen.is_some() {
            end = "chosen";
            break;
        }
        if gate && rd.may_fork(g) {
            let c = board.add(0);
            children.push(None);
            child_g.push(g);
            flight.push((c, g + gen.below(6)));
        }
        g += 1;
    }
    // the window's end: the tries left end, then resolve_final
    for (c, _) in std::mem::take(&mut flight) {
        finish(c, &mut children, &child_g, &board, &ff);
    }
    resolve_tries(&mut rd, &board, &ff, Some(&dw), &children, &child_g);
    if rd.chosen.is_none() {
        if let Some(f) = rd.delay.as_ref().and_then(|d| d.fallback) {
            rd.chosen = Some(f);
        }
    }
    Run {
        record: rd.delay.as_ref().unwrap().j(),
        chosen_child: rd.chosen,
        children: children.into_iter().map(|c| c.unwrap()).collect(),
        end,
    }
}

const FAILS: [&str; 4] = ["mismatch", "refuted", "spent", "deviation"];

/// Amendment (2): with no K, the first end point among the landing frames (E010's choice); the record's k only for a
/// matching try.
fn check_unbounded(t: &Table, gen: &mut Gen) -> Result<(), String> {
    let gs: Vec<u64> = (t.g0..t.g0 + t.frames).filter(|&g| t.gated[t.at(g)]).collect();
    let mut lands: Vec<u64> = gs.iter().filter_map(|&g| t.issue(g)).collect();
    lands.sort();
    lands.dedup();
    let f0 = lands.first().copied();
    let end = lands
        .iter()
        .copied()
        .find(|&f| matches!(t.outcome[t.at(f)], "success" | "goal" | "deviation"));
    let r = play(t, gen);
    let got = r.chosen_child.and_then(|c| r.children[c].first);
    if got != end {
        return Err(format!("chosen first {got:?}, the first end point {end:?}"));
    }
    let want_k = end
        .filter(|&f| matches!(t.outcome[t.at(f)], "success" | "goal"))
        .map(|f| f - f0.unwrap());
    let rec_k = r.record.get("chosen").and_then(J::int).map(|x| x as u64);
    if rec_k != want_k || r.record.get("k_max") != Some(&J::Null) {
        return Err(format!("record chosen {rec_k:?} k_max {:?}, want {want_k:?}", r.record.get("k_max")));
    }
    Ok(())
}

/// Checks one table; returns a description of the difference, if any.
fn check(t: &Table, gen: &mut Gen) -> Result<(), String> {
    if delay::k_max(t.p).is_none() {
        return check_unbounded(t, gen);
    }
    let (f0, want) = t.expected();
    let r = play(t, gen);
    let got = r.record.get("chosen").and_then(J::int).map(|x| x as u64);
    if got != want {
        return Err(format!("chosen k {got:?}, the least matching k {want:?} (f0 {f0:?})"));
    }
    match want {
        Some(k) => {
            let c = r.chosen_child.ok_or("no child chosen")?;
            let ch = &r.children[c];
            if ch.first.map(|f| f - f0.unwrap()) != Some(k) || !matches!(ch.outcome.as_str(), "success" | "goal") {
                return Err(format!("the chosen child {c}: first {:?} outcome {}", ch.first, ch.outcome));
            }
        }
        None => {
            // no match within K: refuted (when the bound was reached inside the window), and the first deviation in
            // fork order is chosen, else nothing
            let kk = delay::k_max(t.p).unwrap();
            let reached = f0.is_some_and(|f0| f0 + kk <= t.g0 + t.frames);
            if reached && r.record.get("refuted") != Some(&J::Bool(true)) {
                return Err(format!("not refuted (end {})", r.end));
            }
            let dev = r
                .children
                .iter()
                .enumerate()
                .filter(|(_, ch)| ch.outcome == "deviation" && ch.first.is_some_and(|f| f < f0.unwrap_or(0) + kk))
                .map(|(i, _)| i)
                .next();
            if r.chosen_child != dev {
                return Err(format!("chosen child {:?}, the first deviation {:?}", r.chosen_child, dev));
            }
        }
    }
    Ok(())
}

#[test]
fn r3_every_table_up_to_six_tries() {
    // every outcome of k = 0..5 (gates open, the input lands at g + 1), for K = 7 and K = 3
    let mut gen = Gen(0x9E37_79B9_7F4A_7C15);
    let kinds = ["success", "deviation", "mismatch"];
    let mut n = 0;
    for p in [0.5, 0.9] {
        for code in 0..3usize.pow(6) {
            // the try forked at g0 lands at g0 + 1: k is the outcome's index minus 1
            let mut out = vec!["mismatch"; 120];
            let mut x = code;
            for k in 0..6 {
                out[k + 1] = kinds[x % 3];
                x /= 3;
            }
            let t = Table {
                g0: 1000,
                frames: 40,
                gated: vec![true; 120],
                issuable: vec![true; 120],
                outcome: out,
                p,
                workers: 3,
            };
            if let Err(e) = check(&t, &mut gen) {
                panic!("p {p}, table {code}: {e}");
            }
            n += 1;
        }
    }
    assert_eq!(n, 2 * 729);
}

#[test]
fn r3_random_tables() {
    let mut gen = Gen(0xD1B5_4A32_D192_ED03);
    let ps = [0.5, 0.25, 1.0 / 21.0, 0.9, 1.0 / 3.0];
    let (mut chosen, mut refuted, mut no_try) = (0, 0, 0);
    for i in 0..2000 {
        let frames = 30 + gen.below(200);
        let n = frames as usize + 80;
        let gate_den = 1 + gen.below(4);
        let issue_den = 1 + gen.below(3);
        let p_match = gen.below(25);
        let gated: Vec<bool> = (0..n).map(|_| gen.chance(gate_den.max(1) * 3 / 4 + 1, gate_den + 1)).collect();
        let issuable: Vec<bool> = (0..n).map(|_| gen.chance(issue_den, issue_den + 1)).collect();
        let outcome: Vec<&'static str> = (0..n)
            .map(|_| {
                if gen.below(100) < p_match {
                    if gen.chance(1, 10) {
                        "goal"
                    } else {
                        "success"
                    }
                } else {
                    FAILS[gen.below(4) as usize]
                }
            })
            .collect();
        let t = Table {
            g0: 5000 + gen.below(100),
            frames,
            gated,
            issuable,
            outcome,
            p: ps[gen.below(ps.len() as u64) as usize],
            workers: 1 + gen.below(3) as usize,
        };
        if let Err(e) = check(&t, &mut gen) {
            panic!("table {i}: {e}");
        }
        match t.expected() {
            (_, Some(_)) => chosen += 1,
            (Some(_), None) => refuted += 1,
            (None, None) => no_try += 1,
        }
    }
    eprintln!("R3 random tables: {chosen} chosen, {refuted} with no match within K, {no_try} with no try");
    assert!(chosen > 200 && refuted > 200, "{chosen} {refuted}");
}

#[test]
fn r3_unbounded_tables() {
    // amendment (2): draws with no share and no range (no K): the first end point, as E010 chooses
    let mut gen = Gen(0x3C6E_F372_FE94_F82B);
    let mut dev = 0;
    for i in 0..1000 {
        let frames = 30 + gen.below(150);
        let n = frames as usize + 80;
        let p_end = 1 + gen.below(15);
        let gated: Vec<bool> = (0..n).map(|_| gen.chance(3, 4)).collect();
        let issuable: Vec<bool> = (0..n).map(|_| gen.chance(2, 3)).collect();
        let outcome: Vec<&'static str> = (0..n)
            .map(|_| {
                if gen.below(100) < p_end {
                    ["success", "goal", "deviation"][gen.below(3) as usize]
                } else {
                    FAILS[gen.below(3) as usize]
                }
            })
            .collect();
        let t = Table {
            g0: 7000,
            frames,
            gated,
            issuable,
            outcome,
            p: 0.0,
            workers: 1 + gen.below(3) as usize,
        };
        if let Err(e) = check(&t, &mut gen) {
            panic!("table {i}: {e}");
        }
        let r = play(&t, &mut gen);
        dev += r.chosen_child.is_some_and(|c| r.children[c].outcome == "deviation") as u32;
    }
    assert!(dev > 50, "{dev}");
}
