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
