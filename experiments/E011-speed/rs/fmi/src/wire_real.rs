//! The real ports (the integration's adapters): fmi-engine's Host and Pool, fmi-reading's Reading and reader-scope,
//! fmi-plan's Host, fmi-play's Search.

use fmi_core::*;
use fmi_explore::ports::*;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// Returns over the engine's kept-entry pool. An entry the pool does not hold yet is registered with the nearest entry
/// returned to before whose movie is a prefix of its own as its parent, so the pool makes it from that one's kept
/// process (a fork and the rest of the movie, exact by A0) instead of from boot.
pub struct PoolReturns {
    pub pool: fmi_engine::Pool,
    known: Vec<(u64, u64, Vec<Event>)>,
}

impl PoolReturns {
    pub fn new(pool: fmi_engine::Pool) -> PoolReturns {
        PoolReturns { pool, known: Vec::new() }
    }
}

fn movie_to(events: &[Event], frame: u64) -> Vec<&Event> {
    events.iter().filter(|e| e.frame <= frame).collect()
}

impl Returns for PoolReturns {
    fn return_to(&mut self, e: &EntryRef, _out: &Path) -> EResult<Box<dyn Engine>> {
        let id = e.id.max(0) as u64;
        if !self.pool.is_kept(id) && !self.known.iter().any(|k| k.0 == id) {
            let parent = self
                .known
                .iter()
                .filter(|(_, f, ev)| *f <= e.frame && movie_to(ev, *f) == movie_to(&e.events, *f))
                .max_by_key(|(_, f, _)| *f)
                .map(|k| k.0);
            self.pool.add(fmi_engine::EntrySpec { id, parent, frame: e.frame, events: e.events.clone() });
            self.known.push((id, e.frame, e.events.clone()));
        }
        Ok(Box::new(self.pool.ret(id)?))
    }
}

/// The readings: reader-scope with --out-dir, opened by fmi-reading. `cache`: fmi-reading's exact reading cache
/// (FMI_READING_CACHE, a folder; off when unset or empty); a hit's files are linked into the request's folder.
/// `memo`: readcore's job memo across readings (FMI_MEMO, a folder, given to reader-scope as --memo; off when unset or
/// empty). `prep`: reader-scope's --prep (FMI_READER_PREP=1; e011-r2-read: the workers work out the reading thread's
/// and the sink's part of their records; the same files), off unless set.
pub struct RealReadings {
    pub reader: PathBuf,
    pub opened: Mutex<HashMap<PathBuf, Arc<fmi_reading::Reading>>>,
    pub reuse: Mutex<Option<Arc<fmi_reading::reuse::ReuseGame>>>,
    pub root: PathBuf,
    pub cache: Option<fmi_reading::cache::ReadingCache>,
    pub memo: Option<PathBuf>,
    pub prep: bool,
    pub shas: std::sync::OnceLock<(String, String)>,
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var(name).ok().filter(|v| !v.is_empty()).map(PathBuf::from)
}

impl RealReadings {
    pub fn new(reader: PathBuf, root: PathBuf) -> Result<RealReadings, String> {
        let cache = match env_dir("FMI_READING_CACHE") {
            Some(d) => Some(fmi_reading::cache::ReadingCache::new(&d).map_err(|e| format!("{}: {e}", d.display()))?),
            None => None,
        };
        Ok(RealReadings {
            reader,
            opened: Mutex::new(Default::default()),
            reuse: Mutex::new(None),
            root,
            cache,
            memo: env_dir("FMI_MEMO"),
            prep: std::env::var("FMI_READER_PREP").is_ok_and(|v| v == "1"),
            shas: std::sync::OnceLock::new(),
        })
    }

    fn run_reader(&self, spec: &fmi_reading::cache::ReadSpec, dir: &Path, q: &ReadReq) -> Result<(), String> {
        let mut cmd = std::process::Command::new(&self.reader);
        cmd.args(spec.args(dir, &q.name, q.threads as usize));
        if let Some(m) = &self.memo {
            cmd.arg("--memo").arg(m);
        }
        if self.prep {
            cmd.arg("--prep");
        }
        let out = cmd.output().map_err(|e| format!("reader-scope: {e}"))?;
        std::fs::write(dir.join("reader-scope.err"), &out.stderr).ok();
        if !out.status.success() {
            return Err(format!("reader-scope failed ({}): {}", out.status, String::from_utf8_lossy(&out.stderr).chars().rev().take(600).collect::<String>().chars().rev().collect::<String>()));
        }
        Ok(())
    }
}

/// Every file of a stored reading, linked into `to` (a hard link, else a symbolic one); the store's own marks stay.
fn link_files(from: &Path, to: &Path) -> std::io::Result<()> {
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let name = e.file_name();
        if !e.file_type()?.is_file() || name == "used" {
            continue;
        }
        let dst = to.join(&name);
        let _ = std::fs::remove_file(&dst);
        if std::fs::hard_link(e.path(), &dst).is_err() {
            std::os::unix::fs::symlink(e.path(), &dst)?;
        }
    }
    Ok(())
}

impl Readings for RealReadings {
    fn open(&self, p: &Path) -> Result<Arc<dyn ReadingSource>, String> {
        if let Some(r) = self.opened.lock().unwrap().get(p) {
            return Ok(r.clone());
        }
        let r = Arc::new(fmi_reading::Reading::open_head(p).map_err(|e| format!("{}: {e}", p.display()))?);
        self.opened.lock().unwrap().insert(p.to_path_buf(), r.clone());
        Ok(r)
    }
    fn reuse_keys(&self, rd: &dyn ReadingSource) -> Result<HashSet<String>, String> {
        let p = rd.dir().join("reading.json");
        let r = self.opened.lock().unwrap().get(&p).cloned().ok_or("reuse keys of a reading not opened here")?;
        let mut g = self.reuse.lock().unwrap();
        if g.is_none() {
            *g = Some(Arc::new(fmi_reading::reuse::ReuseGame::load(&self.root).map_err(|e| format!("the game: {e}"))?));
        }
        let ks: BTreeSet<String> = g.as_ref().unwrap().reuse_keys(&r).map_err(|e| format!("reuse keys: {e}"))?;
        Ok(ks.into_iter().collect())
    }
    fn read(&self, q: &ReadReq, _cancel: &AtomicBool) -> Result<PathBuf, String> {
        std::fs::create_dir_all(&q.out).map_err(|e| format!("{}: {e}", q.out.display()))?;
        let slot = if q.slot_locals.is_empty() {
            None
        } else {
            let f = q.out.join("slot-locals.jsonl");
            std::fs::write(&f, q.slot_locals.iter().map(|x| x.to_string() + "\n").collect::<String>()).map_err(|e| e.to_string())?;
            Some(f)
        };
        let spec = fmi_reading::cache::ReadSpec {
            rooms: q.rooms.iter().map(|r| *r as i32).collect(),
            seed: q.seed,
            goal: q.goal_text.clone(),
            domain_goals: q.domain_goals.clone(),
            elsewhere: q.elsewhere,
            live_locals: q.live_locals,
            split_limit: q.split_limit,
            switches: q.switches.clone(),
            reach: q.reach,
            frame_costs: q.frame_costs,
            reach_also: q.reach_also.clone(),
            start_rows: q.start_rows.clone(),
            fact_vars: q.fact_vars.iter().map(|v| *v as i32).collect(),
            slot_locals: slot,
        };
        let Some(cache) = &self.cache else {
            self.run_reader(&spec, &q.out, q)?;
            return Ok(q.out.join("reading.json"));
        };
        let (rs, gs) = match self.shas.get() {
            Some(s) => s.clone(),
            None => {
                let rs = fmi_reading::cache::file_sha1(&self.reader).map_err(|e| format!("{}: {e}", self.reader.display()))?;
                let gi = fmi_reading::cache::game_index(&self.root);
                let gs = fmi_reading::cache::file_sha1(&gi).map_err(|e| format!("{}: {e}", gi.display()))?;
                self.shas.get_or_init(|| (rs, gs)).clone()
            }
        };
        let key = spec.key(&rs, &gs, &self.root).map_err(|e| format!("the reading's key: {e}"))?;
        let hit = cache.dir_of(&key).join("reading.json").is_file();
        let dir = if hit {
            cache.dir_of(&key)
        } else {
            let tmp = cache.scratch().map_err(|e| e.to_string())?;
            self.run_reader(&spec, &tmp, q)?;
            let d = cache.put(&key, &tmp).map_err(|e| e.to_string())?;
            cache.evict(std::slice::from_ref(&d)).map_err(|e| format!("the reading cache's eviction: {e}"))?;
            d
        };
        let _ = std::fs::write(dir.join("used"), b"");
        link_files(&dir, &q.out).map_err(|e| format!("{} -> {}: {e}", dir.display(), q.out.display()))?;
        std::fs::write(q.out.join("cache.json"), format!("{{\"key\": \"{key}\", \"hit\": {hit}}}\n")).ok();
        Ok(q.out.join("reading.json"))
    }
}

fn j2v(j: &fmi_play::pyj::J) -> Value {
    fmi_explore::pyjson::loads(&fmi_play::pyj::dumps(j)).unwrap_or(Value::Null)
}

/// The window search as the harness reads it.
pub struct PlayAdapter {
    p: fmi_play::StrictPlayer,
    notes: usize,
    kept: Arc<PlayKept>,
}

/// What one attempt's window search leaves the next (FMI_PLAY_KEEP, on; 0 frees them with the search): the game's IR
/// (Game.load of the game files, which never change in a run) and the last reading's derived data with the setting
/// it was made with (the search makes it again when the reading's id or the setting differs).
#[derive(Default)]
pub struct PlayKept {
    game: Mutex<Option<Arc<scumm_ir::Game>>>,
    reading: Mutex<Option<(bool, Arc<fmi_play::reading::ReadingData>)>>,
}

impl Drop for PlayAdapter {
    fn drop(&mut self) {
        if let Some(r) = self.p.search.root.take() {
            r.quit();
        }
        if play_keep() {
            if let Some(g) = self.p.search.game.take() {
                *self.kept.game.lock().unwrap() = Some(g);
            }
            if let Some(d) = self.p.search.reading.take() {
                let old = self.kept.reading.lock().unwrap().replace((self.p.search.cfg.unobserved_actor_rooms, d));
                drop(old);
            }
        }
    }
}

fn play_keep() -> bool {
    std::env::var("FMI_PLAY_KEEP").map(|v| v != "0").unwrap_or(true)
}

impl Player for PlayAdapter {
    fn observe(&mut self, ctx: &PlanCtx<'_>, ps: &mut PlayState) -> EResult<()> {
        self.p.observe(ctx, ps)
    }
    fn play_plan(&mut self, ctx: &PlanCtx<'_>, ps: &mut PlayState, n: u32) -> PlayOutcome {
        self.p.play_plan(ctx, ps, n)
    }
}

impl WindowPlayer for PlayAdapter {
    fn goal_credited(&self) -> bool {
        self.p.search.goal_credited
    }
    fn replan_cause(&self) -> Option<String> {
        self.p.search.replan_cause.clone()
    }
    fn replan_cause_detail(&self) -> Option<Value> {
        self.p.search.replan_cause_detail.as_ref().map(j2v)
    }
    fn short(&self, cause: &Value) -> String {
        let j = fmi_play::pyj::parse(&cause.to_string()).ok();
        fmi_play::strict::short(j.as_ref())
    }
    fn plan_notes(&mut self) -> Option<Value> {
        let n = &self.p.search.plan_notes;
        if n.len() > self.notes {
            self.notes = n.len();
            n.last().map(j2v)
        } else {
            None
        }
    }
    fn movie(&self) -> Vec<Event> {
        self.p.search.movie.clone()
    }
    fn flush(&mut self, ps: &mut PlayState) {
        self.p.search.flush(ps)
    }
    fn summary(&mut self, _ps: &PlayState) -> Value {
        j2v(&self.p.search.summary())
    }
    fn windows(&self) -> Vec<Value> {
        self.p.search.windows.iter().map(j2v).collect()
    }
}

#[derive(Default)]
pub struct PlayHost {
    kept: Arc<PlayKept>,
    /// E012's delay fork: the game's draws (component tested's `DrawGame`), loaded once in the process when the fork
    /// is on (None: the game did not load; the fork reads the checks' reasons alone)
    draws: std::sync::OnceLock<Option<Arc<fmi_reading::draws::DrawGame>>>,
}

impl PlayHost {
    /// tested's `draws` as the window search reads a record's draws (delay's `DrawsOf`): check, range and share.
    fn draws_of(&self) -> Option<fmi_play::delay::DrawsOf> {
        let g = self.draws.get_or_init(|| fmi_reading::draws::DrawGame::load(&fmi_explore::guide::default_root()).ok().map(Arc::new)).clone()?;
        Some(Arc::new(move |rd: &dyn ReadingSource, i: u32| {
            fmi_reading::draws::draws(rd, i, Some(&g)).into_iter().map(|d| Draw { check: d.check, range: d.range, share: d.share }).collect()
        }))
    }
}

impl PlayerHost for PlayHost {
    fn player(&self, cfg: &WindowCfg) -> Box<dyn WindowPlayer> {
        self.player_with(cfg, &PlayExtras::default())
    }
    /// The window search, with E012's delay fork's settings when it is on (FMI_DELAY_FORK=1, read by the search):
    /// tested's draws, the try's T keys and novelty.
    fn player_with(&self, cfg: &WindowCfg, x: &PlayExtras) -> Box<dyn WindowPlayer> {
        let dmax = cfg.frame_costs.get("dmax").and_then(|d| d.as_i64());
        let mut s = fmi_play::Search::for_step(cfg.workers as usize, cfg.fork_each, cfg.ready, dmax).expect("the window search's config");
        if play_keep() {
            s.game = self.kept.game.lock().unwrap().clone();
            if let Some((flag, d)) = self.kept.reading.lock().unwrap().as_ref() {
                if *flag == s.cfg.unobserved_actor_rooms {
                    s.reading = Some(d.clone());
                }
            }
        }
        if s.delay_on {
            s.draws_of = self.draws_of();
            s.novel_of = x.novel_of.clone();
            s.tested_keys = x.tested_keys.as_ref().map(|k| Arc::new(k.iter().copied().collect()));
        }
        Box::new(PlayAdapter { p: fmi_play::StrictPlayer { search: s }, notes: 0, kept: self.kept.clone() })
    }
}

/// E012's ports of the try (fmi_explore::mech::Mech): T from fmi-reading's `tested`, and the wait component's wait.
pub fn mech() -> fmi_explore::mech::Mech {
    fmi_explore::mech::Mech { tested: Some(Arc::new(RealTested::default())), wait: Some(Arc::new(fmi_explore::mech::RealWait)) }
}

/// T of a reading (component tested, `Tested::of`), made once per reading in the process (a reading's files never
/// change once made; at most 16 kept).
#[derive(Default)]
pub struct RealTested {
    made: Mutex<Vec<([u8; 20], Arc<fmi_explore::mech::TestedT>)>>,
}

impl fmi_explore::mech::TestedPort for RealTested {
    fn tested(&self, rd: &dyn ReadingSource) -> Result<fmi_explore::mech::TestedT, String> {
        let id = rd.id();
        if let Some((_, t)) = self.made.lock().unwrap().iter().find(|(k, _)| *k == id) {
            return Ok((**t).clone());
        }
        let t0 = std::time::Instant::now();
        let t = fmi_reading::tested::Tested::of(rd);
        let mut rec = t.json();
        rec["of_s"] = serde_json::json!(t0.elapsed().as_secs_f64());
        let tt = Arc::new(fmi_explore::mech::TestedT::new(t.flat(), rec));
        let mut m = self.made.lock().unwrap();
        if m.len() >= 16 {
            m.remove(0);
        }
        m.push((id, tt.clone()));
        Ok((*tt).clone())
    }
}
