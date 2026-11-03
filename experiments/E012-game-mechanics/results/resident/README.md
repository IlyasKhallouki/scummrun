# E012 `resident`: the reader in fmi's process, the job memo resident (2026-11-03)

Branch e012-resident. Method: E012 plan.md, "Amendment 2026-11-03, component `resident`" and its amendment "the second
entry"; the argument: E011 memo.md, "Amendment 2026-11-03 (E012, component `resident`)". Laptop, seed 1, nice 10, 3
reader threads, this branch's release builds (`resident/build.sh`: fmi and reader-scope, debug info off). Drivers:
`resident/rs1.sh`, `resident/kitchen.sh`, `resident/heavy.sh`, each reading through `fmi read` (fmi's readings port:
reader-scope as a child, or the process itself with FMI_READER_LIB=1).

## RS1: byte for byte on G6's recorded inputs

Reader-scope (a child of `fmi read`, no memo, no `--prep`; under the guard, load 4-6) made the reference readings;
then one fmi process with the switch (the store in memory, `--prep`, the key's units as a set; under the heavy lock
and the guard) read the five in RS1's order. Per-file sha1s: `rs1-sums.txt`; per-reading lines: `rs1-ref.jsonl`,
`rs1-inproc.jsonl`.

| reading | five files vs reader-scope | tc, reached, actions, lists vs G6's recorded sha1 | reading.json (cmp_reading.py) | memo: taken / run | wall | CPU |
|---|---|---|---|---|---:|---:|
| ring(33) j00089 | same | a1ed4f5c 9b0c87b7 079a02d1 9ab7e2db: same | same, but the measured fields | 16,673 / 22,842 (its own pass 1) | 38.9 s | 112.7 s |
| ring(33) j00119 | same | 41eea348 9b0c87b7: same (G6 has no line-file sha1) | same | 39,515 / 0 | 14.0 s | 35.8 s |
| refinement j00089 [103, 194] | same | 397fb6f1 ab30e2ef 7781f700 36d3ed77: same | same | 11,249 / 23,077 | 17.3 s | 49.8 s |
| the same again | same | the same | same | 34,326 / 0 | 5.9 s | 13.4 s |
| ring(38) j00091 | same | 00dbb753 90b3d43a 66b4c63e 5d99b2b5: same | same | 1,967 / 3,790 | 2.0 s | 5.5 s |

RS1 passes. The store held 181 MB after the first reading and 307 MB after the five; fmi's peak resident set was
1,073 MB.

## The keys step (ring(33), the kitchen check's starts)

One fmi process with the switch and the units in their order (FMI_READER_UNITS_SET=0), a disk folder for the key
texts: entries 0 (j00000), 191 (j00057), 189 (j00059). `keys-r33.jsonl`, `keys-r33.txt`.

- Entry 191's key differs from entry 0's in the `units` line alone: the same 140 units, Global(2) first at place 134
  where entry 0 has it last (script 2 runs at entry 191's start). Its reading has 0 records (0.2 s), as the check's
  had: it reaches no job.
- Entry 189's key is entry 0's. Its reading took 39,115 of 39,515 runs and ran 400 (a consulted key reads otherwise):
  15.3 s of wall and 40.4 CPU s, against 38.9 s and 113.0 CPU s for entry 0's fresh reading in the same process.

## RS2 and the units as a set

- **The units part is not exercised by the check's readings.** On (28, 33, 41) the keys step gave j00061's start
  (entry 205) and j00062's (entry 204) the same key with the units in their order (`measure-28-33-41-keys.jsonl`).
  In the check's store the folders that differ in the units' order alone are, on both scopes, a reading whose start
  runs script 2 (entries 191 and 208, j00057 and j00063), and each such reading has 0 records (the check's log, and
  entry 191's reading here). So on these inputs the key's units as a set lets no further run be taken. It stays in
  the switch: its argument holds (memo.md) and it is tested (`memo::tests::the_keys_units_as_a_set`), but this
  measure gives it no gain.
- **RS2's files pass.** Every reading of the measure below was summed and compared with reader-scope's reading of the
  same arguments (`kitchen.sh ... ref`: a child, no memo, no `--prep`), and reading.json by cmp_reading.py:
  ring(33) entry 0 and entry 189 in `before` (2), `disk` (1) and `after` (2, each with entry 189 twice), and
  (28, 33, 41) j00061 and j00062 in `before` and `after`: 17 readings, all the same (`rs2-cmp.txt`).

## The measure: a fresh ring(33) reading from a second entry

Entry 0's start (j00000) then entry 189's (j00059), each phase one `fmi read` (one process) under the heavy lock and
the guard, before and after interleaved, at load 4-6 (other experiments on the laptop). Wall and CPU of the reading
as `fmi read` saw it (CPU: fmi's own in the process, the child's otherwise). Per reading: `measure-*.jsonl`.

| phase | entry 0 (the first reading) | entry 189 (the second) | entry 189 again (every run taken: the floor) | runs of entry 189 taken / run |
|---|---|---|---|---|
| before 1: round 2's way (reader-scope as a child, `--prep`, no memo) | 46.1 s, 128.4 CPU s | 47.3 s, 133.3 CPU s | | 0 / 39,515 |
| after 1: FMI_READER_LIB=1 | 45.1 s, 132.4 CPU s | **20.2 s, 51.9 CPU s** | 17.2 s, 44.1 CPU s | 39,115 / 400 |
| before 2 | 39.0 s, 109.0 CPU s | 39.0 s, 108.9 CPU s | | 0 / 39,515 |
| after 2 | 39.1 s, 113.3 CPU s | **15.7 s, 41.4 CPU s** | 13.6 s, 35.5 CPU s | 39,115 / 400 |
| disk: reader-scope as a child, `--prep`, round 1's disk memo (floor at 5 GB) | 35.8 s, 104.0 CPU s | 16.7 s, 40.1 CPU s | | 39,115 / 400 |

- **Before to after**, the second entry's reading: 39.0-47.3 s to 15.7-20.2 s of wall (0.40-0.43 of it), 108.9-133.3
  to 41.4-51.9 CPU s (0.38-0.39). After / floor: 1.15-1.18. Against E011's floor for this part (the memo-taken
  reading, 12-16 s with the store on disk): 15.7-20.2 s.
- **The first reading** costs the same in the process as in a child (45.1 and 39.1 s against 46.1 and 39.0 s): the
  process start and the game's load were never the cost.
- **The disk memo does as well** (16.7 s) when its disk is above its floor. Round 2's B3 ran with the memo off because
  the disk's level turned the store off (E011's amendment "B3 without the job memo"). The resident store has no disk
  floor: it holds 189 MB for the two readings, fmi's peak resident set is 1.1 GB.
- **(28, 33, 41)**, j00061 then j00062: before 11.3 s and 30.2 CPU s; after 6.7 s and 17.7 CPU s (16,239 runs taken,
  1,821 run); the floor 4.1 s and 9.5 CPU s.

**Where the floor goes** (entry 189 read again, after 2; the reading's report): 11.9 s of the reading's 12.7 s are
its runs; the pool's job seconds are 23.5 over 39,515 jobs (two passes), of which taking the stored runs (lookup,
inflate, decode) is 5.2 s and the rest is `--prep`'s work on the taken records (the packed file's actions, the line
files' lines, the reached values' effects); the reading thread 6.2 CPU s, the sink 2.4 CPU s. With 3 workers the
jobs' part alone is about 7.8 s of wall. A fresh reading's job seconds are 96-98.

## Against the prediction

| | predicted | measured |
|---|---|---|
| RS1-RS3, G0 | pass | RS1 and RS2's files pass; RS2's units part not exercised (above); RS3: memo.md's amendment, the cargo tests pass (scumm-reader memo 5 of 5, fmi-reading 8 of 8); G0 by construction (the switch off: the child path's arguments and allocator unchanged) |
| before | 35-42 s, 105-140 CPU s | 39.0-47.3 s, 108.9-133.3 CPU s |
| with the disk memo | as before (the key differs) | 16.7 s: entry 189's key is entry 0's (the prediction assumed the units' order would differ) |
| after: runs taken | at least 80% | 99.0% (39,115 of 39,515) |
| after: wall, CPU | 5-15 s, 15-45 CPU s | 15.7-20.2 s, 41.4-51.9 CPU s |
| the floor | 4-12 s | 13.6-17.2 s |
| fmi's peak memory | at most 3 GB | 1.1 GB |
