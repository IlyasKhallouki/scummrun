//! Reading the packed file (format: README.md). The tables (strings, facts, add tables, goal, starts) are read
//! first through the footer; the actions then stream in through a buffered reader, one record at a time, each
//! turned into domain indices as it comes (the file holds value strings; the model holds indices).
//!
//! What this does that task.py's `_expand` does, so that the writer and the reductions see only sets of indices:
//! a precondition or condition set is cut to the domain (values the domain lacks are dropped); a written value
//! the domain lacks makes a `K_BAD` effect (the writer raises on it in a live action, as `_expand` does); an `add`
//! becomes, per current value `u` its own condition admits, `when key = u: key := u + d`, a guess where `u + d`
//! is several values (`Domain.add`, read from the file's add table), exactly `_expand`'s own expansion.

use crate::model::*;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

pub const MAGIC: u32 = 0x524F_4354; // "TCOR"
pub const END_MAGIC: u32 = 0x444E_4554; // "TEND"
pub const VERSION: u32 = 1;
const OP_SET: u32 = 0;
const OP_GUESS: u32 = 1;
const OP_ADD: u32 = 2;

fn words_of(bytes: &[u8]) -> Vec<u32> {
    bytes.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

fn read_words<R: Read>(r: &mut R, n: usize, buf: &mut Vec<u32>) -> std::io::Result<()> {
    let mut bytes = vec![0u8; n * 4];
    r.read_exact(&mut bytes)?;
    buf.clear();
    buf.extend(bytes.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])));
    Ok(())
}

pub struct Loaded {
    pub task: Task,
    pub file_bytes: u64,
    pub add_effects: u64,
}

pub fn load(path: &Path) -> Result<Loaded, String> {
    let err = |e: std::io::Error| format!("{}: {}", path.display(), e);
    let mut f = File::open(path).map_err(err)?;
    let len = f.metadata().map_err(err)?.len();
    if len < 40 {
        return Err(format!("{}: too short", path.display()));
    }
    let mut hdr = Vec::new();
    read_words(&mut f, 2, &mut hdr).map_err(err)?;
    if hdr[0] != MAGIC || hdr[1] != VERSION {
        return Err(format!("{}: not a taskcore file of version {}", path.display(), VERSION));
    }
    f.seek(SeekFrom::End(-32)).map_err(err)?;
    let mut foot = Vec::new();
    read_words(&mut f, 8, &mut foot).map_err(err)?;
    if foot[7] != END_MAGIC {
        return Err(format!("{}: bad footer", path.display()));
    }
    let (body_start, nact, o_str, o_facts, o_adds, o_goal, o_starts) =
        (foot[0] as u64, foot[1] as usize, foot[2] as usize, foot[3] as usize, foot[4] as usize, foot[5] as usize, foot[6] as usize);
    // the tables
    let tab_start = o_str as u64 * 4;
    f.seek(SeekFrom::Start(tab_start)).map_err(err)?;
    let mut tb = vec![0u8; (len - 32 - tab_start) as usize];
    f.read_exact(&mut tb).map_err(err)?;
    let nstr = u32::from_le_bytes([tb[0], tb[1], tb[2], tb[3]]) as usize;
    let lens = words_of(&tb[4..4 + 4 * nstr]);
    let mut p = 4 + 4 * nstr;
    let mut list = Vec::with_capacity(nstr);
    for &l in &lens {
        let s = std::str::from_utf8(&tb[p..p + l as usize]).map_err(|e| format!("string table: {}", e))?;
        list.push(s.to_string());
        p += l as usize;
    }
    let strs = Strings::from_list(list);
    let tw = words_of(&tb[(o_facts - o_str) * 4..]); // facts .. starts, word 0 = facts table
    let at = |o: usize| o - o_facts;
    // facts
    let mut q = at(o_facts);
    let nf = tw[q] as usize;
    q += 1;
    let mut facts = Facts { key: Vec::with_capacity(nf), dom_off: vec![0], dom: Vec::new() };
    let mut vmap: FxMap<u64, u32> = FxMap::default(); // (fact << 32 | value string) -> index
    for fi in 0..nf {
        facts.key.push(tw[q]);
        let n = tw[q + 1] as usize;
        for (j, &v) in tw[q + 2..q + 2 + n].iter().enumerate() {
            vmap.entry(((fi as u64) << 32) | v as u64).or_insert(j as u32);
            facts.dom.push(v);
        }
        facts.dom_off.push(facts.dom.len() as u32);
        q += 2 + n;
    }
    let idx = |f: u32, v: u32| vmap.get(&(((f as u64) << 32) | v as u64)).copied();
    // add tables: (fact, d) -> per domain value the indices of u + d (sorted)
    let mut q = at(o_adds);
    let nadd = tw[q] as usize;
    q += 1;
    let mut adds: FxMap<(u32, u32), Vec<Vec<u32>>> = FxMap::default();
    for _ in 0..nadd {
        let (fact, d, n) = (tw[q], tw[q + 1], tw[q + 2] as usize);
        q += 3;
        let mut tbl = Vec::with_capacity(n);
        for _ in 0..n {
            let m = tw[q] as usize;
            let mut vs: Vec<u32> = tw[q + 1..q + 1 + m].iter().filter_map(|&v| idx(fact, v)).collect();
            vs.sort_unstable();
            vs.dedup();
            tbl.push(vs);
            q += 1 + m;
        }
        adds.insert((fact, d), tbl);
    }
    // goal
    let mut q = at(o_goal);
    let ng = tw[q] as usize;
    q += 1;
    let mut goal = Vec::with_capacity(ng);
    for _ in 0..ng {
        let (fact, n) = (tw[q], tw[q + 1] as usize);
        let mut vs: Vec<u32> = tw[q + 2..q + 2 + n].iter().filter_map(|&v| idx(fact, v)).collect();
        vs.sort_unstable();
        vs.dedup();
        goal.push((fact, vs));
        q += 2 + n;
    }
    // starts
    let mut q = at(o_starts);
    let ns = tw[q] as usize;
    q += 1;
    let mut starts = Vec::with_capacity(ns);
    for _ in 0..ns {
        let name = tw[q];
        q += 1;
        let vals: Vec<u32> = (0..nf as u32).map(|fi| {
            let v = tw[q + fi as usize];
            if v == NONE { NONE } else { idx(fi, v).unwrap_or(NONE) }
        }).collect();
        q += nf;
        starts.push((name, vals));
    }
    drop(tw);
    drop(tb);
    // the actions, streamed
    f.seek(SeekFrom::Start(body_start * 4)).map_err(err)?;
    let mut r = BufReader::with_capacity(1 << 20, f);
    let mut acts = Actions::default();
    let mut buf = Vec::new();
    let mut one = Vec::new();
    let mut add_effects = 0u64;
    let cut = |f: u32, sids: &[u32]| -> Vec<u32> {
        let mut vs: Vec<u32> = sids.iter().filter_map(|&v| idx(f, v)).collect();
        vs.sort_unstable();
        vs.dedup();
        vs
    };
    for ai in 0..nact {
        read_words(&mut r, 1, &mut one).map_err(err)?;
        read_words(&mut r, one[0] as usize, &mut buf).map_err(err)?;
        let b = &buf;
        let meta = Meta { rec: b[0], label: b[1], kind: b[2], end: b[3], cont: b[4], cost: b[5] };
        let mut c = 7;
        let mut act = OAct::default();
        for _ in 0..b[6] {
            let (fact, n) = (b[c], b[c + 1] as usize);
            act.pre.push(OSet { fact, vals: cut(fact, &b[c + 2..c + 2 + n]) });
            c += 2 + n;
        }
        let neff = b[c];
        c += 1;
        for _ in 0..neff {
            let (op, fact) = (b[c], b[c + 1]);
            c += 2;
            let (payload, d): (Vec<u32>, u32) = match op {
                OP_SET => {
                    c += 1;
                    (vec![b[c - 1]], 0)
                }
                OP_GUESS => {
                    let n = b[c] as usize;
                    c += 1 + n;
                    (b[c - n..c].to_vec(), 0)
                }
                OP_ADD => {
                    c += 1;
                    (Vec::new(), b[c - 1])
                }
                _ => return Err(format!("action {}: unknown effect op {}", ai, op)),
            };
            let nw = b[c];
            c += 1;
            let mut when = Vec::with_capacity(nw as usize);
            for _ in 0..nw {
                let (wf, n) = (b[c], b[c + 1] as usize);
                when.push(OSet { fact: wf, vals: cut(wf, &b[c + 2..c + 2 + n]) });
                c += 2 + n;
            }
            match op {
                OP_SET => match idx(fact, payload[0]) {
                    Some(v) => act.eff.push(OEff { fact, kind: K_SET, vals: vec![v], when }),
                    None => act.eff.push(OEff { fact, kind: K_BAD, vals: vec![payload[0]], when }),
                },
                OP_GUESS => {
                    let mut vs: Vec<u32> = Vec::with_capacity(payload.len());
                    let mut bad = None;
                    for &s in &payload {
                        match idx(fact, s) {
                            Some(v) => vs.push(v),
                            None => bad = Some(s),
                        }
                    }
                    vs.sort_unstable();
                    vs.dedup();
                    match bad {
                        None => act.eff.push(OEff { fact, kind: K_GUESS, vals: vs, when }),
                        Some(s) => act.eff.push(OEff { fact, kind: K_BAD, vals: vec![s], when }),
                    }
                }
                _ => {
                    // task._expand: own = the last condition on the key itself (a set that is not the whole
                    // domain), rest = the other conditions; one effect per admitted current value
                    add_effects += 1;
                    let size = facts.size(fact);
                    let own = when.iter().rev().find(|s| s.fact == fact && s.vals.len() < size as usize).map(|s| s.vals.clone());
                    let rest: Vec<OSet> = when.iter().filter(|s| s.fact != fact).cloned().collect();
                    let tbl = adds.get(&(fact, d)).ok_or_else(|| format!("action {}: no add table for fact {} step {}", ai, fact, d as i32))?;
                    for u in 0..size {
                        if let Some(o) = &own {
                            if !contains(o, u) {
                                continue;
                            }
                        }
                        let mut w = rest.clone();
                        w.push(OSet { fact, vals: vec![u] });
                        let nxt = &tbl[u as usize];
                        if size == 1 && nxt.len() == 1 && nxt[0] == u {
                            // _expand keeps the condition key = u (it does not cut its own conditions to the
                            // domain), and then leaves the write out as one of the value the key already has; the
                            // writer here cuts every condition that holds for the whole domain, so leave it out now
                            continue;
                        }
                        match nxt.len() {
                            0 => act.eff.push(OEff { fact, kind: K_BAD, vals: vec![NONE], when: w }),
                            1 => act.eff.push(OEff { fact, kind: K_SET, vals: nxt.clone(), when: w }),
                            _ => act.eff.push(OEff { fact, kind: K_GUESS, vals: nxt.clone(), when: w }),
                        }
                    }
                }
            }
        }
        acts.push(meta, &[ai as u32], &act);
    }
    Ok(Loaded { task: Task { strs, facts, goal, starts, acts, n_orig: nact as u32 }, file_bytes: len, add_effects })
}
