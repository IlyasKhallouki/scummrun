# E013 component refuse: results

Branch `e013-refuse`; plan.md method B, amendments (2) (a) and (3). Every switch is off by default. Counts, frames
and numbers only: no game text.

## What was built

- **Engine patch 0007** (`patches/0007-scumm-speedrun-input.patch`, on 0006-e014; `engine/build.sh`). Taken from the
  paused e013-input engine work, without the input report and the engine rule (amendment (2) (a)).
  - `SPEEDRUN_INPUT=1` turns on the input log: for each frame with an injected key or click, its path through
    `checkExecVerbs` and the handler.
    - Dropped: by the gate, the zone, the handler, "cleared" or "unread".
    - Otherwise the handler run's reads and writes, and its verdict: ignored or taken.
    - The reads also hold the path's own reads: the gate, and `VAR_VERB_SCRIPT`.
  - The Digest command is always available. It hashes the save stream, the RNG, the input registers and the sound
    state.
  - The JSON twins of both commands exist.
- **fmi-core and fmi-engine:**
  - the types `InputLog`, `InputRecord`, `Verdict`, `Touch`, `TouchKind`, `LogOp` and `Digest`;
  - `Engine::input_log` and `Engine::digest`;
  - SPEEDRUN_INPUT is passed to the engines when `SPEEDRUN_INPUT=1` or `FMI_REFUSE=1`.
- **The window search** (`fmi-play` `refuse.rs`, `window.rs`, `executor.rs`; `FMI_REFUSE=1`).
  - A ready window's try that fails at the interface reads its log, and the executor names its failed check.
  - Amendment (3)'s rows give the cause and the values watched.
  - The parent forks the next try only at a frame where a watched value differs from its value at the refused try's
    fork frame.
  - When the window reaches its bound with no change since a refused try, the window ends at that try's fork frame,
    from a fork of the parent kept there ("refused input: CAUSE").
  - Tries run one at a time.
  - The window record has a "refuse" field.
- **The affordance step and the free tries** (`fmi-explore` `afford.rs`; through `AffordCfg::default()`).
  - A trial whose inputs were all dropped or ignored ends at the frame after its last click, with no wait.
  - A trial whose stop has the digest of an earlier trial's stop, or of the no-input twin at that frame, shares that
    wait.
  - `FMI_REFUSE_DEDUPE=0` turns the sharing off. `FMI_REFUSE_CHECK=1` runs every shared wait again and compares the
    two (gate B4).
  - The step's record has "verdicts", "verdict_s" and "refuse".
- **The checks** (`fmi-checks` `refuse-check`): `m1`, `b3`, `b1`, `verify`, `b4`, `digest`.
  - `refuse/rerun.sh` runs recorded tries again from their job.json.
  - `refuse/a1.sh` runs E011's engine gates on patch 0007.

## M1 (`m1.jsonl`, `m1-summary.json`)

| | predicted | measured |
|---|---|---|
| the 12 "verb key" windows | the key does not reach the handler as a click on the verb | held, 12/12. In every case the key was dropped at the gate, and the parent's gate was open at that frame: the try's own approach click (taken, a walk) shut the gate |
| the 4 "never came on screen" windows | no engine input | **missed.** Each issued 3-4 approach clicks, all taken |

All 16 first tries, played again, give their recorded message.

## Gates

**A1, patch 0007 inert** (`refuse/a1.sh`). E011's G2, G3 and G4 pass with `SPEEDRUN_INPUT` off and on: 0
differences against svm-w3b, and 0 in G4's 400-frame walk. G1 with it on: see `a1-g1.txt`. The cargo tests
`fmi-engine --test input` also pass:
- the replies' decoding;
- the log's paths at the kitchen root;
- the switch inert over 200 frames with clicks and keys;
- the digest's A0 checks.

**B1, skips on recorded windows.** Recorded tries were run again with `FMI_REFUSE=1`, B4's check, E012's switches
and round 2's fast settings.

- **The E012 kitchen archive's 53 jobs.**
  - 76 windows started at the recorded state. 68 make the recorded choice and end there. The other 8 are the windows
    B ended, all of them bound windows.
  - All 2,344 recorded tries inside B's skips were refused in E012's search with the refused try's message. 0 differ.
  - B ended no window that E012's search ended on an end point.
  - The 16 bound windows (`kitchen-b1-bound.jsonl`):
    - 8 were reached at their recorded start, and B ended all 8 within 0-4 frames of their first try: 1 or 5 forks
      instead of 296, 3,595-3,599 frames skipped.
    - 6 more are second windows of the same attempts. The attempt reached them at an earlier frame, because the
      window before ended early, and B ended them at their first try.
    - 2 were not reached: the attempt stopped earlier, by the harness's "replan with the same start and bans" rule.
  - Prediction "B ends 14-16 of the 16 within 10 frames": 14 of the 14 reached. 2 were not reached.
  - The causes met: "gate shut by the try" (the 12 verb-key windows) and "no input formed" (the never-on-screen
    windows, where the camera and the actors change at every frame until ego stops).
- **E012's B3 tries** (the 15 tries with every E012 switch on): 22 windows matched, 22 the same as recorded. 12
  recorded tries inside skips, all refused with the same message.

**B2 and rule (2).** Every new entry of the reruns verifies from boot in a fresh lean engine:
- the kitchen jobs: 127 entries, 0 differences;
- B3's tries: 76 entries, 0 differences.

With `FMI_REFUSE=1` the kitchen's goal job (j00052) still reaches owner:566 = 1 at frame 18,416, as recorded.
Whether the kitchen is still reached from the boot is K1's measure.

**B3, the verdicts on fixtures** (`b3.jsonl`, E011's kitchen entries 203 and 205): 5/5.

| fixture | verdict |
|---|---|
| a verb click at 203 | taken, the slot clicked |
| a key no slot has at 203 | handled in the key area (area 4), ignored |
| a dialog line at 205 | taken |
| a scene click during that dialog | ignored |
| a key with the gate shut after the line | dropped: gate |

**B4, the digest.**

- A fork run 200 frames on and asked the digest at every frame equals a replay from boot to the same frame, and the
  parent run there with no digest asked (equal exact dumps). Checked at 6 kitchen entries (`digest.jsonl`).
- 200 of 200 consecutive idle frames differ.
- 0.18-0.27 ms per digest; the save stream is about 67 KB.
- In the kitchen rerun's 40 affordance steps (`kitchen-b4.json`), 1,379 trial pairs had equal digests. With the
  check, their own waits were run again: 1,379 gave the same stops and last checkpoint, 0 differed. The dedupe can
  stay on.

**With FMI_REFUSE unset.** B3's 15 tries run again on this build give 22 of 22 windows the same as recorded, and 76
entries verify. The cargo tests of fmi-core, fmi-play (R3's delay fixture included) and fmi-explore pass.

## Measurements (kitchen rerun, b = 1, B4's check on, the reading cache off)

| | recorded | rerun |
|---|---:|---:|
| the 8 jobs with a bound window | 584.9 s | 294.5 s |
| all 53 jobs | 1,658.1 s | 1,740.7 s |

The whole rerun's time includes running 1,379 shared waits again (the check). Timing under the run's real settings
is K1's measure.

In the affordance steps:
- 4,831 trials were taken, 11 ignored (ended at once);
- 1,379 trials (28.5%) shared an earlier trial's wait;
- 0 trials shared the no-input twin's;
- 0.49 ms per digest, the twin's runs included.

## Deviations

1. **Patch 0007 was built in this component.** No input builder ran. COORD.md has the IFACE lines.
2. **How a try is read: amendment (3)**, written after M1 and before B's code.
   - A drop at a gate the parent holds open is the try's own doing. It is read as "taken, then the check failed",
     over all of the try's records.
   - A change is the parent against itself at the fork frame.
   - The input report is not built. Its stand-in is the status's gate, cursor, cutscene, room and screen layout, and
     the dump's verb slots.
3. **"ignored" means the run changed nothing outside its locals**, not that it wrote nothing:
   - a write of the value already held counts as no write;
   - a random draw counts as a change;
   - a started slot still alive counts as a change.
4. **With FMI_REFUSE the ready window runs its tries one at a time.** The choice is E010's, in fork order.
5. **`digest-check` is the `digest` mode of `refuse-check`.**
6. **B4's pair sample** comes from the kitchen jobs run again, not from the A3 and K1 samples. A3 is not run.
7. **B1 played windows again by rerunning each job from its job.json**, with the reading cache off. A window was
   compared only where the rerun reached the recorded start.
8. **The engine tree is on /home** (`~/.cache/fast-mi-e013/svm-e013-refuse`), for the disk floors.
