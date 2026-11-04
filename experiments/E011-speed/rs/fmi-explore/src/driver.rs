//! explore.py's driver (`run`, `Job`): up to b jobs at once, committed in start order (or as they finish, E010's
//! `commit_finished`), the stop rules, resume, the guard's kills, eviction and the floor.
//!
//! A job runs either as explore.py runs it, a process (`StepMode::Command`: the guard script, `flock` for a heavy
//! scope, `pid.json`, a resume that waits for a job still running), or inside this process (`StepMode::InProcess`: a
//! thread running a `Step`, so the readings, planners and kept engines stay warm from job to job). In process, the
//! guard's rule is the driver's: MemAvailable under 3 GB kills this process's children and cancels the running jobs,
//! with the guard's lines in each job's log, so the job is classified as guard-killed exactly as explore.py does.

use crate::archive::{Archive, GIB, HEAVY_ROOMS};
use crate::files::{self, read_json, say, write_json, Res};
use crate::machine::{self, Machine};
use crate::pyjson;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// guard.sh's floor: MemAvailable under 3 GB (3145728 kB) kills the job
pub const GUARD_KB: u64 = 3_145_728;

/// A job's log file (JOB_DIR/log), shared by the step's lines.
#[derive(Clone)]
pub struct JobLog {
    f: Arc<Mutex<fs::File>>,
}

impl JobLog {
    pub fn create(path: &Path) -> Res<JobLog> {
        let f = fs::OpenOptions::new().create(true).write(true).truncate(true).open(path).map_err(|e| files::io_err(path, e))?;
        Ok(JobLog { f: Arc::new(Mutex::new(f)) })
    }
    pub fn append(path: &Path) -> Res<JobLog> {
        let f = fs::OpenOptions::new().create(true).append(true).open(path).map_err(|e| files::io_err(path, e))?;
        Ok(JobLog { f: Arc::new(Mutex::new(f)) })
    }
    pub fn line(&self, s: &str) {
        if let Ok(mut f) = self.f.lock() {
            let _ = writeln!(f, "{s}");
            let _ = f.flush();
        }
    }
    /// step.py `say`: "[HH:MM:SS] step: msg"
    pub fn say(&self, msg: &str) {
        self.line(&format!("[{}] step: {msg}", files::strftime("%H:%M:%S")));
    }
}

/// One job run inside this process (step.py's `run JOB_DIR`): writes JOB_DIR/result.json unless `cancel` was set
/// (the guard's kill) and returns step.py's exit code (0 ok, 3 the return differs, 1 another error).
pub trait Step: Send + Sync {
    fn run(&self, job_dir: &Path, log: &JobLog, cancel: &AtomicBool) -> i32;
}

/// How the driver runs a job.
#[derive(Clone)]
pub enum StepMode {
    /// explore.py's: `GUARD JOB_DIR/log CMD... run JOB_DIR` as a process (under flock for a heavy scope)
    Command(Vec<String>),
    /// a thread of this process
    InProcess(Arc<dyn Step>),
}

/// What runs the goal entry's verify at a goal stop (explore.py `Archive.verify`); None with `--no-verify`.
pub trait Verifier {
    fn verify(&self, a: &Archive, entry: usize) -> Res<Value>;
}

struct Thread {
    handle: Option<JoinHandle<(i32, f64)>>,
    cancel: Arc<AtomicBool>,
    log: JobLog,
    min_kb: u64,
    low: bool,
}

/// One step process or thread (explore.py `Job`).
pub struct Job {
    pub name: String,
    pub dir: PathBuf,
    pub doc: Value,
    pub entry: usize,
    pub scope: Vec<i64>,
    pub cell: String,
    child: Option<std::process::Child>,
    thread: Option<Thread>,
    pub pid: Option<i32>,
    pid_start: Option<u64>,
    pub done: bool,
    pub rc: Option<i32>,
    rusage: Option<Value>,
    started: Option<f64>,
    ended: Option<f64>,
    /// None while running; then "finished" or "guard-killed"
    pub state: Option<&'static str>,
}

fn waitstatus_to_exitcode(status: i32) -> i32 {
    if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else if libc::WIFSIGNALED(status) {
        -libc::WTERMSIG(status)
    } else {
        status
    }
}

impl Job {
    pub fn new(a: &Archive, name: &str) -> Res<Job> {
        let dir = a.root.join("jobs").join(name);
        let doc = read_json(&dir.join("job.json"))?;
        let entry = doc["entry"]["id"].as_u64().ok_or("job.json: entry id")? as usize;
        let scope = doc["scope"].as_array().map(|s| s.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
        Ok(Job {
            name: name.to_string(),
            cell: a.rows[entry].cell.clone(),
            dir,
            doc,
            entry,
            scope,
            child: None,
            thread: None,
            pid: None,
            pid_start: None,
            done: false,
            rc: None,
            rusage: None,
            started: None,
            ended: None,
            state: None,
        })
    }

    pub fn in_use(&self) -> Vec<String> {
        let mut out: Vec<String> = self.doc["readings"]
            .as_array()
            .map(|a| a.iter().filter_map(|r| r["path"].as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        out.push(Path::new(self.doc["reading_dir"].as_str().unwrap_or("")).join("reading.json").to_string_lossy().into_owned());
        out
    }

    fn heavy(&self, m: &Machine) -> bool {
        self.scope.len() >= HEAVY_ROOMS && m.heavy_lock != "none"
    }

    pub fn command(&self, step_cmd: &[String], m: &Machine) -> Vec<String> {
        let mut cmd = vec![m.guard.clone(), self.dir.join("log").to_string_lossy().into_owned()];
        cmd.extend(step_cmd.iter().cloned());
        cmd.push("run".into());
        cmd.push(self.dir.to_string_lossy().into_owned());
        if self.heavy(m) {
            let mut c = vec!["flock".to_string(), m.heavy_lock.clone()];
            c.extend(cmd);
            cmd = c;
        }
        cmd
    }

    /// Before a run (again): what an earlier, unfinished run of the job left, except its logs.
    pub fn clear(&self, a: &Archive) {
        for f in ["result.json", "pid.json"] {
            let _ = fs::remove_file(self.dir.join(f));
        }
        for d in ["new", "run"] {
            let _ = fs::remove_dir_all(self.dir.join(d));
        }
        let rd = PathBuf::from(self.doc["reading_dir"].as_str().unwrap_or(""));
        let root = crate::archive::resolve(&a.root);
        if rd.exists() && crate::archive::resolve(&rd).starts_with(&root) && crate::archive::resolve(&rd) != root {
            let _ = fs::remove_dir_all(&rd);
        }
        if self.dir.join("log").exists() {
            let mut k = 1;
            while self.dir.join(format!("log.{k}")).exists() {
                k += 1;
            }
            let _ = fs::rename(self.dir.join("log"), self.dir.join(format!("log.{k}")));
        }
    }

    pub fn start(&mut self, a: &Archive, mode: &StepMode, m: &Machine) -> Res<()> {
        self.clear(a);
        self.started = Some(files::now_s());
        match mode {
            StepMode::Command(step_cmd) => {
                use std::os::unix::process::CommandExt;
                let out = fs::OpenOptions::new().create(true).append(true).open(self.dir.join("spawn.log")).map_err(|e| files::io_err(&self.dir, e))?;
                let err = out.try_clone().map_err(|e| files::io_err(&self.dir, e))?;
                let cmd = self.command(step_cmd, m);
                let mut command = std::process::Command::new(&cmd[0]);
                command
                    .args(&cmd[1..])
                    .stdin(std::process::Stdio::null())
                    .stdout(out)
                    .stderr(err);
                // start_new_session=True (setsid), as explore.py's Popen
                unsafe {
                    command.pre_exec(|| {
                        libc::setsid();
                        Ok(())
                    });
                }
                let child = command
                    .spawn()
                    .map_err(|e| format!("{}: {e}", cmd[0]))?;
                let pid = child.id() as i32;
                self.pid = Some(pid);
                self.pid_start = machine::proc_start(pid);
                self.child = Some(child);
            }
            StepMode::InProcess(step) => {
                let log = JobLog::create(&self.dir.join("log"))?;
                let cancel = Arc::new(AtomicBool::new(false));
                let (step, dir, log2, cancel2) = (step.clone(), self.dir.clone(), log.clone(), cancel.clone());
                let heavy = if self.heavy(m) { Some(m.heavy_lock.clone()) } else { None };
                let handle = std::thread::Builder::new()
                    .name(format!("job {}", self.name))
                    .spawn(move || {
                        let c0 = machine::thread_cpu_s();
                        let _lock = heavy.map(|p| HeavyLock::take(&p));
                        let rc = step.run(&dir, &log2, &cancel2);
                        let out = (rc, machine::thread_cpu_s() - c0);
                        wake();
                        out
                    })
                    .map_err(|e| format!("a job thread: {e}"))?;
                self.pid = Some(std::process::id() as i32);
                self.pid_start = machine::proc_start(std::process::id() as i32);
                self.thread = Some(Thread { handle: Some(handle), cancel, log, min_kb: u64::MAX, low: false });
            }
        }
        let mut pj = json!({"pid": self.pid, "start": self.pid_start, "started": self.started});
        if self.thread.is_some() {
            pj["thread"] = Value::Bool(true);
        }
        write_json(&self.dir.join("pid.json"), &pj, Some(1))?;
        self.done = false;
        self.state = None;
        self.ended = None;
        Ok(())
    }

    /// On a resume: the job's process from an earlier driver, if it still runs.
    pub fn attach(&mut self) -> bool {
        let p = self.dir.join("pid.json");
        let Ok(d) = read_json(&p) else { return false };
        let Some(pid) = d["pid"].as_i64() else { return false };
        let start = d["start"].as_u64();
        if pid as u32 == std::process::id() || !machine::proc_alive(pid as i32, start) {
            return false;
        }
        self.pid = Some(pid as i32);
        self.pid_start = start;
        self.started = d["started"].as_f64();
        true
    }

    /// The in-process guard's look at MemAvailable (guard.sh's loop, once per driver poll).
    fn guard_tick(&mut self, avail_kb: u64) -> bool {
        let Some(t) = self.thread.as_mut() else { return false };
        if self.done || t.low {
            return false;
        }
        t.min_kb = t.min_kb.min(avail_kb);
        if avail_kb < GUARD_KB {
            t.log.line(&format!("GUARD: MemAvailable {avail_kb} kB, killing group {}", std::process::id()));
            t.cancel.store(true, Ordering::SeqCst);
            t.low = true;
            return true;
        }
        false
    }

    pub fn poll(&mut self) -> bool {
        if self.done {
            return true;
        }
        if let Some(t) = self.thread.as_mut() {
            let finished = t.handle.as_ref().map(|h| h.is_finished()).unwrap_or(true);
            if !finished {
                return false;
            }
            let (rc, cpu) = t.handle.take().map(|h| h.join().unwrap_or((1, 0.0))).unwrap_or((1, 0.0));
            let rc = if t.low && !self.dir.join("result.json").exists() { 143 } else { rc };
            // explore.py's rc is guard.sh's own exit, 0 whatever the step's (its log line has the step's)
            self.rc = Some(0);
            let (peak, _) = machine::peaks_mb();
            self.rusage = Some(json!({"cpu_s": pyjson::round(cpu, 1), "maxrss_mb": pyjson::round(peak as f64, 1)}));
            let wall = self.started.map(|s| files::now_s() - s).unwrap_or(0.0);
            let min = if t.min_kb == u64::MAX { machine::mem_available_kb() } else { t.min_kb };
            t.log.line(&format!("GUARD: rc {rc} wall {} s low {} min MemAvailable {min} kB",
                                pyjson::float_repr(pyjson::round(wall, 1)), t.low as i32));
        } else if let Some(child) = self.child.as_ref() {
            let pid = child.id() as i32;
            let mut status = 0;
            let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
            let r = unsafe { libc::wait4(pid, &mut status, libc::WNOHANG, &mut ru) };
            if r == 0 {
                return false;
            }
            if r == pid {
                self.rc = Some(waitstatus_to_exitcode(status));
                let cpu = ru.ru_utime.tv_sec as f64 + ru.ru_utime.tv_usec as f64 * 1e-6 + ru.ru_stime.tv_sec as f64
                    + ru.ru_stime.tv_usec as f64 * 1e-6;
                self.rusage = Some(json!({"cpu_s": pyjson::round(cpu, 1),
                                          "maxrss_mb": pyjson::round(ru.ru_maxrss as f64 / 1024.0, 1)}));
            }
            self.child = None;
        } else if let Some(pid) = self.pid {
            if machine::proc_alive(pid, self.pid_start) {
                return false;
            }
        }
        self.done = true;
        self.ended = Some(files::now_s());
        true
    }

    pub fn kill(&mut self) {
        if self.done {
            return;
        }
        if let Some(t) = self.thread.as_ref() {
            t.cancel.store(true, Ordering::SeqCst);
            for p in machine::my_descendants().into_iter().rev() {
                unsafe {
                    libc::kill(p, libc::SIGKILL);
                }
            }
            if let Some(h) = self.thread.as_mut().and_then(|t| t.handle.take()) {
                let _ = h.join();
            }
            self.done = true;
            self.ended = Some(files::now_s());
            return;
        }
        if let Some(pid) = self.pid {
            machine::kill_tree(pid, 5.0);
            self.poll();
        }
    }

    pub fn result_ok(&self) -> bool {
        match read_json(&self.dir.join("result.json")) {
            Ok(r) => r.get("job").and_then(|j| j.as_str()) == Some(self.name.as_str()),
            Err(_) => false,
        }
    }

    pub fn guard_killed(&self) -> bool {
        let p = self.dir.join("log");
        p.exists()
            && fs::read(&p)
                .map(|b| {
                    let t = String::from_utf8_lossy(&b).into_owned();
                    files::py_splitlines(&t).iter().any(|l| l.starts_with("GUARD: MemAvailable") && l.contains("killing group"))
                })
                .unwrap_or(false)
    }

    /// After the process ended: "finished" (a result.json, the step's or one that says it left none) or
    /// "guard-killed".
    pub fn classify(&self) -> Res<&'static str> {
        if self.result_ok() {
            return Ok("finished");
        }
        if self.guard_killed() {
            return Ok("guard-killed");
        }
        let mut tail = String::new();
        if self.dir.join("log").exists() {
            let t = String::from_utf8_lossy(&fs::read(self.dir.join("log")).unwrap_or_default()).into_owned();
            let lines = files::py_splitlines(&t);
            tail = lines[lines.len().saturating_sub(3)..].join(" | ");
        }
        let rc = self.rc.map(|r| r.to_string()).unwrap_or_else(|| "None".into());
        write_json(
            &self.dir.join("result.json"),
            &json!({"job": self.name, "entry": self.entry, "ok": false,
                    "error": format!("the step ended (exit {rc}) with no result.json"), "log_tail": tail,
                    "return": null, "reading": null, "targets": null, "probes": [], "attempt": null,
                    "readings_made": [], "new": [], "tried": [], "exhausted": false}),
            Some(1),
        )?;
        Ok("finished")
    }

    pub fn extra(&self) -> Map<String, Value> {
        let ended = files::now_s();
        let mut guard = Value::Null;
        if let Ok(b) = fs::read(self.dir.join("log")) {
            let t = String::from_utf8_lossy(&b).into_owned();
            if let Some(l) = files::py_splitlines(&t).iter().rev().find(|l| l.starts_with("GUARD: rc")) {
                guard = Value::from(*l);
            }
        }
        let mut m = Map::new();
        m.insert("started".into(), json!(self.started));
        m.insert("ended".into(), json!(self.ended.unwrap_or(ended)));
        m.insert("driver_wall_s".into(), self.started.map(|s| json!(pyjson::round(ended - s, 1))).unwrap_or(Value::Null));
        m.insert("rc".into(), json!(self.rc));
        m.insert("rusage".into(), self.rusage.clone().unwrap_or(Value::Null));
        m.insert("guard".into(), guard);
        m
    }
}

fn waker() -> &'static (Mutex<u64>, std::sync::Condvar) {
    static W: std::sync::OnceLock<(Mutex<u64>, std::sync::Condvar)> = std::sync::OnceLock::new();
    W.get_or_init(|| (Mutex::new(0), std::sync::Condvar::new()))
}

/// An in-process job has ended: the driver's wait ends at once (it polls the processes' jobs every poll_s).
fn wake() {
    let (m, c) = waker();
    *m.lock().unwrap() += 1;
    c.notify_all();
}

/// The driver's wait between its looks: poll_s, or until an in-process job ends.
fn wait(poll_s: f64, seen: &mut u64) {
    let (m, c) = waker();
    let g = m.lock().unwrap();
    if *g != *seen {
        *seen = *g;
        return;
    }
    let (g, _) = c.wait_timeout(g, Duration::from_secs_f64(poll_s)).unwrap();
    *seen = *g;
}

/// `flock` on the heavy lock for an in-process job of a heavy scope, held until dropped.
pub struct HeavyLock {
    _f: Option<fs::File>,
}

impl HeavyLock {
    pub fn take(path: &str) -> HeavyLock {
        use std::os::unix::io::AsRawFd;
        let f = fs::OpenOptions::new().create(true).append(true).open(path).ok();
        if let Some(f) = f.as_ref() {
            unsafe {
                libc::flock(f.as_raw_fd(), libc::LOCK_EX);
            }
        }
        HeavyLock { _f: f }
    }
}

/// explore.py `plan_env()`: the builds a run uses, set where the environment does not name them and the build is
/// there; the values in use (stage.json keeps them).
pub fn plan_env() -> Value {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let defaults = [
        ("FAST_MI_SCUMMVM_BIN", home.join(".cache/fast-mi/svm-w3b/build/scummvm/scummvm")),
        ("DOWNWARD_DIR", home.join("projects/fast-mi/third_party/downward")),
        ("TASKCORE_BIN", home.join(".cache/fast-mi-walk/target/release/taskcore")),
        ("READER_SCOPE_BIN", home.join(".cache/fast-mi-e008/target-scumm/release/reader-scope")),
    ];
    let mut m = Map::new();
    for (k, v) in defaults {
        if std::env::var(k).map(|x| x.is_empty()).unwrap_or(true) && v.exists() {
            std::env::set_var(k, &v);
        }
        m.insert(k.into(), std::env::var(k).ok().map(Value::from).unwrap_or(Value::Null));
    }
    Value::Object(m)
}

fn count_kills(a: &Archive) -> HashMap<String, i64> {
    let mut out = HashMap::new();
    for x in &a.logs {
        if x.get("outcome").and_then(|o| o.as_str()) == Some("guard-killed") {
            *out.entry(x["job"].as_str().unwrap_or("").to_string()).or_insert(0) += 1;
        }
    }
    out
}

/// What one run of the driver measured of itself (the driver's own time per job, the plan's target).
#[derive(Clone, Debug, Default)]
pub struct DriverStats {
    pub jobs: usize,
    /// wall seconds spent in the driver's own work (selection, job.json, ingest, the files), polls not counted
    pub own_s: f64,
    pub own_by_job: Vec<f64>,
}

/// E014 amendment (8): `FMI_STOP_WALL=S`, the run's wall limit in seconds of this driver's wall. Once it is reached no
/// try starts; the running tries finish and are committed; the stop is "wall limit". Unset (or not a number): no limit.
pub fn stop_wall_s() -> Option<f64> {
    std::env::var("FMI_STOP_WALL").ok().and_then(|v| v.trim().parse::<f64>().ok()).filter(|v| *v >= 0.0)
}

/// E016 (plan.md "Stop rule"): `FMI_STOP_FILE=PATH`, a stop asked from outside the run (the ladder's watcher, after 40
/// minutes of wall with no new rung). Once the file exists no try starts; the running tries finish and are committed;
/// the stop is "stop file", its detail the file's first line. Unset: no such stop.
pub fn stop_file() -> Option<PathBuf> {
    std::env::var("FMI_STOP_FILE").ok().filter(|v| !v.trim().is_empty()).map(PathBuf::from)
}

/// The job loop (explore.py `run`): until a stop rule fires. Returns summary.json's contents.
pub fn run(root: &Path, workers: usize, mode: &StepMode, verifier: Option<&dyn Verifier>, stats: &mut DriverStats) -> Res<Value> {
    let mut a = Archive::open(root)?;
    if a.root.join("summary.json").exists() {
        let s = read_json(&a.root.join("summary.json"))?;
        say(&format!("{}: stopped already ({}); summary.json", a.root.display(), files::py_str(s.get("stop"))));
        return Ok(s);
    }
    // E013 (component free): FMI_FREE's share, the progress order and stall's port, checked before any job
    crate::free::check_env(&a)?;
    let m = Machine::from_env();
    a.stage["workers"] = json!(workers);
    a.stage["env"] = plan_env();
    write_json(&a.root.join("stage.json"), &a.stage, Some(1))?;
    // E013 (FMI_STALLS=1): the stall table from the archive's files, then one try more after each commit, stalls.json
    // and its line each time (stall.rs; free's port keeps its own, stall::ArchivePort)
    let mut stalls = if crate::stall::on() { Some(crate::stall::Live::open(&a)?) } else { None };
    let free = || machine::disk_free(&m.disk_path);
    for name in a.job_names() {
        // a dir the driver made but did not finish writing: its selection is made again
        if !a.root.join("jobs").join(&name).join("job.json").exists() {
            let _ = fs::remove_dir_all(a.root.join("jobs").join(&name));
        }
    }
    let mut running: Vec<Job> = Vec::new();
    let mut restart: Vec<String> = Vec::new();
    let mut kills = count_kills(&a);
    for name in a.unfinished_jobs() {
        let mut j = Job::new(&a, &name)?;
        a.busy.insert(name.clone(), (j.entry, j.cell.clone()));
        if j.attach() {
            say(&format!("{name}: still running (pid {}); waiting for it", j.pid.unwrap_or(0)));
        } else if j.result_ok() {
            j.done = true;
            j.state = Some("finished");
        } else {
            restart.push(name.clone());
        }
        running.push(j);
    }
    let (mut stop, mut detail, mut stopping): (Option<String>, Value, bool) = (None, Value::Null, false);
    let stop_jobs = files::as_i64(a.e009.get("stop_jobs")).unwrap_or(0);
    let mut bound = false;
    let (wall0, stop_wall) = (Instant::now(), stop_wall_s());
    let mut walled = false;
    let stop_f = stop_file();
    let mut asked = false;
    let commit_finished = files::truthy(a.e010.get("commit_finished"));
    let in_process = matches!(mode, StepMode::InProcess(_));
    let mut woken = 0u64;

    let can_start = |running: &Vec<Job>, restart: &Vec<String>| -> bool {
        if free() < m.floor_gb * GIB {
            return false;
        }
        let active = running.iter().any(|j| !j.done && !restart.contains(&j.name));
        !active || machine::mem_available_gb() >= m.mem_second_gb
    };

    loop {
        let t_own = Instant::now();
        if in_process {
            // the guard's rule (guard.sh), once per poll, for the jobs this process runs
            let avail = machine::mem_available_kb();
            let mut fired = false;
            for j in running.iter_mut() {
                fired |= j.guard_tick(avail);
            }
            if fired {
                for p in machine::my_descendants().into_iter().rev() {
                    unsafe {
                        libc::kill(p, libc::SIGKILL);
                    }
                }
            }
        }
        for j in running.iter_mut() {
            if !restart.contains(&j.name) && j.poll() && j.state.is_none() {
                let st = j.classify()?;
                j.state = Some(st);
                if st == "guard-killed" {
                    let k = kills.entry(j.name.clone()).or_insert(0);
                    *k += 1;
                    let k = *k;
                    let mut line = Map::new();
                    line.insert("job".into(), json!(j.name));
                    line.insert("entry".into(), json!(j.entry));
                    line.insert("outcome".into(), json!("guard-killed"));
                    for (key, v) in j.extra() {
                        line.insert(key, v);
                    }
                    a.log_event(Value::Object(line))?;
                    say(&format!("{}: the guard killed it ({k}); not counted as tried", j.name));
                    if k >= 2 {
                        stop = Some("repeated failure".into());
                        detail = json!({"job": j.name, "entry": j.entry, "error": "the guard killed the job twice"});
                        stopping = true;
                    } else {
                        restart.push(j.name.clone());
                    }
                }
            }
        }
        let mut committed = false;
        let ready = if commit_finished {
            running.iter().position(|x| x.done && x.state == Some("finished"))
        } else if running.first().map(|x| x.done && x.state == Some("finished")).unwrap_or(false) {
            Some(0)
        } else {
            None
        };
        let mut t_ingest = 0.0;
        if let Some(ix) = ready {
            committed = true;
            let j = running.remove(ix);
            let t = Instant::now();
            let line = a.ingest(&j.dir, Some(&j.extra()))?;
            t_ingest = t.elapsed().as_secs_f64();
            a.busy.remove(&j.name);
            say(&format!(
                "{}: entry {} {}; {} entries, {} new cells, {} new atoms; archive {} entries, {} cells",
                j.name, j.entry, line["outcome"].as_str().unwrap_or(""),
                line["new_entries"].as_array().map(|x| x.len()).unwrap_or(0), line["new_cells"],
                line["new_r"].as_array().map(|x| x.len()).unwrap_or(0), a.rows.len(), a.cells.len()
            ));
            if let Some(s) = stalls.as_mut() {
                s.commit(&a, &line)?;
                say(&s.stalls.line());
            }
            if line["outcome"] == "return differs" {
                stop = Some("return differs".into());
                detail = json!({"job": j.name, "entry": j.entry, "return": line["return"]});
                for k in running.iter_mut() {
                    k.kill();
                    a.busy.remove(&k.name);
                }
                running.clear();
                restart.clear();
            } else if !stopping && line["outcome"] == "error" && a.failures_of(j.entry as i64, &line["error"]) >= 2 {
                stop = Some("repeated failure".into());
                detail = json!({"job": j.name, "entry": j.entry, "error": line["error"]});
                stopping = true;
            }
        }
        if stop.as_deref() == Some("return differs") {
            stats.own_s += t_own.elapsed().as_secs_f64();
            break;
        }
        if !stopping && !a.goal_entries().is_empty() {
            stop = Some("goal".into());
            detail = json!({"entry": a.goal_entries()[0]});
            stopping = true;
        }
        if !restart.is_empty() && !stopping {
            for name in restart.clone() {
                if can_start(&running, &restart) {
                    restart.retain(|n| *n != name);
                    say(&format!("{name}: run again"));
                    let a_ref = &a;
                    if let Some(j) = running.iter_mut().find(|j| j.name == name) {
                        j.start(a_ref, mode, &m)?;
                    }
                }
            }
        } else if stopping {
            for name in restart.drain(..) {
                running.retain(|j| j.name != name);
                a.busy.remove(&name);
            }
        }
        let mut floor = false;
        if !stopping && !a.stagnant() {
            let level = if files::truthy(a.e009.get("evict_by_size")) { a.evict_level_gb(m.floor_gb) } else { m.evict_gb };
            if free() < level * GIB {
                let in_use: std::collections::HashSet<String> = running.iter().flat_map(|j| j.in_use()).collect();
                let mut f = free;
                let gone = a.evict(&mut f, &in_use, level)?;
                if !gone.is_empty() {
                    say(&format!("evicted {} readings (level {level:.2} GB)", gone.len()));
                }
            }
            while running.len() < workers {
                if stop_jobs != 0 && a.job_names().len() as i64 >= stop_jobs {
                    bound = true;
                    break;
                }
                if stop_wall.is_some_and(|w| wall0.elapsed().as_secs_f64() >= w) {
                    walled = true;
                    break;
                }
                if stop_f.as_ref().is_some_and(|p| p.exists()) {
                    asked = true;
                    break;
                }
                if free() < m.floor_gb * GIB {
                    floor = true;
                    break;
                }
                if !can_start(&running, &restart) {
                    break;
                }
                // E013 (FMI_FREE, component free): the job's kind and its entry; unset, E012's selection
                let Some((id, kind)) = crate::free::pick(&a)? else { break };
                let d = a.prepare_job_as(id, kind.as_ref())?;
                let name = d.file_name().unwrap().to_string_lossy().into_owned();
                let mut j = Job::new(&a, &name)?;
                a.busy.insert(name.clone(), (j.entry, j.cell.clone()));
                j.start(&a, mode, &m)?;
                say(&format!("{}: entry {} (room {}), scope {:?}, key {}{}", j.name, j.entry,
                             a.rows[j.entry].room.map(|r| r.to_string()).unwrap_or("None".into()), j.scope,
                             pyjson::compact(&read_json(&d.join("select.json"))?["key"]),
                             kind.map(|k| format!(", {} ({})", k.text(), k.why)).unwrap_or_default()));
                running.push(j);
                stats.jobs += 1;
            }
        }
        if running.is_empty() {
            if stop.is_none() && bound && a.goal_entries().is_empty() {
                stop = Some("prediction bound".into());
                detail = json!({"jobs": a.job_names().len(), "stop_jobs": stop_jobs});
            }
            if stop.is_none() && walled && a.goal_entries().is_empty() {
                stop = Some("wall limit".into());
                detail = json!({"wall_s": pyjson::round(wall0.elapsed().as_secs_f64(), 1), "stop_wall_s": stop_wall,
                                "jobs": a.job_names().len()});
            }
            if stop.is_none() && asked && a.goal_entries().is_empty() {
                let p = stop_f.as_ref().unwrap();
                let why = fs::read_to_string(p).ok().and_then(|t| t.lines().next().map(|l| l.to_string())).unwrap_or_default();
                stop = Some("stop file".into());
                detail = json!({"wall_s": pyjson::round(wall0.elapsed().as_secs_f64(), 1), "file": p.to_string_lossy(),
                                "why": why, "jobs": a.job_names().len()});
            }
            if stop.is_none() {
                if a.select(1).is_empty() {
                    stop = Some("exhausted".into());
                } else if a.stagnant() {
                    stop = Some("stagnation".into());
                } else if floor {
                    stop = Some("disk floor".into());
                    detail = json!({"free_gb": pyjson::round(free() / GIB, 1), "floor_gb": m.floor_gb});
                } else {
                    stop = Some("no job could start".into());
                }
            }
            let own = t_own.elapsed().as_secs_f64();
            stats.own_s += own;
            break;
        }
        let own = t_own.elapsed().as_secs_f64();
        stats.own_s += own;
        if committed {
            stats.own_by_job.push(own.max(t_ingest));
        }
        if !committed {
            wait(m.poll_s, &mut woken);
        }
    }
    let mut summary = json!({"stop": stop, "detail": detail, "jobs": a.committed_jobs().len(), "entries": a.rows.len(),
                             "cells": a.cells.len(), "goal_entries": a.goal_entries(),
                             "time": files::strftime("%Y-%m-%dT%H:%M:%S")});
    if stop.as_deref() == Some("goal") {
        match verifier {
            Some(v) => {
                let e = detail["entry"].as_u64().unwrap_or(0) as usize;
                say(&format!("goal entry {e}: verifying its movie from boot"));
                let r = v.verify(&a, e)?;
                let mut s = Map::new();
                for k in ["entry", "same", "goal", "frames", "qf", "seconds", "inputs", "segments"] {
                    s.insert(k.into(), r.get(k).cloned().unwrap_or(Value::Null));
                }
                summary["verify"] = Value::Object(s);
            }
            None => summary["verify"] = Value::Null,
        }
    }
    write_json(&a.root.join("summary.json"), &summary, Some(1))?;
    say(&format!("stop: {} {}", stop.as_deref().unwrap_or("None"), if detail.is_null() { String::new() } else { files::py_repr(&detail) }));
    Ok(summary)
}
