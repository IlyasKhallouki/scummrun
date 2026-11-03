# E012 `afford`, gates A1-A3 and the walk boxes (2026-11-03)

Branch e012-afford. Method: plan.md section 4 and the two amendments "component afford".

- **Code:**
  - `fmi-explore/src/afford.rs`: switch FMI_AFFORD, off by default; walk boxes under FMI_BOXES.
  - Tests: `fmi-explore/tests/afford.rs`, 9 tests on a toy engine.
  - The check: `fmi-checks/src/bin/afford_check.rs`.
  - Scripts: `afford/gates.sh` and `afford/boxes.sh`. Rows: `gates.jsonl` and `boxes.jsonl`.
- **Setup:**
  - Laptop, seed 1, nice 10, at load 6-8 (other E012 components were building and running).
  - Engine for A1-A3: round 2's `svm-e011-r2int` (patches 0001-0004, lean, cuts). Walk boxes: end's `svm-e012-end`
    (patches 0001-0005).
  - Entries: E011's kitchen archive, read only. The 13 entries are M4's probe entries.
  - T's keys: the plan's 100 keys of `readings/28-33-41-j00065`, the set `tested`'s gate T1 counts.
  - Novelty in these runs: (key, value) pairs of those keys and the room, over every archive entry's last checkpoint.
    This stands in for the archive's novelty over T.

## Gates

| gate | clause | result |
|---|---|---|
| A1 | kept entries verify from boot | 8 kept at the 13 entries. 8 of 8 verified from boot in a fresh lean engine, with 0 differences. The check is fmi-engine's `replay::verify` (E011 G11's): from the hello, through the root's, the entry's and the kept checkpoint |
| A2 | at 205: two outcomes, the line that returns control at +26 | M4's form (600 frames): 4 trials, 2 outcomes. Line 4 (verb 123) is idle in a sentence at +26. Trials, their order and outcomes equal the probe's `afford.tsv` name for name |
| A2 | at 203: M4's 8 outcomes with the fixed horizon | M4's form (400 frames): 18 verbs and 12 targets hit, 246 trials, 8 outcomes, equal to the probe name for name. At 4 also equal: 4 trials, 1 outcome |
| A3 | ≤ 20 ms per trial on round 2's engine | The try's form at 203 (270 trials). CPU per trial (the forks' and the control's engine CPU and the step's own): 3.7-4.0 ms over 5 steps. Wall per trial: 5.95-22.7 ms over the 33 steps measured, at load 5.7-11; 32 of the 33 under 20 ms. At the 13 entries in one run at load 6-7: 3.4-11.5 ms |

A1 and A2 pass, with no fix round of the component's code; A2 passed again with FMI_BOXES off on end's engine. A3
passes on the step's own cost: its CPU is 3.7-4.0 ms per trial. Its wall is under 20 ms in 32 of the 33 steps measured
at 203. The one above, 22.7 ms, ran at load 9.6. At that load the wall per trial is about four times the CPU: the
shared laptop's other work (the other E012 components' builds and gates) was taking the CPUs.

## The step at the probe's entries (the try's form)

The try's form: each trial stops at the bridge's idle stop or after W = 1,600 frames, with the third click when
composing.

| entry | frame | offer at | idle | verbs | targets | trials | third | composing | outcomes (less control keys) | kept | step s | ms/trial |
|---:|---:|---:|---|---:|---:|---:|---:|---:|---|---:|---:|---:|
| 0 | 2728 | 2728 | sentence | 18 | 2 | 128 | 72 | 36 | 1 (1) | 0 | 0.50 | 3.9 |
| 2 | 2848 | 2848 | sentence | 18 | 6 | 348 | 216 | 36 | 4 (4) | 1 | 2.85 | 8.2 |
| 4 | 2862 | 2862 | dialog | 4 | 6 | 4 | 0 | 0 | 1 (1) | 1 | 0.03 | 6.7 |
| 49 | 3048 | 3064 | sentence | 18 | 10 | 428 | 220 | 22 | 5 (4) | 0 | 1.99 | 4.6 |
| 67 | 3121 | 3121 | sentence | 18 | 13 | 265 | 0 | 0 | 3 (3) | 0 | 1.05 | 4.0 |
| 89 | 3249 | 3253 | sentence | 18 | 5 | 223 | 110 | 22 | 3 (3) | 1 | 0.75 | 3.4 |
| 203 | 3393 | 3393 | sentence | 18 | 12 | 270 | 24 | 2 | 8 (7) | 3 | 1.81 | 6.7 |
| 204 | 3395 | 3443 | dialog | 4 | 11 | 4 | 0 | 0 | 3 (2) | 0 | 0.02 | 6.0 |
| 205 | 3443 | 3443 | dialog | 4 | 11 | 4 | 0 | 0 | 3 (2) | 0 | 0.05 | 11.5 |
| 209 | 3422 | 3443 | dialog | 4 | 11 | 4 | 0 | 0 | 3 (2) | 0 | 0.02 | 5.5 |
| 222 | 3432 | 3443 | dialog | 4 | 11 | 4 | 0 | 0 | 3 (2) | 0 | 0.03 | 7.3 |
| 224 | 3445 | 3530 | dialog | 4 | 11 | 4 | 0 | 0 | 2 (2) | 1 | 0.04 | 9.5 |
| 225 | 3530 | 3530 | dialog | 4 | 11 | 4 | 0 | 0 | 2 (2) | 1 | 0.02 | 6.1 |

- **Not idle at the entry's frame.** 6 of the 13 entries (49, 89, 204, 209, 222, 224) were not idle there. The offer
  was made at the first idle stop of a no-input fork, 4-85 frames later. The kept entry from 224 verifies through it.
- **Composing**, the rule as written:
  - At 203, 2 trials compose (verbs 4 and 10 with object 323). Their 24 third clicks keep nothing new.
  - At 0 and 2, 36 trials each compose, and at 49 and 89, 22 each.
  - At 0, every verb with either of the 2 targets is taken by the input script and nothing else runs. Its 72 third
    clicks change no world key. Where clicks are absorbed this way, the rule costs about 3-4 ms per extra trial.
- **Rooms from 203.** Trials reach rooms 79, 81 and 82, where the 3 kept entries are. 22 trials reach room 33:
  - 20 that M4's form saw in room 72 at its 400-frame horizon. Run on to the idle stop at +507 to +509, they pass room
    72 and stop in room 33.
  - 2 third clicks, stopped at +512.
- **Outcomes against no input** depend on the stop frame in this form. At 205, lines 1 and 2 stop at +86 and line 3
  at +76. On every world key these are 2 outcomes, because the animation locals 208 and 209 are in another phase. Less
  the script-control keys they are 1, so the 4 lines give 2 outcomes, as in M4.

## Cost

- **Per trial at 203** (270 trials, 1.81 s at load 6-7): forks 0.39 s (1.4 ms each), runs 1.24 s, reads (facts,
  runs, dumps) 0.12 s, quits 0.01 s, the no-input control 0.03 s. The engine's frames dominate.
- **CPU per trial at 203:** 3.7-4.0 ms (engine 0.77 s and the step's own 0.23 s for 270 trials). At load 10.4 the
  same steps took 15.3-19.0 ms of wall per trial.
- **M4's form at 203** (400 frames per trial): 20.2-39.0 ms per trial over four runs, at load 6-11. M4's probe took
  47.5 ms on round 1's engine.
- **Per step**, against plan.md's prediction:
  - a dialog: 0.02-0.05 s (predicted at most 0.5 s);
  - a sentence idle: 0.5-2.9 s (predicted 1-5 s);
  - per trial: 3.7-4.0 ms of CPU, and 3.4-22.7 ms of wall at load 5.7-11 (predicted 3-20 ms).

## Walk boxes (FMI_BOXES), measured first

On end's engine, with FMI_BOXES off and on at the 13 entries. Dialog entries ask no boxes. The 6 sentence entries:

| entry | boxes | walkable | with a floor point | trials off → on | outcomes off → on | kept off → on | step s off → on |
|---:|---:|---:|---:|---|---|---|---|
| 0 | 14 | 13 | 2 | 128 → 130 | 1 → 1 | 0 → 0 | 0.48 → 0.56 |
| 2 | 6 | 5 | 3 | 348 → 351 | 4 → 4 | 1 → 1 | 2.90 → 3.30 |
| 49 | 24 | 21 | 12 | 428 → 440 | 5 → 5 | 0 → 0 | 2.31 → 2.88 |
| 67 | 10 | 9 | 8 | 265 → 273 | 3 → 3 | 0 → 0 | 1.20 → 1.59 |
| 89 | 15 | 14 | 3 | 223 → 226 | 3 → 3 | 1 → 1 | 1.11 → 1.29 |
| 203 | 12 | 10 | 4 | 270 → 274 | 8 → 8 | 3 → 3 | 2.09 → 2.28 |

- **Totals over the 13 entries:** 1,690 → 1,722 trials and 10.3 → 12.1 s of steps (+17%).
- **Yield:** no new outcome and no new kept entry. Each walk stops in its own room, at an idle stop 4-52 frames
  later. The 8 kept entries are the same and all verify from boot with 0 differences.
- **Why few floor points:** 72 boxes are walkable, and 32 of them have a candidate point on screen where the hit test
  answers no object and no actor. The others lie off screen in the wide rooms, or under room objects.
- **In short:** on these entries the walk clicks reach nothing that the verb and target clicks do not, and they cost
  17% more step time. FMI_BOXES is off by default, like every E012 switch; whether a run turns it on is the run's
  setting (the plan's kitchen rerun turns every E012 switch on).

## For try

- The try does not call the step yet. Try wires the triggers (R0, no target that plans, the same empty window twice)
  and passes:
  - T's keys (`Tested::keys`);
  - the archive's novelty (`afford::Novelty`);
  - the wait's stops (`then`) and its horizon.

  It commits each `Kept` as an entry whose movie is the entry's, then `Kept::events`. A trial that ended carries the
  End's reason (end's `is_game_end` makes it a dead mark).
- E011's `HostVerifier::replay_verify` (`fmi explore verify`, the driver's goal verify) fails on its first
  checkpoint. It boots the engine to that frame and then asks to run to the same frame, which the bridge refuses;
  replay.py's verify starts from the hello instead. This was posted in COORD.md. The checks here use fmi-engine's
  `replay::verify`.
- G0: with FMI_AFFORD and FMI_BOXES off, no existing code path changes. The component adds a module, a check binary,
  and end's two commits cherry-picked unchanged. The tests pass: fmi-core 3, fmi-engine 7, fmi-explore 79 (afford's
  9 among them).
