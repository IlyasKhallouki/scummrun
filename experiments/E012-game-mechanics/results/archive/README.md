# E012 component `archive`: gates S1 and S2

Branch e012-archive. Code: `fmi-explore/src/progress.rs` and the hooks in `fmi-explore/src/archive.rs`, behind
`FMI_SELECT=progress` (off by default). The method is plan.md's amendment "component `archive`". Laptop, seed 1,
nice 10, the build's dev profile at opt-level 2. The laptop was shared with the other components' runs (load 5-6).

## S1: unit tests

`cargo test -p fmi-explore`: the 13 tests of `tests/archive_progress.rs` and the 4 of `progress.rs` pass, with E011's
own 28 archive tests and the crate's other tests unchanged.

- **Atoms.** An atom's truth equals `atom_holds` through `Domain::label` for every value from −3 to 5 over six value
  sets, `below` and `above` among them.
- **Cells.** A cell is the room, the owned objects and T's held atoms. The control keys and E009's idle set are left
  out.
- **Novelty.** `new_t` and `new_rooms` per try. A "return differs" line counts in neither count.
- **The order.** Each key alone: n, chosen, rooms on the chain, novel atoms, frame, id. Also the candidate within a
  cell.
- **Marks.** A dead entry is never selected and holds nothing for novelty. Its dead moves come back in job.json. An
  exhausted entry is skipped until T grows.
- **Opened again.** Rows, cells.json, T, the marks and the order are the same when the archive is opened again, and
  after T grows (the rows keyed again from the store equal a fresh open from the files).
- **Stagnation** over `new_t` and `new_rooms`.
- **Switch off.** No E012 field appears in stage.json, the rows, cells.json, the log or job.json, and the cell key is
  E008's.
- **Brute force.** On a generated archive (60 tries from xorshift32 seed 3182679328: 51 rows, 23 cells, T of 24 atoms,
  dead entries, exhausted marks and T growth), the whole order after every try equals a brute force. The brute force
  recomputes T from the log, every row's atoms and cell from its file, the counts from the log lines, and the least
  holders. job.json's novelty equals a brute force over the live rows' facts on 153 states: every row's own state, and
  two states moved off it per row.

## The switch off on real results (G0 for this component)

E011's `reingest_check` ingests the kitchen check's 65 committed tries again into a fresh archive
(`archive-s2 reingest-off`, switch unset). Compared with the recorded archive:

- entries.jsonl, cells.json and log.jsonl are byte for byte the same;
- the 226 entry files are the same;
- readings.jsonl is the same but for the eviction's fields.

The output also equals that of E011's own binary (`target-e011/release/fmi check reingest`), byte for byte for those
four files.

Both builds report the same job.json difference at j00017-j00022 ("rotation", and "held" at j00019). This is the
check's estimate of the moment a job is prepared with two workers, the same with and without this branch.

## S2: the kitchen archive under the cells over T

**T.** The 19 readings the check's committed tries used were made again from their start files
(`archive/s2-readings.sh`: round 1's reader-scope with the tries' arguments, 358 s). Their record and action counts
equal the check's log. The archive's one surviving reading, 28-33-41-j00065, was added through links. Per reading,
T's atoms came from `Tested::of_reading` (component tested, merged unchanged) under the reading's domains.

- 97 to 577 atoms per reading.
- The union is 975 atoms, 33 of them on control keys.
- 835 atoms are held by some entry, 783 of them already by the root (room 33).
- 140 are never held.

Tables: `s2-recount.md`.

**Recount** (the amendment's method: one T, the union; `new_t` and `new_rooms` from the rows in commit order):

| | E011's cells | cells over T |
|---|---:|---:|
| cells | 86 | 29 |
| entries in the largest | 18 | 73 |
| median entries per cell | 2 | 4 |
| cells of one entry | 35 | 6 |

- 17 new cells hold entries of more than one E011 cell; 7 E011 cells are split; 5 are the same.
- 19 of the 29 cells were chosen. The most chosen is a room-29 cell with owner:377 = 1: 22 tries, 73 entries, and no
  new atom since its first try (n = 22).
- 8 of the 65 tries gave a new atom of T or a new room: j00000, j00015, j00017, j00024, j00025, j00059, j00062 and
  j00060.
- The order's first entries are in rooms 28, 34, 33 and 35, with 3-4 rooms on their chains. E011's are its newest
  room-28 and room-33 entries.

**Replay** (`s2-replay.md`): the 65 results ingested again with the switch on. Each result carries the T of its own
reading, so T grows as it did along the check: 559 atoms after the first try, 975 after j00061. Three tries read the
two empty readings (F9, F14) and carry no T.

- The cells, the counts and the first 20 of the order at the end equal the recount's.
- **Stagnation over T fires after the 4th try** (j00003). After j00000 the frontier is 3 selectable cells, and
  j00001-j00003 give no new atom of T and no new room. The rule holds after 26 of the 65 tries: j00003 to j00014, then
  j00045 to j00058.
- In the recount, with the whole T from the start, it fires at j00004 (frontier 4) and at j00046.
- E009's rule over R did not fire in the check.

**Cost.**

- **Interleaved re-ingests** of the 65 tries, three pairs. The archive's time (job.json, the selection, the ingest)
  was 25.5, 26.2 and 27.3 s with the switch off, and 26.5, 26.9 and 29.4 s with it on.
- **Per try with the switch on:** ingest median 52 ms. The maximum, 2.8-3.1 s, is a try whose idle keys grew, so that
  E009's rekey reads every entry file, as with the switch off.
- **Opening the copy:** 3.0 s with the switch on, 4.2 s with it off. Both key every row from the files.
- **Keying every row again from the store when T grows:** 0.19-0.25 s for 226 rows. The store keeps 11,627 values.
- **job.json's "novelty":** 6-10 ms and 65-68 KB per try (`seen` with the keys that are 0 in every live entry left
  out).
- **The selection:** select(1) with the selectable cells, 0.15-0.37 ms.

## Files

- `s2-recount.md` and `.json`: `archive-s2 recount` of a copy of the kitchen archive.
- `s2-replay.md` and `.json`: `archive-s2 replay`.
- `../../archive/s2/`: the tool, its own crate, which depends on fmi-explore and fmi-reading.
- `../../archive/s2-readings.sh` and `s2-readings.tsv`: the readings made again, and the map from reading path to
  name.
- T.jsonl, the readings' T, is not kept (2.6 MB). The script makes it again.
