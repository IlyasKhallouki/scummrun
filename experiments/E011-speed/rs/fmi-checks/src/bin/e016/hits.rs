//! `e016 hits` (E022 amendment (12)'s probe): the movie to frame F, an optional click W at F + 1, then frame by frame
//! for K frames the engine's hit test at point P and the state of object S. Writes OUT/hits.json.

use crate::common::*;
use fmi_core::*;
use fmi_engine::Host;
use serde_json::{json, Value};
use std::path::PathBuf;

pub fn run(argv: &[String]) -> Res<()> {
    let a = args(argv);
    let out = PathBuf::from(a.pos.first().ok_or("OUT")?);
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(a.s("movie")?).map_err(e2s)?).map_err(e2s)?;
    let n = a.num("clicks")? as usize;
    let frame = a.num("frame")? as u64;
    let mut movie: Vec<Event> = doc["movie"]
        .as_array()
        .ok_or("the file has no \"movie\"")?
        .iter()
        .take(n)
        .map(|c| Event { frame: c[0].as_u64().unwrap_or(0), what: Input::Click(c[1].as_i64().unwrap_or(0) as i32, c[2].as_i64().unwrap_or(0) as i32) })
        .collect();
    let xy = |s: &str| -> Res<(i32, i32)> {
        let (x, y) = s.split_once(',').ok_or(format!("a point: {s}"))?;
        Ok((x.parse().map_err(e2s)?, y.parse().map_err(e2s)?))
    };
    let p = xy(a.s("point")?)?;
    let start = match a.s("walk").ok() {
        Some(w) => {
            let (x, y) = xy(w)?;
            movie.push(Event { frame: frame + 1, what: Input::Click(x, y) });
            frame + a.num_or("settle", 40)? as u64
        }
        None => frame,
    };
    let obj = a.num("state-obj")? as u64;
    let k = a.num_or("frames", 60)? as u64;
    let h = Host::from_env().map_err(e2s)?;
    let mut e = boot(&h, &out.join("boot"), &movie, start)?;
    // the room's objects in the dump's order (a parent is an index in it), with their classes
    let o = e.obs(&ObsReq::default()).map_err(e2s)?;
    let objs: Vec<Value> = o
        .room_objects
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|r| {
            let cls = o.classes.iter().find(|(id, _)| *id as i32 == r.id).map(|(_, c)| c.clone()).unwrap_or_default();
            json!({"id": r.id, "state": r.state, "parent": r.parent, "xywh": [r.x, r.y, r.w, r.h], "classes": cls})
        })
        .collect();
    println!("{}", json!({"objects": objs}));
    let mut rows = Vec::new();
    for _ in 0..k {
        let f = e.status().frame;
        let at = e.at_many(&[p]).map_err(e2s)?;
        let main = at.first().and_then(|x| x.main).map(|m| json!([m.2, m.3]));
        let st = e.facts().map_err(e2s)?.val(Key::parse(&format!("state:{obj}")));
        rows.push(json!({"frame": f, "hit": main, "state": st}));
        if e.run(&RunSpec { max_frame: Some(f + 1), ..Default::default() }).is_err() {
            break;
        }
    }
    let doc = json!({"point": [p.0, p.1], "state_obj": obj, "rows": rows});
    std::fs::write(out.join("hits.json"), serde_json::to_string(&doc).map_err(e2s)?).map_err(e2s)?;
    println!("{}", serde_json::to_string(&doc).map_err(e2s)?);
    Ok(())
}
