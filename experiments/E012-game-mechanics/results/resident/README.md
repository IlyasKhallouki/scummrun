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
