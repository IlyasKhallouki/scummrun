//! `e016 asap`: a movie's clicks re-timed by the engine. From boot (with the bridge's automatic '.' and Esc from frame
//! 1 under `--skips`: SPEEDRUN_SKIP_TEXT and _CUTSCENES, keys only on frames where they act), each click is played on
//! the frame after the engine's next idle stop that follows the previous click (at least 2 frames after it); then the
//! run goes on to the game's end. Writes OUT/asap.jsonl (the re-timed movie, the skips event first) and prints the end
//! as JSON: its frame, tick and reason, or where the run stood when it did not end.

use crate::common::*;
use crate::g3::to_idle;
use fmi_core::*;
use fmi_engine::Host;
use serde_json::json;
use std::path::PathBuf;

pub fn run(argv: &[String]) -> Res<()> {
    let a = args(argv);
    let out = PathBuf::from(a.pos.first().ok_or("OUT")?);
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let evs = read_events(a.s("events")?)?;
    let limit = a.num_or("limit", 60000)? as u64;
    let pre: Vec<Event> = if a.flag("skips") {
        vec![Event { frame: 1, what: Input::Skips { text: true, cutscenes: true, spare: None } }]
    } else {
        vec![]
    };
    let h = Host::from_env().map_err(e2s)?;
    let mut e = boot(&h, &out.join("boot"), &pre, 2)?;
    let mut movie = pre.clone();
    let mut prev = 0u64;
    let mut ended: Option<Status> = None;
    for ev in evs.iter().filter(|x| !matches!(x.what, Input::Skips { .. })) {
        match to_idle(&mut e)? {
            Ok(_) => {}
            Err(_) => {
                ended = Some(e.status().clone());
                break;
            }
        }
        let f = (e.status().frame + 1).max(prev + 2);
        let ne = Event { frame: f, what: ev.what.clone() };
        e.inputs(std::slice::from_ref(&ne)).map_err(e2s)?;
        match e.run(&RunSpec { frame: Some(f), ..Default::default() }) {
            Ok(_) => {}
            Err(EngineError::Ended(st)) => {
                movie.push(ne);
                ended = Some(*st);
                break;
            }
            Err(x) => return Err(e2s(x)),
        }
        movie.push(ne);
        prev = f;
    }
    if ended.is_none() {
        loop {
            let f = e.status().frame;
            if f >= limit {
                break;
            }
            match e.run(&RunSpec { frame: Some((f + 3000).min(limit)), ..Default::default() }) {
                Ok(_) => {}
                Err(EngineError::Ended(st)) => {
                    ended = Some(*st);
                    break;
                }
                Err(x) => return Err(e2s(x)),
            }
        }
    }
    write_events(&out.join("asap.jsonl"), &movie)?;
    let r = match &ended {
        Some(st) => json!({"ended": true, "frame": st.frame, "tick": st.tick, "reason": st.reason, "room": st.room, "clicks": movie.len()}),
        None => {
            let st = e.status();
            json!({"ended": false, "frame": st.frame, "tick": st.tick, "room": st.room, "idle": st.idle, "clicks": movie.len()})
        }
    };
    println!("{r}");
    Ok(())
}
