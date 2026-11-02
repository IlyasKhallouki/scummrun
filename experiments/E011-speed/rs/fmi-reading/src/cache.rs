//! The exact reading cache (experiments/E011-speed/plan.md, readio item 3).
//!
//! * **What a reading is read from** is a [`ReadSpec`]: reader-scope's inputs as replay_part1.py `read_rust` gives
//!   them (rooms, seed and start rows, goal, the reader's switches, fact variables, reach-also starts, slot
//!   locals). [`ReadSpec::args`] is reader-scope's command line for it, writing the reading's files itself
//!   (`--out-dir`).
//! * **The key** ([`ReadSpec::key`]) is sha256 over a text of: the reader binary's sha1, the game data's index
//!   (sha1 of data/scripts-dos-cd/index.json), the rooms, the switches, the goal text, the fact variables, the start
//!   facts (sorted texts, from the start row the reading reads), the contents of the reach-also start files, and the
//!   slot locals' file. The reader is deterministic (the profile's repeats gave the same sha1), so equal keys give
//!   the same files.
//! * **The store** is `ROOT/<key>/` (ROOT: `~/.cache/fast-mi-e011/readings`), one reading's files per key, made in
//!   `ROOT/tmp-*` and renamed into place. A hit is the first reading made for the key: its reading.json names that
//!   request's name, start rows path and reach-also paths (files whose contents are in the key) and its own times.
//! * **Eviction** is E009's level (explore.py `evict_level_gb`, `--evict-by-size`): while the free space of the
//!   disk (E008_DISK_PATH, default /home) is under the floor (E008_FLOOR_GB, default 8 GB) plus twice the largest
//!   reading the store ever held, the least recently used reading not in use goes.

use crate::Reading;
use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::fmt::Write as _;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::{SystemTime, UNIX_EPOCH};

/// E009's reader switches (step.py SWITCHES).
pub const E009_SWITCHES: &str = "value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost";
const USED: &str = "used";
const LARGEST: &str = "largest";
const KEY_FORMAT: &str = "fmi-reading cache key 1";

/// A reading's inputs (module docstring).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadSpec {
    pub rooms: Vec<i32>,
    pub seed: i64,
    /// reader-scope `--goal` (FACT=V,...)
    pub goal: String,
    /// `--domain-goals`
    pub domain_goals: String,
    pub elsewhere: bool,
    pub live_locals: bool,
    pub split_limit: bool,
    /// `--switches`, "" for none
    pub switches: String,
    pub reach: bool,
    pub frame_costs: bool,
    pub reach_also: Vec<PathBuf>,
    /// `--start-rows`; None: results/start-part1.jsonl
    pub start_rows: Option<PathBuf>,
    pub fact_vars: Vec<i32>,
    pub slot_locals: Option<PathBuf>,
}

impl ReadSpec {
    /// E009's fresh reading (step.py `read_fresh`): rooms sorted, seed 1, domain goals "none", --elsewhere
    /// --live-locals --split-limit, E009's switches, --reach --frame-costs, the entry's start row.
    pub fn e009(rooms: &[i32], goal: &str, start_rows: &Path) -> ReadSpec {
        let mut rooms = rooms.to_vec();
        rooms.sort_unstable();
        ReadSpec {
            rooms,
            seed: 1,
            goal: goal.to_string(),
            domain_goals: "none".into(),
            elsewhere: true,
            live_locals: true,
            split_limit: true,
            switches: E009_SWITCHES.into(),
            reach: true,
            frame_costs: true,
            reach_also: vec![],
            start_rows: Some(start_rows.to_path_buf()),
            fact_vars: vec![],
            slot_locals: None,
        }
    }

    /// reader-scope's arguments (read_rust's, with `--out-dir DIR` for its files and report).
    pub fn args(&self, dir: &Path, name: &str, threads: usize) -> Vec<String> {
        let mut a: Vec<String> = vec![
            "--rooms".into(),
            self.rooms.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(","),
            "--start-seed".into(),
            self.seed.to_string(),
            "--domain-goals".into(),
            self.domain_goals.clone(),
            "--goal".into(),
            self.goal.clone(),
            "--name".into(),
            name.into(),
            "--threads".into(),
            threads.to_string(),
            "--out-dir".into(),
            dir.display().to_string(),
        ];
        let mut flag = |on: bool, f: &str| {
            if on {
                a.push(f.into());
            }
        };
        flag(self.elsewhere, "--elsewhere");
        flag(self.live_locals, "--live-locals");
        flag(self.split_limit, "--split-limit");
        if !self.switches.is_empty() {
            a.extend(["--switches".into(), self.switches.clone()]);
        }
        if self.reach {
            a.push("--reach".into());
        }
        for f in &self.reach_also {
            a.extend(["--reach-also".into(), f.display().to_string()]);
        }
        if self.frame_costs {
            a.push("--frame-costs".into());
        }
        if let Some(r) = &self.start_rows {
            a.extend(["--start-rows".into(), r.display().to_string()]);
        }
        if !self.fact_vars.is_empty() {
            a.extend(["--fact-vars".into(), self.fact_vars.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(",")]);
        }
        if let Some(s) = &self.slot_locals {
            a.extend(["--slot-locals".into(), s.display().to_string()]);
        }
        a
    }

    /// The start row's facts (`key\tvalue` lines, sorted by key text) of `seed` in the start rows (`root`: the
    /// repository root, for results/start-part1.jsonl).
    pub fn start_text(&self, root: &Path) -> io::Result<String> {
        let rows = self.start_rows.clone().unwrap_or_else(|| root.join("experiments/E003-path-through-cloud/results/start-part1.jsonl"));
        let text = std::fs::read_to_string(&rows)?;
        for line in text.lines() {
            let v: serde_json::Value = serde_json::from_str(line).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            if v["seed"].as_i64() != Some(self.seed) {
                continue;
            }
            let facts = v["facts"].as_object().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no facts"))?;
            let mut out = String::new();
            for (k, x) in facts {
                let _ = writeln!(out, "{k}\t{x}");
            }
            return Ok(out);
        }
        Err(io::Error::new(io::ErrorKind::NotFound, format!("no seed {} in {}", self.seed, rows.display())))
    }

    /// The key's text (module docstring).
    pub fn key_text(&self, reader_sha1: &str, game_index_sha1: &str, root: &Path) -> io::Result<String> {
        let mut t = String::new();
        let _ = writeln!(t, "{KEY_FORMAT}");
        let _ = writeln!(t, "reader {reader_sha1}");
        let _ = writeln!(t, "game {game_index_sha1}");
        let _ = writeln!(t, "rooms {}", self.rooms.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(","));
        let _ = writeln!(
            t,
            "switches domain_goals={} elsewhere={} live_locals={} split_limit={} switches={} reach={} frame_costs={}",
            self.domain_goals, self.elsewhere as u8, self.live_locals as u8, self.split_limit as u8, self.switches,
            self.reach as u8, self.frame_costs as u8
        );
        let _ = writeln!(t, "goal {}", self.goal);
        let _ = writeln!(t, "fact_vars {}", self.fact_vars.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(","));
        let start = self.start_text(root)?;
        let _ = write!(t, "start {}\n{start}", start.len());
        let _ = writeln!(t, "reach_also {}", self.reach_also.len());
        for f in &self.reach_also {
            let c = std::fs::read_to_string(f)?;
            let _ = write!(t, "{}\n{c}", c.len());
        }
        let sl = match &self.slot_locals {
            Some(p) => std::fs::read_to_string(p)?,
            None => String::new(),
        };
        let _ = write!(t, "slot_locals {}\n{sl}", sl.len());
        Ok(t)
    }

    /// The cache key: sha256 (hex) of `key_text`.
    pub fn key(&self, reader_sha1: &str, game_index_sha1: &str, root: &Path) -> io::Result<String> {
        let t = self.key_text(reader_sha1, game_index_sha1, root)?;
        Ok(Sha256::digest(t.as_bytes()).iter().map(|b| format!("{b:02x}")).collect())
    }
}

/// sha1 (hex) of a file: the reader binary's, the game data index's.
pub fn file_sha1(path: &Path) -> io::Result<String> {
    Ok(sha1_smol::Sha1::from(std::fs::read(path)?).digest().to_string())
}

/// The game data's index under the repository root (the reader's data/scripts-dos-cd/index.json).
pub fn game_index(root: &Path) -> PathBuf {
    root.join("data").join("scripts-dos-cd").join("index.json")
}

/// Bytes free for an unprivileged user on the file system holding `path` (statvfs).
pub fn disk_free(path: &Path) -> io::Result<u64> {
    let c = CString::new(path.as_os_str().as_bytes()).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    // SAFETY: statvfs fills the struct it is given
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(s.f_bavail as u64 * s.f_frsize as u64)
}

fn dir_bytes(d: &Path) -> u64 {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(d) {
        for e in rd.flatten() {
            match e.metadata() {
                Ok(m) if m.is_dir() => n += dir_bytes(&e.path()),
                Ok(m) => n += m.len(),
                Err(_) => {}
            }
        }
    }
    n
}

/// One reading of the store.
#[derive(Clone, Debug)]
pub struct Entry {
    pub key: String,
    pub dir: PathBuf,
    pub bytes: u64,
    /// when it was last made or opened (the `used` file's time)
    pub used: SystemTime,
}

/// The store (module docstring).
pub struct ReadingCache {
    pub root: PathBuf,
    pub disk: PathBuf,
    pub floor_bytes: u64,
}

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

impl ReadingCache {
    /// The store at `root`, with explore.py's machine: E008_DISK_PATH (/home), E008_FLOOR_GB (8).
    pub fn new(root: &Path) -> io::Result<ReadingCache> {
        std::fs::create_dir_all(root)?;
        Ok(ReadingCache {
            root: root.to_path_buf(),
            disk: PathBuf::from(std::env::var("E008_DISK_PATH").unwrap_or_else(|_| "/home".into())),
            floor_bytes: (env_f64("E008_FLOOR_GB", 8.0) * (1u64 << 30) as f64) as u64,
        })
    }

    /// `~/.cache/fast-mi-e011/readings`
    pub fn default_root() -> PathBuf {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".cache/fast-mi-e011/readings")
    }

    pub fn dir_of(&self, key: &str) -> PathBuf {
        self.root.join(key)
    }

    fn touch(&self, dir: &Path) {
        let _ = std::fs::write(dir.join(USED), b"");
    }

    /// The stored reading of `key`, opened (and marked used), or None.
    pub fn get(&self, key: &str) -> Option<Reading> {
        let d = self.dir_of(key);
        if !d.join("reading.json").is_file() {
            return None;
        }
        let rd = Reading::open(&d).ok()?;
        self.touch(&d);
        Some(rd)
    }

    /// The largest reading the store has held (bytes).
    pub fn largest(&self) -> u64 {
        std::fs::read_to_string(self.root.join(LARGEST)).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0)
    }

    /// Moves a reading made in `made` (a folder on the store's file system) into the store as `key`; when the key
    /// is there already (made meanwhile by another), `made` goes and the stored one stays.
    pub fn put(&self, key: &str, made: &Path) -> io::Result<PathBuf> {
        let d = self.dir_of(key);
        let bytes = dir_bytes(made);
        if std::fs::rename(made, &d).is_err() {
            if d.join("reading.json").is_file() {
                std::fs::remove_dir_all(made)?;
            } else {
                return Err(io::Error::other(format!("{} could not go to {}", made.display(), d.display())));
            }
        }
        self.touch(&d);
        if bytes > self.largest() {
            let tmp = self.root.join(format!("{LARGEST}.{}.tmp", std::process::id()));
            std::fs::write(&tmp, bytes.to_string())?;
            std::fs::rename(&tmp, self.root.join(LARGEST))?;
        }
        Ok(d)
    }

    /// A new folder for a reading being made (in the store, so `put` is a rename).
    pub fn scratch(&self) -> io::Result<PathBuf> {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let d = self.root.join(format!("tmp-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d)?;
        Ok(d)
    }

    /// The reading of `spec`: the stored one, or one made by `reader` (reader-scope) run through `run` (which
    /// may wrap it: nice, the heavy lock, the guard) and stored. Returns it and whether it was a hit.
    pub fn read(
        &self,
        spec: &ReadSpec,
        key: &str,
        reader: &Path,
        name: &str,
        threads: usize,
        run: &mut dyn FnMut(Command) -> io::Result<ExitStatus>,
    ) -> io::Result<(Reading, bool)> {
        if let Some(rd) = self.get(key) {
            return Ok((rd, true));
        }
        let tmp = self.scratch()?;
        let mut cmd = Command::new(reader);
        cmd.args(spec.args(&tmp, name, threads));
        let st = run(cmd)?;
        if !st.success() {
            return Err(io::Error::other(format!("reader-scope failed ({st}); its folder {}", tmp.display())));
        }
        let d = self.put(key, &tmp)?;
        Ok((Reading::open(&d)?, false))
    }

    /// The store's readings, least recently used first.
    pub fn entries(&self) -> Vec<Entry> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&self.root) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("tmp-") || !e.path().is_dir() {
                    continue;
                }
                let used = std::fs::metadata(e.path().join(USED)).and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH);
                out.push(Entry { key: name, bytes: dir_bytes(&e.path()), dir: e.path(), used });
            }
        }
        out.sort_by(|a, b| (a.used, &a.key).cmp(&(b.used, &b.key)));
        out
    }

    /// E009's level: the floor plus twice the largest reading.
    pub fn level(&self) -> u64 {
        self.floor_bytes + 2 * self.largest()
    }

    /// Evicts readings, least recently used first and none in `in_use` (their folders), while the disk's free
    /// space is under `level`. Returns the folders removed.
    pub fn evict(&self, in_use: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
        let level = self.level();
        let mut out = Vec::new();
        for e in self.entries() {
            if disk_free(&self.disk)? >= level {
                break;
            }
            if in_use.iter().any(|p| p == &e.dir) {
                continue;
            }
            std::fs::remove_dir_all(&e.dir)?;
            out.push(e.dir);
        }
        Ok(out)
    }
}
