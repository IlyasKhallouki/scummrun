//! E022's offline gates: `ctx_gate ARCHIVE ENTRY` with FMI_PLACE_CTX set or not. G0: room 49's input keys and the ones
//! seen cleared. G1: the live rows' places (count, the largest). G2: ENTRY's place and its job.json "held" atoms of
//! the keys asked in GATE_KEYS (comma-separated).

use fmi_explore::archive::Archive;
use std::collections::BTreeMap;
use std::path::Path;

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let a = Archive::open(Path::new(&argv[1])).unwrap_or_else(|e| panic!("{}: {e}", argv[1]));
    let e: usize = argv[2].parse().expect("ENTRY");
    let room = a.rows[e].room.unwrap_or(-1);
    let (ks, gone) = fmi_explore::ctx::report(room);
    println!("G0 room {room}: {} input keys: {:?}", ks.len(), ks);
    println!("G0 cleared among them: {:?}", gone);
    let mut places: BTreeMap<String, usize> = BTreeMap::new();
    for (i, r) in a.rows.iter().enumerate() {
        if r.dead.is_none() {
            if let Some(p) = a.place_of_row(i) {
                *places.entry(p).or_insert(0) += 1;
            }
        }
    }
    let mut big: Vec<(&String, &usize)> = places.iter().collect();
    big.sort_by(|x, y| y.1.cmp(x.1));
    println!("G1 live places: {}; largest: {:?}", places.len(), &big[..big.len().min(8)]);
    println!("G2 entry {e} place {:?}", a.place_of_row(e));
    let d = a.prepare_job(e).expect("prepare_job");
    let job = fmi_explore::files::read_json(&d.join("job.json")).expect("job.json");
    let want: Vec<String> = std::env::var("GATE_KEYS").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(String::from).collect();
    let held: Vec<&serde_json::Value> = job["held"].as_array().map(|v| v.iter().filter(|h| want.iter().any(|k| h[0].as_str() == Some(k))).collect()).unwrap_or_default();
    println!("G2 held {} atoms; of {:?}: {:?}", job["held"].as_array().map(|v| v.len()).unwrap_or(0), want, held);
    let _ = std::fs::remove_dir_all(&d);
}
