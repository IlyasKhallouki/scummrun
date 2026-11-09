//! E024 Part 1 (experiments/E024-shorten/plan.md, `FMI_READY_END`): a ready window whose tries stopped with none chosen
//! chooses nothing in the frames left, so it may end at the stop.
//!
//! The fixture is delay_fixture's parent loop with E010's rule (no delay fork) and a small bound on failed tries: a try
//! forked at g lands its first input at g + 1 and its outcome is the table's there; the tries end after a random
//! number of the parent's frames, in any order. At the first frame where `Ready::spent` holds, every try after the
//! cursor is cancelled; the window run on to its frame bound (the old end) forks no try after that frame and chooses
//! nothing, as the window ended there does.

use super::*;

/// xorshift64*, as delay_fixture's.
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
}

struct Out {
    /// the first frame where the tries were spent
    spent_at: Option<u64>,
    /// the last try's fork frame
    last_fork: Option<u64>,
    chosen: Option<usize>,
}

/// The ready window's parent loop over `outcome` (by the input's frame - g0), run to its frame bound (not ended at the
/// stop); at the first frame the tries are spent, every try after the cursor must be cancelled.
fn play(outcome: &[&'static str], frames: u64, workers: usize, sequential: bool, max_failed: u32, gen: &mut Gen) -> Out {
    let g0 = 1000u64;
    let board = Board::new();
    let ff: Mutex<FxHashSet<u64>> = Mutex::new(FxHashSet::default());
    let mut rd = Ready {
        workers,
        max_failed,
        limit: 1,
        cursor: 0,
        next: None,
        failed: 0,
        chosen: None,
        stopped: false,
        order: vec![],
        cancelled: 0,
        parallel_from: None,
        delay: None,
        sequential,
    };
    let mut children: Vec<Option<Child>> = vec![];
    let mut child_g: Vec<u64> = vec![];
    let mut flight: Vec<(usize, u64)> = vec![];
    let finish = |c: usize, children: &mut Vec<Option<Child>>, child_g: &[u64], board: &Board, ff: &Mutex<FxHashSet<u64>>| {
        let g = child_g[c];
        let mut ch = Child::new(g, 0, false, vec![], Arc::new(View::new()));
        if board.cancelled(c) {
            ch.outcome = "cancelled".into();
        } else {
            let f = g + 1;
            ch.first = Some(f);
            ch.outcome = if ff.lock().unwrap().contains(&f) {
                "duplicate".into()
            } else {
                outcome[(f - g0) as usize].to_string()
            };
        }
        children[c] = Some(ch);
    };
    let mut out = Out { spent_at: None, last_fork: None, chosen: None };
    let mut g = g0;
    loop {
        let mut due: Vec<usize> = flight.iter().filter(|x| x.1 <= g).map(|x| x.0).collect();
        flight.retain(|x| x.1 > g);
        while !due.is_empty() {
            let c = due.swap_remove(gen.below(due.len() as u64) as usize);
            finish(c, &mut children, &child_g, &board, &ff);
        }
        if g - g0 >= frames {
            break;
        }
        resolve_tries(&mut rd, &board, &ff, None, &children, &child_g);
        while rd.may_fork(g) && (child_g.len() - rd.cursor >= rd.limit || flight.len() >= rd.workers) {
            let (c, _) = flight.swap_remove(gen.below(flight.len() as u64) as usize);
            finish(c, &mut children, &child_g, &board, &ff);
            resolve_tries(&mut rd, &board, &ff, None, &children, &child_g);
        }
        if rd.chosen.is_some() {
            assert!(!rd.spent(), "spent with a try chosen");
            break;
        }
        if rd.spent() && out.spent_at.is_none() {
            out.spent_at = Some(g);
            for c in rd.cursor..child_g.len() {
                assert!(board.cancelled(c), "try {c} after the cursor {} not cancelled at the stop", rd.cursor);
            }
        }
        if rd.may_fork(g) {
            let c = board.add(0);
            children.push(None);
            child_g.push(g);
            flight.push((c, g + gen.below(6)));
            out.last_fork = Some(g);
        }
        g += 1;
    }
    for (c, _) in std::mem::take(&mut flight) {
        finish(c, &mut children, &child_g, &board, &ff);
    }
    resolve_tries(&mut rd, &board, &ff, None, &children, &child_g);
    out.chosen = rd.chosen;
    out
}

#[test]
fn ready_end_spent_tries_choose_nothing_after() {
    let mut gen = Gen(0x5851_F42D_4C95_7F2D);
    let fails = ["mismatch", "refuted", "spent", "interface"];
    let (mut spent, mut chosen) = (0, 0);
    for i in 0..3000 {
        let frames = 20 + gen.below(120);
        let p_end = gen.below(4);
        let outcome: Vec<&'static str> = (0..frames + 64)
            .map(|_| {
                if gen.below(100) < p_end {
                    ["success", "goal", "deviation"][gen.below(3) as usize]
                } else {
                    fails[gen.below(4) as usize]
                }
            })
            .collect();
        let workers = 1 + gen.below(3) as usize;
        let sequential = gen.below(2) == 0;
        let max_failed = 1 + gen.below(12) as u32;
        let o = play(&outcome, frames, workers, sequential, max_failed, &mut gen);
        if let Some(s) = o.spent_at {
            spent += 1;
            assert_eq!(o.chosen, None, "table {i}: a try chosen after the tries were spent at {s}");
            assert!(o.last_fork.is_none_or(|f| f < s), "table {i}: a try forked at {:?} after the stop at {s}", o.last_fork);
        }
        chosen += o.chosen.is_some() as u32;
    }
    eprintln!("E024 ready end: {spent} tables spent, {chosen} with a try chosen");
    assert!(spent > 500 && chosen > 200, "{spent} {chosen}");
}

#[test]
fn ready_end_waits_for_the_delay_fallback() {
    // the delay fork's fallback (its first deviation) is chosen at the window's end: the tries are not spent
    let mut rd = Ready {
        workers: 1,
        max_failed: 1,
        limit: 1,
        cursor: 1,
        next: None,
        failed: 1,
        chosen: None,
        stopped: true,
        order: vec![],
        cancelled: 0,
        parallel_from: None,
        delay: Some(delay::Fork::new(delay::forced(0.5))),
        sequential: true,
    };
    assert!(rd.spent());
    rd.delay.as_mut().unwrap().fallback = Some(0);
    assert!(!rd.spent());
    rd.delay = None;
    rd.chosen = Some(0);
    assert!(!rd.spent());
}
