//! The switch pass2_reuse (experiments/E004-fast-exact/pass2-reuse.md has the proof): pass 1 keeps each run's output with
//! what the run depended on; a run of pass 2 whose job, guides, widened facts, folded lists and reached values agree
//! takes that output without running. The files and counters are byte for byte those without the switch.

use crate::builder::{Builder, Deltas, Job, Machine};
use crate::domain::Labels;
use crate::reach::Guide;
use crate::record::{Pins, Record};
use crate::table::FactTable;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

/// One question a run asked about the reached values, with its answer.
#[derive(Clone, Debug)]
pub struct Query {
    pub key: String,
    pub set: Labels,
    pub before: bool,
    pub answer: bool,
}

/// What a run depended on beyond its job and guides.
#[derive(Clone, Debug, Default)]
pub struct RunLog {
    /// the facts whose kind and domain the machine half or the Builder consulted
    pub touched: FxHashSet<String>,
    /// list numbers asked about, and whether they were folded
    pub sigs: Vec<(i64, bool)>,
    pub queries: Vec<Query>,
}

struct MemoRun {
    job: Job,
    guides: Option<Arc<Vec<Guide>>>,
    log: RunLog,
    records: Vec<Record>,
    deltas: Deltas,
    last_pins: Pins,
}

#[derive(Default)]
pub struct RunMemo {
    runs: Mutex<FxHashMap<String, Vec<MemoRun>>>,
    reusing: AtomicBool,
    widened: RwLock<FxHashSet<String>>,
    pub reused: AtomicU64,
    pub stale_job: AtomicU64,
    pub stale_domain: AtomicU64,
    pub stale_list: AtomicU64,
    pub stale_reached: AtomicU64,
    pub none: AtomicU64,
}

impl RunMemo {
    pub fn reusing(&self) -> bool {
        self.reusing.load(Ordering::Relaxed)
    }

    /// Pass 1 is over: the keys it widened.
    pub fn start_reuse(&self, widened: impl IntoIterator<Item = String>) {
        *self.widened.write().unwrap() = widened.into_iter().collect();
        self.reusing.store(true, Ordering::Relaxed);
    }

    /// The outputs left go (nothing more will take them).
    pub fn clear(&self) {
        self.runs.lock().unwrap().clear();
    }

    /// Pass 1's run of the same job with the same guides (jobs of two rooms can share a label).
    fn take(&self, job: &Job, guides: &Option<Arc<Vec<Guide>>>) -> Option<MemoRun> {
        let mut runs = self.runs.lock().unwrap();
        let list = runs.get_mut(&job.label)?;
        let i = list.iter().position(|r| {
            r.job == *job
                && match (&r.guides, guides) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a == b,
                    _ => false,
                }
        })?;
        Some(list.swap_remove(i))
    }

    /// Whether a run's output holds now (pass2-reuse.md, items 1 to 5); which item failed is counted.
    fn holds(&self, run: &MemoRun, job: &Job, table: &FactTable, folded: &std::collections::BTreeSet<i64>) -> bool {
        if run.job != *job {
            self.stale_job.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        {
            let w = self.widened.read().unwrap();
            if !w.is_empty() && run.log.touched.iter().any(|k| w.contains(k)) {
                self.stale_domain.fetch_add(1, Ordering::Relaxed);
                return false;
            }
        }
        if run.log.sigs.iter().any(|(s, f)| folded.contains(s) != *f) {
            self.stale_list.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        if let Some(r) = &table.reach {
            if !r.answers_hold(table, &run.log.queries) {
                self.stale_reached.fetch_add(1, Ordering::Relaxed);
                return false;
            }
        }
        true
    }
}

impl Builder<'_> {
    /// `run_job` on a worker's Builder, whose counters hold only this job's when it starts.
    pub(crate) fn run_job_memo(&mut self, memo: &RunMemo, m: &mut dyn Machine, job: &Job) -> Vec<Record> {
        let guides = match (&self.table.reach, &self.cur_job_key) {
            (Some(r), Some(k)) => r.guides_of(k),
            _ => None,
        };
        if memo.reusing() {
            match memo.take(job, &guides) {
                Some(run) => {
                    if memo.holds(&run, job, &self.table, &self.folded) {
                        memo.reused.fetch_add(1, Ordering::Relaxed);
                        self.set_deltas(run.deltas);
                        self.last_pins = run.last_pins;
                        return run.records;
                    }
                }
                None => {
                    memo.none.fetch_add(1, Ordering::Relaxed);
                }
            }
            return self.run_job_real(m, job);
        }
        self.memo_log = Some(RunLog::default());
        let records = self.run_job_real(m, job);
        let log = self.memo_log.take().unwrap();
        let deltas = self.take_deltas();
        let run = MemoRun { job: job.clone(), guides, log, records: records.clone(), deltas: deltas.clone(), last_pins: self.last_pins.clone() };
        memo.runs.lock().unwrap().entry(job.label.clone()).or_default().push(run);
        self.set_deltas(deltas);
        records
    }
}
