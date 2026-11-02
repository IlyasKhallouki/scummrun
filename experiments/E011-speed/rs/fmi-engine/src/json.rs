//! A small JSON reader for the differential tests: the C7 dump's text (the engine's StateJson, svm-w3b's dumps) as
//! fmi-core's `Obs`, and the reference's JSON files. Objects keep their key order. Numbers are integers (the dump
//! holds no other kind); anything else is an error.

use fmi_core::*;

#[derive(Clone, PartialEq, Debug)]
pub enum J {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

pub type JResult<T> = Result<T, String>;

pub fn parse(text: &str) -> JResult<J> {
    let mut p = P {
        s: text.as_bytes(),
        i: 0,
    };
    let v = p.value(0)?;
    p.ws();
    if p.i != p.s.len() {
        return Err(format!("JSON: text after the value at {}", p.i));
    }
    Ok(v)
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }
    fn err<T>(&self, what: &str) -> JResult<T> {
        Err(format!("JSON: {what} at {}", self.i))
    }
    fn lit(&mut self, w: &str, v: J) -> JResult<J> {
        if self.s[self.i..].starts_with(w.as_bytes()) {
            self.i += w.len();
            Ok(v)
        } else {
            self.err("bad literal")
        }
    }
    fn value(&mut self, depth: u32) -> JResult<J> {
        if depth > 64 {
            return self.err("nested too deep");
        }
        self.ws();
        match self.s.get(self.i) {
            None => self.err("end of text"),
            Some(b'n') => self.lit("null", J::Null),
            Some(b't') => self.lit("true", J::Bool(true)),
            Some(b'f') => self.lit("false", J::Bool(false)),
            Some(b'"') => Ok(J::Str(self.string()?)),
            Some(b'[') => {
                self.i += 1;
                let mut v = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(J::Arr(v));
                }
                loop {
                    v.push(self.value(depth + 1)?);
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(J::Arr(v));
                        }
                        _ => return self.err("expected , or ]"),
                    }
                }
            }
            Some(b'{') => {
                self.i += 1;
                let mut v = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(J::Obj(v));
                }
                loop {
                    self.ws();
                    if self.s.get(self.i) != Some(&b'"') {
                        return self.err("expected a key");
                    }
                    let k = self.string()?;
                    self.ws();
                    if self.s.get(self.i) != Some(&b':') {
                        return self.err("expected :");
                    }
                    self.i += 1;
                    v.push((k, self.value(depth + 1)?));
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(J::Obj(v));
                        }
                        _ => return self.err("expected , or }"),
                    }
                }
            }
            Some(c) if *c == b'-' || c.is_ascii_digit() => {
                let st = self.i;
                self.i += 1;
                while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                    self.i += 1;
                }
                if matches!(self.s.get(self.i), Some(b'.' | b'e' | b'E')) {
                    return self.err("not an integer");
                }
                let t = std::str::from_utf8(&self.s[st..self.i]).unwrap();
                t.parse::<i64>()
                    .map(J::Int)
                    .or_else(|_| self.err("integer out of range"))
            }
            Some(_) => self.err("unexpected character"),
        }
    }
    fn hex4(&mut self) -> JResult<u32> {
        let h = self.s.get(self.i..self.i + 4).ok_or("JSON: short \\u escape")?;
        let t = std::str::from_utf8(h).map_err(|e| e.to_string())?;
        self.i += 4;
        u32::from_str_radix(t, 16).map_err(|e| e.to_string())
    }
    fn string(&mut self) -> JResult<String> {
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = *self.s.get(self.i).ok_or("JSON: unterminated string")?;
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).map_err(|e| e.to_string()),
                b'\\' => {
                    let e = *self.s.get(self.i).ok_or("JSON: bad escape")?;
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let mut u = self.hex4()?;
                            if (0xD800..0xDC00).contains(&u) && self.s[self.i..].starts_with(b"\\u") {
                                self.i += 2;
                                let lo = self.hex4()?;
                                u = 0x10000 + ((u - 0xD800) << 10) + (lo - 0xDC00);
                            }
                            char::from_u32(u).ok_or("JSON: bad \\u code point")?
                        }
                        _ => return self.err("bad escape"),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                _ => out.push(c),
            }
        }
    }
}

impl J {
    pub fn get(&self, k: &str) -> Option<&J> {
        match self {
            J::Obj(v) => v.iter().find(|(a, _)| a == k).map(|(_, b)| b),
            _ => None,
        }
    }
    pub fn at(&self, k: &str) -> JResult<&J> {
        self.get(k).ok_or_else(|| format!("JSON: no key {k:?}"))
    }
    pub fn int(&self) -> JResult<i64> {
        match self {
            J::Int(v) => Ok(*v),
            _ => Err(format!("JSON: not an integer: {self:?}")),
        }
    }
    pub fn i32(&self) -> JResult<i32> {
        i32::try_from(self.int()?).map_err(|e| e.to_string())
    }
    pub fn boolean(&self) -> JResult<bool> {
        match self {
            J::Bool(v) => Ok(*v),
            _ => Err(format!("JSON: not a bool: {self:?}")),
        }
    }
    pub fn arr(&self) -> JResult<&[J]> {
        match self {
            J::Arr(v) => Ok(v),
            _ => Err(format!("JSON: not an array: {self:?}")),
        }
    }
    pub fn obj(&self) -> JResult<&[(String, J)]> {
        match self {
            J::Obj(v) => Ok(v),
            _ => Err("JSON: not an object".into()),
        }
    }
    pub fn text(&self) -> JResult<&str> {
        match self {
            J::Str(s) => Ok(s),
            _ => Err(format!("JSON: not a string: {self:?}")),
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, J::Null)
    }
}

fn opt<'a>(j: &'a J, k: &str) -> Option<&'a J> {
    j.get(k).filter(|v| !v.is_null())
}

fn pair(j: &J) -> JResult<(i32, i32)> {
    let a = j.arr()?;
    if a.len() != 2 {
        return Err("JSON: not a pair".into());
    }
    Ok((a[0].i32()?, a[1].i32()?))
}

fn exact_val(j: &J) -> JResult<ExactVal> {
    Ok(match j {
        J::Null => ExactVal::Null,
        J::Bool(b) => ExactVal::Bool(*b),
        J::Int(v) => ExactVal::Int(*v),
        J::Str(s) => ExactVal::Text(s.clone()),
        J::Arr(v) => ExactVal::List(v.iter().map(exact_val).collect::<JResult<_>>()?),
        J::Obj(v) => ExactVal::Map(
            v.iter()
                .map(|(k, x)| Ok((k.clone(), exact_val(x)?)))
                .collect::<JResult<_>>()?,
        ),
    })
}

fn obj_pairs(j: &J) -> JResult<Vec<(u32, i32)>> {
    j.obj()?
        .iter()
        .map(|(k, v)| Ok((k.parse::<u32>().map_err(|e| e.to_string())?, v.i32()?)))
        .collect()
}

/// The C7 dump's JSON (`{"tick": ..., ...}`, the "state" reply's value) as `Obs`; names kept only with `names`.
pub fn obs_of(j: &J, names: bool) -> JResult<Obs> {
    let name = |o: &J| -> JResult<Option<String>> {
        if names {
            Ok(Some(o.at("name")?.text()?.to_string()))
        } else {
            Ok(None)
        }
    };
    let classes = j
        .at("classes")?
        .obj()?
        .iter()
        .map(|(k, v)| {
            Ok((
                k.parse::<u32>().map_err(|e| e.to_string())?,
                v.arr()?
                    .iter()
                    .map(|c| Ok(c.int()? as u32))
                    .collect::<JResult<Vec<u32>>>()?,
            ))
        })
        .collect::<JResult<_>>()?;
    let room_objects = match j.get("room_objects") {
        None => None,
        Some(v) => Some(
            v.arr()?
                .iter()
                .map(|o| {
                    Ok(RoomObject {
                        id: o.at("id")?.i32()?,
                        state: o.at("state")?.i32()?,
                        parent: o.at("parent")?.i32()?,
                        x: o.at("x")?.i32()?,
                        y: o.at("y")?.i32()?,
                        w: o.at("w")?.i32()?,
                        h: o.at("h")?.i32()?,
                        walk: pair(o.at("walk")?)?,
                        name: name(o)?,
                    })
                })
                .collect::<JResult<_>>()?,
        ),
    };
    let verbs = j
        .at("verbs")?
        .arr()?
        .iter()
        .map(|v| {
            Ok(Verb {
                slot: v.at("slot")?.i32()?,
                id: v.at("id")?.i32()?,
                mode: v.at("mode")?.i32()?,
                saveid: v.at("saveid")?.i32()?,
                kind: v.at("type")?.i32()?,
                key: v.at("key")?.i32()?,
                x: v.at("x")?.i32()?,
                y: v.at("y")?.i32()?,
                name: name(v)?,
            })
        })
        .collect::<JResult<_>>()?;
    let slots = j
        .at("slots")?
        .arr()?
        .iter()
        .map(|s| {
            Ok(Slot {
                slot: s.at("slot")?.i32()?,
                number: s.at("number")?.i32()?,
                where_: s.at("where")?.i32()?,
                status: s.at("status")?.i32()?,
                offs: s.at("offs")?.int()? as u64,
                code: s.at("code")?.int()?,
                frozen: s.at("frozen")?.i32()?,
                delay: s.at("delay")?.i32()?,
                cutscene_override: s.at("cutscene_override")?.i32()?,
                locals: match s.get("locals") {
                    Some(l) => Some(l.arr()?.iter().map(J::i32).collect::<JResult<_>>()?),
                    None => None,
                },
                resistant: s.get("resistant").map(J::boolean).transpose()?,
                recursive: s.get("recursive").map(J::boolean).transpose()?,
            })
        })
        .collect::<JResult<_>>()?;
    let cs = j.at("cutscene")?;
    let levels = cs
        .at("levels")?
        .arr()?
        .iter()
        .map(|l| {
            Ok(CutLevel {
                level: l.at("level")?.i32()?,
                data: l.get("data").map(J::i32).transpose()?,
                override_: match opt(l, "override") {
                    None => None,
                    Some(o) => Some(Override {
                        slot: o.at("slot")?.i32()?,
                        number: o.at("number")?.i32()?,
                        where_: o.at("where")?.i32()?,
                        status: o.at("status")?.i32()?,
                        ptr: o.at("ptr")?.int()? as u64,
                        code: o.at("code")?.int()?,
                        stale: o.at("stale")?.boolean()?,
                    }),
                },
            })
        })
        .collect::<JResult<_>>()?;
    let actors = j
        .at("actors")?
        .arr()?
        .iter()
        .map(|a| {
            Ok(ActorPos {
                id: a.at("id")?.i32()?,
                x: a.at("x")?.i32()?,
                y: a.at("y")?.i32()?,
                elevation: a.at("elevation")?.i32()?,
                moving: a.at("moving")?.i32()?,
            })
        })
        .collect::<JResult<_>>()?;
    let exact = match j.get("exact") {
        None => None,
        Some(x) => Some(Exact {
            fields: x
                .obj()?
                .iter()
                .map(|(k, v)| Ok((k.clone(), exact_val(v)?)))
                .collect::<JResult<_>>()?,
        }),
    };
    Ok(Obs {
        tick: j.at("tick")?.int()? as u64,
        frame: j.at("frame")?.int()? as u64,
        room: j.at("room")?.i32()?,
        ego: opt(j, "ego").map(J::i32).transpose()?,
        ego_pos: opt(j, "ego_pos").map(pair).transpose()?,
        seed: opt(j, "seed").map(J::int).transpose()?,
        boot_param: j.at("boot_param")?.i32()?,
        vars: j.at("vars")?.arr()?.iter().map(J::i32).collect::<JResult<_>>()?,
        bits_set: j
            .at("bits_set")?
            .arr()?
            .iter()
            .map(|v| Ok(v.int()? as u32))
            .collect::<JResult<_>>()?,
        inventory: match j.get("inventory") {
            Some(v) => Some(v.arr()?.iter().map(J::i32).collect::<JResult<_>>()?),
            None => None,
        },
        owners: obj_pairs(j.at("owners")?)?,
        states: obj_pairs(j.at("states")?)?,
        classes,
        room_objects,
        verbs,
        slots,
        cutscene: Cutscene {
            depth: cs.at("depth")?.i32()?,
            levels,
        },
        actors,
        exact,
    })
}

/// A string as JSON text (for the facts files the tests compare with the reference's).
pub fn quote(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Facts as a JSON object, keys in Python's string order (`{"bit:4": 1, ...}`, as json.dumps(sort_keys=True)
/// writes them, with ", " and ": ").
pub fn facts_text(f: &Facts) -> String {
    let mut o = String::from("{");
    for (i, (k, v)) in f.text_sorted().iter().enumerate() {
        if i > 0 {
            o.push_str(", ");
        }
        o.push_str(&quote(k));
        o.push_str(": ");
        o.push_str(&val_text(*v));
    }
    o.push('}');
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_dump_forms() {
        let j = parse(r#"{"a": [1, -2, {"b": null}], "c": "xé\"y", "d": true}"#).unwrap();
        assert_eq!(j.at("a").unwrap().arr().unwrap()[1], J::Int(-2));
        assert_eq!(j.at("c").unwrap().text().unwrap(), "x\u{e9}\"y");
        assert!(j.at("d").unwrap().boolean().unwrap());
        assert!(parse("1.5").is_err());
        assert!(parse("[1,]").is_err());
    }
}
