# E014 (a): the mechanic inventory, before any run on the demo

Made by `fmi-checks`' `inventory` (`experiments/E011-speed/rs/fmi-checks/src/bin/inventory.rs`) on 2026-11-03, before
any run of the bridge, the harness or the explorer on the demo:

    inventory --engine-src /var/tmp/fast-mi/svm-e012-end/third_party/scummvm \
      --game mi-dos-cd=<repo>/data/scripts-dos-cd \
      --game fate-demo=/var/tmp/fast-mi/e014-root/data/scripts-fate-demo --out DIR

The demo's extraction is `scripts/dump-scripts.sh` with E014's settings (`SCUMMRP_GAME=atlantis INDEX_NAME=FATE.000
RELEASE=fate-demo`, `OUT_ROOT=/var/tmp/fast-mi/e014-root`): 3 rooms, 72 global scripts, 41 local, 3 entry and 3 exit
scripts, 34 objects (17 with verb code); descumm decoded every script (0 errors, 0 warnings). The same command with
the settings unset gives MI's extraction byte for byte (index.json, every decompiled file, the blocks).

`inventory.md` and `inventory.json` hold the tables. No game text is in them: units by key, statements by their
engine names, notes by class.

## What the demo uses that the reader has never met

| what | demo | MI | the reader's model |
|---|---:|---:|---|
| `print` and `printEgo` with a speech code: descumm prints the code as `sound(0x.., 0x..)` with hex numbers | 146 statements in 29 units | 0 | the IR does not parse them; the machine ends any path there ("unparsed") |
| `soundKludge` (opcode 0x4C/0xCC: iMUSE commands) | 102 in 17 units | 0 | no rule in `heads.rs`: a path ends there ("unmodelled") |
| `saveString`, `loadString` (roomOps 13 and 14: the engine writes and reads a file `TARGET.iq` in the save folder for this game id) | 1 and 2 | 0 | no rule: unmodelled |
| `saveLoad` (roomOps 9: the engine saves, then loads, a temporary state in slot 99 of the save folder) | 2 in 1 unit | 0 | no rule: unmodelled |
| `getActorScale` | 7 in 2 units | 0 | an unknown value (no rule), as for most actor getters |

Every other opcode, statement head and sub-op the demo uses, MI's scripts use too.

## The demo's end sites

One `systemOps` site: sub-op 1 (restart) in room 62's local script 206 (offset 0AEF). No quit site. MI has 9 restart
sites in global 61 and quits in globals 155 and 176.

## The engine for this game id

101 lines of the engine's SCUMM sources (outside the bridge) name `GID_INDY4`. The ones a DOS talkie run can take:

- `scumm.cpp:1747`: with copy protection off and boot param 0, the engine sets boot param -7873 (the game's own skip
  of its copy protection);
- `script_v5.cpp:2571-2610` (roomOps 13, 14): the save and load of `TARGET.iq` in the save folder (above);
- `verbs.cpp:621`: number keys '1'-'9' are remapped to PC scan codes before they reach the input script;
- `string.cpp:705, 794, 1453`: text with byte 0x7F;
- `input.cpp:976, 1093`: keyboard handling for this game id;
- the others are other platforms (Amiga, FM-Towns, Mac), other rooms of the full game, or the IQ points after a load.

## The bridge's verb ids

`speedrun_state.cpp:60-61` (2-10 for a sentence, 120-128 for a dialog), used at `:346` and `:412`, and the E003 plan
player's `speedrun_plan.cpp:42, 439, 473`.

## The second game: the interactive demo (2026-11-03)

`fate-play-demo/inventory.md` and `inventory.json`: the same tool on the interactive demo ("Interactive Demonstration
v2", ScummVM id `atlantis`, Demo/DOS/English, SCUMM v5; files PLAYFATE.000, PLAYFATE.001), made before any run of the
bridge, the harness or the explorer on it:

    inventory --engine-src /var/tmp/fast-mi/svm-e012-end/third_party/scummvm \
      --game mi-dos-cd=<repo>/data/scripts-dos-cd \
      --game fate-play-demo=/var/tmp/fast-mi/e014-root/data/scripts-fate-play-demo --out DIR

Its extraction is `scripts/dump-scripts.sh` with `SCUMMRP_GAME=atlantis INDEX_NAME=PLAYFATE.000
RELEASE=fate-play-demo OUT_ROOT=/var/tmp/fast-mi/e014-root GAME_DIR=/var/tmp/fast-mi/games/fate-play-demo` (no
`GAME_TITLE`): 11 rooms in the files (the index names 97), 93 global scripts, 78 local, 11 entry and 11 exit scripts,
224 objects (66 with verb code, 227 verb entry points); descumm decoded every script (417 files, 0 errors, 0
warnings). 12,998 instructions, twice the talkie demo's.

### What it uses that the reader has never met

| what | play demo | MI | the reader's model |
|---|---:|---:|---|
| `soundKludge` (opcode 0x4C/0xCC: iMUSE commands; the engine queues them and stores a result in `VAR_SOUNDRESULT`) | 275 in 39 units | 0 | no rule: a path ends there ("unmodelled") |
| `saveString`, `loadString` (roomOps 13, 14: a file `TARGET.iq` in the save folder) | 1 and 2 | 0 | no rule |
| `saveLoad` (roomOps 9: a temporary state in save slot 99) | 2 in 1 unit | 0 | no rule |
| `setRoomShadow` (roomOps 12: the shadow palette) | 3 in 3 units | 0 | no rule |
| `debug` (opcode 0x6B) | 1 in 1 unit | 0 | no rule |
| `getActorScale` | 9 in 4 units | 0 | an unknown value |
| `drawObject` with `setImage` (the object's state from the argument) | 13 in 4 units | 0 | modelled (the machine reads `setImage`'s state) |

Unlike the talkie demo, no `print` holds a speech code: the IR parses every statement of this demo. Every other
opcode, statement head and sub-op it uses, MI's scripts use too.

### Its end sites

Two `systemOps` sites, both sub-op 1 (restart): global script 57 at offset 0D0E, and object 582's verb code at
offset 0067. No quit site.
