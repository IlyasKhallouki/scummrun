# E019: where the search loses speed, and the fixes

## The measurement that started it (E018, 2026-11-05)

On the cloud VM (30 cores), 10 and 30 workers made the same ~7-8 committed tries a minute.
- At 30 workers the CPU was 95% busy: 58% kernel time and 37% user time.
- The kernel's process counter rose ~9,700 a second, with ~480,000 context switches a second.
- Memory used was 6-7 GB.
- More cores fed overhead, not the search.

## The audit (four investigators, each checked; two of them stopped early on a relayed instruction)

- **Engine forks are cheap.**
  - Laptop: a fork plus quit costs 0.32 ms, at ~2.2 forks per trial, about 1% of a free try. Rewinding a saved state
    instead of forking would gain little and is dropped.
  - The VM's 9,700 processes a second are not explained by engine forks; that stays open.
- **The wait runs one Step round trip per frame:** ~2,400 frames per wait, ~334 waits per free try.
  - The "read" phase is 96% of a free try's wall: 46 us a frame against the run command's floor of ~32 us.
  - One round trip is two context switches, which fits the VM's switch rate.
- **Identical tries repeat once a run is starved.**
  - play9: repeated free tries took 37.8% of worker wall and kept 1 entry in all.
  - Repeated model tries with 0 targets took 15%.
- **Level-1 offer points that reach the same ready state repeat their trials:** 23% of free-try seconds in a
  sample of 80, keeping 0 new states.
- **Steps that keep almost nothing:** delays took 13% of the run and kept 2 entries; offer points at the wait's stops
  kept 0 of 278k trials.

## Fixes

1. **`FMI_FREE_SAME=1`:** a level-1 offer point whose ready state, (offer frame, state digest), repeats an earlier
   point's in the same try is skipped. The digest covers the save stream, the RNG, the input registers and the timers.
   - Measured on play9's j00007, laptop, deterministic (the unchanged try gave the same result twice):
     - wall 73.9 s and 78.2 s without the skip, 38.7 s with it (-48%);
     - the same 3 distinct kept states;
     - the 2 more "kept" without the skip were duplicates of the same states (reveal texts kept again at the repeated
       point).
2. **`FMI_SKIP_SAME=1`:** a try whose job.json (less its name and reading folder) repeats a committed try's is not
   run. It is committed as that try's result, with no new entries and no time.
   - A try is deterministic, which is what the archive's verification rests on.
   - Test: `a_repeated_job_is_committed_as_the_same_without_running`.
3. **Planned:** the batched wait. A bridge command runs frame by frame inside the engine, collects each frame's status
   and block delta, and replies once: after the first frame where a watched byte range changes, or at the frame
   budget. The search replays the frames through the same `each` closure, so the stops are unchanged. Gate: old and new
   waits give identical stops on recorded states.

## Fix 3, the batched wait: built, gated, no gain

- **What was built:**
  - Engine: StepTrace (`kWStepTrace`, in a copy of the engine at `/var/tmp/fast-mi/svm-e019`, not a committed patch).
  - Search: `BinEngine::wait` uses StepTrace under `FMI_STEP_TRACE=1`, and the wait sets its observed keys as the
    watch (`Engine::set_watch`).
- **Gate:** on play9's j00007 with the new engine, the try's result is identical with and without it (same trials,
  kept states, waits and new entries; only timing fields differ).
- **Speed:** none, 36.2 s both ways.
  - A batch covered 1.66 frames on average: 752k frames in 452k batches, over 141 watched ranges including the
    block's tail (script slots with their code offsets, which move almost every frame).
  - The audit's numbers also cap any batching at ~30%: a frame costs ~46 us against a plain run's ~32 us. The bulk is
    the engine running the frame, not the round trip.
- Left in, off by default. The lever is fewer frames, not fewer messages.

## Next lever: stop a trial's wait when its state rejoins the no-input state

A trial's wait runs to the step's horizon (~2,400 frames). Most trials do nothing that lasts: a message, then idle
again. If at some frame the trial's state digest equals the no-input twin's at that frame, every later frame is the
twin's, and its stops are the entry's own wait's, already read. The trial's wait can stop there with nothing lost.

## Fix 4, a shorter floor for trials' waits: no gain on the gate try

- **What was built:** `FMI_TRIAL_WAIT_FLOOR=N` takes a trial's wait floor to N frames (the entry's wait keeps E009's
  1,600). It rests on the kept free states: all 51 in play9 and all 39 in play8 came within 164 frames of the nearest
  offer (median 3, p95 90-114), and play8's room-42 crossing came ~390 frames after its click.
- **On j00007:** the same 3 kept states and the same wall (36.4 s at the default, 38.3 s at 600).
- **Why:** the waits there are set by the offer's dues, not the floor. A paused slot is due ~2,400 frames ahead (the
  offer's horizon is 2,420 and 2,394), so every trial waits to that wake-up.
- The other gate tries (play9's j00003, j00047, j00051) did not run on the laptop: their job files name the VM's
  readings. Left in, off by default.

## Where E019 stands

- **Validated:**
  - (1) `FMI_FREE_SAME`: -48% on a free try, the same distinct states.
  - (2) `FMI_SKIP_SAME`: a repeated try is not run (test). It is worth 38-53% of worker wall in starved phases (play9).
- **Built, no gain:** (3) StepTrace and (4) the trial floor.
- **The dominant cost left:** each trial waits to the last scheduled wake-up of the offer's state (~2,400 frames,
  ~110 ms), 96% of a free try.
  - Cutting it needs a causal rule: wait for what the click started or changed, not for wake-ups the no-input state
    has as well. This is a design question with a risk to what the search finds.
- **Cheap cuts the audit measured as yielding nothing:**
  - offer points at the wait's stops: 27% of free-try time, 0 kept in 278k trials (partly removed by (1));
  - delays: 13% of the run, 2 entries kept.
