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

## Results (2026-11-02, laptop, load average 10-28)

**Inputs.** The three readings run with E009's switches and the kitchen goal owner:566=1:

- ring(38) [33,38,83,85,96] from j00091's start row;
- ring(33) [28,33,35,38] from j00089's start row;
- the refinement j00089 [103, 194]: ring(33) from j00089's row, `--fact-vars 103,194`, and j00089's two
  `refine-0N/start.txt` as `--reach-also`.

**Binaries.** The reference is E009's binary (2a3cf8a1) through `read_rust`. The new reader is reader-scope built from
4dbb43f, with the scumm workspace's release profile.

Wall times are inflated by the shared load, so CPU seconds are given where the comparison needs them.

**Gates**

| gate | ring(38) | ring(33) | refinement |
|---|---|---|---|
| G6, the four files and reached: same sha1 as the reference; reading.json the reference's but for the measured fields and `from_golden` | pass | pass | pass |
| G8 action and record = PackedTask._action | all 9,286 | 4,988 (every 47th) | all 74,995 |
| G8 by_name = strict.Reading.by_name (every name, plus 207 names no action has) | pass | pass | pass |
| G8 K = step.reuse_keys_of | 61 keys, pass | 45 keys, pass | 45 keys, pass |
| index built from the files = reader-scope's reading.idx | pass | pass | pass |

- **Without `--out-dir`** (ring(33)), the packed file and the reached file have E009's binary's sha1 (a1ed4f5c,
  9b0c87b7).
- **`reusable` and `reusable_detail`, E008's rule and with E009's K,** equal step.py's on ring(38) for four entry
  starts: j00091, boot, j00089 and j00119.

**Measures**

| | before (Python reference) | after (Rust) |
|---|---|---|
| read_rust's Python side | ring(33) 261.2 s, refinement 96.7 s, ring(38) 54.3 s | none |
| files at the reading's end (wall) | | ring(33): packed file read 0.83 s, then 0.33 s; refinement 0.21 + 0.33 s; ring(38) 0.16 + 0.01 s |
| files on the sink's thread, during the reading | | ring(33) 5.1 CPU s (sink thread busy 38.7 s of a 132.6 s read); refinement 1.9 CPU s |
| strict setup (by_name over every action) | ring(33) 103.6 s, refinement 52.6 s, ring(38) 7.6 s | opening the reading: ring(33) 10-17 ms wall, 9.4-11.5 CPU ms |
| cached open (lookup and open) | | ring(33) 13-15 ms, plus 6-8 ms for the key |
| K (reuse_keys_of) | ring(33) 7.3 s, ring(38) 9.8 s, refinement 5.6 s | 0.14-0.23 s, plus 0.24-0.36 s to load the game once per process |
| action(i) | | ring(33) 8.8 µs CPU (mean over 4,988), ring(38) 6 µs, refinement 9-10 µs |
| record(i) | | 19-30 µs CPU |
| by_name | | 0.5-1.0 µs |

- **Files at the reading's end.** "packed file read" is one pass over reading.tc. It replaces the reader's own sha1
  pass over the same file and gives the index its word offsets and names.
- **Cached open.** The key also needs the reader binary's sha1 and the game index's, 70-170 ms, which a resident
  process computes once.

## E012: `tested` (experiments/E012-game-mechanics/plan.md)

- **`tested::Tested`** (src/tested.rs): the reading's tested atoms T, per key the value sets the actions'
  preconditions hold it to, less each event's own unit's keys, plus the goal's; the keys tested by inputs and by
  events; counts. `Tested::of` takes any `ReadingSource`; `Tested::of_reading` gives the same from the files, with
  the preconditions decoded alone (`Reading::pre_of`).
- **`draws::draws`** (src/draws.rs): an action's checks on a random draw, with the draw's range and the share of its
  values on the check's side, worked out from the scripts by `DrawGame` (the game, loaded once per process).
- **Checks:** `cargo test -p fmi-reading` (unit tests on hand-made units); gate T1 with
  `FMI_T1_READING=<the kitchen's 28-33-41-j00065>` (`tests/tested.rs`); `fmi-reading tested DIR --draws --sites`
  for the counts and times. Results: `experiments/E012-game-mechanics/results/tested/t1.md`.
