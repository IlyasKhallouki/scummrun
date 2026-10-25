//! golden.py's turn-level trace (`TraceMachine`): one event per line, in the order they happened.
//!
//! Path ids are given in the order paths first appear in an event (`Tracer::tid`); a fork has none until
//! an event names it. Lines go to a sink as they come (a gzip file in the runner), so a long run does not
//! hold its trace in memory.

use crate::path::Path;
use scumm_ir::json::J;
use std::collections::BTreeMap;
use std::io::Write;

/// Where trace lines go: a writer that can be closed (a gzip file writes its trailer then).
pub trait Sink: Write {
    fn close(self: Box<Self>) -> std::io::Result<()>;
}

impl Sink for Vec<u8> {
    fn close(self: Box<Self>) -> std::io::Result<()> {
        Ok(())
    }
}

impl<W: Write> Sink for flate2::write::GzEncoder<W> {
    fn close(self: Box<Self>) -> std::io::Result<()> {
        let mut w = (*self).finish()?;
        w.flush()
    }
}

pub struct Tracer {
    pub next_tid: u32,
    pub seq: u64,
    sink: Box<dyn Sink>,
    line: String,
}

impl Tracer {
    pub fn new(sink: Box<dyn Sink>) -> Tracer {
        Tracer { next_tid: 0, seq: 0, sink, line: String::with_capacity(512) }
    }

    /// golden.py `_tid`: the path's id, given now if it has none.
    pub fn tid(&mut self, p: &mut Path) -> u32 {
        match p.tid {
            Some(t) => t,
            None => {
                let t = self.next_tid;
                self.next_tid += 1;
                p.tid = Some(t);
                t
            }
        }
    }

    /// golden.py `_ev`: {"seq", "tid", "type", "steps", "na", ...kw}, keys sorted.
    pub fn ev(&mut self, p: &mut Path, typ: &str, kw: Vec<(&'static str, J)>) {
        let tid = self.tid(p);
        let mut m = BTreeMap::new();
        m.insert("seq".to_string(), J::Int(self.seq as i64));
        m.insert("tid".to_string(), J::Int(tid as i64));
        m.insert("type".to_string(), J::Str(typ.to_string()));
        m.insert("steps".to_string(), J::Int(p.steps as i64));
        m.insert("na".to_string(), J::Int(p.assumptions.len() as i64));
        for (k, v) in kw {
            m.insert(k.to_string(), v);
        }
        self.seq += 1;
        self.line.clear();
        J::Obj(m).write(&mut self.line);
        self.line.push('\n');
        self.sink.write_all(self.line.as_bytes()).expect("trace sink");
    }

    /// Closes the sink; returns the number of events.
    pub fn finish(self) -> std::io::Result<u64> {
        self.sink.close()?;
        Ok(self.seq)
    }
}
