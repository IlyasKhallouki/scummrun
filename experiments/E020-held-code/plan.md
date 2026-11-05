# E020: a room's reading reads the code of the objects the actors hold

## Problem

E019's run reached C5 (room 42 holding object 946) and stayed there. C6 is `class:491:12` cleared, which happens when
the player uses 946 on 491. Over the run's room-42 tries the targets were `bit:446`, `var:104`, `var:52` and
`class:491:6`. `class:491:12` was never one of them.

The cause is in the reader's scope. `Scope::of_with` reads an object's code only when the object's room is in the
scope. 946 is defined in room 69, and its Use handler is what starts room 42's local script 212 when it is used on 491
(that script clears the class). So the room-42 reading had no record that could reach `class:491:12 = 0`, and its
sentence inputs took only room 42's objects as the first object.

## Change (switch `FMI_READ_HELD=1`, off by default)

- **Reader (`scope.rs`, `builder.rs`):** an object of another room that an actor holds at the start is read too.
  `Scope.held` lists these objects, and they become sentence objects (A and B) next to the rooms' objects. They are not
  added to `Scope.objects`, which the hit test uses.
- **Archive (`archive.rs`):** readings registered under the switch are marked `held`. Under the switch only those are
  offered, so readings made without the held code are not reused.
- **Reading choice (`tryjob.rs`):** a reading is not reused for an entry whose held set differs from the reading's
  start. An owner key that is held on one side only means a different held set.
- **Tests:** `reader/tests/read_held.rs` and `fmi-explore/tests/grow.rs`
  (`readings_without_held_code_are_not_offered_with_read_held`).

## Offline check (VM, 2026-11-05)

- **Job:** play10's j06448 (entry 9570, room 42, holding 946), with a fresh reading, switch off and then on.
- **Off:** 55 units, 1,507 records, 4 targets (`bit:446`, `var:104`, `var:52`, `var:123`).
- **On:** 63 units, 424,163 records (98 s wall, 1.1 GB peak), 12 targets. `class:491:12 = 0` is among them at tier 1,
  and so are `owner:946`, `bit:20` and `bit:444`. The try planned and reached `bit:20` and kept 8 new entries.
- **Cost:** the larger reading comes from the kit's handlers, which start global scripts 3 and 10. A room is read once
  per held set, and that reading is reused afterwards.

## Run

- **What runs:** play10's archive resumed on the VM with `env.sh` (E019's set plus `FMI_READ_HELD=1`) and 24 workers.
- **Prediction:** C6 within 60 minutes of run wall after the resume.
- **Stop rule:** if C6 has not been reached after 90 minutes of run wall, stop, then diagnose from the room-42 tries'
  targets and stops before changing anything.

## Amendment (1), 2026-11-05 15:30 UTC: the first resume ran out of disk

- **What happened:** the resume at 14:51 (24 workers) stopped after 9 tries. The reader's sink panicked on "No space
  left on device" at 1 GB free.
- **Cost of one held reading:** a room-42 reading holding 946 is 420-439k records, 62-148 s of wall, 176-2,615 CPU s and
  about 330 MB on disk (265 MB of actions, 70 MB packed). The off reading is 1,507 records.
  - Where the records come from: 138k of its 151k actions are events of global script 59, the interface script. The
    kit's use on the suit starts room 42's script 212, whose cutscene runs scripts 19 and 59.
  - What was ruled out: scripts 3, 10, 19 and 59 are already in the room's scope without the switch, so leaving out
    the globals that held objects start changes nothing. Limiting held objects to sentences with a B cut the records
    by only 1% (424k to 420k). Neither change was kept.
- **Why the disk filled:** five tries in room 42 each read the room fresh at once, because no held reading was
  registered yet, and each also wrote reach and refine extensions of about 250 MB.
- **Change, `FMI_READ_SHARE=1` (tryjob.rs):** in one process, a scope is read fresh by one try at a time. A try that
  needs a fresh reading while another try reads the scope waits for it, then takes that reading if the reuse rule
  allows. In play10, 278 of 306 room-42 model tries reused a registered reading (91%).
- **Run settings:** 12 workers. The old reading cache `play10-run` (not used by this run) is removed for space.

## C6, 2026-11-05 15:33 UTC

- **The rung:** C6 (`class:491:12 = 0`) was reached one minute after the 15:32 resume. j06574 is a model try from a
  room-42 entry holding 946; it planned the suit and kept entry 10534 at frame 13899.
- **Verification:** entry 10534 replays from boot with 0 differences (42 inputs, 21 segments).
- **Time to C6:** 1,183 minutes of run wall since play9's start, including the C5 plateau. That is 1 minute after the
  resume with the switch on, against the prediction of 60.

## Amendment (2), 2026-11-05 16:05 UTC: disk and workers

- **Disk:** free space fell from 7.8 GB to 2.5 GB in 15 minutes. After C6, room-42 entries differ on 5-8 keys, so fresh
  330 MB readings keep being made. The VM's boot disk was grown online from 30 to 60 GB.
- **Workers:** with 12 workers the CPU was 11-48% busy, so the run was cleanly stopped and resumed with 20.
  - When the stop file was removed, the old driver had not yet exited (it was waiting on its slowest tries) and
    started new tries next to the new driver for about a minute.
  - Neither driver committed anything after 15:54:29, so both were killed and one driver was started at 16:06.

## Amendment (3), 2026-11-05 16:30 UTC: a return that differs, and iMUSE in wall time

- **What stopped the run:** the 20-worker resume stopped after 2 tries on "return differs" for j06605 (entry 10686,
  room 42, frame 6459). The difference is one fact: `var:56` was −1 in the archive and 0 on the return.
  - Entry 10686 is the end of j06602, another try that planned the suit fix. From boot it differs on the same fact.
    Its parent, 8451, replays with 0 differences.
- **Cause:** `var:56` is `VAR_SOUNDRESULT`, iMUSE's answer to a sound query. The suit's cutscene (script 212) starts
  script 144, a sound command. iMUSE's sequencer advances on the MIDI driver's timer. With SDL audio off, the null
  mixer runs that timer in wall time while the game runs faster than real time, so the sequencer's state at a frame
  depends on the machine's load. The bridge already moves the speech and CD timers onto game time
  (`Sound::advanceTimers`); iMUSE was not covered.
- **Change, patch 0008 (`patches/0008-scumm-speedrun-imuse-ticks.patch`):**
  - With `SPEEDRUN_IMUSE_TICKS=1`, iMUSE detaches the drivers' timers at the first wait.
  - Each driver then gets one `on_timer` per base tempo of game time, from `Sound::advanceTimers`.
  - fmi passes the switch to its engines with `FMI_IMUSE_TICKS=1` (host.rs).
- **Check (VM, patched engine with the switch, from boot):**
  - The rung entries C1-C6 (128, 136, 144, 3412, 7987 and 10534) replay with 0 differences, so the archive stays
    valid.
  - Entry 10686 gives `var:56 = 0` both when the VM is idle and next to 24 busy cores. Its archived −1 came from
    the wall-time engine.
- **Change, `FMI_DIFFERS_DEAD=1` (archive.rs, driver.rs):** an entry whose return differs is marked dead, in the commit
  and in the log's replay when the archive opens. Dead entries are never selected again, and the run goes on instead of
  stopping. Test: `an_entry_whose_return_differs_is_dead`. Entries recorded after C6 by the wall-time engine may differ
  the same way.
