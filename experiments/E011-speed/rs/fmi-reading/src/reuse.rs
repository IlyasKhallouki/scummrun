//! The reuse rules of a cached reading (experiments/E008-go-explore/step.py), ported:
//!
//! * E008 (step 2): [`reusable`], [`reusable_detail`]: the entry's facts differ from the reading's start facts only
//!   on the reading's task facts and on slot keys (`at:`, `script:`, `local:`, `objscript:`, `esc:`).
//! * E009 fix (c): [`ReuseGame::keys_of`] is `reuse_keys_of` (K: the keys `var:N` whose start value the reader reads
//!   at no site), and [`ReuseGame::reuse_keys`] is `reuse_keys` (K of a reading less its fact variables, kept in
//!   reuse-keys.json next to reading.json, the file step.py reads and writes).
//!
//! The game is loaded once ([`ReuseGame::load`]); `named_start_vars` and `hover_guard_vars` depend on it alone and
//! are worked out then.

use crate::pyjson::PJ;
use crate::Reading;
use fmi_core::{Facts, Key, ReadingSource, Val, K_AT, K_ESC, K_LOCAL, K_OBJSCRIPT, K_OTHER, K_SCRIPT};
use scumm_ir::{known, Expr, Game, Stmt, UnitKey};
use scumm_reader::domains;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::time::Instant;

/// step.py SLOT_PREFIXES
pub const SLOT_PREFIXES: [&str; 5] = ["at:", "script:", "local:", "objscript:", "esc:"];
/// step.py REUSE_KEYS_FILE
pub const REUSE_KEYS_FILE: &str = "reuse-keys.json";
/// step.py COUNTER_WHY: a variable domains.build leaves with no domain for this reason keeps E008's rule
pub const COUNTER_WHY: &str = "a counter never compared with a constant";

/// `k.startswith(SLOT_PREFIXES)`.
pub fn is_slot_key(k: Key) -> bool {
    match k.kind() {
        K_AT | K_SCRIPT | K_LOCAL | K_OBJSCRIPT | K_ESC => true,
        K_OTHER => {
            let t = k.text();
            SLOT_PREFIXES.iter().any(|p| t.starts_with(p))
        }
        _ => false,
    }
}

/// step.py `reusable`: (whether the entry's facts differ from the start facts only on `keys` and slot keys, the
/// number of keys they differ on besides those).
pub fn reusable(start: &Facts, keys: &dyn Fn(Key) -> bool, entry: &Facts) -> (bool, usize) {
    let bad = start.diff(entry).into_iter().filter(|&(k, _, _)| !keys(k) && !is_slot_key(k)).count();
    (bad == 0, bad)
}

/// step.py `reusable_detail`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detail {
    pub differ: usize,
    pub task: usize,
    pub slots: usize,
    pub other: usize,
    /// the first 12 other keys (sorted by text): [key, old, new]
    pub first: Vec<(String, Val, Val)>,
}

pub fn reusable_detail(start: &Facts, keys: &dyn Fn(Key) -> bool, entry: &Facts) -> Detail {
    let d = start.diff(entry);
    let mut bad: Vec<(String, Val, Val)> =
        d.iter().filter(|x| !keys(x.0) && !is_slot_key(x.0)).map(|&(k, a, b)| (k.text(), a, b)).collect();
    bad.sort();
    Detail {
        differ: d.len(),
        task: d.iter().filter(|x| keys(x.0)).count(),
        slots: d.iter().filter(|x| !keys(x.0) && is_slot_key(x.0)).count(),
        other: bad.len(),
        first: bad.into_iter().take(12).collect(),
    }
}

/// The task's facts of a reading as a membership test (`set(rd.task.facts)`), with `extra` keys too (E009).
pub fn task_keys<'a>(rd: &'a dyn ReadingSource, extra: &'a BTreeSet<Key>) -> impl Fn(Key) -> bool + 'a {
    move |k| rd.domains().get(k).is_some() || extra.contains(&k)
}

/// step.py `reuse_keys_of`'s result.
#[derive(Clone, Debug, PartialEq)]
pub struct ReuseKeys {
    /// sorted as texts
    pub keys: Vec<String>,
    pub named: Vec<i32>,
    pub guards: Vec<i32>,
    pub s: f64,
}

/// The game and what K needs of it alone (module docstring).
pub struct ReuseGame {
    pub game: Game,
    pub meta_rooms: Vec<i32>,
    pub named: BTreeSet<i32>,
    pub guards: BTreeSet<i32>,
}

fn var_number(name: &str) -> i32 {
    scumm_ir::vars::VARS.iter().find(|v| v.0 == name).map(|v| v.1).unwrap_or_else(|| panic!("no variable {name}"))
}

/// step.py `named_start_vars`: the settings, the hook, cursor, room and boolean variables, VAR_WALKTO_OBJ,
/// VAR_VERB_SCRIPT, VAR_CUTSCENEEXIT_KEY, VAR_TIMER_NEXT, VAR_CHARINC.
pub fn named_start_vars() -> BTreeSet<i32> {
    let mut out: BTreeSet<i32> = domains::SETTINGS.iter().map(|s| s.0).collect();
    out.extend(domains::HOOKS);
    out.extend([scumm_reader::scope::VAR_CURSORSTATE, scumm_reader::scope::VAR_USERPUT]);
    out.extend(domains::ROOM_VARS);
    out.extend(domains::BOOL_VARS);
    out.insert(domains::VAR_WALKTO_OBJ);
    for n in ["VAR_VERB_SCRIPT", "VAR_CUTSCENEEXIT_KEY", "VAR_TIMER_NEXT", "VAR_CHARINC"] {
        out.insert(var_number(n));
    }
    out
}

/// step.py `hover_guard_vars`: the variables any global unit with findObject(VAR_VIRT_MOUSE_X, VAR_VIRT_MOUSE_Y)
/// compares with a constant (or tests alone) in a jump before that findObject whose target lies past it.
pub fn hover_guard_vars(game: &Game) -> BTreeSet<i32> {
    let ast = &game.ast;
    let (mx, my) = (var_number("VAR_VIRT_MOUSE_X"), var_number("VAR_VIRT_MOUSE_Y"));
    let mut out = BTreeSet::new();
    for u in game.ids() {
        if !matches!(game.unit(u).key, UnitKey::Global(_)) {
            continue;
        }
        let ins = game.ins(u);
        let finds: Vec<usize> = ins
            .iter()
            .enumerate()
            .filter(|(_, x)| {
                x.st.is_some_and(|st| {
                    let mut hit = false;
                    ast.walk_stmt(&st, &mut |e| {
                        if let Expr::Getter { name: known::FIND_OBJECT, args } = ast.expr(e) {
                            if matches!(ast.kids(args), [a, b] if ast.expr(*a) == Expr::Var(mx) && ast.expr(*b) == Expr::Var(my)) {
                                hit = true;
                                return false;
                            }
                        }
                        true
                    });
                    hit
                })
            })
            .map(|(i, _)| i)
            .collect();
        for &f in &finds {
            for x in &ins[..f] {
                let Some(Stmt::Jump { cond: Some(c), .. }) = x.st else { continue };
                let target = x.target.map_or(ins.len(), |t| t as usize);
                if target <= f {
                    continue;
                }
                match ast.expr(c) {
                    Expr::Cmp { a, b, .. } => {
                        for (p, q) in [(a, b), (b, a)] {
                            if let (Expr::Var(n), Expr::Const(_)) = (ast.expr(p), ast.expr(q)) {
                                out.insert(n);
                            }
                        }
                    }
                    Expr::Truth(y) | Expr::Not(y) => {
                        if let Expr::Var(n) = ast.expr(y) {
                            out.insert(n);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

impl ReuseGame {
    /// The game under the repository root `root` (scumm_ir::default_root()).
    pub fn load(root: &Path) -> io::Result<ReuseGame> {
        let game = Game::load(root)?;
        let guards = hover_guard_vars(&game);
        Ok(ReuseGame { meta_rooms: domains::meta_rooms(root), named: named_start_vars(), guards, game })
    }

    /// step.py `reuse_keys_of(rooms, start)`: `start` in its text order (reading.json's, sorted).
    pub fn keys_of(&self, rooms: &[i32], start: &Facts) -> ReuseKeys {
        let t = Instant::now();
        let st = scumm_reader::Facts::from_items(start.text_sorted());
        let doms = domains::build(&self.game, &self.meta_rooms, &st, &[]);
        let mut rooms: Vec<i32> = rooms.to_vec();
        rooms.sort_unstable();
        let sc = scumm_reader::Scope::of(&self.game, &rooms, &st);
        let mut keys: BTreeSet<String> = BTreeSet::new();
        for &(n, _) in domains::ENGINE {
            if !self.named.contains(&n) && !self.guards.contains(&n) {
                keys.insert(format!("var:{n}"));
            }
        }
        for (k, why) in &doms.why {
            let n: i32 = k[4..].parse().unwrap_or_else(|_| panic!("a why key that is not var:N: {k}"));
            if self.named.contains(&n)
                || self.guards.contains(&n)
                || domains::SETTINGS.iter().any(|s| s.0 == n)
                || why == COUNTER_WHY
            {
                continue;
            }
            if domains::engine_why(n).is_some() || sc.writes.has(k) {
                keys.insert(k.clone());
            }
        }
        ReuseKeys {
            keys: keys.into_iter().collect(),
            named: self.named.iter().copied().collect(),
            guards: self.guards.iter().copied().collect(),
            s: (t.elapsed().as_secs_f64() * 10.0).round() / 10.0,
        }
    }

    /// step.py `reuse_keys(rd)`: K of the reading less its fact variables, from reuse-keys.json next to its
    /// reading.json when that file is for the reading's start, else worked out and written there (atomically).
    pub fn reuse_keys(&self, rd: &Reading) -> io::Result<BTreeSet<String>> {
        let doc = rd.doc();
        let p = rd.head_path().parent().unwrap_or(Path::new(".")).join(REUSE_KEYS_FILE);
        let start_name = doc["start"]["name"].as_str().unwrap_or_default().to_string();
        if let Ok(text) = std::fs::read_to_string(&p) {
            if let Ok(old) = serde_json::from_str::<serde_json::Value>(&text) {
                if old.get("start").and_then(|s| s.as_str()) == Some(&start_name) {
                    if let Some(ks) = old.get("keys").and_then(|k| k.as_array()) {
                        return Ok(ks.iter().filter_map(|k| k.as_str().map(String::from)).collect());
                    }
                }
            }
        }
        let rooms: Vec<i32> = doc["scope"]["rooms"].as_array().map_or(vec![], |a| a.iter().filter_map(|r| r.as_i64()).map(|r| r as i32).collect());
        let mut k = self.keys_of(&rooms, rd.start());
        let made: BTreeSet<String> = doc
            .get("fact_vars")
            .and_then(|f| f.as_array())
            .map_or_else(BTreeSet::new, |a| a.iter().filter_map(|n| n.as_i64()).map(|n| format!("var:{n}")).collect());
        k.keys.retain(|x| !made.contains(x));
        let out = PJ::obj(vec![
            ("keys", PJ::List(k.keys.iter().map(|x| PJ::str(x.clone())).collect())),
            ("named", PJ::ints(k.named.iter().map(|&n| n as i64))),
            ("guards", PJ::ints(k.guards.iter().map(|&n| n as i64))),
            ("s", PJ::Float(k.s)),
            ("start", PJ::str(start_name)),
        ]);
        let tmp = p.with_file_name(format!("{REUSE_KEYS_FILE}.{}.tmp", std::process::id()));
        std::fs::write(&tmp, out.compact_text() + "\n")?;
        std::fs::rename(&tmp, &p)?;
        Ok(k.keys.into_iter().collect())
    }
}
