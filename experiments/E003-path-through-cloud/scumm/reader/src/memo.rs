//! The switch pass2_reuse (experiments/E004-fast-exact/pass2-reuse.md has the proof): pass 1 keeps each run's output with
//! what the run depended on; a run of pass 2 whose job, guides, widened facts, folded lists and reached values agree
//! takes that output without running. The files and counters are byte for byte those without the switch.

use crate::builder::{Builder, Deltas, Job, Machine};
use crate::domain::Labels;
use crate::reach::Guide;
use crate::record::{Pins, Record};
use crate::table::FactTable;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
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

// --- the job memo across readings (experiments/E011-speed/memo.md) --------------------------------------------------

/// A consulted key as a run read it (memo.md, condition 3): the key, its kind (0 constant, 1 fact, 2 non-fact), its
/// domain and, for a constant, its start value.
pub type Info = (String, u8, Option<crate::domain::Domain>, Option<i64>);

/// Why a stored run was not taken (counted).
#[derive(Copy, Clone, Debug)]
pub enum Miss {
    Key,
    List,
    Reached,
}

/// A run's output: its records, what it added to the Builder's counters, its last pins.
pub type Out = (Vec<Record>, Deltas, Pins);

const FILE_MAGIC: &[u8; 4] = b"FMRM";
const INDEX_MAGIC: &[u8; 4] = b"FMRI";
const VERSION: u32 = 3;

/// One stored run: where its bytes are.
#[derive(Copy, Clone)]
struct Loc {
    file: u32,
    off: u64,
    len: u32,
}

/// One store file's consulted keys (each distinct `Info` once; runs name them by number), and whether each reads the
/// same in this pass's table: 0 not asked yet, 1 yes, 2 no. The answers are the same for every worker (they hold the
/// pass's table; `args:` keys are not among them), and are forgotten between passes.
struct FileInfos {
    infos: Vec<Info>,
    same: Vec<AtomicU8>,
}

impl FileInfos {
    fn push(&mut self, i: Info) {
        self.infos.push(i);
        self.same.push(AtomicU8::new(0));
    }
}

/// A run on its way to the writer: its hash, its consulted keys, and the rest of its bytes (key, lists, questions,
/// output).
struct WRun {
    hash: u64,
    key: Vec<u8>,
    infos: Vec<Info>,
    /// the list answers and reach questions
    rest: Vec<u8>,
    /// the output, before deflate
    out: Vec<u8>,
}

enum WMsg {
    Run(Box<WRun>),
    /// the runs written and the keys numbered since the last flush, once they are on disk
    Flush(std::sync::mpsc::SyncSender<(Vec<(u64, u64, u32)>, Vec<Info>)>),
}

struct Writer {
    tx: std::sync::mpsc::SyncSender<WMsg>,
    handle: std::thread::JoinHandle<std::io::Result<usize>>,
    tmp: std::path::PathBuf,
    /// the file's place in `files` once opened for reading
    file: Option<u32>,
}

/// The counts a reading reports (memo.md, the check).
#[derive(Default)]
pub struct CrossStats {
    pub taken: AtomicU64,
    pub ran_no_run: AtomicU64,
    pub ran_key_differs: AtomicU64,
    pub ran_list_differs: AtomicU64,
    pub ran_reached_differs: AtomicU64,
    pub stored: AtomicU64,
    /// the bytes the writer wrote (outputs deflated there)
    pub stored_bytes: Arc<AtomicU64>,
    /// of the stored bytes: the keys, the rest of the conditions, the outputs before deflate
    pub key_bytes: AtomicU64,
    pub cond_bytes: AtomicU64,
    pub out_raw_bytes: AtomicU64,
    /// nanoseconds the workers spent looking runs up and taking them, and encoding the runs they stored
    pub lookup_ns: AtomicU64,
    pub store_ns: AtomicU64,
}

/// The job memo across readings: the stored runs of every reading of the same whole-reading inputs (memo.md, "The
/// store"), and this reading's file of the runs it makes.
pub struct CrossMemo {
    pub dir: std::path::PathBuf,
    files: RwLock<Vec<(std::fs::File, FileInfos)>>,
    index: RwLock<FxHashMap<u64, Vec<Loc>>>,
    writer: Mutex<Option<Writer>>,
    pub stats: CrossStats,
    /// the finished files found when the reading opened the store, and their runs
    pub opened_files: usize,
    pub opened_runs: usize,
    /// why this reading writes no file (no room on the disk), if it writes none
    pub no_write: Option<String>,
}

/// The FxHash of a run's key bytes (the job and its guides).
pub fn key_hash(b: &[u8]) -> u64 {
    use std::hash::Hasher;
    let mut h = rustc_hash::FxHasher::default();
    h.write(b);
    h.finish()
}

/// A finished file's runs (hash, offset, length) and its keys' table. A file ends with its keys' table, its runs'
/// index, the run count, the two places and the magic.
fn read_footer(f: &std::fs::File) -> std::io::Result<(Vec<(u64, u64, u32)>, Vec<Info>)> {
    use crate::wire::{Rd, Wire};
    use std::os::unix::fs::FileExt;
    let len = f.metadata()?.len();
    let bad = || std::io::Error::new(std::io::ErrorKind::InvalidData, "not a finished memo file");
    if len < 8 + 28 {
        return Err(bad());
    }
    let mut head = [0u8; 8];
    f.read_exact_at(&mut head, 0)?;
    if &head[..4] != FILE_MAGIC || u32::from_le_bytes(head[4..8].try_into().unwrap()) != VERSION {
        return Err(bad());
    }
    let mut tail = [0u8; 28];
    f.read_exact_at(&mut tail, len - 28)?;
    if &tail[24..] != INDEX_MAGIC {
        return Err(bad());
    }
    let n = u64::from_le_bytes(tail[0..8].try_into().unwrap()) as usize;
    let infos_at = u64::from_le_bytes(tail[8..16].try_into().unwrap());
    let index_at = u64::from_le_bytes(tail[16..24].try_into().unwrap());
    if infos_at > index_at || index_at.checked_add(n as u64 * 20).is_none_or(|e| e != len - 28) {
        return Err(bad());
    }
    let mut buf = vec![0u8; (len - 28 - infos_at) as usize];
    f.read_exact_at(&mut buf, infos_at)?;
    let split = (index_at - infos_at) as usize;
    let infos: Vec<Info> = {
        let mut r = Rd::new(&buf[..split]);
        let v = Vec::take(&mut r);
        if !r.at_end() {
            return Err(bad());
        }
        v
    };
    let ix = buf[split..]
        .chunks_exact(20)
        .map(|c| (u64::from_le_bytes(c[0..8].try_into().unwrap()), u64::from_le_bytes(c[8..16].try_into().unwrap()), u32::from_le_bytes(c[16..20].try_into().unwrap())))
        .collect();
    Ok((ix, infos))
}

thread_local! {
    /// this thread's deflate and inflate states, reused from run to run
    static ZIP: std::cell::RefCell<(flate2::Compress, flate2::Decompress)> =
        std::cell::RefCell::new((flate2::Compress::new(flate2::Compression::fast(), false), flate2::Decompress::new(false)));
}

/// `b` deflated (raw deflate, the fast level).
fn deflate(b: &[u8]) -> Vec<u8> {
    ZIP.with(|z| {
        let c = &mut z.borrow_mut().0;
        c.reset();
        let mut out = Vec::with_capacity(b.len() / 3 + 64);
        loop {
            let at = c.total_in() as usize;
            match c.compress_vec(&b[at..], &mut out, flate2::FlushCompress::Finish).expect("deflate") {
                flate2::Status::StreamEnd => return out,
                _ => out.reserve(out.capacity().max(256)),
            }
        }
    })
}

/// `z` inflated, `raw` bytes.
fn inflate(z: &[u8], raw: usize) -> Vec<u8> {
    ZIP.with(|st| {
        let d = &mut st.borrow_mut().1;
        d.reset(false);
        let mut out = Vec::with_capacity(raw + 64);
        let status = d.decompress_vec(z, &mut out, flate2::FlushDecompress::Finish).expect("the memo store: a run's output");
        assert!(status == flate2::Status::StreamEnd && out.len() == raw, "the memo store: a run's output length");
        out
    })
}

/// Bytes free on the file system that holds `dir` (`df`), None when it cannot be told.
fn free_bytes(dir: &std::path::Path) -> Option<u64> {
    let out = std::process::Command::new("df").arg("-B1").arg("--output=avail").arg(dir).output().ok()?;
    String::from_utf8(out.stdout).ok()?.lines().nth(1)?.trim().parse().ok()
}

/// Makes room on the store's disk, by E009's level (memo.md, "The store"): while it has less than its floor plus twice
/// the largest store file (`largest`, the largest under `root` once they are listed), the files of the other folders
/// under `root` go, the oldest first (a reading that opens a file renews its time). The folder `keep` (this reading's,
/// whose files it holds open) is left. Returns the largest file size seen.
fn evict(root: &std::path::Path, keep: &std::path::Path, largest: u64) -> u64 {
    match free_bytes(root) {
        Some(free) => evict_from(root, keep, largest, free, FLOOR_BYTES),
        None => largest,
    }
}

/// `evict` with the disk's free bytes and floor given.
fn evict_from(root: &std::path::Path, keep: &std::path::Path, largest: u64, mut free: u64, floor: u64) -> u64 {
    let mut all: Vec<(std::time::SystemTime, u64, std::path::PathBuf)> = Vec::new();
    for d in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let d = d.path();
        if d == keep || !d.is_dir() {
            continue;
        }
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "runs") {
                if let Ok(m) = e.metadata() {
                    all.push((m.modified().unwrap_or(std::time::UNIX_EPOCH), m.len(), p));
                }
            }
        }
    }
    let largest = all.iter().map(|f| f.1).max().unwrap_or(0).max(largest);
    all.sort();
    for (_, len, p) in all {
        if free >= floor + 2 * largest {
            break;
        }
        if std::fs::remove_file(&p).is_ok() {
            eprintln!("[memo] {}: evicted ({} MB), the disk under its floor", p.display(), len >> 20);
            free += len;
        }
    }
    largest
}

/// The disk's floor under the store (memo.md, "The store"): 8 GB.
pub const FLOOR_BYTES: u64 = 8 << 30;

impl CrossMemo {
    /// Opens the store folder `root/<key>`: indexes every finished file in it, and starts this reading's own file
    /// unless the disk is under its floor plus twice the folder's largest file.
    pub fn open(root: &std::path::Path, key: &str) -> std::io::Result<CrossMemo> {
        let dir = root.join(key);
        std::fs::create_dir_all(&dir)?;
        let mut names: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "runs"))
            .collect();
        names.sort();
        let mut files = Vec::new();
        let mut index: FxHashMap<u64, Vec<Loc>> = FxHashMap::default();
        let (mut runs, mut largest) = (0usize, 0u64);
        for p in &names {
            let f = std::fs::File::open(p)?;
            let Ok((ix, infos)) = read_footer(&f) else {
                eprintln!("[memo] {}: not a finished memo file of this version, left out", p.display());
                continue;
            };
            largest = largest.max(f.metadata()?.len());
            let fi = files.len() as u32;
            for (h, off, len) in ix {
                index.entry(h).or_default().push(Loc { file: fi, off, len });
                runs += 1;
            }
            let same = infos.iter().map(|_| AtomicU8::new(0)).collect();
            // the file is in use: its time is renewed, so that eviction takes the stores no reading uses first
            f.set_modified(std::time::SystemTime::now()).ok();
            files.push((f, FileInfos { infos, same }));
        }
        let opened_files = files.len();
        let largest = evict(root, &dir, largest);
        let no_write = match free_bytes(&dir) {
            Some(f) if f < FLOOR_BYTES + 2 * largest => {
                Some(format!("{} GB free, under the floor of 8 GB plus twice the largest file ({} MB)", f >> 30, largest >> 20))
            }
            None => Some("the free space is not known (df)".to_string()),
            _ => None,
        };
        let stats = CrossStats::default();
        let writer = match &no_write {
            Some(why) => {
                eprintln!("[memo] {}: this reading stores no runs: {why}", dir.display());
                None
            }
            None => Some(Self::start_writer(&dir, stats.stored_bytes.clone())?),
        };
        Ok(CrossMemo {
            dir,
            files: RwLock::new(files),
            index: RwLock::new(index),
            writer: Mutex::new(writer),
            stats,
            opened_files,
            opened_runs: runs,
            no_write,
        })
    }

    fn start_writer(dir: &std::path::Path, written: Arc<AtomicU64>) -> std::io::Result<Writer> {
        use crate::wire::{put_len, Wire};
        use std::io::Write;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let tmp = dir.join(format!("{stamp}-{}.tmp", std::process::id()));
        let f = std::fs::File::create(&tmp)?;
        let (tx, rx) = std::sync::mpsc::sync_channel::<WMsg>(4096);
        let handle = std::thread::Builder::new().name("memo-writer".into()).spawn(move || {
            let mut out = std::io::BufWriter::with_capacity(1 << 20, f);
            out.write_all(FILE_MAGIC)?;
            out.write_all(&VERSION.to_le_bytes())?;
            let mut at: u64 = 8;
            let mut ix: Vec<(u64, u64, u32)> = Vec::new();
            // each distinct consulted key, numbered in the order first stored
            let mut ids: FxHashMap<Vec<u8>, u32> = FxHashMap::default();
            let mut infos: Vec<Info> = Vec::new();
            let (mut ix_sent, mut infos_sent) = (0usize, 0usize);
            let mut b: Vec<u8> = Vec::with_capacity(1 << 16);
            for m in rx {
                match m {
                    WMsg::Run(run) => {
                        let WRun { hash, key, infos: run_infos, rest, out: raw } = *run;
                        b.clear();
                        hash.put(&mut b);
                        put_len(&mut b, key.len());
                        b.extend_from_slice(&key);
                        put_len(&mut b, run_infos.len());
                        for i in run_infos {
                            let mut ib = Vec::new();
                            i.put(&mut ib);
                            let n = ids.len() as u32;
                            let id = *ids.entry(ib).or_insert_with(|| {
                                infos.push(i);
                                n
                            });
                            id.put(&mut b);
                        }
                        b.extend_from_slice(&rest);
                        let z = deflate(&raw);
                        put_len(&mut b, raw.len());
                        put_len(&mut b, z.len());
                        b.extend_from_slice(&z);
                        written.fetch_add(b.len() as u64, Ordering::Relaxed);
                        out.write_all(&b)?;
                        ix.push((hash, at, b.len() as u32));
                        at += b.len() as u64;
                    }
                    WMsg::Flush(reply) => {
                        out.flush()?;
                        reply.send((ix[ix_sent..].to_vec(), infos[infos_sent..].to_vec())).ok();
                        ix_sent = ix.len();
                        infos_sent = infos.len();
                    }
                }
            }
            // the keys' table and the runs' index after the runs, then the count, the two places and the magic: a
            // file is finished when it ends so
            let infos_at = at;
            let mut tb = Vec::new();
            infos.put(&mut tb);
            out.write_all(&tb)?;
            let index_at = infos_at + tb.len() as u64;
            for (h, off, len) in &ix {
                out.write_all(&h.to_le_bytes())?;
                out.write_all(&off.to_le_bytes())?;
                out.write_all(&len.to_le_bytes())?;
            }
            out.write_all(&(ix.len() as u64).to_le_bytes())?;
            out.write_all(&infos_at.to_le_bytes())?;
            out.write_all(&index_at.to_le_bytes())?;
            out.write_all(INDEX_MAGIC)?;
            out.flush()?;
            out.into_inner().map_err(|e| e.into_error())?.sync_all()?;
            Ok(ix.len())
        })?;
        Ok(Writer { tx, handle, tmp, file: None })
    }

    /// Whether this reading stores the runs it makes.
    pub fn writes(&self) -> bool {
        self.writer.lock().unwrap().is_some()
    }

    /// The stored run of these key bytes for which the conditions hold here (memo.md, conditions 2-5), decoded:
    /// `info` tells whether a consulted key reads the same here (asked once per file and key in a pass), `rest`
    /// checks the list answers and the reach questions. Candidates of the same hash are compared byte for byte.
    pub fn take(
        &self,
        key: &[u8],
        info: &mut dyn FnMut(&Info) -> bool,
        rest: &mut dyn FnMut(&[(i64, bool)], &[Query]) -> Result<(), Miss>,
    ) -> Option<Out> {
        use crate::wire::{take_len, Rd, Wire};
        use std::os::unix::fs::FileExt;
        let h = key_hash(key);
        let locs: Vec<Loc> = match self.index.read().unwrap().get(&h) {
            Some(v) => v.clone(),
            None => {
                self.stats.ran_no_run.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        };
        let files = self.files.read().unwrap();
        let mut last: Option<Miss> = None;
        let mut buf = Vec::new();
        'run: for l in locs {
            let (f, fi) = &files[l.file as usize];
            buf.resize(l.len as usize, 0);
            f.read_exact_at(&mut buf, l.off).expect("the memo store: a run's bytes");
            let mut r = Rd::new(&buf);
            let _hash = u64::take(&mut r);
            let n = take_len(&mut r);
            if r.bytes(n) != key {
                continue;
            }
            let n = take_len(&mut r);
            for _ in 0..n {
                let id = u32::take(&mut r) as usize;
                let same = match fi.same[id].load(Ordering::Relaxed) {
                    0 => {
                        let ok = info(&fi.infos[id]);
                        fi.same[id].store(if ok { 1 } else { 2 }, Ordering::Relaxed);
                        ok
                    }
                    x => x == 1,
                };
                if !same {
                    last = Some(Miss::Key);
                    continue 'run;
                }
            }
            let sigs: Vec<(i64, bool)> = Vec::take(&mut r);
            let queries: Vec<Query> = Vec::take(&mut r);
            if let Err(m) = rest(&sigs, &queries) {
                last = Some(m);
                continue;
            }
            let raw = take_len(&mut r);
            let zn = take_len(&mut r);
            let z = r.bytes(zn);
            assert!(r.at_end(), "the memo store: bytes after a run");
            let o = inflate(z, raw);
            let mut r = Rd::new(&o);
            let records = Vec::take(&mut r);
            let deltas = Deltas::take(&mut r);
            let pins = std::collections::BTreeMap::take(&mut r);
            assert!(r.at_end(), "the memo store: bytes after a run's output");
            self.stats.taken.fetch_add(1, Ordering::Relaxed);
            return Some((records, deltas, pins));
        }
        let c = match last {
            None => &self.stats.ran_no_run,
            Some(Miss::Key) => &self.stats.ran_key_differs,
            Some(Miss::List) => &self.stats.ran_list_differs,
            Some(Miss::Reached) => &self.stats.ran_reached_differs,
        };
        c.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Stores a run this reading made: its key bytes, the keys it consulted, its list answers and reach questions,
    /// its output (deflated: records and counters, much of them repeated text).
    pub fn store(&self, key: Vec<u8>, infos: Vec<Info>, sigs: &[(i64, bool)], queries: &[Query], out: &Out) {
        use crate::wire::Wire;
        let tx = match self.writer.lock().unwrap().as_ref() {
            Some(w) => w.tx.clone(),
            None => return,
        };
        let mut rest = Vec::with_capacity(4096);
        sigs.to_vec().put(&mut rest);
        queries.to_vec().put(&mut rest);
        let cond = rest.len();
        let mut o = Vec::with_capacity(4096);
        out.0.put(&mut o);
        out.1.put(&mut o);
        out.2.put(&mut o);
        let s = &self.stats;
        s.key_bytes.fetch_add(key.len() as u64, Ordering::Relaxed);
        s.cond_bytes.fetch_add((cond + 4 * infos.len()) as u64, Ordering::Relaxed);
        s.out_raw_bytes.fetch_add(o.len() as u64, Ordering::Relaxed);
        s.stored.fetch_add(1, Ordering::Relaxed);
        // the output is deflated on the writer's thread, off the workers'
        tx.send(WMsg::Run(Box::new(WRun { hash: key_hash(&key), key, infos, rest, out: o }))).expect("the memo writer");
    }

    /// Between passes: the runs this reading stored so far join the index (memo.md: pass 2 takes pass 1's runs), and
    /// every key's answer is forgotten (the next pass's table may read it otherwise).
    pub fn index_own(&self) {
        let mut wg = self.writer.lock().unwrap();
        if let Some(w) = wg.as_mut() {
            let (rtx, rrx) = std::sync::mpsc::sync_channel(1);
            w.tx.send(WMsg::Flush(rtx)).expect("the memo writer");
            let (ix, new_infos) = rrx.recv().expect("the memo writer");
            let mut files = self.files.write().unwrap();
            let fi = match w.file {
                Some(fi) => fi,
                None => {
                    let f = std::fs::File::open(&w.tmp).expect("the memo store's own file");
                    files.push((f, FileInfos { infos: Vec::new(), same: Vec::new() }));
                    let fi = files.len() as u32 - 1;
                    w.file = Some(fi);
                    fi
                }
            };
            for i in new_infos {
                files[fi as usize].1.push(i);
            }
            let mut index = self.index.write().unwrap();
            for (h, off, len) in ix {
                index.entry(h).or_default().push(Loc { file: fi, off, len });
            }
        }
        for (_, fi) in self.files.read().unwrap().iter() {
            for a in &fi.same {
                a.store(0, Ordering::Relaxed);
            }
        }
    }

    /// The reading is over: its file is finished and takes its place in the folder. Returns its runs.
    pub fn finish(&self) -> std::io::Result<usize> {
        let Some(w) = self.writer.lock().unwrap().take() else { return Ok(0) };
        drop(w.tx);
        let n = w.handle.join().expect("the memo writer")?;
        let done = w.tmp.with_extension("runs");
        std::fs::rename(&w.tmp, &done)?;
        Ok(n)
    }
}

/// sha1 of a file's bytes, hex.
fn file_sha1(p: &std::path::Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(p)?;
    let mut h = sha1_smol::Sha1::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.digest().to_string())
}

impl Builder<'_> {
    /// The canonical text of the whole reading's fixed inputs (memo.md, item 3): the reader's code, the game, the
    /// scope, the Builder's switches and the table's parts that are not per key. Its sha1 names the store folder.
    pub fn memo_inputs(&self) -> String {
        use std::fmt::Write;
        let mut t = String::new();
        let exe = std::env::current_exe().expect("the reader's binary");
        let root = scumm_ir::default_root();
        writeln!(t, "reader {}", file_sha1(&exe).expect("the reader's binary")).unwrap();
        writeln!(t, "game {}", file_sha1(&root.join("data").join("scripts-dos-cd").join("index.json")).expect("the game index")).unwrap();
        let s = &self.scope;
        let units: Vec<scumm_ir::UnitKey> = s.units.iter().map(|&u| self.game.unit(u).key).collect();
        writeln!(t, "rooms {:?}\nunits {units:?}\nobjects {:?}\nverbs {:?}\ndialog {:?}\nwalk {:?}", s.rooms, s.objects, s.verbs, s.dialog_verbs, s.walk_verb).unwrap();
        writeln!(t, "universe {:?}\nwrites {:?}\nscope switches {:?}\nscope counts {:?}", s.universe, s.writes, s.switches, s.counts).unwrap();
        let tb = &self.table;
        writeln!(t, "points {:?}\nengine {:?}\nrefined {:?}", tb.points, tb.engine, tb.refined).unwrap();
        writeln!(
            t,
            "flags elsewhere {} live_locals {} value_sets {} split_limit {} grow_limit {} havoc_cut {} step_limit {} havoc_starts {} havoc_rooms {} havoc_args {} wait_both {} reach {}",
            tb.elsewhere,
            tb.live_locals.is_some(),
            tb.value_sets,
            tb.split_limit,
            tb.grow_limit,
            tb.havoc_cut,
            tb.step_limit,
            tb.havoc_starts,
            tb.havoc_rooms,
            tb.havoc_args.is_some(),
            tb.wait_both,
            tb.reach.is_some()
        )
        .unwrap();
        match &tb.settled {
            None => writeln!(t, "settled none").unwrap(),
            Some(r) => {
                let mut reads: Vec<_> = r.reads.iter().collect();
                reads.sort();
                writeln!(
                    t,
                    "settled plain {:?} vars {} {} {} {} {} hover {:?} wait {} guards {:?} reads {reads:?} installed {:?} anywhere {:?}",
                    r.plain, r.v, r.a, r.b, r.p, r.last, r.hover, r.wait, r.guards, r.installed, r.anywhere
                )
                .unwrap()
            }
        }
        writeln!(t, "builder {:?} sig_limit {} path_limit {} machine path limit {}", self.switches, self.sig_limit, self.path_limit_now, crate::machine::PATH_LIMIT).unwrap();
        writeln!(t, "slot locals {:?}", self.slot_locals).unwrap();
        t
    }

    /// The switch `--memo DIR` (memo.md): opens the store for this reading's whole-reading inputs. Called by
    /// `read_all_into` once the table has the switches; refuses the settings the proof does not cover.
    pub(crate) fn open_memo(&mut self, root: &std::path::Path) {
        let sw = &self.switches;
        let refuse = |what: &str| -> ! { panic!("--memo: not with {what} (experiments/E011-speed/memo.md covers E009's settings only)") };
        if !sw.reach {
            refuse("the switch --reach off");
        }
        if !sw.split_limit {
            refuse("the switch --split-limit off");
        }
        for (on, name) in [
            (sw.pass2_reuse, "pass2_reuse"),
            (sw.shared_runs, "shared_runs"),
            (sw.havoc_cut, "havoc_cut"),
            (sw.grow_limit, "grow_limit"),
            (sw.due_first, "due_first"),
            (sw.step_limit != 0, "step_limit"),
            (sw.havoc_starts || sw.havoc_rooms || sw.havoc_args, "the havoc switches"),
            (sw.reach_pairs, "reach_pairs"),
        ] {
            if on {
                refuse(name);
            }
        }
        let text = self.memo_inputs();
        let key = sha1_smol::Sha1::from(text.as_bytes()).digest().to_string();
        let cm = CrossMemo::open(root, &key).unwrap_or_else(|e| panic!("--memo {}: {e}", root.display()));
        let inputs = cm.dir.join("inputs.txt");
        if !inputs.exists() {
            std::fs::write(&inputs, &text).ok();
        }
        eprintln!("[memo] {}: {} files, {} runs stored", cm.dir.display(), cm.opened_files, cm.opened_runs);
        self.table.cross = Some(Arc::new(cm));
    }

    /// What the reading's memo did (the report's `memo`), None with the switch off.
    pub fn memo_json(&self) -> Option<serde_json::Value> {
        let cm = self.table.cross.as_ref()?;
        let n = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let s = &cm.stats;
        Some(serde_json::json!({
            "dir": cm.dir.display().to_string(),
            "files_opened": cm.opened_files,
            "runs_opened": cm.opened_runs,
            "no_write": cm.no_write,
            "taken": n(&s.taken),
            "ran_no_stored_run": n(&s.ran_no_run),
            "ran_key_differs": n(&s.ran_key_differs),
            "ran_list_differs": n(&s.ran_list_differs),
            "ran_reached_differs": n(&s.ran_reached_differs),
            "stored": n(&s.stored),
            "stored_mb": (n(&s.stored_bytes) as f64 / 1048576.0 * 10.0).round() / 10.0,
            "stored_key_mb": (n(&s.key_bytes) as f64 / 1048576.0 * 10.0).round() / 10.0,
            "stored_cond_mb": (n(&s.cond_bytes) as f64 / 1048576.0 * 10.0).round() / 10.0,
            "stored_out_raw_mb": (n(&s.out_raw_bytes) as f64 / 1048576.0 * 10.0).round() / 10.0,
            "lookup_s": (n(&s.lookup_ns) as f64 / 1e7).round() / 100.0,
            "store_s": (n(&s.store_ns) as f64 / 1e7).round() / 100.0,
        }))
    }

    /// A key's kind, domain and (a constant's) start value in this reading (memo.md, condition 3), kept for the
    /// table's pass (`sync_table`, `reset` forget them).
    fn key_info(&mut self, k: &str) -> (u8, Option<crate::domain::Domain>, Option<i64>) {
        if let Some(i) = self.memo_infos.get(k) {
            return i.clone();
        }
        let kind = self.table.kind(k, &self.scope);
        let dom = self.table.domain(k).map(|d| d.into_owned());
        let start = (kind == crate::table::Kind::Const).then(|| self.table.start.get0(k));
        let tag = match kind {
            crate::table::Kind::Const => 0,
            crate::table::Kind::Fact => 1,
            crate::table::Kind::NonFact => 2,
        };
        let i = (tag, dom, start);
        self.memo_infos.insert(k.to_string(), i.clone());
        i
    }

    /// memo.md, condition 3 for one key: it reads in this table as it read where the run was made.
    fn info_holds(&mut self, i: &Info) -> bool {
        let (kind, dom, start) = self.key_info(&i.0);
        kind == i.1 && dom == i.2 && start == i.3
    }

    /// memo.md, conditions 4 and 5, on this reading as the job starts.
    fn rest_holds(&self, sigs: &[(i64, bool)], queries: &[Query]) -> Result<(), Miss> {
        if sigs.iter().any(|(s, f)| self.folded.contains(s) != *f) {
            return Err(Miss::List);
        }
        if !queries.is_empty() {
            let ok = self.table.reach.as_ref().is_some_and(|r| r.answers_hold(&self.table, queries));
            if !ok {
                return Err(Miss::Reached);
            }
        }
        Ok(())
    }

    /// `run_job` on a worker's Builder with the switch `--memo`: a stored run whose conditions hold here is taken;
    /// else the job runs, logging what it consults, and its run is stored.
    pub(crate) fn run_job_cross(&mut self, cm: &CrossMemo, m: &mut dyn Machine, job: &Job) -> Vec<Record> {
        use crate::wire::Wire;
        let t = std::time::Instant::now();
        let guides = match (&self.table.reach, &self.cur_job_key) {
            (Some(r), Some(k)) => r.guides_of(k),
            _ => None,
        };
        let mut key = Vec::with_capacity(1024);
        job.put(&mut key);
        crate::wire::put_guides(guides.as_deref(), &mut key);
        let got = {
            let me = std::cell::RefCell::new(&mut *self);
            cm.take(&key, &mut |i| me.borrow_mut().info_holds(i), &mut |sigs, qs| me.borrow().rest_holds(sigs, qs))
        };
        cm.stats.lookup_ns.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
        if let Some((records, deltas, pins)) = got {
            // as the run leaves the worker: its records' `args:` values in its table, its counters, its last pins
            for (k, s) in &deltas.args {
                self.table.args_vals.entry(k.clone()).or_default().insert(*s);
            }
            self.set_deltas(deltas);
            self.last_pins = pins;
            return records;
        }
        self.memo_log = Some(RunLog::default());
        let records = self.run_job_real(m, job);
        let log = self.memo_log.take().unwrap();
        let deltas = self.take_deltas();
        let ts = std::time::Instant::now();
        let out = if cm.writes() {
            let mut keys: Vec<&String> = log.touched.iter().filter(|k| !k.starts_with("args:")).collect();
            keys.sort();
            let infos = keys.into_iter().map(|k| {
                let (kind, dom, start) = self.key_info(k);
                (k.clone(), kind, dom, start)
            });
            let infos: Vec<Info> = infos.collect();
            let out = (records, deltas, self.last_pins.clone());
            cm.store(key, infos, &log.sigs, &log.queries, &out);
            out
        } else {
            (records, deltas, Pins::new())
        };
        cm.stats.store_ns.fetch_add(ts.elapsed().as_nanos() as u64, Ordering::Relaxed);
        let (records, deltas, _) = out;
        self.set_deltas(deltas);
        records
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Domain, Label};
    use crate::record::{Val, What};

    fn out(n: i64) -> Out {
        let rec = Record {
            kind: "event".into(),
            label: format!("event(local:{n})"),
            pre: [("var:5".to_string(), vec![Label::Int(1)])].into_iter().collect(),
            effects: vec![("var:6".to_string(), crate::record::Eff::Set(Label::Int(n)))],
            checks: vec![],
            end: Some("yield".into()),
            notes: vec![],
            next_events: vec![],
            source: What::Verb(n),
            either: vec![],
            engine_pre: vec![],
            unknown: vec![],
        };
        let mut d = Deltas::default();
        d.counts.inc("runs: event");
        d.args.push(("args:local:200".into(), 77));
        (vec![rec], d, [("var:5".to_string(), Val::Int(1))].into_iter().collect())
    }

    /// A run stored by one reading is taken by another when its consulted keys read the same, and not otherwise; a
    /// run joins its own reading's index between passes.
    #[test]
    fn a_stored_run_is_taken_where_its_keys_read_the_same() {
        let root = std::env::temp_dir().join(format!("memo-test-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let info: Info = ("var:7".into(), 1, Some(Domain::of([0, 1, 2], None, None)), None);
        let key = b"job and guides".to_vec();
        {
            let cm = CrossMemo::open(&root, "k").unwrap();
            if !cm.writes() {
                return; // no room on this disk: nothing to check
            }
            cm.store(key.clone(), vec![info.clone()], &[(5, false)], &[], &out(3));
            assert!(cm.take(&key, &mut |_| true, &mut |_, _| Ok(())).is_none(), "not indexed before the pass ends");
            cm.index_own();
            let got = cm.take(&key, &mut |i| *i == info, &mut |s, _| if s == [(5, false)] { Ok(()) } else { Err(Miss::List) });
            assert_eq!(got.as_ref().map(|o| (&o.0, &o.2)), Some((&out(3).0, &out(3).2)));
            assert_eq!(got.unwrap().1.args, out(3).1.args);
            assert_eq!(cm.finish().unwrap(), 1);
        }
        let cm = CrossMemo::open(&root, "k").unwrap();
        assert_eq!((cm.opened_files, cm.opened_runs), (1, 1));
        assert!(cm.take(b"another job", &mut |_| true, &mut |_, _| Ok(())).is_none());
        assert!(cm.take(&key, &mut |_| false, &mut |_, _| Ok(())).is_none(), "a key that reads otherwise");
        cm.index_own(); // a new pass: the keys are asked again
        assert!(cm.take(&key, &mut |_| true, &mut |_, _| Err(Miss::Reached)).is_none());
        assert!(cm.take(&key, &mut |_| true, &mut |_, _| Ok(())).is_some());
        assert_eq!(cm.stats.taken.load(Ordering::Relaxed), 1);
        cm.finish().unwrap();
        std::fs::remove_dir_all(&root).ok();
    }

    /// Under the floor, the other folders' files go oldest first, until the floor plus twice the largest file is
    /// free; the reading's own folder is left.
    #[test]
    fn eviction_takes_the_oldest_files_of_other_folders() {
        let root = std::env::temp_dir().join(format!("memo-evict-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let at = |s: u64| std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000 + s);
        let file = |dir: &str, name: &str, t: u64| {
            std::fs::create_dir_all(root.join(dir)).unwrap();
            let p = root.join(dir).join(name);
            let f = std::fs::File::create(&p).unwrap();
            f.set_len(1000).unwrap();
            f.set_modified(at(t)).unwrap();
            p
        };
        let own = file("k", "0.runs", 0);
        let old = file("a", "0.runs", 1);
        let new = file("b", "0.runs", 3);
        let mid = file("a", "1.runs", 2);
        // 1500 free, floor 0: 2000 wanted, one file of 1000 is enough
        assert_eq!(evict_from(&root, &root.join("k"), 0, 1500, 0), 1000);
        assert!(own.exists() && !old.exists() && mid.exists() && new.exists());
        // 0 free: every file of the other folders goes, the own folder's stays
        evict_from(&root, &root.join("k"), 0, 0, 10_000);
        assert!(own.exists() && !mid.exists() && !new.exists());
        std::fs::remove_dir_all(&root).ok();
    }
}
