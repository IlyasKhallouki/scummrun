//! The reading cache's store: put, the largest reading, and eviction by E009's level, least recently used first.

use fmi_reading::cache::{disk_free, ReadSpec, ReadingCache};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

fn store(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A made reading of `bytes` bytes, put into the store as `key`, last used `age` seconds ago.
fn put(c: &ReadingCache, key: &str, bytes: usize, age: u64) -> PathBuf {
    let m = c.scratch().unwrap();
    std::fs::write(m.join("reading.json"), "{}").unwrap();
    std::fs::write(m.join("reading.tc"), vec![0u8; bytes]).unwrap();
    let d = c.put(key, &m).unwrap();
    let f = std::fs::File::options().write(true).open(d.join("used")).unwrap();
    f.set_modified(SystemTime::now() - Duration::from_secs(age)).unwrap();
    d
}

#[test]
fn put_largest_and_lru_order() {
    let root = store("cache-order");
    let c = ReadingCache { root: root.clone(), disk: root.clone(), floor_bytes: 0 };
    let a = put(&c, "a", 1000, 30);
    let b = put(&c, "b", 5000, 10);
    let _ = put(&c, "c", 3000, 20);
    assert!(c.largest() >= 5000 && c.largest() < 5100, "the largest reading: {}", c.largest());
    let keys: Vec<String> = c.entries().into_iter().map(|e| e.key).collect();
    assert_eq!(keys, ["a", "c", "b"], "least recently used first");
    // a second put of a key that is there: the made folder goes, the stored one stays
    let m = c.scratch().unwrap();
    std::fs::write(m.join("reading.json"), "{\"other\": 1}").unwrap();
    assert_eq!(c.put("a", &m).unwrap(), a);
    assert!(!m.exists());
    assert_eq!(std::fs::read_to_string(a.join("reading.json")).unwrap(), "{}");
    assert!(b.exists());
}

#[test]
fn evicts_down_to_the_level_never_in_use() {
    let root = store("cache-evict");
    // a floor no disk has free: every reading not in use goes, least recently used first
    let mut c = ReadingCache { root: root.clone(), disk: root.clone(), floor_bytes: u64::MAX / 4 };
    let a = put(&c, "a", 100, 30);
    let b = put(&c, "b", 100, 20);
    let d = put(&c, "d", 100, 10);
    let gone = c.evict(std::slice::from_ref(&b)).unwrap();
    assert_eq!(gone, vec![a.clone(), d.clone()]);
    assert!(b.exists() && !a.exists() && !d.exists());
    // a floor under the free space: nothing goes
    c.floor_bytes = 0;
    let e = put(&c, "e", 100, 5);
    assert!(disk_free(&root).unwrap() >= c.level());
    assert!(c.evict(&[]).unwrap().is_empty());
    assert!(e.exists() && b.exists());
}

#[test]
fn key_follows_every_input() {
    let root = store("cache-key");
    let rows = root.join("start.jsonl");
    std::fs::write(&rows, "{\"seed\": 2, \"facts\": {\"var:1\": 5}}\n{\"seed\": 1, \"facts\": {\"var:2\": 3, \"room\": 33}}\n")
        .unwrap();
    let also = root.join("also.txt");
    std::fs::write(&also, "var:2\t4\n").unwrap();
    let s = ReadSpec::e009(&[38, 33], "owner:566=1", &rows);
    assert_eq!(s.rooms, vec![33, 38]);
    let k = |s: &ReadSpec| s.key("r", "g", &root).unwrap();
    let k0 = k(&s);
    assert_eq!(k0.len(), 64);
    assert_eq!(k0, k(&s.clone()), "the same inputs, the same key");
    assert!(s.key_text("r", "g", &root).unwrap().contains("start 16\nroom\t33\nvar:2\t3\n"), "the start sorted");
    let mut t = s.clone();
    t.fact_vars = vec![103];
    assert_ne!(k(&t), k0);
    let mut t = s.clone();
    t.reach_also = vec![also.clone()];
    let ka = k(&t);
    assert_ne!(ka, k0);
    std::fs::write(&also, "var:2\t5\n").unwrap();
    assert_ne!(k(&t), ka, "the reach-also file's contents are in the key");
    let mut t = s.clone();
    t.goal = "bit:108=1".into();
    assert_ne!(k(&t), k0);
    let mut t = s.clone();
    t.switches = String::new();
    assert_ne!(k(&t), k0);
    assert_ne!(s.key("r2", "g", &root).unwrap(), k0, "the reader binary is in the key");
    assert_ne!(s.key("r", "g2", &root).unwrap(), k0, "the game data is in the key");
    let mut t = s.clone();
    t.seed = 2;
    assert_ne!(k(&t), k0, "another start row");
    let args = s.args(Path::new("/x"), "n", 3);
    assert_eq!(args[..14].join(" "), "--rooms 33,38 --start-seed 1 --domain-goals none --goal owner:566=1 --name n --threads 3 --out-dir /x");
}
