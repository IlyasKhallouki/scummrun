//! The reading on threads. [`Pool`] is a [`Machine`] that runs each batch of jobs (one `_run_jobs` call: the inputs,
//! or one event round) on N worker threads, and hands the Builder each job's records and counters in the jobs'
//! order, so the reading is the same for every N: the same records in the same order, the same counters.
//!
//! * Each worker has its own machine half ([`Reader`]) and its own Builder ([`Builder::worker`]), which takes the
//!   reading Builder's table and folded lists at the start of each batch. A job's records and its Builder's
//!   counter increments ([`JobOut`]) go back to the reading's Builder, which merges them in the jobs' order
//!   ([`Builder::merge_job`]) as soon as every job before it is done, and streams the records on.
//! * The long jobs of a batch start first, longest first, by an estimate: the seconds the same job (its label)
//!   took before, else the mean of jobs with the same label up to its list of locals (`#...`), else the mean of
//!   all jobs so far; the others start in the jobs' order, so that few records wait for an earlier job.
//! * A run past the path limit that `Builder.run` splits (reader.py's own rule: once per value of its first fork on
//!   a read, at most SPLIT_DEPTH levels and SPLIT_BUDGET runs) has its parts made on idle workers
//!   ([`Machine::prefetch`]); the job's own worker takes each part's run in order, so the split's decisions and
//!   records are as made in one thread. Nothing else is split: what is read does not change.
//!
//! Why the order cannot change the reading: within a batch, a job reads only what the batch started with (the
//! table, the folded lists); what it changes (counters, the `args:` values of `table.args_vals`, the last pins) is
//! merged in the jobs' order. `args:` values a job adds are not read by a later job of the same batch: the machine
//! never reads an `args:` key, `record` reads an `args:` domain only for the precondition of the job's own
//! `args:` pin, which always holds 0 and the pin's value (PORT-NOTES.md, "The reading on threads").

use crate::builder::{Builder, Ctx, Job, JobOut, Machine, RecordSink, RunOut};
use crate::machine::Reader;
use crate::record::{Pins, Pre, Record, What};
use crate::scope::Scope;
use crate::table::FactTable;
use rustc_hash::FxHashMap;
use scumm_ir::Game;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

/// The reading Builder's table and folded lists as one batch starts: the table of the pass (`base`, the same
/// object until a pass widens a domain; its `args:` values aside) and the `args:` values.
struct Snap {
    base: Arc<FactTable>,
    args_vals: BTreeMap<String, BTreeSet<i64>>,
    folded: BTreeSet<i64>,
    sig_limit: usize,
}

enum PartState {
    Waiting,
    Running,
    Done(RunOut),
    Taken,
    /// the run panicked where it was made (the worker reports it)
    Failed,
}

/// One part of a split, which an idle worker may run before the job's own worker gets to it.
struct Part {
    snap: Arc<Snap>,
    label: String,
    pins: Pins,
    pre: Pre,
    what: What,
    state: Mutex<PartState>,
    cv: Condvar,
}

impl Part {
    /// An idle worker takes the part if nobody has.
    fn claim(&self) -> bool {
        let mut st = self.state.lock().unwrap();
        if matches!(*st, PartState::Waiting) {
            *st = PartState::Running;
            true
        } else {
            false
        }
    }
    fn finish(&self, out: RunOut) {
        *self.state.lock().unwrap() = PartState::Done(out);
        self.cv.notify_all();
    }
}

#[derive(Default)]
struct Queue {
    snap: Option<Arc<Snap>>,
    jobs: Arc<Vec<Job>>,
    /// the batch's jobs not started yet, in the order they start
    order: VecDeque<usize>,
    /// split parts waiting for a worker (before any job)
    parts: VecDeque<Arc<Part>>,
    results: Vec<Option<JobOut>>,
    failed: Option<String>,
    shutdown: bool,
}

struct Shared {
    q: Mutex<Queue>,
    /// workers wait here for work
    work: Condvar,
    /// the reading's thread waits here for results
    done: Condvar,
    /// split parts made by a worker other than the job's own
    parts_elsewhere: AtomicU64,
    parts_made: AtomicU64,
    /// nanoseconds the workers spent taking a batch's table
    sync_ns: AtomicU64,
    /// records of jobs done and not yet handed on (a job before them is still running)
    buffered: AtomicU64,
    /// past this many waiting records a worker starts no new job but the one the reading waits for
    max_waiting: AtomicU64,
    /// the job the reading waits for (the first not handed on)
    frontier: AtomicU64,
}

/// What the pool did: for the measurements.
#[derive(Clone, Debug, Default)]
pub struct PoolStats {
    pub batches: u64,
    pub jobs: u64,
    /// the jobs' seconds, summed
    pub job_secs: f64,
    /// the longest jobs: (seconds, label, batch), at most 20
    pub longest: Vec<(f64, String, u64)>,
    /// split parts sent to the queue, and those another worker ran
    pub parts_made: u64,
    pub parts_elsewhere: u64,
    /// per batch: (jobs, wall seconds, the longest job's seconds, the jobs' seconds summed)
    pub per_batch: Vec<(usize, f64, f64, f64)>,
    /// seconds: the batches' snapshots (the reading's thread), the workers taking them, merging and streaming
    /// the jobs' outputs (the reading's thread)
    pub snap_secs: f64,
    pub sync_secs: f64,
    pub merge_secs: f64,
    /// of which merging the counters, and handing the records on (the sink, then dropping them)
    pub merge_job_secs: f64,
    pub each_secs: f64,
    /// the most records done and waiting for an earlier job (the reorder buffer)
    pub max_buffered: u64,
}

/// Records done and waiting for an earlier job past which workers start no new job but that one (`Pool::set_max_waiting`).
pub const DEFAULT_MAX_WAITING: u64 = 100_000;

/// The pool's side of the reading: a [`Machine`] whose `run_batch` runs the batch on the workers.
pub struct Pool<'a, 'g> {
    sh: &'a Shared,
    threads: usize,
    /// runs a batch here when there are no workers
    main: Reader<'g>,
    /// the table of the pass, shared with the workers
    base: Option<Arc<FactTable>>,
    exact: FxHashMap<String, f64>,
    coarse: FxHashMap<String, (f64, u32)>,
    all: (f64, u64),
    pub stats: PoolStats,
}

/// The label up to its list of locals (`event(script:2@721#a2bf3d907238)` -> `event(script:2@721`), the kind first.
fn coarse_key(job: &Job) -> String {
    let base = job.label.split('#').next().unwrap();
    format!("{}|{base}", job.kind)
}

impl<'a, 'g> Pool<'a, 'g> {
    /// Past `n` records done and waiting for an earlier job (they are handed on in the jobs' order), the workers
    /// start no new job but the one the reading waits for: memory stays bounded when one job runs long. It changes
    /// when jobs start, not what they read.
    pub fn set_max_waiting(&mut self, n: u64) {
        self.sh.max_waiting.store(n, Ordering::Relaxed);
    }

    fn estimate(&self, job: &Job) -> f64 {
        if let Some(&s) = self.exact.get(&job.label) {
            return s;
        }
        if let Some(&(s, n)) = self.coarse.get(&coarse_key(job)) {
            return s / n as f64;
        }
        if self.all.1 > 0 {
            self.all.0 / self.all.1 as f64
        } else {
            0.0
        }
    }

    /// The order the batch's jobs start in: the long ones first, longest first (an estimate past an eighth of a
    /// worker's share of the batch, at most four per worker), then the others in the jobs' order, so that the
    /// records of jobs done before an earlier one wait as little as possible (they are handed on in the jobs'
    /// order).
    fn order(&self, jobs: &[Job]) -> Vec<usize> {
        let est: Vec<f64> = jobs.iter().map(|j| self.estimate(j)).collect();
        let total: f64 = est.iter().sum();
        let bar = total / (self.threads as f64 * 8.0);
        let mut long: Vec<usize> = (0..jobs.len()).filter(|&i| est[i] > 0.0 && est[i] >= bar).collect();
        long.sort_by(|&x, &y| est[y].partial_cmp(&est[x]).unwrap().then(x.cmp(&y)));
        long.truncate(self.threads * 4);
        let mut first = vec![false; jobs.len()];
        for &i in &long {
            first[i] = true;
        }
        long.extend((0..jobs.len()).filter(|&i| !first[i]));
        long
    }

    fn learn(&mut self, job: &Job, secs: f64) {
        let e = self.exact.entry(job.label.clone()).or_insert(0.0);
        *e = e.max(secs);
        let c = self.coarse.entry(coarse_key(job)).or_insert((0.0, 0));
        c.0 += secs;
        c.1 += 1;
        self.all.0 += secs;
        self.all.1 += 1;
    }
}

impl Machine for Pool<'_, '_> {
    fn run(&mut self, cx: &Ctx, label: &str, pins: &Pins, pre: &Pre, what: &What) -> RunOut {
        self.main.run(cx, label, pins, pre, what)
    }

    fn run_batch(&mut self, b: &mut Builder<'_>, jobs: &Arc<Vec<Job>>, each: &mut dyn FnMut(&mut Builder<'_>, usize, Vec<Record>)) -> bool {
        if self.threads == 0 {
            return false;
        }
        let t0 = Instant::now();
        let n = jobs.len();
        let batch = self.stats.batches;
        self.stats.batches += 1;
        if n == 0 {
            self.stats.per_batch.push((0, 0.0, 0.0, 0.0));
            return true;
        }
        let ts = Instant::now();
        let base = match &self.base {
            Some(t) if t.extra == b.table.extra => t.clone(),
            _ => {
                let t = Arc::new(b.table.clone());
                self.base = Some(t.clone());
                t
            }
        };
        let snap = Arc::new(Snap { base, args_vals: b.table.args_vals.clone(), folded: b.folded.clone(), sig_limit: b.sig_limit });
        let order = self.order(jobs);
        self.sh.frontier.store(0, Ordering::Relaxed);
        {
            let mut q = self.sh.q.lock().unwrap();
            q.snap = Some(snap);
            q.jobs = jobs.clone();
            q.order = order.into();
            q.results = (0..n).map(|_| None).collect();
        }
        self.stats.snap_secs += ts.elapsed().as_secs_f64();
        self.sh.work.notify_all();
        let mut next = 0;
        let (mut longest, mut sum) = (0.0f64, 0.0f64);
        while next < n {
            let ready: Vec<JobOut> = {
                let mut q = self.sh.q.lock().unwrap();
                loop {
                    if let Some(f) = &q.failed {
                        panic!("a worker failed: {f}");
                    }
                    let mut ready = Vec::new();
                    while next + ready.len() < n {
                        match q.results[next + ready.len()].take() {
                            Some(o) => ready.push(o),
                            None => break,
                        }
                    }
                    if !ready.is_empty() {
                        break ready;
                    }
                    q = self.sh.done.wait(q).unwrap();
                }
            };
            let tm = Instant::now();
            let buffered = self.sh.buffered.load(Ordering::Relaxed);
            self.stats.max_buffered = self.stats.max_buffered.max(buffered);
            for o in ready {
                self.sh.buffered.fetch_sub(o.records.len() as u64, Ordering::Relaxed);
                let job = &jobs[next];
                self.learn(job, o.secs);
                longest = longest.max(o.secs);
                sum += o.secs;
                let st = &mut self.stats;
                st.jobs += 1;
                st.job_secs += o.secs;
                if st.longest.len() < 20 || o.secs > st.longest.last().unwrap().0 {
                    st.longest.push((o.secs, job.label.clone(), batch));
                    st.longest.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
                    st.longest.truncate(20);
                }
                let t1 = Instant::now();
                b.merge_job(o.deltas, o.last_pins);
                let t2 = Instant::now();
                self.stats.merge_job_secs += (t2 - t1).as_secs_f64();
                each(b, next, o.records);
                self.stats.each_secs += t2.elapsed().as_secs_f64();
                next += 1;
                self.sh.frontier.store(next as u64, Ordering::Relaxed);
            }
            // a worker may have stopped taking jobs (too many records waiting): they are fewer now, and the
            // frontier moved (the lock: no worker is between its check and its wait)
            {
                let _q = self.sh.q.lock().unwrap();
                self.sh.work.notify_all();
            }
            self.stats.merge_secs += tm.elapsed().as_secs_f64();
        }
        {
            let mut q = self.sh.q.lock().unwrap();
            q.snap = None;
            q.jobs = Arc::new(Vec::new());
            q.results.clear();
            q.parts.clear();
        }
        self.stats.parts_made = self.sh.parts_made.load(Ordering::Relaxed);
        self.stats.parts_elsewhere = self.sh.parts_elsewhere.load(Ordering::Relaxed);
        self.stats.sync_secs = self.sh.sync_ns.load(Ordering::Relaxed) as f64 * 1e-9;
        self.stats.per_batch.push((n, t0.elapsed().as_secs_f64(), longest, sum));
        true
    }
}

/// A worker's machine for one job: its own [`Reader`], and the split parts it sent to the queue.
struct WorkerMachine<'a, 'g> {
    rd: &'a mut Reader<'g>,
    sh: &'a Shared,
    snap: &'a Arc<Snap>,
    parts: Vec<Arc<Part>>,
}

impl Machine for WorkerMachine<'_, '_> {
    fn run(&mut self, cx: &Ctx, label: &str, pins: &Pins, pre: &Pre, what: &What) -> RunOut {
        if let Some(i) = self.parts.iter().position(|p| p.pins == *pins && p.pre == *pre) {
            let p = self.parts.remove(i);
            let mut st = p.state.lock().unwrap();
            loop {
                match std::mem::replace(&mut *st, PartState::Taken) {
                    PartState::Waiting => {
                        drop(st);
                        return self.rd.run(cx, label, pins, pre, what);
                    }
                    PartState::Done(out) => {
                        self.sh.parts_elsewhere.fetch_add(1, Ordering::Relaxed);
                        return out;
                    }
                    PartState::Running => {
                        *st = PartState::Running;
                        st = p.cv.wait(st).unwrap();
                    }
                    PartState::Taken => unreachable!("a split part taken twice"),
                    PartState::Failed => panic!("a split part's run failed on another worker"),
                }
            }
        }
        self.rd.run(cx, label, pins, pre, what)
    }

    fn prefetch(&mut self, _cx: &Ctx, label: &str, runs: &[(Pins, Pre)], what: &What) {
        // the first part is run here next; the others wait at the front of the queue for an idle worker
        if runs.len() < 2 {
            return;
        }
        let parts: Vec<Arc<Part>> = runs[1..]
            .iter()
            .map(|(pins, pre)| {
                Arc::new(Part {
                    snap: self.snap.clone(),
                    label: label.to_string(),
                    pins: pins.clone(),
                    pre: pre.clone(),
                    what: what.clone(),
                    state: Mutex::new(PartState::Waiting),
                    cv: Condvar::new(),
                })
            })
            .collect();
        {
            let mut q = self.sh.q.lock().unwrap();
            for p in parts.iter().rev() {
                q.parts.push_front(p.clone());
            }
        }
        self.sh.parts_made.fetch_add(parts.len() as u64, Ordering::Relaxed);
        self.sh.work.notify_all();
        self.parts.extend(parts);
    }
}

enum Task {
    Part(Arc<Part>),
    Job(usize, Arc<Snap>, Arc<Vec<Job>>),
}

fn worker(game: &Game, scope: &Scope, sh: &Shared) {
    let mut rd = Reader::new(game);
    let mut wb: Option<Builder> = None;
    // the batch and the pass's table this worker's Builder has
    let mut synced: Option<Arc<Snap>> = None;
    let mut base: Option<Arc<FactTable>> = None;
    loop {
        let task = {
            let mut q = sh.q.lock().unwrap();
            loop {
                if q.shutdown {
                    return;
                }
                if let Some(p) = q.parts.pop_front() {
                    break Task::Part(p);
                }
                if !q.order.is_empty() {
                    if sh.buffered.load(Ordering::Relaxed) <= sh.max_waiting.load(Ordering::Relaxed) {
                        let i = q.order.pop_front().unwrap();
                        break Task::Job(i, q.snap.clone().unwrap(), q.jobs.clone());
                    }
                    // too many records wait for an earlier job: only that job may start
                    let f = sh.frontier.load(Ordering::Relaxed) as usize;
                    if let Some(pos) = q.order.iter().position(|&i| i == f) {
                        let i = q.order.remove(pos).unwrap();
                        break Task::Job(i, q.snap.clone().unwrap(), q.jobs.clone());
                    }
                }
                q = sh.work.wait(q).unwrap();
            }
        };
        let r = panic::catch_unwind(AssertUnwindSafe(|| match task {
            Task::Part(p) => {
                if p.claim() {
                    let cx = Ctx { game, scope, table: &p.snap.base };
                    match panic::catch_unwind(AssertUnwindSafe(|| rd.run(&cx, &p.label, &p.pins, &p.pre, &p.what))) {
                        Ok(out) => p.finish(out),
                        Err(e) => {
                            *p.state.lock().unwrap() = PartState::Failed;
                            p.cv.notify_all();
                            panic::resume_unwind(e);
                        }
                    }
                }
            }
            Task::Job(i, snap, jobs) => {
                let b = wb.get_or_insert_with(|| {
                    Builder::worker(game, scope.clone(), (*snap.base).clone(), snap.folded.clone(), snap.sig_limit)
                });
                if !synced.as_ref().is_some_and(|s| Arc::ptr_eq(s, &snap)) {
                    let ts = Instant::now();
                    let new_base = !base.as_ref().is_some_and(|t| Arc::ptr_eq(t, &snap.base));
                    b.sync_table(new_base.then_some(&*snap.base), &snap.args_vals, &snap.folded);
                    b.sig_limit = snap.sig_limit;
                    base = Some(snap.base.clone());
                    synced = Some(snap.clone());
                    sh.sync_ns.fetch_add(ts.elapsed().as_nanos() as u64, Ordering::Relaxed);
                }
                let t = Instant::now();
                let mut m = WorkerMachine { rd: &mut rd, sh, snap: &snap, parts: Vec::new() };
                let records = b.run_job(&mut m, &jobs[i]);
                let out = JobOut {
                    records,
                    deltas: b.take_deltas(),
                    last_pins: std::mem::take(&mut b.last_pins),
                    secs: t.elapsed().as_secs_f64(),
                };
                let mut q = sh.q.lock().unwrap();
                sh.buffered.fetch_add(out.records.len() as u64, Ordering::Relaxed);
                q.results[i] = Some(out);
                drop(q);
                sh.done.notify_all();
            }
        }));
        if let Err(e) = r {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "a panic".into());
            let mut q = sh.q.lock().unwrap();
            q.failed = Some(msg);
            q.shutdown = true;
            drop(q);
            sh.done.notify_all();
            sh.work.notify_all();
            return;
        }
    }
}

/// Runs `f` with a [`Pool`] of `threads` workers for the reading of `scope` (0: no workers, every run made on
/// the calling thread, as `machine::Reader` alone does). The workers stop when `f` returns.
pub fn with_pool<R>(game: &Game, scope: &Scope, threads: usize, f: impl FnOnce(&mut Pool) -> R) -> R {
    let sh = Shared {
        q: Mutex::new(Queue::default()),
        work: Condvar::new(),
        done: Condvar::new(),
        parts_elsewhere: AtomicU64::new(0),
        parts_made: AtomicU64::new(0),
        sync_ns: AtomicU64::new(0),
        buffered: AtomicU64::new(0),
        max_waiting: AtomicU64::new(DEFAULT_MAX_WAITING),
        frontier: AtomicU64::new(0),
    };
    std::thread::scope(|s| {
        for w in 0..threads {
            let sh = &sh;
            std::thread::Builder::new()
                .name(format!("reader-{w}"))
                .stack_size(64 << 20)
                .spawn_scoped(s, move || worker(game, scope, sh))
                .expect("a worker thread");
        }
        let mut pool = Pool {
            sh: &sh,
            threads,
            main: Reader::new(game),
            base: None,
            exact: FxHashMap::default(),
            coarse: FxHashMap::default(),
            all: (0.0, 0),
            stats: PoolStats::default(),
        };
        let r = panic::catch_unwind(AssertUnwindSafe(|| f(&mut pool)));
        {
            let mut q = sh.q.lock().unwrap();
            q.shutdown = true;
        }
        sh.work.notify_all();
        match r {
            Ok(r) => r,
            Err(e) => panic::resume_unwind(e),
        }
    })
}

enum SinkMsg {
    Batch(usize, Vec<Record>, Option<Arc<FactTable>>),
    Reset,
}

/// A [`RecordSink`] on its own thread: the records the Builder hands over (`batch_owned`) go through a bounded
/// channel to `S`, in order, with the reading's table as it is when they are made (sent again only when a pass
/// widened a domain: the table's other changes within a pass, its `args:` values, do not change how a sink writes
/// a record, packed.rs `RecordStream`). The reading's thread no longer writes or frees them. [`SinkThread::finish`]
/// waits for the thread and gives `S` back.
pub struct SinkThread<S: RecordSink + Send + 'static> {
    tx: Option<std::sync::mpsc::SyncSender<SinkMsg>>,
    handle: Option<std::thread::JoinHandle<S>>,
    sent: Option<BTreeMap<String, crate::domain::Domain>>,
}

impl<S: RecordSink + Send + 'static> SinkThread<S> {
    /// `bound`: the batches (jobs' records) the channel holds before the reading waits.
    pub fn new(mut sink: S, bound: usize) -> SinkThread<S> {
        let (tx, rx) = std::sync::mpsc::sync_channel::<SinkMsg>(bound);
        let handle = std::thread::Builder::new()
            .name("reader-sink".into())
            .spawn(move || {
                let mut table: Option<Arc<FactTable>> = None;
                for m in rx {
                    match m {
                        SinkMsg::Batch(first, recs, t) => {
                            if t.is_some() {
                                table = t;
                            }
                            sink.batch(first, &recs, table.as_ref().expect("a table"));
                        }
                        SinkMsg::Reset => sink.reset(),
                    }
                }
                sink
            })
            .expect("the sink thread");
        SinkThread { tx: Some(tx), handle: Some(handle), sent: None }
    }

    /// Waits for the records sent so far to be written, and gives the sink back.
    pub fn finish(mut self) -> S {
        drop(self.tx.take());
        self.handle.take().unwrap().join().expect("the sink thread")
    }

    fn send(&self, m: SinkMsg) {
        if self.tx.as_ref().unwrap().send(m).is_err() {
            // the thread stopped: its panic says why
            let h = self.handle.as_ref().unwrap();
            assert!(!h.is_finished(), "the sink thread stopped");
            panic!("the sink thread stopped");
        }
    }
}

impl<S: RecordSink + Send + 'static> RecordSink for SinkThread<S> {
    fn batch(&mut self, first: usize, recs: &[Record], table: &FactTable) {
        self.batch_owned(first, recs.to_vec(), table)
    }
    fn batch_owned(&mut self, first: usize, recs: Vec<Record>, table: &FactTable) {
        let t = if self.sent.as_ref() != Some(&table.extra) {
            self.sent = Some(table.extra.clone());
            Some(Arc::new(table.clone()))
        } else {
            None
        };
        self.send(SinkMsg::Batch(first, recs, t));
    }
    fn reset(&mut self) {
        self.send(SinkMsg::Reset);
    }
}
