//! reader-scope's reading as a library (E011 round 2, `run`): the binary src/bin/reader_scope.rs calls it once per
//! process; fmi calls it in its own process for every reading, with the game loaded once ([`Resident`]).
//!
//! reader-scope: the reading of a scope at Part I's scale (size_scaling.py's start, goal and scopes), on N threads
//! (par.rs), its records streamed into taskcore's packed file as made (packed.rs `RecordStream`), measured. It
//! reads the game files, results/start-part1.jsonl and results/end-part1.json (with --part1 also
//! results/groundtruth-part1.jsonl) only, no oracle file.
//!
//! usage: reader-scope (--k K | --part1 | --rooms R,R,...) [--name NAME] [--threads N] [--out DIR] [--packed FILE]
//!                     [--cap-min M] [--max-rss-gb G] [--min-free-gb G] [--hash] [--keep] [--lines FILE]
//!                     [--elsewhere] [--live-locals] [--split-limit] [--golden FILE] [--max-files-gb G] [--frame-costs]
//!                     [--step-limit N] [--out-dir DIR]
//!
//! --part1       the Part I scope: the rooms of results/groundtruth-part1.jsonl's moves (scaling.rs `part1_rooms`)
//! --threads N   worker threads (0: the reading's own thread alone, as `machine::Reader` alone reads)
//! --out DIR     the report, DIR/NAME.json (and the packed file DIR/NAME.tc unless --packed)
//! --cap-min M   stop the reading after M minutes (default 60); --max-rss-gb G (default 8) and --min-free-gb G
//!               (MemAvailable, default 3) stop it too, and --max-files-gb G (the packed file and --golden's body
//!               together, default no limit), and --max-cpu-s N (the reading's CPU seconds, all threads, default no
//!               limit); the report then says where it stopped (exit code 3)
//! --max-waiting N  records done and waiting for an earlier job past which workers start no new job but that
//!               one (default 100,000: memory stays bounded when one job runs long)
//! --hash        sha1 of the records' lines (golden.py's "record" lines, as made, before `_read_once`'s rewrite of
//!               folded lists) and of the counters' line, to compare readings across thread counts
//! --keep        keep the records in memory (the Builder's `records`); --lines FILE writes golden.py's reader file
//!               (with --keep)
//! --elsewhere   the switch for the scope-1 finding (PORT-NOTES.md, "ELSEWHERE"): a slot's place that is none of the
//!               scope's points is written as ELSEWHERE, which no precondition accepts. Off by default. The report
//!               lists such places either way (`outside_points`: fact -> place -> paths).
//! --live-locals the switch for the lists of locals (PORT-NOTES.md, "Live locals"): an event's list of locals, its
//!               number and the locals its run starts with keep only the locals its unit's code reads (from the IR;
//!               all of them where the IR does not show the reads exactly), and the number covers the slot's flags
//!               (resistant, owned, and whether it is recursive). Off by default. The report counts the scope's units
//!               by what is known of their reads (`live_locals_units`), and the reading's head (--golden) lists each
//!               slot's read locals, which packed_reading.py copies into reading.json.
//! --split-limit the reader switch split_limit (PORT-NOTES.md, "The path limit"): a run past the path limit is
//!               split on its first fork on a read until no part passes the limit, and a run that passes it with no
//!               fact left to split on, or a path the step limit ended, stops the reading. Off by default (reader.py's
//!               rule: some runs past the limit are not split, and the paths the limit ended are dropped).
//! --reach       forward reachability (reach.rs, experiments/E004-fast-exact/reach.md): a job or a fork's branch that needs
//!               a value not reached from the start does not run, and runs when the value arrives. Needs --split-limit.
//!               Writes NAME.reached next to the packed file (taskcore --reached). Off by default.
//! --reach-also FILE  a further start to read from (taskcore's --start-file format, key<TAB>value), with --reach;
//!               given more than once, a start per file. Only int values are read, and `pos:`/`escpos:` come from the
//!               file's `at:`/`esc:` facts (task.rs `task_start`), so a start from the engine is given as its facts.py
//!               map (the replay harness's reach extension, replay_part1.py `--reach-extend`)
//! --switches S,... further reader switches by name (`Switches::parse`), as reader-port's: fix round 1's value_sets,
//!               draw_clears and actor_rooms, grow_limit, havoc_cut (havoc.rs), shared_runs (shared.rs). Not due_first,
//!               which writes on the kept records after the reading. Off by default.
//! --step-limit N  the rough reading (experiments/E004-fast-exact/rough-reading.md): every job read with the machine's
//!               step limit N in place of its own (5,000); a path past N steps becomes a havoc record. Needs
//!               --switches havoc_cut. The report counts the havoc records (`havoc_records`). Off by default.
//! --memo DIR   the job memo across readings (memo.rs `CrossMemo`, experiments/E011-speed/memo.md): a job takes the run
//!               another reading (or this reading's earlier pass) stored under DIR for the same whole-reading inputs
//!               when what the run consulted reads the same here; the runs made are stored. The output is byte for
//!               byte the same. Needs --reach and --split-limit. Off by default.
//! --memo-mem   E012 resident: the job memo with the store a resident process keeps in memory alone (`Resident::memo`;
//!               with --memo DIR the store in memory spills to DIR). In a process with no resident store, no memo.
//! --memo-units-set  E012 resident (memo.md's amendment of 2026-11-03): the memo's store key holds the scope's units
//!               as a set, so that readings of the same scope whose units come in another order share their runs. The
//!               output is byte for byte the same. Off by default.
//! --prep       E011 round 2: each worker works out, from the records of the job it ran, what the reading's thread
//!               (the reached values' effects, reach.rs `reach_prep`) and the sink (the packed file's actions, the
//!               line files' lines: `TeePrep`) would work out of them, so that those threads only take it in the
//!               jobs' order. The output is byte for byte the same. Off by default.
//! --pass2-reuse pass 2 takes from pass 1 each run nothing widened touches (memo.rs,
//!               experiments/E004-fast-exact/pass2-reuse.md): the output is byte for byte the same. Off by default.
//! --frame-costs the switch for frame costs (costs.rs): each action of the packed file costs its record's frames, a
//!               lower bound of its duration from the engine source, and the fold key holds the cost; the reading's
//!               head (--golden) gives the parameters (`frame_costs`), which packed_reading.py copies into
//!               reading.json. Off by default: every action costs 1, as before.
//! --fact-vars N,... the reader switch for a refinement (experiments/E007-fact-refinement/plan.md, Lemma R): each
//!               listed variable that is a non-fact of the reading (the scope writes it, no engine or settings variable,
//!               no domain) is made a fact, its domain seeded with its start value and its value in each --reach-also
//!               start and widened by the reader's passes (table.rs `add_fact_vars`); the report's `fact_vars` says
//!               what was done per variable, its domain at the end, and the values the last pass still wrote outside
//!               it. Off by default: the reading is byte for byte as before.
//! --golden FILE golden.py's reader file streamed as the records are made, with no records kept: the head, one
//!               "record" line per record (as made, before `_read_once`'s rewrite of folded lists, which changes only
//!               `args:` values), the builder's counters; gzip. What packed_reading.py from-golden reads with the
//!               packed file to make the reading the Part I runner (replay_part1.py) plays from. The lines are made
//!               and compressed on --golden-threads N threads (default --threads, at most 16), one gzip member per
//!               batch, written in order: the same lines as one stream, other compressed bytes.
//! --start-seed N  the start: results/start-part1.jsonl's seed N (default 1)
//! --start-rows FILE  the start's rows from FILE (start-part1.jsonl's format: one JSON object per line with "seed" and
//!               "facts") in place of results/start-part1.jsonl, the row of --start-seed. Off by default.
//! --domain-goals part1|none
//!               the goal domains.build counts as compared constants: results/end-part1.json (default), or none
//!               (golden.read_scope's and replay_part1.py export's readings)
//! --goal part1|FACT=V,...
//!               the packed file's goal (default results/end-part1.json's)
//! --slot-locals FILE
//!               fix round 1, F10: slots live in the engine with the locals the bridge dumped, one JSON object per
//!               line ({"name", "room", "w", "esc", "locals", "override", "cut_data", "owned"}, and "flags":
//!               {"resistant", "recursive", "owned"} from a dump that has them, which --live-locals needs); their
//!               events are read with the start's (`Builder.slot_locals`). Without it the reading is as before.
//! --out-dir DIR the reading's own files (E011, experiments/E011-speed/plan.md, readio), written as the records are
//!               made, with no golden stream and no Python: DIR/reading.tc (the packed file), DIR/reading.reached (with
//!               --reach), DIR/reading.actions.jsonl, DIR/reading.lists.jsonl and DIR/reading.json, byte for byte what
//!               replay_part1.py `read_rust` (with `stream`) makes with packed_reading.py `reading_from_golden` from the
//!               golden stream, but for reading.json's measured times and its `from_golden` (the golden file when
//!               --golden is given, else null; the packed file); and DIR/reading.idx (plan.md, F3: the line offsets,
//!               the actions' word offsets, the names by hash, the three files' sha1). The report is DIR/NAME.json
//!               unless --out names another folder (NAME is the reading's name, as read_rust gives it: not
//!               "reading"). The bounds in reading.json are the reader's own domains'. Off by default: no such file.
//! --sample N --seed S --dump FILE
//!               the spot-check's jobs: of the last pass, N per stratum (the room a job's unit or object belongs
//!               to, or "global"), drawn by reservoir sampling with seed S; each written to FILE with what its batch
//!               started with (the `args:` values, the folded lists) and its records (golden.py's encoding), for
//!               reader_job_dump.py to make again in Python

use crate::builder::{Builder, RecordSink, SlotFlags, SlotLocals};
use crate::domain::Label;
use crate::packed::RecordStream;
use crate::py::Py;
use crate::builder::Job;
use crate::record::{Record, Val, What};
use crate::table::FactTable;
use crate::{domains, par, peak_rss_kb, scaling, task, Scope};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub mod files;
pub mod pyjson;
use pyjson::PJ;

/// Where a reading's progress lines go: stderr (the binary), or a file (a reading in another process's run, fmi's:
/// the folder's `reader-scope.err`, as when it ran reader-scope with its stderr there).
#[derive(Clone)]
pub struct Log(Option<Arc<Mutex<std::fs::File>>>);

impl Log {
    pub fn stderr() -> Log {
        Log(None)
    }
    pub fn file(path: &Path) -> std::io::Result<Log> {
        Ok(Log(Some(Arc::new(Mutex::new(std::fs::File::create(path)?)))))
    }
    pub fn line(&self, s: String) {
        match &self.0 {
            None => eprintln!("{s}"),
            Some(f) => {
                let mut f = f.lock().unwrap();
                let _ = writeln!(f, "{s}");
            }
        }
    }
}

macro_rules! log {
    ($lg:expr, $($a:tt)*) => { $lg.line(format!($($a)*)) };
}

/// What stays loaded from reading to reading in one process: the game, the repository root. The binary makes one
/// per process; fmi keeps one.
pub struct Resident {
    pub root: PathBuf,
    pub game: scumm_ir::Game,
    /// the job memo's store in memory (memo.rs `MemStore`): with `--memo DIR`, a reading takes runs from it and
    /// stores its own in it, DIR being where its oldest files spill; None: the store is DIR alone
    pub memo: Option<Arc<crate::memo::MemStore>>,
}

impl Resident {
    /// The game under `root` (scumm_ir::default_root() for the binary's).
    pub fn load(root: &Path) -> std::io::Result<Resident> {
        Ok(Resident { root: root.to_path_buf(), game: scumm_ir::Game::load(root)?, memo: None })
    }
}

/// A reading run inside another process (fmi): its log, the flag that stops it (set by the caller, or by the watchdog
/// when a limit passes), and why the watchdog stopped it. The binary runs with none: the watchdog then ends the
/// process (exit code 3), as reader-scope always did.
pub struct Ctl {
    pub log: Log,
    pub abort: Arc<std::sync::atomic::AtomicBool>,
    pub stopped: Arc<Mutex<Option<String>>>,
}

impl Ctl {
    pub fn new(log: Log, abort: Arc<std::sync::atomic::AtomicBool>) -> Ctl {
        Ctl { log, abort, stopped: Arc::new(Mutex::new(None)) }
    }
}

/// The watchdog's loop ends when the reading does (a reading in another process's run).
struct Done(Arc<std::sync::atomic::AtomicBool>);

impl Drop for Done {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}


fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).map(|i| args[i + 1].clone())
}

use crate::cpu_secs;

fn status_kb(field: &str) -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    s.lines().find(|l| l.starts_with(field)).and_then(|l| l.split_whitespace().nth(1)).and_then(|x| x.parse().ok()).unwrap_or(0)
}

fn mem_available_kb() -> u64 {
    let s = std::fs::read_to_string("/proc/meminfo").unwrap();
    s.lines().find(|l| l.starts_with("MemAvailable:")).and_then(|l| l.split_whitespace().nth(1)).and_then(|x| x.parse().ok()).unwrap_or(0)
}

fn loadavg() -> String {
    std::fs::read_to_string("/proc/loadavg").unwrap_or_default().split_whitespace().take(3).collect::<Vec<_>>().join(" ")
}

/// What the reading has done so far (the progress callback and the sink write it; the watchdog reads it).
#[derive(Default)]
struct Prog {
    /// the highest RSS the watchdog saw (MB), and where the reading was then (the pass, the batch it was in)
    rss_peak_mb: u64,
    rss_peak_at: String,
    /// the batch running now
    current: String,
    records: usize,
    pass: usize,
    batches: Vec<(String, usize, usize, f64)>,
    jobs: usize,
}

/// The sink: the packed file, and optionally the records' lines hashed (and written).
struct Tee {
    busy: f64,
    stream: Option<RecordStream>,
    hash: Option<sha1_smol::Sha1>,
    /// --golden: the records' lines, gzip, in a body file next to the final one (the head comes first and is known
    /// only at the end), made and compressed on `golden_threads` threads (`GoldenPool`)
    body: Option<(PathBuf, GoldenPool)>,
    golden_threads: usize,
    kinds: BTreeMap<String, usize>,
    /// the pass's records that end "havoc" (the switch havoc_cut)
    havoc: usize,
    prog: Arc<Mutex<Prog>>,
    /// --out-dir: the reading's line files as the records come (files.rs)
    files: Option<files::Files>,
    /// the switch --prep: the texts' numbers (packed.rs `Interner`), and the kinds' names by number
    nums: Option<Arc<crate::packed::Interner>>,
    kind_names: Vec<Option<String>>,
}

impl RecordSink for Tee {
    fn batch(&mut self, first: usize, recs: &[Record], table: &FactTable) {
        let t = Instant::now();
        self.batch_timed(first, recs, table);
        if let Some((_, pool)) = &mut self.body {
            pool.send(first, recs.to_vec());
        }
        self.busy += t.elapsed().as_secs_f64();
    }
    fn batch_owned(&mut self, first: usize, recs: Vec<Record>, table: &FactTable) {
        let t = Instant::now();
        self.batch_timed(first, &recs, table);
        if let Some((_, pool)) = &mut self.body {
            pool.send(first, recs);
        }
        self.busy += t.elapsed().as_secs_f64();
    }
    fn batch_prepared(&mut self, first: usize, recs: Vec<Record>, table: &FactTable, prep: Box<dyn std::any::Any + Send>) {
        let t = Instant::now();
        let prep = *prep.downcast::<TeePrep>().expect("--prep: the sink's part");
        // the records themselves (none when they stayed with the worker, `Builder::prep_lite`) only for --hash and
        // --golden
        self.batch_timed_prepared(first, &recs, prep);
        let _ = table;
        if let Some((_, pool)) = &mut self.body {
            pool.send(first, recs);
        }
        self.busy += t.elapsed().as_secs_f64();
    }
    fn reset(&mut self) {
        if let Some(s) = &mut self.stream {
            s.reset().expect("the packed file");
        }
        if let Some(f) = &mut self.files {
            f.reset().expect("the reading's files");
        }
        if self.hash.is_some() {
            self.hash = Some(sha1_smol::Sha1::new());
        }
        if let Some((path, pool)) = self.body.take() {
            pool.finish();
            self.body = Some((path.clone(), GoldenPool::new(&path, self.golden_threads)));
        }
        self.kinds.clear();
        self.havoc = 0;
        let mut p = self.prog.lock().unwrap();
        p.records = 0;
        p.pass += 1;
    }
}

impl Tee {
    /// The packed file, the hash and the kinds; the golden lines go to the pool (`batch`, `batch_owned`).
    fn batch_timed(&mut self, first: usize, recs: &[Record], table: &FactTable) {
        if let Some(s) = &mut self.stream {
            match &mut self.files {
                Some(f) => {
                    let mut written = Vec::new();
                    s.records_with(first, recs, table, &mut |j| written.push(j)).expect("the packed file");
                    f.batch(first, recs, &written).expect("the reading's files");
                }
                None => s.records(first, recs, table).expect("the packed file"),
            }
        }
        self.tally(first, recs);
    }
}

/// The sink's part of the switch --prep (`Builder::prep_sink`): the packed file's and the line files' work on a job's
/// records, made where the job ran.
pub struct TeePrep {
    recs: Vec<crate::packed::RecPrep>,
    lines: Option<Vec<files::LinePrep>>,
}

/// The switch --prep's function for the sink (`Tee`): `costs` the frame costs, `lines` whether the line files are
/// written (--out-dir), `flags` the list lines' slot flags (--live-locals).
fn tee_prep(costs: Option<Arc<crate::costs::FrameCosts>>, lines: bool, flags: bool, nums: Arc<crate::packed::Interner>) -> Arc<crate::builder::PrepFn> {
    Arc::new(move |recs: &[Record], table: &FactTable| -> Box<dyn std::any::Any + Send> {
        Box::new(TeePrep {
            recs: crate::packed::prep_records(costs.as_deref(), recs, table, &nums),
            lines: lines.then(|| files::prep_lines(recs, flags)),
        })
    })
}

impl Tee {
    /// `batch_timed` on records the switch --prep worked out where their job ran (`recs`: the records, or none when
    /// they stayed there).
    fn batch_timed_prepared(&mut self, first: usize, recs: &[Record], prep: TeePrep) {
        let TeePrep { recs: rp, lines } = prep;
        let n = rp.len();
        assert!(recs.is_empty() || recs.len() == n, "--prep: one part per record");
        let nums = self.nums.clone().expect("--prep: the texts' numbers");
        for p in &rp {
            let k = p.kind as usize;
            if k >= self.kind_names.len() {
                self.kind_names.resize(k + 1, None);
            }
            let name = self.kind_names[k].get_or_insert_with(|| nums.text(p.kind).to_string()).clone();
            self.count(&name, p.havoc);
        }
        if let Some(s) = &mut self.stream {
            match (&mut self.files, lines) {
                (Some(f), Some(lines)) => {
                    let mut written = Vec::new();
                    s.records_prepared(first, rp, &nums, &mut |j| written.push(j)).expect("the packed file");
                    f.batch_prepared(first, lines, &written).expect("the reading's files");
                }
                (None, None) => s.records_prepared(first, rp, &nums, &mut |_| {}).expect("the packed file"),
                _ => panic!("--prep: the line files' part does not match the sink's"),
            }
        }
        if let Some(h) = &mut self.hash {
            assert_eq!(recs.len(), n, "--hash: the records stay with the sink");
            for (j, r) in recs.iter().enumerate() {
                h.update(record_line(first + j, r).as_bytes());
                h.update(b"\n");
            }
        }
        self.prog.lock().unwrap().records = first + n;
    }

    /// A record's kind and whether it ends "havoc", counted.
    fn count(&mut self, kind: &str, havoc: bool) {
        match self.kinds.get_mut(kind) {
            Some(n) => *n += 1,
            None => {
                self.kinds.insert(kind.to_string(), 1);
            }
        }
        if havoc {
            self.havoc += 1;
        }
    }

    /// The hash (--hash), the kinds, the havoc records, the progress.
    fn tally(&mut self, first: usize, recs: &[Record]) {
        for (j, r) in recs.iter().enumerate() {
            if let Some(h) = &mut self.hash {
                h.update(record_line(first + j, r).as_bytes());
                h.update(b"\n");
            }
            self.count(&r.kind, r.end.as_deref() == Some("havoc"));
        }
        self.prog.lock().unwrap().records = first + recs.len();
    }
}

/// golden.py's "record" line of record number `index`.
fn record_line(index: usize, r: &Record) -> String {
    Py::dict(vec![("kind", Py::str("record")), ("index", Py::Int(index as i64)), ("record", r.py())]).text()
}

/// --golden's body made off the sink thread: each batch's lines are made and compressed into one gzip member on one
/// of `n` threads, and a writer thread writes the members in the order the batches came. The lines and their order
/// are the same as one gzip stream's; only the compressed bytes differ (several members, which every reader of the
/// golden file reads: lib.rs `MultiGzDecoder`, Python's gzip).
struct GoldenPool {
    txs: Vec<std::sync::mpsc::SyncSender<(usize, usize, Vec<Record>)>>,
    next: usize,
    workers: Vec<std::thread::JoinHandle<()>>,
    writer: std::thread::JoinHandle<std::io::Result<()>>,
}

impl GoldenPool {
    fn new(path: &Path, n: usize) -> GoldenPool {
        let f = std::fs::File::create(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut out = std::io::BufWriter::with_capacity(1 << 20, f);
        let (wtx, wrx) = std::sync::mpsc::sync_channel::<(usize, Vec<u8>)>(4 * n.max(1));
        let writer = std::thread::Builder::new()
            .name("golden-writer".into())
            .spawn(move || {
                let mut pending: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
                let mut want = 0usize;
                for (seq, bytes) in wrx {
                    pending.insert(seq, bytes);
                    while let Some(b) = pending.remove(&want) {
                        out.write_all(&b)?;
                        want += 1;
                    }
                }
                assert!(pending.is_empty(), "the golden pool lost a batch");
                out.flush()
            })
            .expect("the golden writer");
        let mut txs = Vec::new();
        let mut workers = Vec::new();
        for i in 0..n.max(1) {
            let (tx, rx) = std::sync::mpsc::sync_channel::<(usize, usize, Vec<Record>)>(8);
            let wtx = wtx.clone();
            workers.push(
                std::thread::Builder::new()
                    .name(format!("golden-{i}"))
                    .spawn(move || {
                        for (seq, first, recs) in rx {
                            let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(6));
                            for (j, r) in recs.iter().enumerate() {
                                gz.write_all(record_line(first + j, r).as_bytes()).expect("the golden file");
                                gz.write_all(b"\n").expect("the golden file");
                            }
                            wtx.send((seq, gz.finish().expect("the golden file"))).expect("the golden writer");
                        }
                    })
                    .expect("a golden thread"),
            );
            txs.push(tx);
        }
        GoldenPool { txs, next: 0, workers, writer }
    }

    fn send(&mut self, first: usize, recs: Vec<Record>) {
        let k = self.next % self.txs.len();
        self.txs[k].send((self.next, first, recs)).expect("a golden thread");
        self.next += 1;
    }

    /// Waits for every batch to be written and the file flushed.
    fn finish(self) {
        drop(self.txs);
        for w in self.workers {
            w.join().expect("a golden thread");
        }
        self.writer.join().expect("the golden writer").expect("the golden file");
    }
}

/// One gzip member holding `line` and a newline.
fn gz_member(out: &mut impl Write, line: &str) {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(6));
    gz.write_all(line.as_bytes()).unwrap();
    gz.write_all(b"\n").unwrap();
    out.write_all(&gz.finish().unwrap()).unwrap();
}

/// splitmix64
fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The spot-check's sample: per stratum a reservoir of jobs (the last pass's), each with its batch's start and
/// its records, as golden.py-encoded lines.
#[derive(Default)]
struct Sample {
    per: usize,
    seed: u64,
    pass: usize,
    /// stratum -> (jobs seen, picked: (the job's place in the pass, its line))
    strata: BTreeMap<String, (u64, Vec<(u64, String)>)>,
    seen: u64,
}

impl Sample {
    fn offer(&mut self, pass: usize, stratum: String, line: impl FnOnce(u64) -> String) {
        if pass != self.pass {
            self.pass = pass;
            self.strata.clear();
            self.seen = 0;
        }
        let g = self.seen;
        self.seen += 1;
        let h = stratum.bytes().fold(self.seed, |a, c| mix(a ^ c as u64));
        let (n, picked) = self.strata.entry(stratum).or_default();
        *n += 1;
        if picked.len() < self.per {
            picked.push((g, line(g)));
        } else {
            let j = (mix(h ^ *n) % *n) as usize;
            if j < self.per {
                picked[j] = (g, line(g));
            }
        }
    }
}

/// The room a job belongs to: its event's unit's room, its object's room, the room it is pinned to, or "global".
fn stratum(job: &Job, object_room: &rustc_hash::FxHashMap<i32, i32>) -> String {
    use scumm_ir::UnitKey;
    let room = match &job.what {
        What::Event(ev) => match ev.unit {
            UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => Some(r),
            UnitKey::Obj(o) => object_room.get(&o).copied(),
            UnitKey::Global(_) => return "global".into(),
        },
        What::Sentence { a, .. } => object_room.get(&(*a as i32)).copied(),
        What::Verb(_) => match job.pins.get("room") {
            Some(Val::Int(r)) => Some(*r as i32),
            _ => None,
        },
    };
    room.map_or_else(|| "none".to_string(), |r| format!("room {r}"))
}

fn batches_json(batches: &[(String, usize, usize, f64)], per_batch: &[(usize, f64, f64, f64)]) -> Value {
    // the inputs and the first five rounds of each pass, the rest summed; with the pool's numbers when it ran them
    // (the longest job's seconds, the jobs' seconds summed)
    let mut out = Vec::new();
    let mut rest = (0usize, 0usize, 0usize, 0.0f64, 0.0f64, 0.0f64);
    let mut pass = 0;
    let r1 = |x: f64| (x * 10.0).round() / 10.0;
    for (i, (what, jobs, recs, secs)) in batches.iter().enumerate() {
        let (longest, job_s) = per_batch.get(i).map_or((0.0, 0.0), |p| (p.2, p.3));
        if what == "inputs" {
            if rest.0 > 0 {
                out.push(json!({"what": format!("pass {pass}: later rounds ({})", rest.0), "jobs": rest.1, "records": rest.2, "s": r1(rest.3), "longest_job_s": r1(rest.4), "job_s": r1(rest.5)}));
            }
            rest = (0, 0, 0, 0.0, 0.0, 0.0);
            pass += 1;
        }
        let round: usize = what.strip_prefix("event round ").and_then(|r| r.parse().ok()).unwrap_or(0);
        if round <= 5 {
            out.push(json!({"what": format!("pass {pass}: {what}"), "jobs": jobs, "records": recs, "s": r1(*secs), "longest_job_s": r1(longest), "job_s": r1(job_s)}));
        } else {
            rest.0 += 1;
            rest.1 += jobs;
            rest.2 += recs;
            rest.3 += secs;
            rest.4 = rest.4.max(longest);
            rest.5 += job_s;
        }
    }
    if rest.0 > 0 {
        out.push(json!({"what": format!("pass {pass}: later rounds ({})", rest.0), "jobs": rest.1, "records": rest.2, "s": r1(rest.3), "longest_job_s": r1(rest.4), "job_s": r1(rest.5)}));
    }
    Value::Array(out)
}

/// `--goal`: "part1" (results/end-part1.json) or "FACT=V,FACT=V".
fn goal_arg(text: &str, root: &Path) -> Vec<(String, Vec<i64>)> {
    if text == "part1" {
        return scaling::part1_goal(root);
    }
    text.split(',')
        .map(|p| {
            let (k, v) = p.split_once('=').expect("FACT=V");
            (k.trim().to_string(), vec![v.trim().parse().expect("an int goal value")])
        })
        .collect()
}

/// `--slot-locals`: one slot per line (module docstring).
fn slot_locals(path: &str) -> Vec<SlotLocals> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let ints = |v: &Value| -> Vec<i64> { v.as_array().expect("a list").iter().map(|x| x.as_i64().expect("an int")).collect() };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).expect("a JSON line");
            SlotLocals {
                name: v["name"].as_str().expect("name").to_string(),
                room: v["room"].as_i64().expect("room") as i32,
                w: v["w"].as_i64().expect("w"),
                esc: v["esc"].as_i64(),
                locals: ints(&v["locals"]),
                override_: v["override"].as_i64().unwrap_or(0),
                cut_data: v.get("cut_data").map_or(vec![], ints),
                owned: v["owned"].as_bool().unwrap_or(false),
                flags: v.get("flags").filter(|f| !f.is_null()).map(|f| SlotFlags {
                    resistant: f["resistant"].as_bool().expect("flags.resistant"),
                    recursive: f["recursive"].as_bool().expect("flags.recursive"),
                    owned: f["owned"].as_bool().expect("flags.owned"),
                }),
            }
        })
        .collect()
}

/// The facts of the start row of `seed` in a file of start-part1.jsonl's format, as json.loads gives them.
fn start_row_facts(path: &Path, seed: i64) -> PJ {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    for line in text.lines() {
        let v = crate::scaling::Json::parse(line);
        if v.get("seed").and_then(crate::scaling::Json::int) == Some(seed) {
            return PJ::from_json(v.get("facts").expect("facts"));
        }
    }
    panic!("no seed {seed} in {}", path.display())
}

fn write_json(path: &Path, v: &Value) {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(v).unwrap() + "\n").unwrap();
    std::fs::rename(&tmp, path).unwrap();
}

/// The binary's main: the game loaded, one reading, the process's exit (3 when the watchdog stops the reading).
pub fn main_of(args: &[String]) {
    let t_all = Instant::now();
    let res = Resident::load(&scumm_ir::default_root()).expect("the game");
    run(args, &res, None, t_all);
}

/// One reading by reader-scope's command line `args` (the module docstring), with `res`'s game. With `ctl` (a reading
/// in another process's run), the progress goes to its log, and a stop (its flag, or a limit the watchdog sees)
/// ends the reading by a panic, after the report says why; the caller catches it. Returns the report.
/// `t_all`: when the reading's process (or call) began, for the report's `total_s`.
pub fn run(args: &[String], res: &Resident, ctl: Option<&Ctl>, t_all: Instant) -> Value {
    let lg = ctl.map_or_else(Log::stderr, |c| c.log.clone());
    let root = res.root.clone();
    let game = &res.game;
    let start_seed: i64 = arg(&args, "--start-seed").map_or(scaling::SEED, |n| n.parse().unwrap());
    let start_rows = arg(&args, "--start-rows");
    let start = match &start_rows {
        Some(f) => scaling::start_facts_in(Path::new(f), start_seed),
        None => scaling::start_facts(&root, start_seed),
    };
    let goal = goal_arg(&arg(&args, "--goal").unwrap_or_else(|| "part1".into()), &root);
    let domain_goals = arg(&args, "--domain-goals").unwrap_or_else(|| "part1".into());
    let dom_goals = match domain_goals.as_str() {
        "part1" => scaling::part1_goal(&root),
        "none" => vec![],
        x => panic!("--domain-goals part1|none, not {x}"),
    };
    let slots = arg(&args, "--slot-locals").map(|p| slot_locals(&p)).unwrap_or_default();
    let k: Option<usize> = arg(&args, "--k").map(|k| k.parse().unwrap());
    let part1 = args.iter().any(|a| a == "--part1");
    let rooms: Vec<i32> = match (&k, arg(&args, "--rooms")) {
        (Some(k), _) => scaling::scopes(&game, start.get0("room") as i32)[*k].clone(),
        (None, Some(r)) => r.split(',').map(|x| x.parse().unwrap()).collect(),
        _ if part1 => scaling::part1_rooms(&game, &root),
        _ => panic!("--k K, --part1 or --rooms R,R,..."),
    };
    let name = arg(&args, "--name").unwrap_or_else(|| match k {
        Some(k) => format!("k{k}"),
        None if part1 => "part1".to_string(),
        None => "rooms".to_string(),
    });
    let threads: usize = arg(&args, "--threads").map_or(6, |n| n.parse().unwrap());
    let golden_threads: usize = arg(&args, "--golden-threads").map_or(threads.clamp(1, 16), |n| n.parse().unwrap());
    let out_dir: Option<PathBuf> = arg(&args, "--out-dir").map(PathBuf::from);
    let out = PathBuf::from(arg(&args, "--out").or_else(|| out_dir.as_ref().map(|d| d.display().to_string())).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&out).unwrap();
    let packed = match &out_dir {
        Some(d) => {
            std::fs::create_dir_all(d).unwrap();
            // an older reading's head and index go first: the folder holds a whole reading only once they are back
            for f in [files::HEAD, files::IDX] {
                std::fs::remove_file(d.join(f)).ok();
            }
            let p = d.join(files::PACKED);
            if let Some(q) = arg(&args, "--packed") {
                assert_eq!(PathBuf::from(q), p, "--out-dir writes the packed file as DIR/{}", files::PACKED);
            }
            assert!(!(out == *d && format!("{name}.json") == files::HEAD), "--out-dir: the report NAME.json would be {}", files::HEAD);
            p
        }
        None => arg(&args, "--packed").map_or_else(|| out.join(format!("{name}.tc")), PathBuf::from),
    };
    let cap_s = arg(&args, "--cap-min").map_or(60.0, |m| m.parse::<f64>().unwrap()) * 60.0;
    // E020: FMI_READ_MAX_RSS_GB, the bound when no --max-rss-gb is given (8 GB): in process the RSS is the driver's, so a
    // reading's growth there counts the other tries' too
    let max_rss_gb = arg(&args, "--max-rss-gb")
        .or_else(|| std::env::var("FMI_READ_MAX_RSS_GB").ok().filter(|v| !v.is_empty()))
        .map_or(8.0, |g| g.parse::<f64>().unwrap());
    let max_rss_kb = (max_rss_gb * 1024.0 * 1024.0) as u64;
    let min_free_kb = (arg(&args, "--min-free-gb").map_or(3.0, |g| g.parse::<f64>().unwrap()) * 1024.0 * 1024.0) as u64;
    let max_files_b = arg(&args, "--max-files-gb").map(|g| (g.parse::<f64>().unwrap() * 1073741824.0) as u64);
    let max_cpu_s: Option<f64> = arg(&args, "--max-cpu-s").map(|c| c.parse().unwrap());
    let hash = args.iter().any(|a| a == "--hash");
    let keep = args.iter().any(|a| a == "--keep");
    let elsewhere = args.iter().any(|a| a == "--elsewhere");
    let live_locals = args.iter().any(|a| a == "--live-locals");
    let split_limit = args.iter().any(|a| a == "--split-limit");
    let frame_costs = args.iter().any(|a| a == "--frame-costs");
    let reach = args.iter().any(|a| a == "--reach");
    let pass2_reuse = args.iter().any(|a| a == "--pass2-reuse");
    let prep = args.iter().any(|a| a == "--prep");
    let step_limit: u32 = arg(&args, "--step-limit").map_or(0, |n| n.parse().expect("--step-limit N"));
    let fact_vars: Option<Vec<i32>> = arg(&args, "--fact-vars")
        .map(|s| s.split(',').filter(|x| !x.trim().is_empty()).map(|x| x.trim().parse().expect("--fact-vars N,...")).collect());
    let named = crate::builder::Switches::parse(&arg(&args, "--switches").unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    assert!(!named.due_first, "--switches due_first: the records are streamed as made, and due_first writes on the kept records");
    let golden: Option<PathBuf> = arg(&args, "--golden").map(PathBuf::from);
    let report_path = out.join(format!("{name}.json"));
    let sample_n: usize = arg(&args, "--sample").map_or(0, |n| n.parse().unwrap());
    let dump = arg(&args, "--dump");

    let t_setup = Instant::now();
    let doms = domains::build_with(&game, &domains::meta_rooms(&root), &start, &dom_goals, named.scope());
    let scope = Scope::of_with(&game, &rooms, &start, named.scope());
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for &u in &scope.units {
        let kind = match game.unit(u).key {
            scumm_ir::UnitKey::Entry(_) => "entry",
            scumm_ir::UnitKey::Exit(_) => "exit",
            scumm_ir::UnitKey::Local(..) => "local",
            scumm_ir::UnitKey::Obj(_) => "obj",
            scumm_ir::UnitKey::Global(_) => "global",
        };
        *kinds.entry(kind).or_default() += 1;
    }
    let base_start = match &start_rows {
        Some(f) => format!("{f} seed {start_seed}"),
        None => format!("results/start-part1.jsonl seed {start_seed}"),
    };
    let mut base = json!({
        "name": name,
        "k": k,
        "rooms": rooms,
        "n_rooms": scope.rooms.len(),
        "units": kinds,
        "n_units": scope.units.len(),
        "objects": scope.objects.len(),
        "slot_facts": scope.universe.len(),
        "start": base_start.clone(),
        "goal": goal.iter().map(|g| g.0.clone()).collect::<Vec<_>>(),
        "domain_goals": domain_goals,
        "slot_locals": slots.len(),
        "threads": threads,
        "elsewhere": elsewhere,
        "live_locals": live_locals,
        "split_limit": split_limit,
        "reach": reach,
        "switches": named.names(),
        "frame_costs": frame_costs,
        "cap_min": cap_s / 60.0,
        "load_at_start": loadavg(),
        "setup_s": 0.0,
    });
    if step_limit != 0 {
        base["step_limit"] = json!(step_limit);
    }
    if prep {
        base["prep"] = json!(true);
    }
    base["setup_s"] = json!((t_setup.elapsed().as_secs_f64() * 100.0).round() / 100.0);
    log!(lg, 
        "[{name}] {} rooms {:?}, {} units, {} slot facts; {threads} threads; load {}",
        scope.rooms.len(),
        rooms,
        scope.units.len(),
        scope.universe.len(),
        loadavg()
    );

    let mut b = Builder::new(&game, scope.clone(), doms);
    b.keep_records = keep;
    b.table.elsewhere = elsewhere;
    b.switches = named;
    b.switches.split_limit |= split_limit;
    b.switches.reach |= reach;
    b.switches.pass2_reuse |= pass2_reuse;
    b.memo_dir = arg(&args, "--memo").map(PathBuf::from);
    if b.memo_dir.is_some() || args.iter().any(|a| a == "--memo-mem") {
        b.memo_mem = res.memo.clone();
    }
    b.memo_units_set = args.iter().any(|a| a == "--memo-units-set");
    if step_limit != 0 {
        b.switches.step_limit = step_limit;
    }
    if reach {
        b.reach_starts.push(start.clone());
        // every --reach-also, in the order given: the closure does not depend on the order (reach.md)
        let also: Vec<&String> = args.windows(2).filter(|w| w[0] == "--reach-also").map(|w| &w[1]).collect();
        for f in &also {
            b.reach_starts.push(crate::reach::start_file(f));
        }
        if !also.is_empty() {
            base["reach_also"] = json!(also);
        }
    }
    // the switch --fact-vars (E007, Lemma R): the listed non-facts made facts, seeded with every start's value
    let fact_vars_done = fact_vars.as_ref().map(|vars| {
        let starts: Vec<crate::Facts> = b.reach_starts.iter().skip(1).cloned().collect();
        let refs: Vec<&crate::Facts> = starts.iter().collect();
        let scope_ = b.scope.clone();
        let done = b.table.add_fact_vars(&scope_, vars, &refs);
        for d in &done {
            log!(lg, "[{name}] --fact-vars var:{}: {} (seeds {:?})", d.n, d.what, d.seeds);
        }
        done
    });
    if live_locals {
        let ll = crate::locals::LiveLocals::of(&game);
        base["live_locals_units"] = json!(ll.counts(&game, &scope));
        b.table.live_locals = Some(Arc::new(ll));
    }
    b.slot_locals = slots;
    assert!(frame_costs || !b.switches.wait_pass_cost, "--switches wait_pass_cost: a cost switch, it needs --frame-costs");
    if frame_costs {
        let fc = crate::costs::FrameCosts::of_with(&game, &scope, b.switches.wait_pass_cost);
        base["frame_costs_params"] = serde_json::from_str(&fc.py().text()).unwrap_or(Value::Null);
        b.frame_costs = Some(Arc::new(fc));
    }
    let prog = Arc::new(Mutex::new(Prog { pass: 1, ..Default::default() }));
    {
        let prog = prog.clone();
        let t0 = Instant::now();
        let name = name.clone();
        let lg = lg.clone();
        b.progress = Some(Box::new(move |what: &str, jobs: usize, recs: usize, secs: f64| {
            let mut p = prog.lock().unwrap();
            p.batches.push((what.to_string(), jobs, recs, secs));
            p.jobs += jobs;
            let round: usize = what.strip_prefix("event round ").and_then(|r| r.parse().ok()).unwrap_or(0);
            if jobs >= 200 || secs >= 2.0 || round <= 3 || round % 100 == 0 {
                log!(lg, 
                    "[{name}] pass {} {what}: {jobs} runs, {recs} records, {secs:.1} s; {} records in the pass, {:.0} s, RSS {} MB",
                    p.pass,
                    p.records,
                    t0.elapsed().as_secs_f64(),
                    status_kb("VmRSS:") / 1024
                );
            }
        }));
    }
    let sample = Arc::new(Mutex::new(Sample {
        per: sample_n,
        seed: arg(&args, "--seed").map_or(1, |n| n.parse().unwrap()),
        ..Default::default()
    }));
    if sample_n > 0 {
        let sample = sample.clone();
        let prog = prog.clone();
        let object_room = game.object_room.clone();
        b.on_job = Some(Box::new(move |job: &Job, recs: &[Record], args_vals, folded| {
            let pass = prog.lock().unwrap().pass;
            let st = stratum(job, &object_room);
            sample.lock().unwrap().offer(pass, st.clone(), |g| {
                Py::dict(vec![
                    ("kind", Py::str("job")),
                    ("pass", Py::Int(pass as i64)),
                    ("g", Py::Int(g as i64)),
                    ("stratum", Py::str(&st)),
                    ("job", job.py()),
                    (
                        "args_vals",
                        Py::Dict(args_vals.iter().map(|(k, v)| (k.clone(), Py::Set(v.iter().map(|&x| Py::Int(x)).collect()))).collect()),
                    ),
                    ("folded", Py::Set(folded.iter().map(|&x| Py::Int(x)).collect())),
                    ("records", Py::List(recs.iter().map(|r| r.py()).collect())),
                ])
                .text()
            });
        }));
    }
    {
        let prog = prog.clone();
        let name = name.clone();
        let lg = lg.clone();
        b.on_batch_start = Some(Box::new(move |what: &str, jobs: usize, events: usize| {
            let mut p = prog.lock().unwrap();
            p.current = format!("{what} ({jobs} jobs)");
            if jobs >= 1000 {
                log!(lg, 
                    "[{name}] pass {} {what}: {jobs} runs from {events} events start; RSS {} MB",
                    p.pass,
                    status_kb("VmRSS:") / 1024
                );
            }
        }));
    }
    let nums = prep.then(crate::packed::Interner::new);
    if let Some(nums) = &nums {
        b.prep = true;
        b.prep_sink = Some(tee_prep(b.frame_costs.clone(), out_dir.is_some(), live_locals, nums.clone()));
        // the records stay with the workers unless the sink writes them itself (--hash, --golden)
        b.prep_lite = !hash && golden.is_none() && !keep;
    }
    let mut tee = Tee {
        busy: 0.0,
        stream: Some({
            let mut st = RecordStream::create(&packed).expect("the packed file");
            st.costs = b.frame_costs.clone();
            st
        }),
        hash: hash.then(sha1_smol::Sha1::new),
        body: golden.as_ref().map(|g| {
            let p = g.with_extension("body.gz");
            let f = GoldenPool::new(&p, golden_threads);
            (p, f)
        }),
        golden_threads,
        kinds: BTreeMap::new(),
        havoc: 0,
        prog: prog.clone(),
        files: out_dir.as_ref().map(|d| {
            let mut f = files::Files::create(d, live_locals).expect("the reading's files");
            f.sha_at_end = prep;
            f
        }),
        nums,
        kind_names: Vec::new(),
    };

    // the watchdog: the cap, the process's memory, the machine's free memory (and, in another process's run, the
    // caller's stop); it ends with the reading
    let t_read = Instant::now();
    let cpu0 = cpu_secs();
    let done = Done(Arc::new(std::sync::atomic::AtomicBool::new(false)));
    // in another process's run, the memory the reading added to it
    let rss0 = if ctl.is_some() { status_kb("VmRSS:") } else { 0 };
    let abort = ctl.map(|c| c.abort.clone());
    {
        let prog = prog.clone();
        let base = base.clone();
        let report_path = report_path.clone();
        let packed = packed.clone();
        let body = arg(&args, "--golden").map(|g| PathBuf::from(g).with_extension("body.gz"));
        let lg = lg.clone();
        let done = done.0.clone();
        let abort = abort.clone();
        let stopped_w: Option<Arc<Mutex<Option<String>>>> = ctl.map(|c| c.stopped.clone());
        std::thread::spawn(move || loop {
            for _ in 0..10 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if done.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
            }
            let wall = t_read.elapsed().as_secs_f64();
            let rss = status_kb("VmRSS:").saturating_sub(rss0);
            let free = mem_available_kb();
            {
                let mut p = prog.lock().unwrap();
                if rss / 1024 > p.rss_peak_mb {
                    p.rss_peak_mb = rss / 1024;
                    p.rss_peak_at = format!("pass {}, {}, {:.0} s", p.pass, p.current, wall);
                }
            }
            let why = if abort.as_ref().is_some_and(|a| a.load(std::sync::atomic::Ordering::Relaxed)) {
                Some("the caller stopped the reading".to_string())
            } else if wall > cap_s {
                Some(format!("the reading passed {:.0} minutes", cap_s / 60.0))
            } else if max_cpu_s.is_some_and(|m| cpu_secs() - cpu0 > m) {
                Some(format!("the reading passed {:.0} CPU s", max_cpu_s.unwrap()))
            } else if rss > max_rss_kb {
                Some(format!("the {}RSS passed {:.1} GB", if rss0 > 0 { "reading's " } else { "process's " }, max_rss_kb as f64 / 1048576.0))
            } else if free < min_free_kb {
                Some(format!("MemAvailable fell under {:.1} GB", min_free_kb as f64 / 1048576.0))
            } else if let Some(max) = max_files_b.filter(|&max| {
                let len = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                len(&packed) + body.as_deref().map_or(0, len) > max
            }) {
                Some(format!("the packed and golden files passed {:.1} GB", max as f64 / 1073741824.0))
            } else {
                None
            };
            if let Some(why) = why {
                let p = prog.lock().unwrap();
                let cpu = cpu_secs() - cpu0;
                let mut r = base.clone();
                r["status"] = json!(format!("stopped: {why}"));
                r["read_wall_s"] = json!((wall * 10.0).round() / 10.0);
                r["read_cpu_s"] = json!((cpu * 10.0).round() / 10.0);
                r["cpu_use"] = json!((cpu / wall * 100.0).round() / 100.0);
                r["peak_rss_mb"] = json!(status_kb("VmHWM:") / 1024);
                r["rss_peak_seen"] = json!({"mb": p.rss_peak_mb, "at": p.rss_peak_at});
                r["pass"] = json!(p.pass);
                r["records_in_pass"] = json!(p.records);
                r["runs_done_in_finished_batches"] = json!(p.jobs);
                r["batches"] = batches_json(&p.batches, &[]);
                r["packed_bytes_so_far"] = json!(std::fs::metadata(&packed).map(|m| m.len()).unwrap_or(0));
                r["load_at_end"] = json!(loadavg());
                write_json(&report_path, &r);
                log!(lg, "STOPPED: {why}; {} records in pass {}; report {}", p.records, p.pass, report_path.display());
                match (&abort, &stopped_w) {
                    (Some(a), Some(st)) => {
                        *st.lock().unwrap() = Some(why);
                        a.store(true, std::sync::atomic::Ordering::Relaxed);
                        return;
                    }
                    _ => std::process::exit(3),
                }
            }
        });
    }

    // the records are written on their own thread (par.rs `SinkThread`), unless they are kept or the reading has
    // no workers (one thread in all)
    let max_waiting: u64 = arg(&args, "--max-waiting").map_or(par::DEFAULT_MAX_WAITING, |n| n.parse().unwrap());
    let main_cpu0 = par::thread_cpu();
    let mut sink_cpu = 0.0;
    let stats = if keep || threads == 0 {
        par::with_pool(&game, &scope, threads, |pool| {
            pool.set_max_waiting(max_waiting);
            if let Some(a) = &abort {
                pool.set_abort(a.clone());
            }
            b.read_all_into(pool, true, None, 2, Some(&mut tee));
            pool.stats.clone()
        })
    } else {
        let mut sink = par::SinkThread::new(tee, 256);
        let stats = par::with_pool(&game, &scope, threads, |pool| {
            pool.set_max_waiting(max_waiting);
            if let Some(a) = &abort {
                pool.set_abort(a.clone());
            }
            b.read_all_into(pool, true, None, 2, Some(&mut sink));
            pool.stats.clone()
        });
        (tee, sink_cpu) = sink.finish_cpu();
        stats
    };
    let read_wall = t_read.elapsed().as_secs_f64();
    let main_cpu = par::thread_cpu() - main_cpu0;

    let read_cpu = cpu_secs() - cpu0;
    let read_rss = peak_rss_kb().unwrap_or(0);
    b.progress = None;
    log!(lg, "[{name}] read: {} records, {read_wall:.1} s wall, {read_cpu:.1} CPU s, peak RSS {} MB", b.n_records, read_rss / 1024);

    // the packed file's tables and footer, with the start
    let t_fin = Instant::now();
    let stream = tee.stream.take().unwrap();
    let keys = stream.fact_keys();
    let goal_labels: Vec<(String, Vec<Label>)> = goal.iter().map(|(k, v)| (k.clone(), v.iter().map(|&x| Label::Int(x)).collect())).collect();
    // E014 (FMI_GOAL_END=1): a goal key that is no fact of the game (the game's own end, `end`) has no domain; it gets
    // {its start value, the goal's values}. Nothing writes it, so the task never reaches it. Off: as before (every
    // goal key of a fact goal has its domain).
    if std::env::var("FMI_GOAL_END").is_ok_and(|v| v == "1") {
        for (k, vs) in &goal {
            if b.table.domain(k).is_none() {
                let mut v: Vec<i64> = vs.clone();
                v.push(start.get0(k));
                b.table.extra.insert(k.clone(), crate::domain::Domain::of(v, None, None));
            }
        }
    }
    let s0 = task::task_start(&mut b, &start, keys.iter().chain(goal.iter().map(|g| &g.0)));
    let fact_keys: Vec<String> = keys.iter().cloned().chain(goal.iter().map(|g| g.0.clone())).collect();
    let (info, folded, kept_actions, written) = if prep {
        let (i, f, k, w) = stream.finish_written(&mut b, &goal_labels, &[("start".to_string(), s0)]).expect("the packed file");
        (i, f, k, Some(w))
    } else {
        let (i, f, k) = stream.finish_keep(&mut b, &goal_labels, &[("start".to_string(), s0)]).expect("the packed file");
        (i, f, k, None)
    };
    // --prep: the packed file's sha1 on a thread of its own, while the rest is written; what the index and the head
    // read of the file comes from the writer (`Written`), not from reading it again
    let packed_sha = written.as_ref().map(|_| {
        let p = packed.clone();
        std::thread::spawn(move || files::file_sha1(&p).expect("the packed file"))
    });
    let reached_path = packed.with_extension("reached");
    if let Some(reached) = &b.table.reach {
        std::fs::write(&reached_path, reached.lines(&b.table, fact_keys.iter()).join("\n") + "\n").expect("the reached values");
        log!(lg, "[{name}] reached values: {}", reached_path.display());
    } else {
        // an older reading's, under this packed file's name
        std::fs::remove_file(&reached_path).ok();
    }
    let fin_s = t_fin.elapsed().as_secs_f64();
    // --out-dir: the packed file read once for the index and the head, which gives its sha1 too
    let t_tc = Instant::now();
    let hex = |d: &[u8; 20]| -> String { d.iter().map(|b| format!("{b:02x}")).collect() };
    // --prep with --out-dir: the sha1 is waited for where the index needs it
    let (mut tc, mut sha_pending) = match (written, packed_sha) {
        (Some(w), Some(h)) if out_dir.is_some() => (Some(files::Tc::from_written(w, [0; 20])), Some(h)),
        (Some(_), Some(h)) => (None, Some(h)),
        _ => (out_dir.as_ref().map(|_| files::Tc::read(&packed).expect("the packed file")), None),
    };
    let tc_s = t_tc.elapsed().as_secs_f64();
    let packed_sha1 = if let (Some(tc), None) = (&tc, &sha_pending) {
        tc.sha1_hex()
    } else if tc.is_some() {
        String::new()
    } else if let Some(h) = sha_pending.take() {
        hex(&h.join().expect("the packed file's sha1"))
    } else {
        let mut h = sha1_smol::Sha1::new();
        let mut f = std::fs::File::open(&packed).unwrap();
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let n = std::io::Read::read(&mut f, &mut buf).unwrap();
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        h.digest().to_string()
    };
    let counters = b.counters_py().text();
    if let (Some(g), Some((body, pool))) = (&golden, tee.body.take()) {
        let t = Instant::now();
        pool.finish();
        let mut out = std::io::BufWriter::with_capacity(1 << 20, std::fs::File::create(g).expect("the golden file"));
        let head = Py::dict(vec![("kind", Py::str("reading")), ("reading", b.reading_py(&[("name".to_string(), Py::str(&name))]))]).text();
        gz_member(&mut out, &head);
        std::io::copy(&mut std::fs::File::open(&body).expect("the golden body"), &mut out).expect("the golden file");
        gz_member(&mut out, &Py::dict(vec![("kind", Py::str("builder")), ("builder", b.counters_py())]).text());
        out.flush().expect("the golden file");
        std::fs::remove_file(&body).ok();
        log!(lg, "[{name}] golden file {} in {:.1} s", g.display(), t.elapsed().as_secs_f64());
    }
    let mut r = base;
    r["status"] = json!("done");
    r["records"] = json!(b.n_records);
    r["records_by_kind"] = json!(tee.kinds);
    if b.switches.havoc_cut {
        r["havoc_records"] = json!(tee.havoc);
    }
    let pass_counts: BTreeMap<String, i64> =
        b.counts.0.iter().filter(|(k, _)| k.starts_with("pass ") || k.starts_with("reach: ") || k.as_str() == "event rounds" || k.as_str() == "records").map(|(k, v)| (k.clone(), *v)).collect();
    r["reader_counts"] = json!(pass_counts);
    r["out_of_domain_keys"] = json!(b.out_of_domain.len());
    // the lists of locals the last pass folded (their `args:` numbers rewritten to 0 at its end): golden lines
    // streamed as made (--golden without --keep) still carry their numbers (replay_part1.py read_rust's stream)
    r["folded_lists"] = json!(b.folded.iter().copied().collect::<Vec<i64>>());
    r["outside_points"] = json!(b
        .outside_points
        .iter()
        .map(|(k, m)| (k.clone(), m.iter().map(|(w, n)| (w.to_string(), *n)).collect::<BTreeMap<_, _>>()))
        .collect::<BTreeMap<_, _>>());
    r["read_wall_s"] = json!((read_wall * 10.0).round() / 10.0);
    r["read_cpu_s"] = json!((read_cpu * 10.0).round() / 10.0);
    r["cpu_use"] = json!((read_cpu / read_wall * 100.0).round() / 100.0);
    r["read_peak_rss_mb"] = json!(read_rss / 1024);
    r["packed"] = json!({
        "file": packed.display().to_string(),
        "bytes": info.bytes,
        "actions": info.actions,
        "facts": info.facts,
        "strings": info.strings,
        "folded": folded,
        "args_values_zeroed": info.zeroed,
        "finish_s": (fin_s * 100.0).round() / 100.0,
        "sha1": packed_sha1,
    });
    if let Some(h) = &tee.hash {
        r["records_sha1"] = json!(h.digest().to_string());
        r["counters_sha1"] = json!(sha1_smol::Sha1::from(counters.as_bytes()).digest().to_string());
    }
    r["pool"] = json!({
        "batches": stats.batches,
        "jobs": stats.jobs,
        "job_s": (stats.job_secs * 10.0).round() / 10.0,
        "split_parts_queued": stats.parts_made,
        "split_parts_run_elsewhere": stats.parts_elsewhere,
        "snapshots_s": (stats.snap_secs * 100.0).round() / 100.0,
        "workers_sync_s": (stats.sync_secs * 100.0).round() / 100.0,
        "merge_and_stream_s": (stats.merge_secs * 100.0).round() / 100.0,
        "merge_counters_s": (stats.merge_job_secs * 100.0).round() / 100.0,
        "stream_records_s": (stats.each_secs * 100.0).round() / 100.0,
        "max_records_waiting": stats.max_buffered,
        "longest_jobs": stats.longest.iter().map(|(s, l, b)| json!([(s * 100.0).round() / 100.0, l, b])).collect::<Vec<_>>(),
    });
    if let Some(m) = &b.table.memo {
        let n = |a: &std::sync::atomic::AtomicU64| a.load(std::sync::atomic::Ordering::Relaxed);
        r["pass2_reuse"] = json!({
            "reused": n(&m.reused),
            "ran_job_differs": n(&m.stale_job),
            "ran_widened_fact": n(&m.stale_domain),
            "ran_folded_list": n(&m.stale_list),
            "ran_reached_value": n(&m.stale_reached),
            "ran_not_in_pass_1": n(&m.none),
        });
    }
    if let Some(m) = b.memo_json() {
        r["memo"] = m;
    }
    if let Some(sh) = &b.table.shared {
        let n = |a: &std::sync::atomic::AtomicU64| a.load(std::sync::atomic::Ordering::Relaxed);
        r["shared_runs"] = json!({
            "made": n(&sh.made),
            "taken": n(&sh.taken),
            "not_kept": n(&sh.not_kept),
            "waited_s": (n(&sh.waited_ns) as f64 / 1e7).round() / 100.0,
        });
    }
    if let Some(done) = &fact_vars_done {
        // the switch --fact-vars: per variable, what was done, its domain at the end, and the values the last pass
        // wrote outside it (Lemma R's premise R0 asks for none)
        r["fact_vars"] = json!(done
            .iter()
            .map(|d| {
                let key = format!("var:{}", d.n);
                let dom: Vec<Value> = b
                    .table
                    .domain(&key)
                    .map(|x| x.values.iter().map(|l| l.int().map_or_else(|| json!(l.text()), |v| json!(v))).collect())
                    .unwrap_or_default();
                json!({
                    "var": d.n,
                    "what": d.what,
                    "seeds": d.seeds,
                    "kind": b.kind(&key).as_str(),
                    "domain": dom,
                    "outside_after_last_pass": b.out_of_domain.get(&key).map(|s| s.iter().copied().collect::<Vec<i64>>()).unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>());
    }
    r["sink_thread_busy_s"] = json!((tee.busy * 10.0).round() / 10.0);
    // the reading's thread's and the sink thread's CPU seconds while it read (the workers' are the rest)
    r["reading_thread_cpu_s"] = json!((main_cpu * 100.0).round() / 100.0);
    r["sink_thread_cpu_s"] = json!((sink_cpu * 100.0).round() / 100.0);
    r["phases_s"] = json!(b.phase_secs.iter().map(|(k, v)| (k.to_string(), (v * 100.0).round() / 100.0)).collect::<BTreeMap<_, _>>());
    {
        let p = prog.lock().unwrap();
        r["batches"] = batches_json(&p.batches, &stats.per_batch);
        r["rss_peak_seen"] = json!({"mb": p.rss_peak_mb, "at": p.rss_peak_at});
    }
    // F10: the lists the dumped slots' events were read with (the harness matches the slots against them after a
    // reading made again), each as packed_reading's list lines
    // (under --live-locals with the slot's flags, which the number covers, as packed_reading.py's lines have them)
    let flag = |p: &Py| match p {
        Py::Bool(x) => json!(x),
        Py::None => Value::Null,
        other => json!({"unknown": other.text()}),
    };
    r["slot_events"] = json!(task::slot_lists(&b)
        .into_iter()
        .flat_map(|((name, w), ls)| ls.into_iter().map(move |l| (name.clone(), w, l)))
        .map(|(name, w, l)| {
            let mut x = json!({
                "name": name,
                "w": w,
                "sig": l.sig,
                "locals": l.locals.iter().map(|(n, v)| json!([n, v.int().expect("a dumped local is a number")])).collect::<Vec<_>>(),
                "override": l.override_,
                "cut_data": l.cut_data,
            });
            if live_locals {
                x["flags"] = json!({"recursive": flag(&l.recursive), "resistant": flag(&l.resistant), "owned": flag(&l.owned)});
            }
            x
        })
        .collect::<Vec<_>>());
    r["peak_rss_mb"] = json!(peak_rss_kb().unwrap_or(0) / 1024);
    r["total_s"] = json!((t_all.elapsed().as_secs_f64() * 10.0).round() / 10.0);
    r["load_at_end"] = json!(loadavg());
    write_json(&report_path, &r);
    if let (Some(dir), Some(fl), Some(tc)) = (&out_dir, tee.files.take(), &mut tc) {
        // --out-dir: the line files made final, the index, the head (files.rs)
        let t = Instant::now();
        let extra: Vec<PJ> = r["slot_events"].as_array().map_or_else(Vec::new, |xs| xs.iter().map(PJ::from_serde).collect());
        let lines = fl.finish(&kept_actions, &b.folded, &extra).expect("the reading's line files");
        if let Some(h) = sha_pending.take() {
            tc.sha1 = h.join().expect("the packed file's sha1");
            r["packed"]["sha1"] = json!(tc.sha1_hex());
        }
        let tc = &*tc;
        files::write_idx(&dir.join(files::IDX), tc, &lines).expect("the reading's index");
        let bounds = files::bounds_pj(tc, &|k| b.table.domain(k).map(|d| d.into_owned())).unwrap_or_else(|e| panic!("{e}"));
        let rows = match &start_rows {
            Some(f) => PathBuf::from(f),
            None => root.join("experiments/E003-path-through-cloud/results/start-part1.jsonl"),
        };
        let start_facts = start_row_facts(&rows, start_seed);
        let slot_locals_pj: Vec<PJ> = arg(&args, "--slot-locals").map_or_else(Vec::new, |p| {
            std::fs::read_to_string(&p)
                .unwrap_or_else(|e| panic!("{p}: {e}"))
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| PJ::from_json(&crate::scaling::Json::parse(l)))
                .collect()
        });
        // replay_part1.py read_rust's info, in its order
        let mut rs: Vec<(String, PJ)> = ["records", "read_wall_s", "peak_rss_mb", "threads", "slot_locals"]
            .iter()
            .map(|k| (k.to_string(), PJ::from_serde(&r[*k])))
            .collect();
        if reach {
            rs.push(("read_cpu_s".into(), PJ::from_serde(&r["read_cpu_s"])));
        }
        let mut info: Vec<(String, PJ)> = vec![
            ("name".into(), PJ::str(name.clone())),
            ("rooms".into(), PJ::ints(rooms.iter().map(|&x| x as i64))),
            ("domain_goals".into(), PJ::str(domain_goals.clone())),
            ("reader".into(), PJ::str("rust")),
            ("read_s".into(), PJ::Float((t_all.elapsed().as_secs_f64() * 10.0).round() / 10.0)),
            ("reader_scope".into(), PJ::Obj(rs)),
            ("slot_locals".into(), PJ::List(slot_locals_pj)),
        ];
        let flag = |info: &mut Vec<(String, PJ)>, k: &str, on: bool| {
            if on {
                info.push((k.into(), PJ::Bool(true)));
            }
        };
        flag(&mut info, "elsewhere", elsewhere);
        flag(&mut info, "split_limit", split_limit);
        if let Some(sw) = arg(&args, "--switches").filter(|s| !s.is_empty()) {
            info.push(("switches".into(), PJ::str(sw)));
        }
        flag(&mut info, "reach", reach);
        let also: Vec<PJ> = args.windows(2).filter(|w| w[0] == "--reach-also").map(|w| PJ::str(w[1].clone())).collect();
        if !also.is_empty() {
            info.push(("reach_also".into(), PJ::List(also)));
        }
        if let Some(fv) = fact_vars.as_ref().filter(|v| !v.is_empty()) {
            info.push(("fact_vars".into(), PJ::ints(fv.iter().map(|&n| n as i64))));
            info.push(("fact_vars_report".into(), PJ::from_serde(&r["fact_vars"])));
        }
        let n_lists = lines.list_off.len() - 1;
        let doc = files::head(
            tc,
            files::HeadIn {
                name: &name,
                n_lists,
                n_actions: tc.words.len(),
                n_records: lines.n_records,
                start_name: base_start.clone(),
                start_facts: &start_facts,
                rooms_scope: &b.scope.rooms,
                walk_verb: b.scope.walk_verb,
                verbs: &b.scope.verbs,
                dialog_verbs: &b.scope.dialog_verbs,
                bounds,
                counts: PJ::from_py(&b.counts.py()),
                golden: golden.as_deref(),
                packed: &packed,
                live_locals: b.table.live_locals.as_ref().map(|ll| {
                    PJ::from_py(&Py::dict(vec![("switch", Py::str("--live-locals")), ("slots", ll.slots_py(&game))]))
                }),
                frame_costs: b.frame_costs.as_ref().map(|fc| PJ::from_py(&fc.py())),
                info,
                slot_lists: extra.len(),
            },
        );
        files::write_atomic(&dir.join(files::HEAD), doc.indent1_text().as_bytes()).expect("reading.json");
        let files_s = t.elapsed().as_secs_f64();
        r["files"] = json!({
            "actions": tc.words.len(),
            "lists": n_lists,
            "records": lines.n_records,
            "sink_s": (lines.sink_s * 100.0).round() / 100.0,
            "sink_cpu_s": (lines.sink_cpu_s * 100.0).round() / 100.0,
            "tc_s": (tc_s * 100.0).round() / 100.0,
            "finish_s": (files_s * 100.0).round() / 100.0,
        });
        write_json(&report_path, &r);
        log!(lg, 
            "[{name}] reading files in {}: {} actions, {} lists; {:.2} s in the sink ({:.2} CPU s), {:.2} s reading the packed file, {:.2} s at the end",
            dir.display(),
            tc.words.len(),
            n_lists,
            lines.sink_s,
            lines.sink_cpu_s,
            tc_s,
            files_s
        );
    }
    if let Some(dump) = &dump {
        let smp = sample.lock().unwrap();
        let mut f = std::io::BufWriter::new(std::fs::File::create(dump).unwrap());
        let widened = Py::List(
            b.widened
                .iter()
                .map(|m| Py::Dict(m.iter().map(|(k, v)| (k.clone(), Py::Set(v.iter().map(|&x| Py::Int(x)).collect()))).collect()))
                .collect(),
        );
        let head = Py::dict(vec![
            ("kind", Py::str("head")),
            ("name", Py::str(&name)),
            ("rooms", Py::List(rooms.iter().map(|&r| Py::Int(r as i64)).collect())),
            ("seed", Py::Int(start_seed)),
            ("pass", Py::Int(smp.pass as i64)),
            ("widened", widened),
            ("per_stratum", Py::Int(smp.per as i64)),
            ("rng_seed", Py::Int(smp.seed as i64)),
            ("strata", Py::Dict(smp.strata.iter().map(|(k, v)| (k.clone(), Py::Int(v.0 as i64))).collect())),
        ]);
        writeln!(f, "{}", head.text()).unwrap();
        let mut picked: Vec<&(u64, String)> = smp.strata.values().flat_map(|v| v.1.iter()).collect();
        picked.sort_by_key(|p| p.0);
        for (_, line) in picked {
            writeln!(f, "{line}").unwrap();
        }
        log!(lg, "[{name}] dumped {} jobs to {dump}", smp.strata.values().map(|v| v.1.len()).sum::<usize>());
    }
    if let Some(lines) = arg(&args, "--lines") {
        assert!(keep, "--lines needs --keep");
        let f = std::fs::File::create(&lines).unwrap();
        let mut gz = flate2::write::GzEncoder::new(std::io::BufWriter::new(f), flate2::Compression::new(6));
        for l in b.reading_lines(&[("name".to_string(), Py::str(&name))]) {
            gz.write_all(l.as_bytes()).unwrap();
            gz.write_all(b"\n").unwrap();
        }
        gz.finish().unwrap();
    }
    log!(lg, 
        "[{name}] done: {} records, read {read_wall:.1} s wall {read_cpu:.1} CPU s ({:.2} CPUs), peak RSS {} MB; packed {} bytes, {} actions ({folded} folded); report {}",
        b.n_records,
        read_cpu / read_wall,
        read_rss / 1024,
        info.bytes,
        info.actions,
        report_path.display()
    );
    drop(done);
    r
}
