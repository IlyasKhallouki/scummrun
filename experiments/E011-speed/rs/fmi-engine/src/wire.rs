//! F1, the engine channel's binary encoding (plan.md "Binary formats"; engine/WIRE.md): little-endian integers at
//! their Rust width, bool u8 0/1, `Option` a u8 tag then the value, `Vec` and `String` a u32 count then the items,
//! structs in fmi-core's field order, enums a u8 variant index then the fields. Framing: u32 payload length, u8 type,
//! the payload.

use fmi_core::*;

// controller to engine
pub const C_RUN: u8 = 0x81;
pub const C_INPUTS: u8 = 0x82;
pub const C_FORK: u8 = 0x83;
pub const C_FORK_EACH: u8 = 0x84;
pub const C_OBS: u8 = 0x85;
pub const C_STATE: u8 = 0x86;
pub const C_HOLDS: u8 = 0x87;
pub const C_AT: u8 = 0x88;
pub const C_SET_RNG: u8 = 0x89;
pub const C_SEGMENT: u8 = 0x8A;
pub const C_RUNS: u8 = 0x8B;
pub const C_EXPLAIN: u8 = 0x8C;
pub const C_EXPLAIN_LOG: u8 = 0x8D;
pub const C_STATE_JSON: u8 = 0x8E;
pub const C_QUIT: u8 = 0x8F;
pub const C_STEP: u8 = 0x90;
/// patch 0004 (round 2, play): a run to a frame answered with the runs, the explain entries and a dump
pub const C_STEP_OBS: u8 = 0x91;
/// patch 0004 (round 2, play): several hit tests in one message
pub const C_AT_MANY: u8 = 0x92;
/// patch 0004 (round 2, play): a run answered with the dump at its stop
pub const C_RUN_OBS: u8 = 0x93;
/// patch 0005 (E012, end): the current room's walk boxes, no payload; replies `R_BOXES`
pub const C_BOXES: u8 = 0x94;
/// patch 0005 (E012, end): the same as the JSON "boxes" reply's text, no payload; replies `R_JSON`
pub const C_BOXES_JSON: u8 = 0x95;
/// patch 0007 (E013, refuse): the input log since the last call (taken), no payload; replies `R_INPUT_LOG`
pub const C_INPUT_LOG: u8 = 0x97;
/// patch 0007 (E013, refuse): the state digest, no payload; replies `R_DIGEST`
pub const C_DIGEST: u8 = 0x98;
/// patch 0007: the JSON "input_log" and "digest" replies' text, no payload; reply `R_JSON`
pub const C_INPUT_LOG_JSON: u8 = 0x99;
pub const C_DIGEST_JSON: u8 = 0x9A;

// engine to controller
pub const R_HELLO: u8 = 0x01;
pub const R_STOP: u8 = 0x02;
pub const R_END: u8 = 0x03;
pub const R_FORKED: u8 = 0x04;
pub const R_OBS: u8 = 0x05;
pub const R_STATE: u8 = 0x06;
pub const R_HOLDS: u8 = 0x07;
pub const R_AT: u8 = 0x08;
pub const R_RUNS: u8 = 0x09;
pub const R_EXPLAIN: u8 = 0x0A;
pub const R_OK: u8 = 0x0B;
pub const R_ERROR: u8 = 0x0C;
pub const R_JSON: u8 = 0x0D;
pub const R_STEP_REPLY: u8 = 0x0E;
/// patch 0004: `Status`, `RunsReply`, `ExplainReply`, `Obs`
pub const R_STEP_OBS_REPLY: u8 = 0x0F;
/// patch 0004: `Vec<At>`
pub const R_AT_MANY: u8 = 0x10;
/// patch 0004: `Status`, `Obs`
pub const R_RUN_OBS_REPLY: u8 = 0x11;
/// patch 0005: `Boxes`
pub const R_BOXES: u8 = 0x12;
/// patch 0007: `InputLog`
pub const R_INPUT_LOG: u8 = 0x14;
/// patch 0007: `Digest`
pub const R_DIGEST: u8 = 0x15;

/// A message being written: the 5-byte header first, its length set by `finish`.
pub struct W(pub Vec<u8>);

impl W {
    pub fn msg(ty: u8) -> W {
        let mut v = Vec::with_capacity(64);
        v.extend_from_slice(&[0, 0, 0, 0, ty]);
        W(v)
    }
    pub fn finish(mut self) -> Vec<u8> {
        let n = (self.0.len() - 5) as u32;
        self.0[..4].copy_from_slice(&n.to_le_bytes());
        self.0
    }
    #[inline]
    pub fn u8(&mut self, v: u8) {
        self.0.push(v)
    }
    #[inline]
    pub fn b(&mut self, v: bool) {
        self.0.push(v as u8)
    }
    #[inline]
    pub fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes())
    }
    #[inline]
    pub fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes())
    }
    #[inline]
    pub fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes())
    }
    #[inline]
    pub fn i64(&mut self, v: i64) {
        self.0.extend_from_slice(&v.to_le_bytes())
    }
    #[inline]
    pub fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes());
    }
    pub fn opt_u64(&mut self, v: Option<u64>) {
        match v {
            Some(x) => {
                self.u8(1);
                self.u64(x)
            }
            None => self.u8(0),
        }
    }
    pub fn cond(&mut self, c: &Cond) {
        self.u8(c.kind as u8);
        self.i32(c.subject);
        self.u8(c.op as u8);
        self.i32(c.value);
        self.u32(c.values.len() as u32);
        for &v in &c.values {
            self.i32(v);
        }
        self.b(c.negate);
    }
    pub fn conds(&mut self, cs: &[Cond]) {
        self.u32(cs.len() as u32);
        for c in cs {
            self.cond(c);
        }
    }
    pub fn run_spec(&mut self, s: &RunSpec) {
        self.opt_u64(s.frame);
        self.opt_u64(s.max_frame);
        self.conds(&s.until);
        self.u32(s.until_any.len() as u32);
        for g in &s.until_any {
            self.conds(g);
        }
        self.b(s.control);
        self.b(s.idle);
        self.b(s.menu);
    }
    pub fn event(&mut self, e: &Event) {
        self.u64(e.frame);
        match &e.what {
            Input::Click(x, y) => {
                self.u8(0);
                self.i32(*x);
                self.i32(*y)
            }
            Input::RClick(x, y) => {
                self.u8(1);
                self.i32(*x);
                self.i32(*y)
            }
            Input::Move(x, y) => {
                self.u8(2);
                self.i32(*x);
                self.i32(*y)
            }
            Input::Key(k) => {
                self.u8(3);
                self.str(k)
            }
            Input::Skips { text, cutscenes, spare } => {
                self.u8(4);
                self.b(*text);
                self.b(*cutscenes);
                match spare {
                    Some(v) => {
                        self.u8(1);
                        self.u32(v.len() as u32);
                        for &x in v {
                            self.i32(x);
                        }
                    }
                    None => self.u8(0),
                }
            }
        }
    }
    pub fn obs_req(&mut self, r: &ObsReq) {
        match &r.objects {
            Some(v) => {
                self.u8(1);
                self.u32(v.len() as u32);
                for &o in v {
                    self.u32(o);
                }
            }
            None => self.u8(0),
        }
        self.b(r.names);
        self.b(r.exact);
    }
    pub fn explain_arm(&mut self, a: Option<&ExplainArm>) {
        match a {
            None => self.u8(0),
            Some(a) => {
                self.u8(1);
                self.u64(a.from);
                self.u64(a.to);
                self.u32(a.units.len() as u32);
                for &(w, n) in &a.units {
                    self.i32(w);
                    self.i32(n);
                }
            }
        }
    }
}

/// A received payload being read; any read past its end is an error.
pub struct R<'a> {
    pub buf: &'a [u8],
    pub pos: usize,
}

pub type DResult<T> = Result<T, String>;

impl<'a> R<'a> {
    pub fn new(buf: &'a [u8]) -> R<'a> {
        R { buf, pos: 0 }
    }
    #[inline]
    fn take(&mut self, n: usize) -> DResult<&'a [u8]> {
        match self.buf.get(self.pos..self.pos + n) {
            Some(s) => {
                self.pos += n;
                Ok(s)
            }
            None => Err(self.short(n)),
        }
    }
    #[cold]
    fn short(&self, n: usize) -> String {
        format!("payload ends at {} (needs {} more)", self.buf.len(), n)
    }
    pub fn done(&self) -> DResult<()> {
        if self.pos == self.buf.len() {
            Ok(())
        } else {
            Err(format!("{} bytes left over", self.buf.len() - self.pos))
        }
    }
    pub fn rest(&self) -> &'a [u8] {
        &self.buf[self.pos..]
    }
    #[inline]
    pub fn u8(&mut self) -> DResult<u8> {
        Ok(self.take(1)?[0])
    }
    #[inline]
    pub fn b(&mut self) -> DResult<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            v => Err(format!("bool {v}")),
        }
    }
    #[inline]
    pub fn u32(&mut self) -> DResult<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    #[inline]
    pub fn i32(&mut self) -> DResult<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    #[inline]
    pub fn u64(&mut self) -> DResult<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    #[inline]
    pub fn i64(&mut self) -> DResult<i64> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    /// A count, no more items than bytes left divided by `min_item`.
    #[inline]
    pub fn count(&mut self, min_item: usize) -> DResult<usize> {
        let n = self.u32()? as usize;
        if min_item > 0 && n.saturating_mul(min_item) > self.buf.len() - self.pos {
            return Err(format!("count {n} past the payload"));
        }
        Ok(n)
    }
    #[inline]
    pub fn bytes(&mut self) -> DResult<&'a [u8]> {
        let n = self.count(1)?;
        self.take(n)
    }
    #[inline]
    pub fn str(&mut self) -> DResult<String> {
        String::from_utf8(self.bytes()?.to_vec()).map_err(|e| e.to_string())
    }
    #[inline]
    pub fn tag(&mut self) -> DResult<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            v => Err(format!("option tag {v}")),
        }
    }
    #[inline]
    pub fn opt_i32(&mut self) -> DResult<Option<i32>> {
        Ok(if self.tag()? { Some(self.i32()?) } else { None })
    }
    #[inline]
    pub fn opt_str(&mut self) -> DResult<Option<String>> {
        Ok(if self.tag()? { Some(self.str()?) } else { None })
    }
    #[inline]
    pub fn opt_bool(&mut self) -> DResult<Option<bool>> {
        Ok(if self.tag()? { Some(self.b()?) } else { None })
    }
    #[inline]
    pub fn i32s(&mut self) -> DResult<Vec<i32>> {
        let n = self.count(4)?;
        let raw = self.take(n * 4)?;
        Ok(raw
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
            .collect())
    }
    #[inline]
    pub fn u32s(&mut self) -> DResult<Vec<u32>> {
        let n = self.count(4)?;
        let raw = self.take(n * 4)?;
        Ok(raw
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect())
    }

    pub fn status(&mut self) -> DResult<Status> {
        let reason = self.str()?;
        let ended = self.b()?;
        let frame = self.u64()?;
        let tick = self.u64()?;
        let qf = self.u64()?;
        let qf0 = self.u64()?;
        let room = self.i32()?;
        let up = self.i32()?;
        let cur = self.i32()?;
        let cs = self.i32()?;
        let v19 = self.i32()?;
        let seg = self.b()?;
        let tick0 = self.u64()?;
        let rng = self.u64()?;
        let idle = self.opt_str()?;
        let blk = self.opt_str()?;
        let ego = if self.tag()? {
            Some((self.i32()?, self.i32()?))
        } else {
            None
        };
        let mv = self.opt_i32()?;
        let walkbox = self.opt_i32()?;
        let cam = self.i32()?;
        let xstart = self.i32()?;
        let room_w = self.i32()?;
        let n = self.count(12)?;
        let mut forks = Vec::with_capacity(n);
        for _ in 0..n {
            forks.push((self.u64()?, self.u32()?));
        }
        // patch 0004: the screen layout (initScreens' b and h, the forced banner screen); None at an end
        let screen = if self.tag()? {
            Some((self.i32()?, self.i32()?, self.b()?))
        } else {
            None
        };
        Ok(Status {
            reason,
            ended,
            frame,
            tick,
            qf,
            qf0,
            room,
            up,
            cur,
            cs,
            v19,
            seg,
            tick0,
            rng,
            idle,
            blk,
            ego,
            mv,
            walkbox,
            cam,
            xstart,
            room_w,
            forks,
            screen,
        })
    }

    pub fn at(&mut self) -> DResult<At> {
        let x = self.i32()?;
        let y = self.i32()?;
        let zone = self.i32()?;
        let main = if self.tag()? {
            Some((self.i32()?, self.i32()?, self.i32()?, self.i32()?))
        } else {
            None
        };
        let verb_slot = self.i32()?;
        Ok(At {
            x,
            y,
            zone,
            main,
            verb_slot,
        })
    }

    /// patch 0005: fmi-core `Boxes` (room, then each box's ul, ur, lr, ll and flags).
    pub fn boxes(&mut self) -> DResult<Boxes> {
        let room = self.i32()?;
        let n = self.count(33)?;
        let mut boxes = Vec::with_capacity(n);
        for _ in 0..n {
            let mut p = [(0, 0); 4];
            for q in &mut p {
                *q = (self.i32()?, self.i32()?);
            }
            boxes.push(WalkBox {
                ul: p[0],
                ur: p[1],
                lr: p[2],
                ll: p[3],
                flags: self.u8()?,
            });
        }
        Ok(Boxes { room, boxes })
    }

    /// patch 0007: fmi-core `InputLog` (speedrun_wire.cpp wireInputLog): per record the frame, the inputs (u8 0 click,
    /// 1 right click, 3 key), the stat, the verdict (u8 0 dropped and its why, 1 ignored, 2 taken), area, value, mode,
    /// slot, zone, key_match, handler, the writes and the reads (u8 kind, a, b, old, now), the ops, the two drop
    /// counts; then the records dropped and the frame.
    pub fn input_log(&mut self) -> DResult<InputLog> {
        let n = self.count(8 + 4 + 4 + 1 + 4 * 6 + 1 + 4 + 4 + 4 + 4 + 4 + 4)?;
        let mut records = Vec::with_capacity(n);
        for _ in 0..n {
            let frame = self.u64()?;
            let ni = self.count(1 + 4)?;
            let mut input = Vec::with_capacity(ni);
            for _ in 0..ni {
                input.push(match self.u8()? {
                    0 => Input::Click(self.i32()?, self.i32()?),
                    1 => Input::RClick(self.i32()?, self.i32()?),
                    3 => Input::Key(self.str()?),
                    k => return Err(format!("input_log: input kind {k}")),
                });
            }
            let stat = self.i32()?;
            let verdict = match self.u8()? {
                0 => Verdict::Dropped(self.str()?),
                1 => Verdict::Ignored,
                2 => Verdict::Taken,
                v => return Err(format!("input_log: verdict {v}")),
            };
            let (area, value, mode, slot, zone) = (self.i32()?, self.i32()?, self.i32()?, self.i32()?, self.i32()?);
            let key_match = self.b()?;
            let handler = self.i32()?;
            let touches = |r: &mut Self| -> DResult<Vec<Touch>> {
                let m = r.count(17)?;
                let mut v = Vec::with_capacity(m);
                for _ in 0..m {
                    let k = r.u8()?;
                    let kind = TouchKind::of_u8(k).ok_or_else(|| format!("input_log: touch kind {k}"))?;
                    v.push(Touch { kind, a: r.i32()?, b: r.i32()?, old: r.i32()?, now: r.i32()? });
                }
                Ok(v)
            };
            let writes = touches(self)?;
            let reads = touches(self)?;
            let no = self.count(4 * 3 + 8 + 1)?;
            let mut ops = Vec::with_capacity(no);
            for _ in 0..no {
                ops.push(LogOp { slot: self.i32()?, number: self.i32()?, where_: self.i32()?, code: self.i64()?, opcode: self.u8()? });
            }
            let ops_dropped = self.u32()?;
            let touches_dropped = self.u32()?;
            records.push(InputRecord {
                frame,
                input,
                stat,
                verdict,
                area,
                value,
                mode,
                slot,
                zone,
                key_match,
                handler,
                writes,
                reads,
                ops,
                ops_dropped,
                touches_dropped,
            });
        }
        let dropped = self.u32()?;
        let frame = self.u64()?;
        Ok(InputLog { records, dropped, frame })
    }

    /// patch 0007: fmi-core `Digest` (frame, digest, bytes).
    pub fn digest(&mut self) -> DResult<Digest> {
        Ok(Digest { frame: self.u64()?, digest: self.u64()?, bytes: self.u32()? })
    }

    pub fn runs_reply(&mut self) -> DResult<RunsReply> {
        let n = self.count(73)?;
        let mut runs = Vec::with_capacity(n);
        for _ in 0..n {
            runs.push(Run {
                frame: self.u64()?,
                after: self.b()?,
                seq: self.u64()?,
                depth: self.i32()?,
                slot: self.i32()?,
                where_: self.i32()?,
                number: self.i32()?,
                room: self.i32()?,
                code0: self.i64()?,
                code1: self.i64()?,
                status: self.i32()?,
                frozen: self.i32()?,
                delay: self.i32()?,
                ops: self.u64()?,
            });
        }
        Ok(RunsReply {
            runs,
            dropped: self.u64()?,
            violations: self.u64()?,
            frame: self.u64()?,
        })
    }

    pub fn explain_reply(&mut self) -> DResult<ExplainReply> {
        let n = self.count(17)?;
        let mut entries = Vec::with_capacity(n);
        for _ in 0..n {
            entries.push(match self.u8()? {
                0 => Explain::Begin {
                    seq: self.u64()?,
                    frame: self.u64()?,
                    after: self.b()?,
                    slot: self.i32()?,
                    where_: self.i32()?,
                    number: self.i32()?,
                    room: self.i32()?,
                    code0: self.i64()?,
                },
                1 => Explain::Op {
                    seq: self.u64()?,
                    code: self.i64()?,
                    opcode: self.u8()?,
                },
                2 => Explain::Read {
                    seq: self.u64()?,
                    var: self.u32()?,
                    value: self.i32()?,
                },
                3 => Explain::Write {
                    seq: self.u64()?,
                    var: self.u32()?,
                    value: self.i32()?,
                },
                4 => Explain::End {
                    seq: self.u64()?,
                    code1: self.i64()?,
                    status: self.i32()?,
                    frozen: self.i32()?,
                    delay: self.i32()?,
                },
                v => return Err(format!("explain kind {v}")),
            });
        }
        Ok(ExplainReply {
            entries,
            dropped: self.u64()?,
            violations: self.u64()?,
        })
    }

    fn exact_val(&mut self, depth: u32) -> DResult<ExactVal> {
        if depth > 16 {
            return Err("exact value nested too deep".into());
        }
        Ok(match self.u8()? {
            0 => ExactVal::Int(self.i64()?),
            1 => ExactVal::Bool(self.b()?),
            2 => ExactVal::Text(self.str()?),
            3 => {
                let n = self.count(1)?;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    v.push(self.exact_val(depth + 1)?);
                }
                ExactVal::List(v)
            }
            4 => ExactVal::Map(self.exact_map(depth + 1)?),
            5 => ExactVal::Null,
            v => return Err(format!("exact value kind {v}")),
        })
    }

    fn exact_map(&mut self, depth: u32) -> DResult<Vec<(String, ExactVal)>> {
        let n = self.count(5)?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            let k = self.str()?;
            v.push((k, self.exact_val(depth)?));
        }
        Ok(v)
    }

    pub fn obs(&mut self) -> DResult<Obs> {
        let tick = self.u64()?;
        let frame = self.u64()?;
        let room = self.i32()?;
        let ego = self.opt_i32()?;
        let ego_pos = if self.tag()? {
            Some((self.i32()?, self.i32()?))
        } else {
            None
        };
        let seed = if self.tag()? { Some(self.i64()?) } else { None };
        let boot_param = self.i32()?;
        let vars = self.i32s()?;
        let bits_set = self.u32s()?;
        let inventory = if self.tag()? { Some(self.i32s()?) } else { None };
        let pairs = |r: &mut R| -> DResult<Vec<(u32, i32)>> {
            let n = r.count(8)?;
            let raw = r.take(8 * n)?;
            Ok(raw
                .chunks_exact(8)
                .map(|c| {
                    (
                        u32::from_le_bytes(c[..4].try_into().unwrap()),
                        i32::from_le_bytes(c[4..].try_into().unwrap()),
                    )
                })
                .collect())
        };
        let owners = pairs(self)?;
        let states = pairs(self)?;
        let n = self.count(8)?;
        let mut classes = Vec::with_capacity(n);
        for _ in 0..n {
            let o = self.u32()?;
            classes.push((o, self.u32s()?));
        }
        let room_objects = if self.tag()? {
            let n = self.count(37)?;
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(RoomObject {
                    id: self.i32()?,
                    state: self.i32()?,
                    parent: self.i32()?,
                    x: self.i32()?,
                    y: self.i32()?,
                    w: self.i32()?,
                    h: self.i32()?,
                    walk: (self.i32()?, self.i32()?),
                    name: self.opt_str()?,
                });
            }
            Some(v)
        } else {
            None
        };
        let n = self.count(33)?;
        let mut verbs = Vec::with_capacity(n);
        for _ in 0..n {
            verbs.push(Verb {
                slot: self.i32()?,
                id: self.i32()?,
                mode: self.i32()?,
                saveid: self.i32()?,
                kind: self.i32()?,
                key: self.i32()?,
                x: self.i32()?,
                y: self.i32()?,
                name: self.opt_str()?,
            });
        }
        let n = self.count(47)?;
        let mut slots = Vec::with_capacity(n);
        for _ in 0..n {
            slots.push(Slot {
                slot: self.i32()?,
                number: self.i32()?,
                where_: self.i32()?,
                status: self.i32()?,
                offs: self.u64()?,
                code: self.i64()?,
                frozen: self.i32()?,
                delay: self.i32()?,
                cutscene_override: self.i32()?,
                locals: if self.tag()? { Some(self.i32s()?) } else { None },
                resistant: self.opt_bool()?,
                recursive: self.opt_bool()?,
            });
        }
        let depth = self.i32()?;
        let n = self.count(6)?;
        let mut levels = Vec::with_capacity(n);
        for _ in 0..n {
            let level = self.i32()?;
            let data = self.opt_i32()?;
            let override_ = if self.tag()? {
                Some(Override {
                    slot: self.i32()?,
                    number: self.i32()?,
                    where_: self.i32()?,
                    status: self.i32()?,
                    ptr: self.u64()?,
                    code: self.i64()?,
                    stale: self.b()?,
                })
            } else {
                None
            };
            levels.push(CutLevel { level, data, override_ });
        }
        let n = self.count(20)?;
        let mut actors = Vec::with_capacity(n);
        for _ in 0..n {
            actors.push(ActorPos {
                id: self.i32()?,
                x: self.i32()?,
                y: self.i32()?,
                elevation: self.i32()?,
                moving: self.i32()?,
            });
        }
        let exact = if self.tag()? {
            Some(Exact {
                fields: self.exact_map(0)?,
            })
        } else {
            None
        };
        Ok(Obs {
            tick,
            frame,
            room,
            ego,
            ego_pos,
            seed,
            boot_param,
            vars,
            bits_set,
            inventory,
            owners,
            states,
            classes,
            room_objects,
            verbs,
            slots,
            cutscene: Cutscene { depth, levels },
            actors,
            exact,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_spec_bytes() {
        let mut w = W::msg(C_RUN);
        w.run_spec(&RunSpec {
            frame: Some(7),
            idle: true,
            ..Default::default()
        });
        let v = w.finish();
        assert_eq!(&v[..5], &[21, 0, 0, 0, C_RUN]);
        assert_eq!(&v[5..14], &[1, 7, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&v[14..], &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0]);
    }

    #[test]
    fn reader_refuses_short_and_bad_counts() {
        let b = [5u8, 0, 0, 0, 1];
        let mut r = R::new(&b);
        assert!(r.i32s().is_err());
        let mut r = R::new(&[2u8]);
        assert!(r.b().is_err());
    }
}
