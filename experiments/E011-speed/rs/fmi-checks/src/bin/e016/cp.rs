//! `e016 cp`: the movie to frame F and the engine's checkpoint there, as an archive entry keeps it
//! ([frame, facts, {tick, rng, actors}]), so a single job can start from a state no entry holds. Writes OUT/cp.json.

use crate::common::*;
use fmi_core::*;
use fmi_engine::Host;
use fmi_explore::{harness, step};
use serde_json::{json, Value};
use std::path::PathBuf;

pub fn run(argv: &[String]) -> Res<()> {
    let a = args(argv);
    let out = PathBuf::from(a.pos.first().ok_or("OUT")?);
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(a.s("movie")?).map_err(e2s)?).map_err(e2s)?;
    let n = a.num("clicks")? as usize;
    let frame = a.num("frame")? as u64;
    let movie: Vec<Event> = doc["movie"]
        .as_array()
        .ok_or("the file has no \"movie\"")?
        .iter()
        .take(n)
        .map(|c| Event { frame: c[0].as_u64().unwrap_or(0), what: Input::Click(c[1].as_i64().unwrap_or(0) as i32, c[2].as_i64().unwrap_or(0) as i32) })
        .collect();
    let h = Host::from_env().map_err(e2s)?;
    let mut e = boot(&h, &out.join("boot"), &movie, frame)?;
    let s = e.obs(&ObsReq::default()).map_err(e2s)?;
    let st = e.status().clone();
    let cp = json!([st.frame, step::facts_of_ordered(&s), harness::extra_of(&s, st.tick, st.rng)]);
    std::fs::write(out.join("cp.json"), serde_json::to_string(&cp).map_err(e2s)?).map_err(e2s)?;
    println!("{}", json!({"frame": st.frame, "reason": st.reason, "idle": st.idle, "room": st.room}));
    Ok(())
}
