# Mechanic inventory

Written by `fmi-checks`' `inventory` (E014, plan.md method (a)). Counts are statements (units in brackets).

| | mi-dos-cd | fate-play-demo |
|---|---:|---:|
| rooms | 83 | 11 |
| global units | 178 | 93 |
| entry units | 83 | 11 |
| exit units | 83 | 11 |
| local units | 377 | 78 |
| object units | 474 | 66 |
| instructions | 40786 | 12998 |

## Opcodes (ScummVM's v5 handler names)

| handler | mi-dos-cd | fate-play-demo | only in fate-play-demo |
|---|---:|---:|---|
| o5_actorFollowCamera | 71 (61) | 24 (14) |  |
| o5_actorFromPos | 7 (5) | 4 (3) |  |
| o5_actorOps | 847 (246) | 177 (41) |  |
| o5_add | 726 (108) | 154 (36) |  |
| o5_animateActor | 1694 (292) | 293 (63) |  |
| o5_beginOverride | 355 (97) | 26 (13) |  |
| o5_breakHere | 4625 (392) | 941 (116) |  |
| o5_chainScript | 51 (33) | 49 (31) |  |
| o5_cursorCommand | 242 (79) | 50 (24) |  |
| o5_cutscene | 315 (225) | 84 (56) |  |
| o5_debug | - | 1 (1) | yes |
| o5_decrement | 28 (23) | 7 (6) |  |
| o5_delay | 616 (155) | 45 (20) |  |
| o5_delayVariable | 41 (33) | 4 (4) |  |
| o5_divide | 8 (3) | 14 (6) |  |
| o5_doSentence | 122 (92) | 41 (26) |  |
| o5_drawBox | 21 (2) | 23 (5) |  |
| o5_drawObject | 585 (91) | 33 (16) |  |
| o5_endCutscene | 315 (225) | 84 (56) |  |
| o5_equalZero | 682 (241) | 199 (82) |  |
| o5_expression | 1032 (206) | 415 (64) |  |
| o5_faceActor | 62 (27) | 59 (25) |  |
| o5_findInventory | 1 (1) | 1 (1) |  |
| o5_findObject | 11 (10) | 11 (5) |  |
| o5_freezeScripts | 2 (2) | 8 (7) |  |
| o5_getActorCostume | 30 (20) | 3 (2) |  |
| o5_getActorElevation | 1 (1) | 1 (1) |  |
| o5_getActorFacing | 14 (13) | 6 (3) |  |
| o5_getActorMoving | 19 (18) | 9 (6) |  |
| o5_getActorRoom | 40 (23) | 20 (13) |  |
| o5_getActorScale | - | 9 (4) | yes |
| o5_getActorWalkBox | 38 (26) | 5 (3) |  |
| o5_getActorWidth | 1 (1) | 3 (2) |  |
| o5_getActorX | 101 (75) | 80 (37) |  |
| o5_getActorY | 43 (36) | 71 (31) |  |
| o5_getDist | 33 (23) | 7 (5) |  |
| o5_getObjectOwner | 142 (81) | 65 (29) |  |
| o5_getObjectState | 130 (73) | 41 (20) |  |
| o5_getRandomNr | 194 (86) | 64 (33) |  |
| o5_getVerbEntrypoint | 5 (4) | 5 (3) |  |
| o5_ifClassOfIs | 154 (83) | 199 (59) |  |
| o5_increment | 832 (181) | 203 (46) |  |
| o5_isEqual | 2146 (401) | 920 (129) |  |
| o5_isGreater | 178 (128) | 113 (46) |  |
| o5_isGreaterEqual | 55 (39) | 50 (22) |  |
| o5_isLess | 364 (203) | 129 (61) |  |
| o5_isLessEqual | 87 (66) | 42 (14) |  |
| o5_isNotEqual | 136 (72) | 82 (45) |  |
| o5_isScriptRunning | 335 (135) | 105 (43) |  |
| o5_isSoundRunning | 29 (28) | 15 (12) |  |
| o5_jumpRelative | 3196 (564) | 1180 (168) |  |
| o5_loadRoom | 183 (42) | 156 (33) |  |
| o5_loadRoomWithEgo | 157 (104) | 17 (10) |  |
| o5_matrixOps | 345 (58) | 30 (9) |  |
| o5_move | 261 (118) | 211 (60) |  |
| o5_move_segafix | 2940 (642) | 1435 (178) |  |
| o5_multiply | 11 (8) | 10 (7) |  |
| o5_notEqualZero | 758 (272) | 246 (92) |  |
| o5_panCameraTo | 37 (27) | 4 (4) |  |
| o5_pickupObject | 219 (59) | 381 (11) |  |
| o5_print | 2306 (215) | 510 (53) |  |
| o5_printEgo | 879 (366) | 228 (62) |  |
| o5_pseudoRoom | 8 (1) | 30 (1) |  |
| o5_putActor | 408 (198) | 156 (29) |  |
| o5_putActorAtObject | 37 (23) | 41 (8) |  |
| o5_putActorInRoom | 499 (238) | 194 (27) |  |
| o5_resourceRoutines | 878 (125) | 30 (11) |  |
| o5_roomOps | 179 (74) | 88 (27) |  |
| o5_saveRestoreVerbs | 23 (3) | 11 (1) |  |
| o5_setCameraAt | 20 (16) | 14 (11) |  |
| o5_setClass | 399 (159) | 252 (50) |  |
| o5_setObjectName | 79 (44) | 90 (20) |  |
| o5_setOwnerOf | 153 (48) | 108 (19) |  |
| o5_setState | 383 (109) | 180 (38) |  |
| o5_setVarRange | 15 (10) | 6 (3) |  |
| o5_soundKludge | - | 275 (39) | yes |
| o5_startObject | 90 (72) | 42 (17) |  |
| o5_startScript | 1657 (552) | 401 (114) |  |
| o5_startSound | 190 (107) | 61 (29) |  |
| o5_stopObjectCode | 2022 (1193) | 402 (259) |  |
| o5_stopScript | 319 (131) | 135 (52) |  |
| o5_stopSound | 98 (76) | 17 (10) |  |
| o5_stringOps | 460 (14) | 206 (15) |  |
| o5_subtract | 626 (73) | 174 (42) |  |
| o5_systemOps | 11 (3) | 2 (2) |  |
| o5_verbOps | 720 (70) | 335 (34) |  |
| o5_wait | 2163 (267) | 246 (52) |  |
| o5_walkActorTo | 351 (131) | 84 (37) |  |
| o5_walkActorToActor | 45 (26) | 6 (5) |  |
| o5_walkActorToObject | 95 (52) | 10 (8) |  |

## Statement heads and the machine's class (scumm-machine heads.rs)

| head | class | mi-dos-cd | fate-play-demo | only in fate-play-demo |
|---|---|---:|---:|---|
| (assignment) | write | 7642 (817) | 3153 (225) |  |
| (jump) | control | 7756 (738) | 3160 (211) |  |
| ActorOps | none | 847 (246) | 177 (41) |  |
| CopyString | none | 1 (1) | - |  |
| CreateString | none | 3 (3) | - |  |
| CursorCommand | none | 1 (1) | - |  |
| CursorHide | write | 25 (22) | 6 (6) |  |
| CursorShow | write | 49 (42) | 11 (9) |  |
| CursorSoftOff | write | 4 (4) | 1 (1) |  |
| CursorSoftOn | write | 4 (4) | 1 (1) |  |
| InitCharset | none | 71 (27) | 20 (12) |  |
| PseudoRoom | none | 8 (1) | 30 (1) |  |
| PutCodeInString | none | 125 (10) | 143 (13) |  |
| Resource.clearHeap | none | 2 (2) | 1 (1) |  |
| Resource.loadCharset | none | 6 (3) | - |  |
| Resource.loadCostume | none | 122 (61) | - |  |
| Resource.loadRoom | none | 5 (5) | - |  |
| Resource.loadScript | none | 179 (44) | - |  |
| Resource.loadSound | none | 98 (36) | 20 (9) |  |
| Resource.lockCostume | none | 67 (33) | - |  |
| Resource.lockRoom | none | 1 (1) | - |  |
| Resource.lockScript | none | 151 (32) | - |  |
| Resource.lockSound | none | 40 (17) | 4 (3) |  |
| Resource.unlockCostume | none | 68 (36) | - |  |
| Resource.unlockRoom | none | 1 (1) | - |  |
| Resource.unlockScript | none | 114 (35) | - |  |
| Resource.unlockSound | none | 24 (11) | 5 (4) |  |
| RoomIntensity | none | 13 (7) | 52 (9) |  |
| RoomScroll | write | 12 (7) | - |  |
| SetPalColor | none | 52 (11) | - |  |
| SetScreen | none | 36 (35) | 10 (9) |  |
| SetStringChar | none | 327 (6) | 58 (4) |  |
| ShakeOff | none | 3 (2) | 3 (3) |  |
| ShakeOn | none | 2 (2) | 3 (3) |  |
| UserputOff | write | 26 (23) | 3 (3) |  |
| UserputOn | write | 52 (44) | 6 (6) |  |
| UserputSoftOff | write | 5 (5) | 1 (1) |  |
| UserputSoftOn | write | 5 (5) | 1 (1) |  |
| VerbOps | write | 720 (70) | 335 (34) |  |
| WaitForActor | wait | 387 (146) | 75 (35) |  |
| WaitForCamera | wait | 44 (28) | 5 (4) |  |
| WaitForMessage | wait | 1730 (190) | 164 (36) |  |
| WaitForSentence | wait | 2 (1) | 2 (2) |  |
| actorFollowCamera | write | 71 (61) | 24 (14) |  |
| animateCostume | none | 1694 (292) | 293 (63) |  |
| beginOverride | write | 224 (96) | 18 (12) |  |
| breakHere | yield | 4625 (392) | 941 (116) |  |
| chainScript | end | 51 (33) | 49 (31) |  |
| createBoxMatrix | none | 5 (4) | 6 (5) |  |
| cutscene | write | 315 (225) | 84 (56) |  |
| debug | unmodelled (no rule) | - | 1 (1) | yes |
| delay | yield | 616 (155) | 45 (20) |  |
| delayVariable | yield | 41 (33) | 4 (4) |  |
| doSentence | deferred | 122 (92) | 41 (26) |  |
| drawBox | none | 21 (2) | 23 (5) |  |
| drawObject | write | 585 (91) | 33 (16) |  |
| endCutscene | write | 315 (225) | 84 (56) |  |
| endOverride | write | 131 (57) | 8 (6) |  |
| faceActor | none | 62 (27) | 59 (25) |  |
| freezeScripts | write | 2 (2) | 8 (7) |  |
| loadRoom | write | 183 (42) | 156 (33) |  |
| loadRoomWithEgo | write | 157 (104) | 17 (10) |  |
| loadString | unmodelled (no rule) | - | 2 (2) | yes |
| panCameraTo | none | 37 (27) | 4 (4) |  |
| pickupObject | write | 219 (59) | 381 (11) |  |
| print | write | 2306 (215) | 510 (53) |  |
| printEgo | none | 879 (366) | 228 (62) |  |
| putActor | none | 408 (198) | 156 (29) |  |
| putActorAtObject | none | 37 (23) | 41 (8) |  |
| putActorInRoom | none | 499 (238) | 194 (27) |  |
| restoreVerbs | write | 11 (2) | 7 (1) |  |
| saveLoad | unmodelled (no rule) | - | 2 (1) | yes |
| saveString | unmodelled (no rule) | - | 1 (1) | yes |
| saveVerbs | write | 12 (2) | 4 (1) |  |
| screenEffect | none | 41 (25) | 11 (7) |  |
| setBoxFlags | none | 340 (58) | 24 (9) |  |
| setCameraAt | write | 20 (16) | 14 (11) |  |
| setClass | write | 399 (159) | 252 (50) |  |
| setObjectName | write | 79 (44) | 90 (20) |  |
| setOwnerOf | write | 153 (48) | 108 (19) |  |
| setRGBRoomIntensity | none | 20 (2) | 1 (1) |  |
| setRoomShadow | unmodelled (no rule) | - | 3 (3) | yes |
| setState | write | 383 (109) | 180 (38) |  |
| setVarRange | write | 15 (10) | 6 (3) |  |
| soundKludge | unmodelled (no rule) | - | 275 (39) | yes |
| startObject | write | 90 (72) | 42 (17) |  |
| startScript | write | 1657 (552) | 401 (114) |  |
| startSound | none | 190 (107) | 61 (29) |  |
| stopObjectCode | end | 2022 (1193) | 402 (259) |  |
| stopScript | write | 319 (131) | 135 (52) |  |
| stopSound | none | 98 (76) | 17 (10) |  |
| systemOps | unmodelled (restart, pause or quit) | 11 (3) | 2 (2) |  |
| walkActorTo | none | 351 (131) | 84 (37) |  |
| walkActorToActor | none | 45 (26) | 6 (5) |  |
| walkActorToObject | none | 95 (52) | 10 (8) |  |

## Sub-ops

| op.sub-op | mi-dos-cd | fate-play-demo | only in fate-play-demo |
|---|---:|---:|---|
| ActorOps.Costume | 333 (175) | 113 (30) |  |
| ActorOps.Elevation | 78 (40) | 13 (5) |  |
| ActorOps.FollowBoxes | 43 (34) | 8 (6) |  |
| ActorOps.IgnoreBoxes | 114 (93) | 25 (14) |  |
| ActorOps.Init | 161 (111) | 31 (20) |  |
| ActorOps.InitAnimNr | 5 (4) | 6 (5) |  |
| ActorOps.Name | 31 (23) | 14 (10) |  |
| ActorOps.NeverZClip | 71 (57) | 15 (9) |  |
| ActorOps.Palette | 116 (32) | 2 (1) |  |
| ActorOps.Scale | 87 (56) | 15 (9) |  |
| ActorOps.SetZClip | 53 (47) | 12 (6) |  |
| ActorOps.Sound | 18 (10) | - |  |
| ActorOps.StandAnimNr | 11 (8) | 4 (4) |  |
| ActorOps.TalkAnimNr | 146 (18) | - |  |
| ActorOps.TalkColor | 141 (57) | 15 (8) |  |
| ActorOps.WalkAnimNr | 18 (13) | 2 (2) |  |
| ActorOps.WalkSpeed | 71 (51) | 28 (15) |  |
| ActorOps.Width | 12 (8) | - |  |
| VerbOps.BackColor | 3 (1) | 22 (3) |  |
| VerbOps.Center | 20 (1) | 1 (1) |  |
| VerbOps.Color | 58 (12) | 38 (8) |  |
| VerbOps.Dim | 3 (2) | 22 (9) |  |
| VerbOps.DimColor | 2 (1) | 4 (1) |  |
| VerbOps.HiColor | 35 (7) | 16 (3) |  |
| VerbOps.Key | 609 (48) | 115 (11) |  |
| VerbOps.New | 61 (8) | 55 (7) |  |
| VerbOps.Off | 55 (24) | 46 (12) |  |
| VerbOps.On | 605 (52) | 99 (12) |  |
| VerbOps.SetToObject | 9 (3) | 13 (3) |  |
| VerbOps.SetToString | 2 (2) | - |  |
| VerbOps.SetXY | 628 (49) | 122 (13) |  |
| VerbOps.Text | 614 (49) | 61 (11) |  |
| drawObject.setImage | - | 13 (4) | yes |
| drawObject.setXY | 409 (6) | - |  |
| print.Center | 49 (25) | 108 (11) |  |
| print.Clipped | 7 (4) | - |  |
| print.Color | 338 (65) | 195 (18) |  |
| print.Left | 57 (12) | 3 (1) |  |
| print.Overhead | 5 (5) | 6 (4) |  |
| print.Pos | 452 (63) | 288 (21) |  |
| print.Text | 2271 (210) | 506 (51) |  |
| printEgo.Center | 6 (5) | - |  |
| printEgo.Color | 3 (3) | - |  |
| printEgo.Overhead | 1 (1) | - |  |
| printEgo.Pos | 13 (7) | - |  |
| printEgo.Text | 869 (362) | 228 (62) |  |

## Getters (evaluated by the machine, or an unknown value)

| getter | evaluated | mi-dos-cd | fate-play-demo | only in fate-play-demo |
|---|---|---:|---:|---|
| GetStringChar | no | 4 (3) | 5 (3) |  |
| actorFromPos | yes | 7 (5) | 4 (3) |  |
| classOfIs | yes | 154 (83) | 199 (59) |  |
| findInventory | no | 1 (1) | 1 (1) |  |
| findObject | yes | 11 (10) | 12 (6) |  |
| getActorCostume | no | 30 (20) | 3 (2) |  |
| getActorElevation | no | 3 (3) | 1 (1) |  |
| getActorFacing | no | 15 (14) | 6 (3) |  |
| getActorMoving | no | 19 (18) | 9 (6) |  |
| getActorRoom | yes | 40 (23) | 20 (13) |  |
| getActorScale | no | - | 15 (5) | yes |
| getActorWalkBox | no | 38 (26) | 6 (4) |  |
| getActorWidth | no | 2 (1) | 9 (4) |  |
| getActorX | no | 134 (93) | 100 (39) |  |
| getActorY | no | 62 (48) | 79 (32) |  |
| getDist | no | 34 (23) | 7 (5) |  |
| getInt | no | 28 (14) | 20 (10) |  |
| getInventoryCount | yes | 1 (1) | 3 (2) |  |
| getName | no | 5 (3) | 6 (3) |  |
| getObjectOwner | yes | 142 (81) | 65 (29) |  |
| getObjectState | yes | 130 (73) | 41 (20) |  |
| getRandomNr | no | 335 (135) | 78 (35) |  |
| getString | no | 18 (11) | 16 (5) |  |
| getVerb | no | 9 (6) | 7 (2) |  |
| getVerbEntryPoint | yes | 5 (4) | 5 (3) |  |
| isScriptRunning | yes | 335 (135) | 105 (43) |  |
| isSoundRunning | no | 29 (28) | 15 (12) |  |
| keepText | no | 40 (10) | - |  |
| newline | no | 54 (13) | 12 (9) |  |
| startAnim | no | - | 71 (11) | yes |
| unknown8 | no | 10 (6) | - |  |
| wait | no | 878 (154) | 113 (31) |  |

## Statements the IR did not parse (by opcode)

| handler | mi-dos-cd | fate-play-demo |
|---|---:|---:|
| none | |

## The IR's notes (by class)

| class | mi-dos-cd | fate-play-demo |
|---|---:|---:|
| ambiguous name from bytes | 2 (2) | 3 (2) |
| else chain | 9 (8) | 4 (4) |
| else-if target from bytes, text rule agrees | 1111 (183) | 388 (80) |
| else-if target from bytes, text rule differs | 2 (2) | 1 (1) |
| empty unit | 2 (2) | - |

## systemOps sites (1 restart, 2 pause, 3 quit)

| game | unit | offset | sub-op |
|---|---|---|---:|
| mi-dos-cd | ('global', 61) | 000C | 1 |
| mi-dos-cd | ('global', 61) | 0015 | 1 |
| mi-dos-cd | ('global', 61) | 0030 | 1 |
| mi-dos-cd | ('global', 61) | 0039 | 1 |
| mi-dos-cd | ('global', 61) | 0042 | 1 |
| mi-dos-cd | ('global', 61) | 004B | 1 |
| mi-dos-cd | ('global', 61) | 0054 | 1 |
| mi-dos-cd | ('global', 61) | 005D | 1 |
| mi-dos-cd | ('global', 61) | 0066 | 1 |
| mi-dos-cd | ('global', 155) | 002E | 3 |
| mi-dos-cd | ('global', 176) | 00A0 | 3 |
| fate-play-demo | ('global', 57) | 0D0E | 1 |
| fate-play-demo | ('obj', 582) | 0067 | 1 |

## The engine: lines of engines/scumm outside the bridge that name GID_INDY4 (101)

| file | line | text |
|---|---:|---|
| engines/scumm/boxes.cpp | 183 | `if (_game.id == GID_INDY4 && _currentRoom == 225 && _roomResource == 94 && box == 8 && enhancementEnabled(kEnhMinorBugFixes))` |
| engines/scumm/charset.cpp | 125 | `_game.id == GID_INDY4 \|\| _game.id == GID_MONKEY \|\| _game.id == GID_MONKEY2 \|\|` |
| engines/scumm/charset.cpp | 1174 | `if (_vm->_game.platform == Common::kPlatformAmiga && _vm->_game.id == GID_INDY4)` |
| engines/scumm/charset.cpp | 1407 | `if (_vm->_game.platform == Common::kPlatformAmiga && _vm->_game.id == GID_INDY4) {` |
| engines/scumm/charset.cpp | 2237 | `} else if (_vm->_game.id != GID_INDY4 && _curId == 1) {` |
| engines/scumm/charset.cpp | 2256 | `const uint8 *htbl = (_vm->_game.id == GID_MONKEY) ? sjisFontHeightM1 : ((_vm->_game.id == GID_INDY4) ? sjisFontHeightI4 : sjisFontHeightM2);` |
| engines/scumm/charset.cpp | 2365 | `((_vm->_game.id == GID_INDY4) && (_curId == 2 \|\| _curId == 3 \|\| _curId == 4))) {` |
| engines/scumm/charset.cpp | 2382 | `if (((_vm->_game.id == GID_MONKEY2 && _curId != 0) \|\| (_vm->_game.id == GID_INDY4 && _curId != 3)) && (chr > 31 && chr != 94 && chr != 95 && chr != 126 && chr !` |
| engines/scumm/charset.h | 176 | `CharsetRendererClassic(ScummEngine *vm) : CharsetRendererClassic(vm, vm->_game.id == GID_INDY4 &&` |
| engines/scumm/costume.cpp | 275 | `if (_vm->_game.platform == Common::kPlatformAmiga && _vm->_game.id == GID_INDY4)` |
| engines/scumm/cursor.cpp | 1280 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4) {` |
| engines/scumm/detection.h | 233 | `GID_INDY4,` |
| engines/scumm/detection_steam.h | 34 | `{ GID_INDY4, Common::kPlatformWindows,   "atlantis.%03d", "ATLANTIS.000",  "Indiana Jones and the Fate of Atlantis.exe", 224336, 12035 },` |
| engines/scumm/detection_steam.h | 35 | `{ GID_INDY4, Common::kPlatformMacintosh, "atlantis.%03d", "ATLANTIS.000",  "The Fate of Atlantis",                       260224, 12035 },` |
| engines/scumm/detection_tables.h | 212 | `{"atlantis", "", 0, GID_INDY4,    5, 0, MDT_PCSPK \| MDT_ADLIB \| MDT_MIDI \| MDT_PREFER_MT32, 0, UNK, GUIO4(GUIO_RENDEREGA, GAMEOPTION_ENHANCEMENTS, GAMEOPTION_OR` |
| engines/scumm/detection_tables.h | 213 | `{"atlantis", "Steam", "steam", GID_INDY4,    5, 0, MDT_PCSPK \| MDT_ADLIB \| MDT_MIDI \| MDT_PREFER_MT32, 0, UNK, GUIO4(GUIO_RENDEREGA, GAMEOPTION_ENHANCEMENTS, GA` |
| engines/scumm/detection_tables.h | 214 | `{"atlantis", "Floppy", 0, GID_INDY4,    5, 0, MDT_PCSPK \| MDT_ADLIB \| MDT_MIDI \| MDT_PREFER_MT32, 0, UNK, GUIO6(GUIO_NOSPEECH, GUIO_RENDEREGA, GAMEOPTION_ENHANC` |
| engines/scumm/detection_tables.h | 215 | `{"atlantis", "Amiga", 0, GID_INDY4,  5, 0, MDT_AMIGA, 0, Common::kPlatformAmiga, GUIO6(GUIO_NOSPEECH, GUIO_MIDIAMIGA, GAMEOPTION_ENHANCEMENTS, GAMEOPTION_ORIGIN` |
| engines/scumm/detection_tables.h | 216 | `{"atlantis", "Mac", 0, GID_INDY4,    5, 0, 0, 0, UNK, GUIO4(GAMEOPTION_ENHANCEMENTS, GAMEOPTION_ORIGINALGUI, GUIO_NOASPECT, GAMEOPTION_TTS)},` |
| engines/scumm/detection_tables.h | 217 | `{"atlantis", "Mac Floppy", 0, GID_INDY4,    5, 0, 0, 0, UNK, GUIO6(GUIO_NOSPEECH, GAMEOPTION_ENHANCEMENTS, GAMEOPTION_ORIGINALGUI, GAMEOPTION_COPY_PROTECTION, G` |
| engines/scumm/detection_tables.h | 218 | `{"atlantis", "FM-TOWNS", 0, GID_INDY4,    5, 0, MDT_TOWNS \| MDT_ADLIB, 0, Common::kPlatformFMTowns, GUIO6(GUIO_MIDITOWNS, GUIO_MIDIADLIB, GAMEOPTION_TRIM_FMTOWN` |
| engines/scumm/detection_tables.h | 945 | `{ GID_INDY4, "", "indy_atlantis_rus_v1.0_cd.exe" }, // CD` |
| engines/scumm/dialogs.cpp | 1485 | `if (gameId == GID_MONKEY \|\| gameId == GID_MONKEY2 \|\| (gameId == GID_INDY4 && extra == "Floppy"))` |
| engines/scumm/gfx.cpp | 1219 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4) {` |
| engines/scumm/gfx.cpp | 1459 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4) {` |
| engines/scumm/gfx.cpp | 1527 | `\|\| (color >= 254 && _game.platform == Common::kPlatformFMTowns && (_game.id == GID_MONKEY2 \|\| _game.id == GID_INDY4))` |
| engines/scumm/gfx.cpp | 2479 | `if (_vm->_game.platform == Common::kPlatformAmiga && _vm->_game.id == GID_INDY4) {` |
| engines/scumm/gfx_gui.cpp | 2243 | `case GID_INDY4:` |
| engines/scumm/help.cpp | 37 | `case GID_INDY4:` |
| engines/scumm/help.cpp | 208 | `case GID_INDY4:` |
| engines/scumm/help.cpp | 295 | `if (gameId == GID_INDY4) {` |
| engines/scumm/help.cpp | 307 | `case GID_INDY4:` |
| engines/scumm/help.cpp | 318 | `if (gameId == GID_INDY4) {` |
| engines/scumm/imuse/drivers/macintosh.cpp | 1312 | `case GID_INDY4:` |
| engines/scumm/imuse/imuse.cpp | 51 | `_dynamicChanAllocation(vm && vm->_game.id != GID_MONKEY2 && vm->_game.id != GID_INDY4), // For the non-iMuse games that (unfortunately) run on this player we ne` |
| engines/scumm/imuse/imuse_part.cpp | 445 | `if (_se->_soundType == MDT_MACINTOSH && (_se->_game_id == GID_MONKEY2 \|\| _se->_game_id == GID_INDY4))` |
| engines/scumm/input.cpp | 976 | `(_game.id == GID_MONKEY \|\| _game.id == GID_MONKEY2 \|\| _game.id == GID_INDY4);` |
| engines/scumm/input.cpp | 1093 | `(_game.id == GID_INDY4 && strcmp(_game.variant, "Floppy") && strcmp(_game.variant, "Amiga")))) &&` |
| engines/scumm/macgui/macgui.cpp | 49 | `case GID_INDY4:` |
| engines/scumm/macgui/macgui_impl.cpp | 363 | `if (_vm->_game.id == GID_INDY4 && !string.contains("Smooth Graphics") && id == 3) {` |
| engines/scumm/macgui/macgui_strings.cpp | 711 | `} else if (_vm->_game.id == GID_INDY4) {` |
| engines/scumm/macgui/macgui_v5.cpp | 238 | `case GID_INDY4:` |
| engines/scumm/macgui/macgui_v6.cpp | 73 | `else if (_vm->_game.id == GID_INDY4)` |
| engines/scumm/macgui/macgui_v6.cpp | 134 | `if (_vm->_game.id == GID_INDY4) {` |
| engines/scumm/macgui/macgui_v6.cpp | 219 | `if (_vm->_game.id == GID_INDY4) {` |
| engines/scumm/macgui/macgui_v6.cpp | 258 | `if (_vm->_game.id == GID_INDY4 && id >= 501 && id < 600)` |
| engines/scumm/macgui/macgui_v6.cpp | 1024 | `if (_vm->_game.id == GID_TENTACLE \|\| _vm->_game.id == GID_INDY4) {` |
| engines/scumm/macgui/macgui_v6.cpp | 1117 | `} else if (_vm->_game.id == GID_TENTACLE \|\| _vm->_game.id == GID_INDY4) {` |
| engines/scumm/metaengine.cpp | 683 | `widget = new Scumm::MacGameOptionsWidget(boss, name, target, GID_INDY4, extra);` |
| engines/scumm/palette.cpp | 254 | `if (_game.id == GID_INDY4 \|\| _game.id == GID_MONKEY2)` |
| engines/scumm/palette.cpp | 638 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4) {` |
| engines/scumm/palette.cpp | 649 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4) {` |
| engines/scumm/palette.cpp | 666 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4) {` |
| engines/scumm/palette.cpp | 678 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4) {` |
| engines/scumm/palette.cpp | 792 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4) {` |
| engines/scumm/palette.cpp | 834 | `if ((_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4) \|\| _enableEGADithering)` |
| engines/scumm/palette.cpp | 969 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4)` |
| engines/scumm/palette.cpp | 1053 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4) {` |
| engines/scumm/palette.cpp | 1426 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4) {` |
| engines/scumm/palette.cpp | 1484 | `} else if (_game.id == GID_INDY4 && _game.platform == Common::kPlatformAmiga) {` |
| engines/scumm/palette.cpp | 1665 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4) {` |
| engines/scumm/resource.cpp | 575 | `if (_game.id == GID_INDY4 && no == 0)` |
| engines/scumm/saveload.cpp | 1915 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4) {` |
| engines/scumm/saveload.cpp | 2009 | `if (_game.id == GID_INDY4)` |
| engines/scumm/script.cpp | 1147 | `if (owner == OF_OWNER_ROOM && _game.id == GID_INDY4 && 336 <= obj && obj <= 340)` |
| engines/scumm/script_v5.cpp | 1822 | `if (_game.id == GID_INDY4 && room == 1 && _objectOwnerTable[23] == 1 && enhancementEnabled(kEnhMinorBugFixes)) {` |
| engines/scumm/script_v5.cpp | 2571 | `if (_game.id == GID_INDY4) {` |
| engines/scumm/script_v5.cpp | 2600 | `if (_game.id == GID_INDY4) {` |
| engines/scumm/script_v5.cpp | 3023 | `if (_game.id == GID_INDY4 && script == 164 && _roomResource == 50 &&` |
| engines/scumm/script_v5.cpp | 3345 | `if (_game.id == GID_INDY4 && currentScriptSlotIs(kScriptNumENCD) &&` |
| engines/scumm/script_v5.cpp | 3612 | `} else if (_game.id == GID_INDY4 && _roomResource == 23 && currentScriptSlotIs(167) &&` |
| engines/scumm/script_v5.cpp | 3628 | `} else if (_game.id == GID_INDY4 && _language == Common::EN_ANY && _roomResource == 10 &&` |
| engines/scumm/script_v5.cpp | 3646 | `} else if (_game.id == GID_INDY4 && currentScriptSlotIs(161) && _actorToPrintStrFor == 2 &&` |
| engines/scumm/scumm.cpp | 312 | `GID_INDY4,			// Not supported in all versions of the original interpreter.` |
| engines/scumm/scumm.cpp | 460 | `_isIndy4Jap = _game.id == GID_INDY4 &&` |
| engines/scumm/scumm.cpp | 1066 | `case GID_INDY4 :` |
| engines/scumm/scumm.cpp | 1190 | `case GID_INDY4:` |
| engines/scumm/scumm.cpp | 1291 | `{ GID_INDY4,    "Fate of Atlantis"         },` |
| engines/scumm/scumm.cpp | 1292 | `{ GID_INDY4,    "Fate of Atlantis 1.1"     },` |
| engines/scumm/scumm.cpp | 1293 | `{ GID_INDY4,    "Indy Fate"                },` |
| engines/scumm/scumm.cpp | 1294 | `{ GID_INDY4,    "fate v1.5"                },` |
| engines/scumm/scumm.cpp | 1295 | `{ GID_INDY4,    "Indy 12/15/92"            },` |
| engines/scumm/scumm.cpp | 1296 | `{ GID_INDY4,    "Indy 12-15-92"            },` |
| engines/scumm/scumm.cpp | 1297 | `{ GID_INDY4,    "Fate of Atlantis v1.5"    },` |
| engines/scumm/scumm.cpp | 1298 | `{ GID_INDY4,    "Fate of Atlantis v.1.5"   },` |
| engines/scumm/scumm.cpp | 1299 | `{ GID_INDY4,    "Fate of Atlantis PowerPC" },` |
| engines/scumm/scumm.cpp | 1300 | `{ GID_INDY4,    "Indy Demo"                },` |
| engines/scumm/scumm.cpp | 1351 | `else if (_game.id == GID_INDY4)` |
| engines/scumm/scumm.cpp | 1376 | `if (_game.id == GID_LOOM \|\| _game.id == GID_TENTACLE \|\| _game.id == GID_SAMNMAX \|\| _game.id == GID_INDY4) {` |
| engines/scumm/scumm.cpp | 1747 | `if (!_copyProtection && _game.id == GID_INDY4 && _bootParam == 0) {` |
| engines/scumm/scumm.cpp | 2019 | `if (_game.platform == Common::kPlatformAmiga && _game.id == GID_INDY4 && !_verbPalette)` |
| engines/scumm/scumm.cpp | 2435 | `if (_game.platform == Common::kPlatformMacintosh && (_game.id == GID_MONKEY2 \|\| _game.id == GID_INDY4)) {` |
| engines/scumm/scumm.cpp | 3921 | `bool processIQPoints = (_game.id == GID_INDY4 && (_saveLoadFlag == 2 \|\| _loadFromLauncher));` |
| engines/scumm/sound.cpp | 779 | `if (mode == 2 && _vm->_game.id == GID_INDY4 && offset == 0x76ccbd4 && _vm->enhancementEnabled(kEnhGameBreakingBugFixes))` |
| engines/scumm/string.cpp | 705 | `if ((_game.id == GID_INDY4 && ltext[ll] == 0x7F) \|\| (_game.id == GID_MONKEY2 && ltext[ll] == 0x07)) {` |
| engines/scumm/string.cpp | 794 | `if (_game.id == GID_INDY4 && ltext[0] == 0x7F) {` |
| engines/scumm/string.cpp | 1453 | `if (_game.id != GID_INDY4 \|\| buf[0] == 127) {` |
| engines/scumm/string.cpp | 1454 | `if (_game.id == GID_INDY4)` |
| engines/scumm/string.cpp | 1726 | `if (_game.id == GID_INDY4 && _language == Common::DE_DEU && _currentRoom == 28 &&` |
| engines/scumm/verbs.cpp | 621 | `if ((_game.id == GID_INDY4 \|\| _game.id == GID_PASS) && _mouseAndKeyboardStat >= '0' && _mouseAndKeyboardStat <= '9') {` |
| engines/scumm/verbs.cpp | 1204 | `((_game.platform == Common::kPlatformFMTowns) && (_game.id == GID_MONKEY2 \|\| _game.id == GID_INDY4) && (vs->bkcolor == _townsOverrideShadowColor)) ? 0 :` |

## The bridge: lines that use its verb id constants (7)

| file | line | text |
|---|---:|---|
| engines/scumm/speedrun/speedrun_plan.cpp | 42 | `const int kDialogVerbFirst = 120, kDialogVerbLast = 128;` |
| engines/scumm/speedrun/speedrun_plan.cpp | 439 | `if (id < kDialogVerbFirst \|\| id > kDialogVerbLast \|\| !verbVisible(slot) \|\| !verbText(slot, true, text))` |
| engines/scumm/speedrun/speedrun_plan.cpp | 473 | `for (int id = kDialogVerbFirst; id <= kDialogVerbLast; id++) {` |
| engines/scumm/speedrun/speedrun_state.cpp | 60 | `const int kActionVerbFirst = 2, kActionVerbLast = 10;` |
| engines/scumm/speedrun/speedrun_state.cpp | 61 | `const int kDialogVerbFirst = 120, kDialogVerbLast = 128;` |
| engines/scumm/speedrun/speedrun_state.cpp | 346 | `if (anyVisibleVerb(kDialogVerbFirst, kDialogVerbLast)) {` |
| engines/scumm/speedrun/speedrun_state.cpp | 412 | `if (anyVisibleVerb(kActionVerbFirst, kActionVerbLast)) {` |
