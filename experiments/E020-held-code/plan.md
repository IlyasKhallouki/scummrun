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
