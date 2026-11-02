# fmi-reading (E011, component readio)

The reading as files the Rust side opens directly, the exact reading cache, and the reuse rules
(experiments/E011-speed/plan.md, "readio").

## What it is

- **reader-scope `--out-dir DIR`** (scumm/reader/src/bin/reader_scope.rs, `reader_scope/files.rs`) writes the
  reading's files while it reads: `reading.tc`, `reading.reached`, `reading.actions.jsonl`, `reading.lists.jsonl`,
  `reading.json` and `reading.idx` (plan.md, F3). There is no golden stream and no Python. The line files are what
  replay_part1.py `read_rust` (with `stream`) makes with packed_reading.py `reading_from_golden`, byte for byte.
  `reading.json` is too, except for two kinds of field. The measured ones (`read_s` and the reader's wall, CPU and
  peak memory) differ from run to run in the reference as well. `from_golden` names the golden file when `--golden`
  is given (null otherwise) and the packed file `reading.tc`. The switch is off by default: without it the reader
  writes what it wrote before.
- **`Reading`** (src/lib.rs) implements `fmi_core::ReadingSource` over the mapped files and the index.
  - Opening reads the packed file's tables (strings, facts, goal), the head and the index.
  - `action(i)` and `record(i)` are `PackedTask._action(i)`, decoded on demand.
  - `by_name` finds the last action of a name through the name hash.
  - A reading made before reading.idx gets one at its first open, and it holds the same bytes reader-scope writes.
- **The cache** (src/cache.rs): `ReadSpec` holds a reading's inputs, `ReadSpec::args` is reader-scope's command
  line, and `ReadSpec::key` is the sha256 key. `ReadingCache` keeps the store `~/.cache/fast-mi-e011/readings/<key>/`
  (`get`, `put`, `read`) and evicts by E009's level.
- **The reuse rules** (src/reuse.rs): step.py's `reusable` and `reusable_detail` (E008), and `reuse_keys_of` and
  `reuse_keys` (E009's K, with the same reuse-keys.json file). They run on a game loaded once (`ReuseGame`).

## Checks

The Python reference is E009's binary (2a3cf8a1) through `read_rust`, plus `PackedTask`, `strict.Reading`'s `by_name`
and step.py.

- `ref/ref_read.py` makes the reference reading.
- `ref/new_read.sh` makes the same reading with `--out-dir`.
- `ref/cmp_reading.py` is G6's file part.
- `ref/g8.sh` (with `ref/dump_reading.py`) is G8: `action` and `record`, `by_name`, K, the index, and the measures.
- `fmi-reading reusable` and `dump_reading.py reusable` compare the reuse rules.
- `cargo test` covers the line reader, the index, the JSON writer and the cache's store.
