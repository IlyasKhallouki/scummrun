//! `e016 inlog` (amendment (1)'s probe): see main.rs.

use crate::common::*;
use fmi_core::*;
use fmi_engine::Host;
use fmi_explore::afford;
use serde_json::{json, Value};
use std::path::PathBuf;

fn status_json(s: &Status) -> Value {
    json!({"frame": s.frame, "reason": s.reason, "room": s.room, "up": s.up, "cs": s.cs, "idle": s.idle,
           "xstart": s.xstart, "screen": s.screen, "ego": s.ego, "mv": s.mv})
}

pub fn run(argv: &[String]) -> Res<()> {
    let a = args(argv);
    let out = PathBuf::from(a.pos.first().ok_or("OUT")?);
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let evs = read_events(a.s("events")?)?;
    let from = a.num("from")? as u64;
    let to = a.num("to")? as u64;
    let h = Host::from_env().map_err(e2s)?;
    let mut e = boot(&h, &out.join("boot"), &evs, from)?;
    let st0 = e.status().clone();
    let o = e.obs(&ObsReq::default()).map_err(e2s)?;
    let of = afford::offer_cfg(&mut e, &o, afford::boxes_on(), afford::offer_screen_on()).map_err(e2s)?;
    let verbs: Vec<Value> = o.verbs.iter().map(|v| json!([v.slot, v.id, v.kind, v.key, v.mode, v.saveid, v.x, v.y])).collect();
    let offer = json!({"verbs": of.verbs.iter().map(|p| json!([p.hit.name(), p.x, p.y])).collect::<Vec<_>>(),
                       "targets": of.targets.iter().map(|p| json!([p.hit.name(), p.x, p.y])).collect::<Vec<_>>(),
                       "walks": of.walks.len()});
    // --runs: the segment's slot runs from F on, the object scripts' (where inventory, room or floating object) listed
    let want_runs = a.flag("runs");
    if want_runs {
        e.segment(true).map_err(e2s)?;
        e.runs().map_err(e2s)?;
    }
    let dropped0 = e.input_log().map_err(e2s)?.records.len();
    // the boot plays the events up to its frame; the later ones are scheduled here
    let later: Vec<Event> = evs.iter().filter(|x| x.frame > from).cloned().collect();
    e.inputs(&later).map_err(e2s)?;
    let (_, ended) = run_until(&mut e, to.saturating_sub(from), &mut |_| Ok(false))?;
    let log = e.input_log().map_err(e2s)?;
    let runs: Vec<Value> = if want_runs {
        e.runs().map_err(e2s)?.runs.iter()
            .filter(|r| [WIO_INVENTORY, WIO_ROOM, WIO_FLOBJECT].contains(&r.where_))
            .map(|r| json!({"frame": r.frame, "slot": r.slot, "where": r.where_, "number": r.number, "room": r.room,
                            "code0": r.code0, "code1": r.code1, "status": r.status, "depth": r.depth}))
            .collect()
    } else {
        vec![]
    };
    let st1 = e.status().clone();
    e.quit();
    let v = json!({"from": status_json(&st0), "to": status_json(&st1), "ended": ended, "records_before": dropped0,
                   "dropped": log.dropped, "verbs_at_from": verbs, "offer_at_from": offer,
                   "inventory_at_from": o.inventory,
                   "objects_at_from": o.room_objects.as_deref().unwrap_or(&[]).iter()
                       .map(|r| json!([r.id, r.x, r.y, r.w, r.h, r.parent, r.walk, o.classes.iter().find(|c| c.0 as i32 == r.id).map(|c| c.1.clone())]))
                       .collect::<Vec<_>>(),
                   "actors_at_from": o.actors.iter().map(|x| json!([x.id, x.x, x.y, x.elevation])).collect::<Vec<_>>(),
                   "records": log.records.iter().map(record_json).collect::<Vec<_>>(), "object_runs": runs});
    std::fs::write(out.join("inlog.json"), serde_json::to_string_pretty(&v).map_err(e2s)?).map_err(e2s)?;
    println!("{v}");
    Ok(())
}
