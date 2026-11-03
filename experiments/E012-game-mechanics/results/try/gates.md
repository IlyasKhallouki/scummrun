# E012 component try: gates G0 and G-on, and the try's cost

Branch e012-try (73477f2 for the code; end, tested, targets, afford, archive, delay and wait merged unchanged). Laptop,
seed 1, nice 10, each sequence one resident `fmi` under the heavy lock and the guard. Runs: `try/gates.sh`. Data:
`~/.cache/fast-mi-e012/try` (results and entries; the readings were removed after each gate).

## G0: every E012 switch off

B3's 15 tries from round 2's job folders, round 2's switches (ON2), every E012 switch unset, compared with
`fmi check tries` (cmp_tries.py's comparisons, and the targets, probes, tried and exhausted).

| sequence | against | binary | tries equal in everything | wall |
|---|---|---|---:|---:|
| fast (`--search fd --tail`, ready) | round 2's `b3f/r2-fast` | 8b773a03aeeb (before the novelty fix) | 15 of 15 | 187.1 s |
| fast, no reading cache | round 2's `b3f/r2-fast` | 73ff54e0eefe | 15 of 15 | 308.9 s |
| reference (`--search fd`, strict) | round 2's `b3/r2-ref` | 73ff54e0eefe | 15 of 15 | 201.3 s |

Files: `g0-off-fast.json` (fast, no cache), `g0-off-ref.json`.

## G-on: every E012 switch on

The same 15 tries in the fast settings with ON3 and `FMI_WAIT=1 FMI_TARGET_PER_KEY=1 FMI_DELAY_FORK=1 FMI_AFFORD=1
FMI_SELECT=progress SPEEDRUN_REPORT_RESTART=1 FMI_BOXES=1`, on end's engine (patches 0001-0005). Every new entry
verified from boot in a fresh lean engine (`fmi check verify-new`: the movie from boot against the root's checkpoints,
the try's entry's and the new entry's).

| run | binary | new entries | verified, 0 differences | by kind |
|---|---|---:|---:|---|
| with the reading cache | 8b773a03aeeb | 76 | 76 | attempt 43 (28 checkpoint, 15 end), wait 18, afford 15, delay 0 |
| no reading cache | 73ff54e0eefe | 76 | 76 | the same |

File: `gon-verify.json` (no cache). The attempts are the off sequence's in every try (stop, target, frames, inputs,
refinements, entries); the extra entries come after them.

What each mechanism did in the 15 tries:

- **T.** 559-577 atoms over 450-451 keys on the ring(33) readings, 97 atoms over 66 keys on ring(38).
  `Tested::of` took 1.7-1.9 s per ring(33) reading and 0.05 s per ring(38) reading, once per reading in the process.
- **Wait.** 15 waits, 0.05-0.13 s each (W4's bound is 0.3 s). Static at entries 0 and 4 (room 38's dialog; W3's).
  Two stops at 9 entries: at +1 (script 2 starts) and at +11 or +51, where the dialog's verbs 120-123 appear or the
  room becomes 38. All 18 stops were novel against the entry and kept.
- **Targets.** The tries' targets are B0's forced ones; the T tier changes no forced target.
- **Delay fork.** 1-2 windows per try, none triggered (no member checks a draw, as delay's amendment measured); 1
  random call per frame over every window's parent frames in rooms 33 and 38 (M1).
- **Affordance step.** All 15 tries stop on Lemma R's R0, so each ran the step at the attempt's end: a dialog, 4
  lines, each tried through the wait's stops, 1 outcome kept per step, 0.27-0.51 s per step.
- **Goal again.** Not exercised: no try's target list holds the goal.
- **Dead moves.** None.

## E011's gates with this branch's fmi

- G9: pass (the driver's 18 scenarios × 2 modes, the guide's 4 goals, step 1,092 and harness 416 cases, the 3
  recorded tries through run_job, the 4 re-ingests).
- G10, test (a) in the fast settings: every E012 switch off and every one on both give round 2's line (goal, from the
  plan, frames 2728 to 3216, 488 frames, qf 11,712, 2 replans, 36 inputs, 8 entries), and the 8 entries verify from
  boot with 0 differences. With the switches on, the wait at the root is static, the delay fork sees 4 windows (none
  triggered) and 1-4 random calls per frame over 321 frames.
- G11: G0 above.
- fmi-explore's and fmi-core's cargo tests pass (with the merged components' tests).

## The try's cost

Off and on with no reading cache (refinements made again in both, so that the cache's disk floor does not decide the
reads), back to back at load 3.0-3.2. Table per try: `tries-off-on.md`.

| | off | on |
|---|---:|---:|
| wall, 15 tries | 307.4 s | 320.8 s |
| T | | 3.7 s |
| E009's idle measurement, or the wait in its place | 0.5 s CPU | 1.1 s wall, 1.4 s CPU |
| affordance steps | | 5.1 s wall, 1.6 s CPU |
| read, plan (CPU) | 547.9 s, 45.5 s | 549.3 s, 48.1 s |

The first G-on run, with the reading cache, took 271 s against G0's 187 s: its refinements were made again because
the cache stored nothing under its disk floor at that moment (E011 round 2's amendment on the memo measured the same
effect), not because of E012's steps.
