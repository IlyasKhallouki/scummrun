# A room change in a havoc closure names the rooms it leaves and loads (`havoc_rooms`)

E005's change (A). The rule and its proof come first, for any SCUMM v5 game. MI1 numbers appear only in the tests and
the notes. Off by default; it needs `havoc_cut` (experiments/E004-fast-exact/havoc-cut.md).

## What it replaces

havoc-cut.md: "when a reachable instruction can change the room, the entry and exit code of every scope room, and the
room hooks". Local scripts started by a global script or an object's code, drawObject's room and the slots a change
kills then also range over every scope room. In E004's rough reading of Part I that put room 85's exit code in every
closure that could change the room, and through it globals 17 and 23.

## The rule

`R0` is the set of rooms the cut path may stand in: the room it wrote, else the set its precondition gives `room`,
else every scope room. Each room change the closure holds has a set of target rooms `T`:

- `loadRoom(R)` and `loadRoomWithEgo(O, R)`: the values of `R`: its constants; for a variable, its values by the
  domain convention, as havoc_starts reads them (havoc-cut.md, "The switch havoc_starts"); for a local, its values by
  havoc_args's analysis when that switch is on (havoc-args.md).
- `actorFollowCamera(A)`: for each actor `A` can be (its constants, or a variable's values as above), the actor's room
  at the cut, and every room an instruction of the closure can put that actor in: `putActorInRoom(A2, R)` where `A2`
  can be the actor, and `loadRoomWithEgo(O, R)` where the actor can be the ego. The actor's room at the cut is the
  path's write of `actorroom:A`, else its precondition's set, else the start's value when no code of the scope writes
  it.
- A pending step of the cut's startScene (havoc-cut.md, "the engine steps still pending"): the room the step holds
  (`Scene`, `SceneBegin`, `SceneLoad`, `Room1`) when it is a number. A pending entry step (`RoomCode` entry) runs the
  entry code of the room the path stands in: `R0`.
- Anything else (a room that is not known, an actor's room at the cut that is not known): every scope room, counted
  (`havoc_rooms: room changes to every scope room`) and given in READER_TRACE's havoc line with its reason.

Only the scope's rooms are read, as everywhere in the reading: a target room outside the scope adds no code, as
before.

The closure's rooms are `R* = R0 ∪` every `T`. When the closure holds a room change:

- the exit code of each room of `R*` (a room it loads can be left again by a later change), and the entry code of each
  room of every `T` (and of `R0` with a pending entry step); the room hooks, as before;
- a local script that a global script or an object's code starts: that number's script in each room of `R*`; one that
  room code starts: its own room's, since room code runs only in its room;
- drawObject in a global script's or an object's code: the objects of the rooms of `R*`;
- F5's `objpos:` from loading a room: the movable objects of the rooms of every `T`;
- the slots a room change kills: the slots of the room code (local scripts, entry, exit) of the rooms of `R*`; object
  slots as before.

When the closure names every unit, every scope room, as before. With the switch off, `closure` is as before, byte for
byte.

## Why the record still covers every engine run of the path

havoc-cut.md's proof needs item 1 (every instruction the continuation `e` runs is in `C(p)`) and the slot rule of item
4. Both hold with the narrower rooms:

1. **Where the room changes.** In v5 the room changes only in `startScene` (room.cpp:42). Its callers in a v5 run are
   `o5_loadRoom` (script_v5.cpp:1850, the operand's room), `o5_loadRoomWithEgo` (1875: `a->putActor(room)` at 1866 and
   then `startScene(a->_room, ...)`, the operand's room), and `setCameraFollows` (camera.cpp:69-70) when the followed
   actor is not in the current room, reached from `o5_actorFollowCamera` (script_v5.cpp:413) and from
   `o5_loadRoomWithEgo` (1895, after its own `startScene`, when the ego is already in the current room). The other
   callers are the save menu and the debugger (scumm.cpp:3826, 3900; debugger.cpp:373), which are not inputs of the
   model.
2. **What a change runs.** `startScene(room)` runs `runExitScript` (script.cpp:989-1020): the exit hooks and the exit
   code of the room loaded now (`_EXCD_offs`). It kills the room's and the local scripts (`killScriptsAndResources`),
   loads `room` and runs `runEntryScript` (1040-1058): the entry hooks and `room`'s entry code. So every exit code a
   continuation runs belongs to a room that is current at some point of it: a room the path stands in, which is in
   `R0`, or one a change of the continuation loaded. Every entry code it runs belongs to a room a change loaded, or to
   the current room when the cut stands inside a `startScene` after its load, which is the pending entry step's case.
3. **The rooms a change loads.** A change of the continuation comes from an instruction of `C(p)` or from a pending
   step. `loadRoom`'s and `loadRoomWithEgo`'s room is their operand's value, which `T` holds: a constant, a variable's
   value by the domain convention havoc_cut already rests on (havoc-cut.md, item 3), or a local's value by havoc_args's
   lemma. `actorFollowCamera` loads the actor's room. In v5 only `o5_putActorInRoom` (script_v5.cpp:2160-2196) and
   `o5_loadRoomWithEgo` for the ego (1866) set an actor's room (actor_rooms.rs, F9: actorOps' Init keeps it). So when
   the instruction runs, the actor's room is its room at the cut or one an instruction of the continuation put it in,
   and those instructions are in `C(p)`. A pending step holds its room.
4. **The code that depends on the room.** A local script number names the current room's script (`runScript` reads
   the room's LSCR), and the current room is always in `R*`; room code runs only while its room is current. drawObject
   acts on the room it runs in (script_v5.cpp:1101-1130), which is in `R*`. A room's objects are put back when that room
   is loaded (room.cpp:195), so only rooms of a `T`. A change kills the slots of the room being left, which is in
   `R*`; object slots are named as before.

So each instruction `e` runs is in the narrower `C(p)`, and items 2 to 5 of havoc-cut.md's proof read only `C(p)` and
the slots above. A fallback to every scope room is the old rule, so it covers what the old rule covered.

## Checks

1. Off: the kitchen's packed sha1 279b7a24; the reader's tests.
2. Tests (`tests/havoc_rooms.rs`): a cut in a global script that loads a constant room and follows the ego after
   putting it in a constant room names those rooms' entry and exit code and the path's room's exit, and not the other
   scope rooms; with the switch off, every scope room; a room change whose room is not known names every scope room.
3. The soundness check of plan.md: the bar and the kitchen, rough at N = 100 with havoc_rooms and havoc_args, reach
   every value their full readings reach.
