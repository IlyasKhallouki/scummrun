# F9: an actor's room as a fact (switch `actor_rooms`)

Fix round 1, item F9 (fix-round1.md). Rust only; model.py, reader.py and domains.py stay the frozen reference (D7).
Off by default. With it off, every golden is byte-identical (below).

## The rule as built

From the engine, for any SCUMM v5 game:

- An actor's room is `Actor::_room`, a byte. o5_putActorInRoom writes it (script_v5.cpp:2160-2196; room 0 also
  puts the actor at 0, 0). o5_loadRoomWithEgo writes the ego's before its startScene (1857-1866); startScene's
  putActor of the ego writes the same room again (room.cpp:230). No other v5 opcode changes it: actorOps' Init
  is initActor(0), which keeps the room (actor.cpp:160-192).
- o5_getActorRoom reads it, and gives 0 for an actor number that is no actor (script_v5.cpp:1289-1322,
  isValidActor actor.cpp:2277-2279).
- actorFollowCamera tests it against the current room and, when they differ, loads the actor's room
  (setCameraFollows, camera.cpp:62-70). An actor number that is no actor is derefActor's error()
  (actor.cpp:2281-2293), in putActorInRoom and actorFollowCamera too.

Facts:

- `actorroom:A` (new family; not `room:A`, which model.py's `family` would put in the ground truth's `room`
  family). The machine writes it at the two opcodes, as a byte. An actor that is not known makes an unknown-target
  write, as model.py does for other families.
- getActorRoom reads the fact. With the actor not known it is model.py's non-fact, as with the switch off.
- actorFollowCamera: with the actor's room and the current room both known, the test is decided: no fork, and the
  room change goes to the actor's room. With the actor's room not known (not held, or the reader's non-fact) it is
  model.py's fork on "room of actor A", unchanged. Its reads never fork (`Hooks::read_soft`; the machine's read is
  the plain read, the reader's is lazy).

The reader (planner side):

- `Scope::of_with` writes `actorroom:A` of every actor a putActorInRoom or loadRoomWithEgo of the scope can reach
  (`scope::actor_room_writes`). An actor operand is a constant or a variable; a variable the scope does not write
  has its start value; any other operand reaches every actor.
- The FactTable gives `actorroom:A` a domain from the scope's own code: the start's value, and every room those
  writes give, as bytes. An operand is a constant, a setting, the start's value of a variable the scope does not
  write, or the values of its domain (VAR_ROOM's are the `room` domain's). An actor that a write whose room is not
  worked out can reach stays a non-fact.
- No start holds an actor's room today (facts.py has no such family). The start's value is then NOT_KNOWN (-1).
  The Reader reads NOT_KNOWN as getActorRoom's non-fact, named as with the switch off, so the frame's memory of
  non-fact tests works as before. `task_start` gives NOT_KNOWN to an actor room the start lacks.

Code: `machine/src/machine/actor_rooms.rs`, the getter, putActorInRoom, loadRoomWithEgo and actorFollowCamera arms
(`machine.rs`), `Hooks::read_soft` (`hooks.rs`), `Fact::ActorRoom` (`ir/src/facts.rs`), the oracle's family
(`oracle.rs`); reader: `scope::fix1_writes` and `scope::actor_room_writes`, `table::actor_room_domains`, `NOT_KNOWN`,
the Reader's `read` and `read_soft` hooks (`machine.rs`), `task::task_start`. `check_rust.py` accepts the switch.

Not built:

- room.cpp:257-261: after the entry script of a loadRoomWithEgo, an ego that nothing positioned goes back to the
  current room. The model keeps an entry script's putActorInRoom of the ego. In MI1 the four entry scripts that move
  the ego put it in their own room, so this changes nothing there.
- putActorInRoom's stopTalk for a talking actor that leaves the current room (2165-2167), as with the switch off.
- script_v5.cpp:2990-3005 (an actor put in room 39 by o5_startScript) is an enhancement for the floppy VGA release only.
- The game's static analyses (ir/src/analysis.rs: loop classes, non-fact variables) still read getActorRoom as a
  non-fact. A waiting loop whose exit tests only an actor's room stays a "time" loop for settle; at run time its test
  is decided when the room is known. About 13 getActorRoom sites in 7 units sit in waiting loops (a text count).

## Off: byte-identical

On the F9 tree, 2026-10-26, nice 10, load up to 14:

| check | result |
|---|---|
| machine golden subsets (rows 0-5 and the three switch runs) | 8 of 8 pass (325 s) |
| `check_rust_rows012` | rows 0-2 equal to results/ (77 s) |
| ir tests (facts, oracle) | 6 of 6 pass |
| reader fast tests (room 33 golden, 880 lines, 1 to 4 threads) | 12 of 12 pass |
| kitchen reading, 3 threads, against golden | 72,258 lines, 0 differences |

## On: the new model-check rows

`results/check-part1-row5-actor_rooms.log` and `results/check-part1-row5-actor_rooms-draw_clears.log` (row 5's
features; `check_rust.py --switches actor_rooms` and `--switches actor_rooms,draw_clears`, 2 jobs, 206 s and 285 s).

| | row 5 | + actor_rooms | + draw_clears | + both |
|---|---|---|---|---|
| precision at done | 76.7% (924/1205) | 76.3% (900/1179) | 77.2% (925/1198) | 76.9% (902/1173) |
| precision at post | 81.7% (985/1205) | 81.5% (961/1179) | 82.3% (986/1198) | 82.1% (963/1173) |
| recall, certain | 920/5880 | 896/5880 | 921/5880 | 898/5880 |
| recall, with possible | 3429/5880 | 3481/5880 | 3445/5880 | 3495/5880 |
| paths, all moves | 9909 | 8834 | 9899 | 8832 |
| moves at the path limit | 11 | 9 | 11 | 9 |
| Esc paths resting on a fork on an actor's room | 32 | 30 | 32 | 30 |
| predictions in families the ground truth lacks | 0 | 395 | 629 | 1024 |

31 of 239 moves change their paths. 5 change their certain predictions:

- S1#11: a new hit, local:211 = 0. Local 216 tests the cook's room after a write in the move.
- S2#23, S2#31, S2#37 (a fight's first choice): all 6, 6 and 7 certain predictions go (all were hits). S2#28 (a
  fight's last answer): 20 certain predictions become 12 (6 hits and both misses go).

Why the fights lose: at a fight's end, global 114 does putActorInRoom(VAR_EGO, 85) and actorFollowCamera(VAR_EGO)
in room 49 ([006E]-[0079]). The engine always changes to room 85 there. Off, model.py forks: one path stays in 49,
which the engine cannot take, and one goes to an unknown room, where no room code runs. On, every such path goes to
85, and settle runs room 85's code: its wandering pirate (local 202) places itself in a loop on getDist, which ends
as "spin" (the same state again with no yield), and meets the ego, which starts the next fight. In S2#23, S2#31 and
S2#37 the fight goes on in run B (room 49, scripts 73 and 114 still live at done); settle ended the fight script
inside the move, and the lost hits came from the impossible stay-in-49 paths. In S2#28 the fight does end (room 85
at done); there the spin paths stop inside room 85's entry, before global 114 ends, and other paths start the next
fight, so 8 predictions that held off become possible. F7 (no settle) and the spin rule on loops that draw random
numbers are where this is fixed; F9 is right on the engine here.

The other 26 moves change only paths and possible sets. In the bar (S1#5 to S1#13) local 216 and 217 test the
cook's room after local 211 or 216 wrote it, so the cook's scripts stop recalling each other: S1#7 has 66 paths
instead of 636. S2#10, S2#35 and S2#40 go through room 85 as above.

With F5 on as well, S1#12 also gains a hit (state:324 = 0).

## On: the readings

- Room 33: equal to golden.py's file. Its code writes actors' rooms, but nothing it reads is known.
- Kitchen (rooms 28, 41): 72,328 records, 72 more (3 threads, 14 s). Per label, the records that do not know an
  actor's room, with every actorroom key taken out, are golden.py's. The 72 new records come from local 216's test
  of the cook's room ([0004], reached from the events of locals 211, 212, 216, 217 and script 2): 27 hold the cook in
  room 0, 18 in 28, 27 in 72. 86 records of the old ones add an actorroom write.
- The same file on 1 and 3 threads (173 s and 14 s).
- The kitchen task with the switch (s17, the oracle's goal; taskcore's defaults; Fast Downward's lama-first): the
  same 10-step plan as with the switch off, by labels, in 39 s each.
- Domains on the kitchen: the cook (6) {not known, 0, 28, 72}; the ego (1) {not known, 28, 33, 41, 79, 81, 82, 85};
  actor 3 {not known, 0, 28}; actors no write reaches are constants.

## Tests

- `machine/tests/fix_round1.rs`: local 203's four paths each hold `actorroom:6` (not held), off they hold
  getActorRoom's non-fact, and with the room known the room test is decided (28: the x test, 0: the wait); local
  211 writes the cook's room and local 216 reads it back; actorFollowCamera decided with the ego's room known, and
  the same fork as off when not held; the fight's end changes to room 85 on one path (off: two); getActorRoom of an
  actor not known equals off; loadRoomWithEgo writes the ego's pseudo room 218.
- `reader/tests/fix_round1.rs`: room 33 unchanged; the kitchen's actor room domains; bar local 203 run from its
  start: 67 records, each holding `actorroom:6` (not known, 0, 28 or 72), the room test a check only when not known;
  the kitchen reading (ignored, about 100 s on 3 threads) as above; the kitchen task (ignored): the cook's room is
  a task fact with NOT_KNOWN in its domain and at the start, and taskcore writes the full and the pruned task.

## What the design had wrong

- "the cook's four are (room-028-bar entry [0076], local-205 [0017], local-211 [0000], local-216 [000B])": global 120
  ([00D9]) also puts the cook in room 72, and it is in the kitchen's scope.
- "New rows for the 32 paths": the ground truth holds no actor rooms (facts.py has no such family; the bridge dumps
  only the visible actors of the current room). At a move's start every actor's room is "not held", so 30 of the 32
  Esc paths fork as before. Only a room the move itself writes is known.
- "local 203's three paths hold room:6": they do in the machine (four paths: WaitForActor forks too) and in the
  reader when local 203 runs. But the kitchen reading never runs local 203: a click goes to the sentence script, not
  to the room's input script, until F1.
- The fight moves' losses were not foreseen (above).

## Gaps for later items

- No start holds an actor's room, so the planner starts at NOT_KNOWN and learns a room only from a planned write.
  A bridge dump of every actor's room (as F10 dumps locals) would give it at every replan.
- replay.py's UNOBSERVED lists `args:` and `saved:` only. Before a replay with `draw_clears` or `actor_rooms`,
  `objpos:` and `actorroom:` must join it: the state dump holds neither.
