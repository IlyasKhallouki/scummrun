//! The C8 input events as the archive and the bridge's raw input files write them: {"frame": f, "click": [x, y]},
//! "rclick", "move", {"key": k} and {"skips": {"text": 0|1, "cutscenes": 0|1[, "spare": [...]]}}.

use fmi_core::{Event, Input};
use serde_json::{json, Value};

pub fn event_json(e: &Event) -> Value {
    match &e.what {
        Input::Click(x, y) => json!({"frame": e.frame, "click": [x, y]}),
        Input::RClick(x, y) => json!({"frame": e.frame, "rclick": [x, y]}),
        Input::Move(x, y) => json!({"frame": e.frame, "move": [x, y]}),
        Input::Key(k) => json!({"frame": e.frame, "key": k}),
        Input::Skips { text, cutscenes, spare } => {
            let mut s = json!({"text": *text as i64, "cutscenes": *cutscenes as i64});
            if let Some(sp) = spare {
                s["spare"] = json!(sp);
            }
            json!({"frame": e.frame, "skips": s})
        }
    }
}

fn point(v: &Value) -> Option<(i32, i32)> {
    let a = v.as_array()?;
    Some((a.first()?.as_i64()? as i32, a.get(1)?.as_i64()? as i32))
}

/// An event of the archive's files (None for a row of another shape).
pub fn event_of(v: &Value) -> Option<Event> {
    let frame = v.get("frame")?.as_u64()?;
    let what = if let Some(p) = v.get("click") {
        let (x, y) = point(p)?;
        Input::Click(x, y)
    } else if let Some(p) = v.get("rclick") {
        let (x, y) = point(p)?;
        Input::RClick(x, y)
    } else if let Some(p) = v.get("move") {
        let (x, y) = point(p)?;
        Input::Move(x, y)
    } else if let Some(k) = v.get("key") {
        Input::Key(match k {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
    } else if let Some(s) = v.get("skips") {
        Input::Skips {
            text: s.get("text")?.as_i64()? != 0,
            cutscenes: s.get("cutscenes")?.as_i64()? != 0,
            spare: s.get("spare").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|n| n.as_i64().map(|n| n as i32)).collect()),
        }
    } else {
        return None;
    };
    Some(Event { frame, what })
}

/// replay._is_input
pub fn is_input(v: &Value) -> bool {
    ["key", "click", "rclick", "move"].iter().any(|k| v.get(*k).is_some())
}
