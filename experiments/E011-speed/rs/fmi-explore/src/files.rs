//! explore.py's file helpers, byte for byte: `write_json` (atomic, `json.dumps(obj, indent=...) + "\n"`), the gzip
//! entry files, `read_jsonl` (a last line cut short is left out), `append_jsonl` (compact, fsync), `rewrite_jsonl`,
//! `dir_bytes`; and the small Python value rules the reference leans on (truthiness, `str()` of a value).

use crate::pyjson::{self, Opts};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::Value;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub type Res<T> = Result<T, String>;

pub fn io_err(p: &Path, e: io::Error) -> String {
    format!("{}: {e}", p.display())
}

pub fn read_text(p: &Path) -> Res<String> {
    fs::read_to_string(p).map_err(|e| io_err(p, e))
}

pub fn read_json(p: &Path) -> Res<Value> {
    let t = read_text(p)?;
    pyjson::loads(&t).map_err(|e| format!("{}: {e}", p.display()))
}

/// explore.py `write_json`: a temporary file renamed over `path`.
pub fn write_json(path: &Path, v: &Value, indent: Option<usize>) -> Res<()> {
    let o = Opts { indent, compact: false, sort_keys: false };
    write_text_atomic(path, &(pyjson::dumps(v, o) + "\n"))
}

pub fn write_text_atomic(path: &Path, text: &str) -> Res<()> {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| io_err(d, e))?;
    }
    let tmp = tmp_of(path, ".tmp");
    fs::write(&tmp, text).map_err(|e| io_err(&tmp, e))?;
    fs::rename(&tmp, path).map_err(|e| io_err(path, e))
}

pub fn tmp_of(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().map(|s| s.to_os_string()).unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

/// explore.py `read_gz`.
pub fn read_gz(p: &Path) -> Res<Value> {
    let f = fs::File::open(p).map_err(|e| python_os_error(p, e))?;
    let mut s = String::new();
    GzDecoder::new(f).read_to_string(&mut s).map_err(|e| io_err(p, e))?;
    pyjson::loads(&s).map_err(|e| format!("{}: {e}", p.display()))
}

/// explore.py `write_gz`: `json.dump(obj, f, separators=(",", ":"))` into gzip, atomically.
pub fn write_gz(p: &Path, v: &Value) -> Res<()> {
    write_gz_text(p, &pyjson::compact(v))
}

/// step.py `write_entry` / fake_step.py `write_gz`: `json.dump(entry, f)` (default separators), not atomic.
pub fn write_gz_default(p: &Path, v: &Value) -> Res<()> {
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| io_err(d, e))?;
    }
    let f = fs::File::create(p).map_err(|e| io_err(p, e))?;
    let mut g = GzEncoder::new(f, Compression::new(9));
    g.write_all(pyjson::dumps_default(v).as_bytes()).map_err(|e| io_err(p, e))?;
    g.finish().map_err(|e| io_err(p, e))?;
    Ok(())
}

pub fn write_gz_text(p: &Path, text: &str) -> Res<()> {
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| io_err(d, e))?;
    }
    let tmp = tmp_of(p, ".tmp");
    {
        let f = fs::File::create(&tmp).map_err(|e| io_err(&tmp, e))?;
        // Python's gzip default level is 9
        let mut g = GzEncoder::new(f, Compression::new(9));
        g.write_all(text.as_bytes()).map_err(|e| io_err(&tmp, e))?;
        g.finish().map_err(|e| io_err(&tmp, e))?;
    }
    fs::rename(&tmp, p).map_err(|e| io_err(p, e))
}

/// The text Python gives an OSError of `open(path)` (`[Errno 2] No such file or directory: '/x'`).
pub fn python_os_error(p: &Path, e: io::Error) -> String {
    let errno = e.raw_os_error().unwrap_or(0);
    let what = match errno {
        2 => "No such file or directory".to_string(),
        13 => "Permission denied".to_string(),
        21 => "Is a directory".to_string(),
        20 => "Not a directory".to_string(),
        _ => e.to_string(),
    };
    format!("[Errno {errno}] {what}: {}", py_repr_str(&p.to_string_lossy()))
}

/// The exception class Python raises for an `open` error.
pub fn python_os_error_class(e: &io::Error) -> &'static str {
    match e.raw_os_error() {
        Some(2) => "FileNotFoundError",
        Some(13) => "PermissionError",
        Some(21) => "IsADirectoryError",
        Some(20) => "NotADirectoryError",
        _ => "OSError",
    }
}

/// Python's `repr()` of a str (single quotes unless the text holds one and no double quote).
pub fn py_repr_str(s: &str) -> String {
    let q = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::new();
    out.push(q);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(q);
    out
}

/// explore.py `read_jsonl`: (rows, clean); a last line that does not parse is a write cut short, left out.
pub fn read_jsonl(p: &Path) -> Res<(Vec<Value>, bool)> {
    if !p.exists() {
        return Ok((Vec::new(), true));
    }
    let text = read_text(p)?;
    let lines: Vec<&str> = py_splitlines(&text);
    let mut rows = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match pyjson::loads(line) {
            Ok(v) => rows.push(v),
            Err(e) => {
                if i == lines.len() - 1 {
                    return Ok((rows, false));
                }
                return Err(format!("{}: line {}: {e}", p.display(), i + 1));
            }
        }
    }
    Ok((rows, true))
}

/// `str.splitlines()` for the line ends the files hold (\n, \r\n, \r).
pub fn py_splitlines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let (mut start, mut i) = (0, 0);
    while i < b.len() {
        match b[i] {
            b'\n' => {
                out.push(&text[start..i]);
                i += 1;
                start = i;
            }
            b'\r' => {
                out.push(&text[start..i]);
                i += if i + 1 < b.len() && b[i + 1] == b'\n' { 2 } else { 1 };
                start = i;
            }
            _ => i += 1,
        }
    }
    if start < b.len() {
        out.push(&text[start..]);
    }
    out
}

fn jsonl_text(rows: &[Value]) -> String {
    let mut s = String::new();
    for r in rows {
        s.push_str(&pyjson::compact(r));
        s.push('\n');
    }
    s
}

/// explore.py `append_jsonl`: the rows appended, flushed and synced.
pub fn append_jsonl(p: &Path, rows: &[Value]) -> Res<()> {
    let mut f = fs::OpenOptions::new().create(true).append(true).open(p).map_err(|e| io_err(p, e))?;
    f.write_all(jsonl_text(rows).as_bytes()).map_err(|e| io_err(p, e))?;
    f.flush().map_err(|e| io_err(p, e))?;
    f.sync_all().map_err(|e| io_err(p, e))
}

/// explore.py `rewrite_jsonl`.
pub fn rewrite_jsonl(p: &Path, rows: &[Value]) -> Res<()> {
    let tmp = tmp_of(p, ".tmp");
    fs::write(&tmp, jsonl_text(rows)).map_err(|e| io_err(&tmp, e))?;
    fs::rename(&tmp, p).map_err(|e| io_err(p, e))
}

/// explore.py `dir_bytes`: the sizes (lstat) of every file under `d`.
pub fn dir_bytes(d: &Path) -> u64 {
    let mut total = 0;
    let mut todo = vec![d.to_path_buf()];
    while let Some(p) = todo.pop() {
        let Ok(rd) = fs::read_dir(&p) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                todo.push(e.path());
            } else if let Ok(m) = fs::symlink_metadata(e.path()) {
                total += m.len();
            }
        }
    }
    total
}

// --- Python's value rules ------------------------------------------------------------------------------------------

/// Python's truth value of a JSON value.
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(m)) => !m.is_empty(),
    }
}

/// `x or default`: the value when truthy, else the default.
pub fn or<'a>(v: Option<&'a Value>, d: &'a Value) -> &'a Value {
    if truthy(v) {
        v.unwrap()
    } else {
        d
    }
}

/// Python's `str()` of a JSON value as an f-string prints it (None, True, 'x' inside containers, ...).
pub fn py_str(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "None".into(),
        Some(Value::String(s)) => s.clone(),
        Some(x) => py_repr(x),
    }
}

/// Python's `repr()` of a JSON value read by json.loads (dicts, lists, str, int, float, bool, None).
pub fn py_repr(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                let f = n.as_f64().unwrap();
                if f.is_infinite() {
                    if f > 0.0 { "inf".into() } else { "-inf".into() }
                } else if f.is_nan() {
                    "nan".into()
                } else {
                    pyjson::float_repr(f)
                }
            }
        }
        Value::String(s) => py_repr_str(s),
        Value::Array(a) => format!("[{}]", a.iter().map(py_repr).collect::<Vec<_>>().join(", ")),
        Value::Object(m) => format!(
            "{{{}}}",
            m.iter().map(|(k, x)| format!("{}: {}", py_repr_str(k), py_repr(x))).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// A JSON int.
pub fn as_i64(v: Option<&Value>) -> Option<i64> {
    v.and_then(|x| x.as_i64().or_else(|| x.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64)))
}

pub fn now_s() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// `time.strftime(fmt)` in local time.
pub fn strftime(fmt: &str) -> String {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        let mut buf = [0u8; 128];
        let cfmt = std::ffi::CString::new(fmt).unwrap();
        let n = libc::strftime(buf.as_mut_ptr() as *mut libc::c_char, buf.len(), cfmt.as_ptr(), &tm);
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }
}

/// explore.py `say`: a timestamped line on stderr.
pub fn say(msg: &str) {
    eprintln!("[{}] {msg}", strftime("%H:%M:%S"));
}
