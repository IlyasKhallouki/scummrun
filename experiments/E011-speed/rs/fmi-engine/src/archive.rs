//! An explorer archive's entries as the checks read them (explore.py's `entries/<id>.json.gz`): the frame, the parent,
//! the movie from boot and the checkpoints. The same reading as the `fmi-engine` tool's own (src/bin/fmi-engine.rs),
//! for E012's checks (src/bin/fmi-engine-end.rs).

use crate::json::{self, J};
use fmi_core::*;
use std::path::Path;

pub type Res<T> = Result<T, String>;

/// One archive entry: its frame, parent, movie (events from boot, absolute frames) and checkpoints.
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: u64,
    pub parent: Option<u64>,
    pub frame: u64,
    pub events: Vec<Event>,
    pub checkpoints: Vec<Checkpoint>,
}

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// `gzip -dc` of a file, as text.
pub fn gunzip(p: &Path) -> Res<String> {
    let out = std::process::Command::new("gzip")
        .arg("-dc")
        .arg(p)
        .output()
        .map_err(e2s)?;
    if !out.status.success() {
        return Err(format!("gzip -dc {}: {}", p.display(), String::from_utf8_lossy(&out.stderr)));
    }
    String::from_utf8(out.stdout).map_err(e2s)
}

/// One movie event (replay.py's event dict).
pub fn event_of(j: &J) -> Res<Event> {
    let frame = j.at("frame")?.int()? as u64;
    let pt = |k: &str| -> Res<Option<(i32, i32)>> {
        match j.get(k) {
            None => Ok(None),
            Some(v) => {
                let a = v.arr()?;
                Ok(Some((a[0].i32()?, a[1].i32()?)))
            }
        }
    };
    let what = if let Some((x, y)) = pt("click")? {
        Input::Click(x, y)
    } else if let Some((x, y)) = pt("rclick")? {
        Input::RClick(x, y)
    } else if let Some((x, y)) = pt("move")? {
        Input::Move(x, y)
    } else if let Some(k) = j.get("key") {
        Input::Key(k.text()?.to_string())
    } else if let Some(s) = j.get("skips") {
        Input::Skips {
            text: s.at("text")?.int()? != 0,
            cutscenes: s.at("cutscenes")?.int()? != 0,
            spare: match s.get("spare") {
                Some(v) => Some(v.arr()?.iter().map(J::i32).collect::<Res<_>>()?),
                None => None,
            },
        }
    } else {
        return Err(format!("unknown event {j:?}"));
    };
    Ok(Event { frame, what })
}

/// One checkpoint: [frame, facts, {tick, rng, actors}].
pub fn checkpoint_of_json(j: &J) -> Res<Checkpoint> {
    let a = j.arr()?;
    let facts = Facts::from_pairs(
        a[1].obj()?
            .iter()
            .map(|(k, v)| Ok((Key::parse(k), v.int()?)))
            .collect::<Res<Vec<_>>>()?,
    );
    let x = &a[2];
    let mut actors = Vec::new();
    for t in x.at("actors")?.arr()? {
        let t = t.arr()?;
        actors.push((t[0].i32()?, t[1].i32()?, t[2].i32()?));
    }
    Ok(Checkpoint {
        frame: a[0].int()? as u64,
        facts,
        tick: x.at("tick")?.int()? as u64,
        rng: x.at("rng")?.int()? as u64,
        actors,
    })
}

/// Entry `idx` of the archive at `arch`.
///
/// A job's new entries (a step's `new/<i>.json.gz`, id null) name their parent in the archive the job ran on, not in
/// their own directory. Those whose parent is that archive's root (0, which has no events) hold their movie from
/// boot, so their chain is themselves: they are read with no parent. Any other such parent is outside the directory,
/// and an error.
pub fn entry(arch: &Path, idx: u64) -> Res<Entry> {
    let j = json::parse(&gunzip(&arch.join("entries").join(format!("{idx}.json.gz")))?)?;
    let mut parent = j.get("parent").filter(|p| !p.is_null()).map(J::int).transpose()?.map(|p| p as u64);
    if j.get("id").is_some_and(J::is_null) {
        match parent {
            Some(0) | None => parent = None,
            Some(p) => return Err(format!("entry {idx}: a job's new entry whose parent {p} is not this directory's")),
        }
    }
    Ok(Entry {
        id: idx,
        parent,
        frame: j.at("frame")?.int()? as u64,
        events: j.at("events")?.arr()?.iter().map(event_of).collect::<Res<_>>()?,
        checkpoints: j
            .at("checkpoints")?
            .arr()?
            .iter()
            .map(checkpoint_of_json)
            .collect::<Res<_>>()?,
    })
}

/// Every entry of the archive, by id.
pub fn load_all(arch: &Path) -> Res<std::collections::BTreeMap<u64, Entry>> {
    ids(arch)?.into_iter().map(|id| Ok((id, entry(arch, id)?))).collect()
}

/// Entry `id` with its whole chain from the root (fmi-explore archive.rs `movie` and `checkpoints`): the events of
/// every entry on the chain, root first, as its movie from boot, and their checkpoints, root first. An entry's file
/// holds only its own segment of the movie (the root's and the root's children's segments start at boot).
pub fn chained(all: &std::collections::BTreeMap<u64, Entry>, id: u64) -> Res<Entry> {
    let mut chain = vec![id];
    while let Some(p) = all.get(chain.last().unwrap()).ok_or_else(|| format!("no entry {}", chain.last().unwrap()))?.parent {
        if chain.contains(&p) || chain.len() > all.len() {
            return Err(format!("entry {id}: the parents make a loop"));
        }
        chain.push(p);
    }
    let me = &all[&id];
    let mut out = Entry {
        id,
        parent: me.parent,
        frame: me.frame,
        events: Vec::new(),
        checkpoints: Vec::new(),
    };
    for j in chain.iter().rev() {
        out.events.extend(all[j].events.iter().cloned());
        out.checkpoints.extend(all[j].checkpoints.iter().cloned());
    }
    Ok(out)
}

/// `chained` for one entry, reading only the files of its chain.
pub fn chain_of(arch: &Path, id: u64) -> Res<Entry> {
    let mut all = std::collections::BTreeMap::new();
    let mut at = Some(id);
    while let Some(i) = at {
        if all.contains_key(&i) {
            return Err(format!("entry {id}: the parents make a loop"));
        }
        let e = entry(arch, i)?;
        at = e.parent;
        all.insert(i, e);
    }
    chained(&all, id)
}

/// The ids of every entry file of the archive, ascending.
pub fn ids(arch: &Path) -> Res<Vec<u64>> {
    let mut v = Vec::new();
    for d in std::fs::read_dir(arch.join("entries")).map_err(e2s)? {
        let n = d.map_err(e2s)?.file_name().to_string_lossy().into_owned();
        if let Some(id) = n.strip_suffix(".json.gz").and_then(|s| s.parse::<u64>().ok()) {
            v.push(id);
        }
    }
    v.sort_unstable();
    Ok(v)
}
