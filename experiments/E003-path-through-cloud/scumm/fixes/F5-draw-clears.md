# F5: drawObject clears the objects that share its place (switch `draw_clears`)

Fix round 1, item F5 (fix-round1.md). Rust only; model.py, reader.py and domains.py stay the frozen reference (D7).
Off by default. With it off, every golden is byte-identical (below).

## The rule as built

From o5_drawObject (script_v5.cpp:1031-1129) and the CDHD (object.cpp:1145-1150), for any SCUMM v5 game:

- A setXY sub-op whose x is not 255 moves the object to (x * 8, y * 8), stored as int16 (1108-1114). x = 255 is the
  engine's "no setXY".
- Then every other object of the room whose width, height and place equal the drawn object's goes to state 0
  (1118-1126), and the drawn object gets its state (1128). The engine compares an int16 place with a uint16 one, so
  a negative coordinate matches nothing.
- Places and sizes come from each object's CDHD (x, y, w, h bytes times 8). Loading a room resets them
  (room.cpp:195, resetRoomObjects), so a move lasts one room visit.

Facts and tables:

- `Game::object_box` (CDHD box per object, read from its OBCD block), `Game::room_objects` (objects per room
  resource), `Game::movable` (`ir/src/places.rs`): an object that a drawObject with setXY can move, with its places:
  the CDHD place first, then every constant target, sorted. A site whose object is not a constant can move any
  object of its unit's room (a global script's: any object).
- `objpos:O` (new fact family): the index of O's place in `movable[O]`, 0 = CDHD place. Written by a setXY (an
  unknown when the target is not known or not in the table), reset to 0 for the room's movable objects at a room
  load (`eng_scene_load`, and `eng_room1` without room_change).
- An object no setXY reaches has a fixed place: no fact.
- Clearing: decided when both places are known; otherwise the other object's state becomes the value set {old, 0}
  (reason "position"). No new fork.

Not built: the walk point setXY also moves (1109, 1111). Nothing in the model reads it (walkActorToObject has no
effect, getDist is a non-fact), so it has no fact. F1's cells will need the place, which `objpos:O` gives.

A limit (found in the lane's review): a setXY whose target is not a constant writes an unknown `objpos:O`, and the
clearing then compares only the places of the table (the CDHD place and the constant targets). Such a move can land
on another object's place outside the table, and that clearing is missed. MI1 has no such target (all 409 are
constants), so no MI1 number changes; a game that has one needs a "may be anywhere" place for the objects it reaches.

Code: `machine/src/machine/draw_clears.rs`, the drawObject arm and the room-load resets (`machine.rs`, `rooms.rs`,
`calls.rs`), `ir/src/places.rs`; reader: `Scope::of_with` (`fix1_writes`: `objpos:` of the scope rooms' movable
objects; a global script's draws can clear any object of a scope room), `domains::build_with` (a global script's
draws add 0 to the drawn objects' rooms' states; `objpos:O` has its places' indices), the Reader's machine takes the
scope's switches (`Reader::sync`). `check_rust.py` accepts the Rust-only switch (`RUST_SWITCHES`).

## Off: byte-identical

On the F5 tree, 2026-10-25, nice 10, load 13 to 19:

| check | result |
|---|---|
| machine golden subsets (rows 0-5 and the three switch runs) | 8 of 8 pass (372 s) |
| `check_rust_rows012` | rows 0-2 equal to results/ (109 s) |
| ir oracle (full, noblocks, synthetic dumps against Python) | pass |
| reader fast tests (room 33 golden, 880 lines, 1 to 4 threads) | 12 of 12 pass |
| kitchen reading, 3 threads, against golden | 72,258 lines, 0 differences |

## On: the new model-check row

`results/check-part1-row5-draw_clears.log` (row 5's features, the switch on; `check_rust.py --switches
draw_clears`, 2 jobs, 171 s).

| | row 5 | row 5 + draw_clears |
|---|---|---|
| precision at done | 76.7% (924/1205) | 77.2% (925/1198) |
| precision at post | 81.7% (985/1205) | 82.3% (986/1198) |
| recall, certain | 920/5880 | 921/5880 |
| recall, with possible | 3429/5880 | 3445/5880 |
| predictions in families the ground truth lacks | 0 | 629 (all `objpos:`, forest moves) |

21 of 239 moves change their score. The ones with certain predictions:

- S1#6: the miss state:324 = 1 (done 0) is gone (now possible {1}); new hits state:325, 358, 360 = 0.
- S1#12: the misses state:325, 326, 327, 360 = 1 (done 0) are gone (now possible); new hit state:358 = 0.
- S2#13: the misses state:834, 835 = 1 (done 0) are gone.
- S2#4: three certain slot predictions (two hits, one miss) become possible. The kitchen's ambient animation
  (room 41's local 200 draws 571, 572, 573 in turn) now clears the other two each frame, so the states rotate and
  one path ends "cycle" instead of "settled" (settle's cycle rule makes the varying facts unknown).

The other 17 moves change only their possible sets (state values 0 added where a draw can clear).

## On: the readings

- Room 33: equal to golden.py's file (its code draws nothing that shares a place).
- Kitchen (rooms 28, 41): the same 72,256 records, no new fork. 307 records add `state:O := 0` for the objects that
  share the drawn one's place and size (the bar's 324-329 and 357-361, the kitchen's 571-576) and drop model.py's
  "not modelled" note. The footer's first-pass record count goes from 68,953 to 68,954.

## Tests

- `machine/tests/fix_round1.rs`: the engine's place rules (x 255, int16, negative places); a frame of the bar's
  animation (local 208 draws 324: 325-329 go to 0, on only); the forest entry's setXY(0, 0) onto the CDHD place of
  objects of its size (cleared; one the oracle puts elsewhere is not; with the ground truth's families the places
  are not known and the states become {old, 0}); a room load resets every movable object of the forest.
- `reader/tests/fix_round1.rs`: room 33 unchanged; the kitchen (ignored, about 35 s on 3 threads): 72,256 records,
  exactly 307 differ, each only by added `state:` := 0 effects and its notes.

## What the design had wrong

- "the 5 misses flip": they leave the misses but do not become hits. The bar's animation scripts (locals 208, 209)
  loop; settle runs an ambient loop once and leaves its values unknown, so the model cannot say which frame shows at
  done. Precision rises because 7 wrong certain predictions go; F7 decides the frames.
- "324-327" share a box with 328 and 329 too (six objects).
- "all 409 setXY sites constant": the targets are, but one site's object is not (room 39's local 213,
  `drawObject(Local[0], setXY(100, 0))`); its target goes to every object of room 39.
- The ground truth holds no object places (facts.py has no such family; the bridge's C7 dump has `room_objects`
  with x and y, which facts_of drops). In the model check every `objpos:` read at pre is "not held", so a clearing
  that involves a movable object is a value set. In Part I's check, the forest's draws come after its room load,
  which writes every `objpos:` there, so their places are known; no forest move's score changed.
