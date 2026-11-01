# Part I: cutscene skips, text skips and var 19

This note is the script half of Task 8.1 (`docs/plan.md` Phase 8). It lists every cutscene and override that the current optimal route runs, and every one in the alternatives of `docs/part1/model.md` §5. For each it says what Esc (cutscene skip) and `.` (text skip) change. It is input for Task 8.2, which implements the skips and the skip-safety harness, and for Task 8.1b.

Citations use `data/scripts/<file> [XXXX]` (descumm offsets) and `third_party/scummvm/engines/scumm/<file>:<line>`. "A*n*" is action *n* of `uv run speedrun plan part1`, the 66-action plan of `model.md` §8 after Task 8.1b.

That plan differs from the one traced in §1 only within ties:

- A7 is now named `walk-out-of-bar-from-right-meanwhile`. It is the same Walk to 315.
- A24 is now `give-meat-to-prisoner` instead of `talk-to-prisoner`.
- `drug-meat-with-petal` moved to A26.

Both A24 variants are analysed in §5. In the compiled plan, the trace `step` index is *n*−1 for A1 to A10. A11 is steps 10 and 11, the two tent walks. From A12 on, the step index equals *n*.

## 0. Findings

1. **No route cutscene is BANNED.** In-segment, the only writes to var 19 (`VAR_TIMER_NEXT`) are the circus pair in `room-051-circus-te/local-208.txt [0005]`/`[005F]`. No override is active while that script runs (§3.2). The logo override `global/script-152.txt [004F]` is the one that skips a var 19 write, and it runs before the segment starts.
2. **No step needs `no_skip`.** Every override on the route reaches the same route-relevant state. A few leave benign differences, and the 8.2 harness must allowlist them (§4.4) instead of turning them into `no_skip`.
3. **Two steps need a different `choose` list when skipping, or the run fails.** Menus sit *inside* two overrides:
   - `global/script-119.txt` (Elaine close-up): `Uh`, `Um`, `Blfft`;
   - `room-053-foyer/local-217.txt` (Fester): `Buzz off`.

   With Esc these menus never appear. A50 `steal-idol` must then use `choose = ["could have it"]`, and A51 `walk-past-fester-to-underwater` must use `choose = []`. Otherwise both steps wait for menus that never come and end in `step_timeout` (contract C4, flow step 5).
4. **Five route overrides have no enclosing `cutscene()`** (cutscene stack level 0):
   - circus `local-207 [0438]`, `[0BCB]` and `[1041]`;
   - `global/script-119.txt [0000]`;
   - `room-053-foyer/local-217.txt [000F]`.

   The pre-segment "Part One" card, `room-096-part1/local-200.txt [0004]`, is a sixth.

   Esc skips them in the engine, so the bridge must trigger on `cutScenePtr[cutSceneStackPointer] != 0`, not on "a cutscene is running" (§4.1). Findings 3 and 4 are coupled. Skipping level-0 overrides requires the shorter `choose` lists, and not skipping them requires the current lists.
5. **Esc must land on the first frame the override is active.** Besides saving time, it matters for state: in the idol room a late Esc can leave the vase 630 in the inventory, which shifts the circus helmet slot for the idol-item guard variants (§5, A38).
6. **Skips shift the RNG stream** (§7.2). So the seed-dependent risks of `model.md` §7 must be re-verified with skips on, over many seeds:
   - the storekeeper being away;
   - citizens closing door 437;
   - the 4th map entry pirate.

   The kitchen-door race is not affected.

## 1. Method and classification

- **Static analysis.** Every listed script was read in `data/scripts`. Override paths are derived statically. Skips are not implemented yet, so no override path has been executed.
- **Dynamic coverage of the normal path.** The compiled plan was replayed with `speedrun.engine.run_engine` on seeds 1 to 8 with `extra_args=["--debugflags=SCRIPTS,OPCODES"]`. Seed 1 was also run with `VARS`.
  - `--debugflags` does not raise `gDebugLevel`, so var 39 stays 0 (`scumm.cpp:270`). The replays reproduced `model.md` §8's ticks exactly: seed 1 107660, seed 2 107054, seed 3 106874, seeds 4 and 5 107366.
  - This gives the exact set of `o5_cutscene`, `o5_beginOverride` and `o5_endCutscene` instructions the route executes, and every write to var 19 (§3.1). The run dirs were scratch output and were not kept.
- **Seed dependence.** Every route cutscene ran on all 8 seeds except:
  - the cook's `global/script-026.txt` door close during A6 (seeds 1, 3, 5, 6);
  - extra citizen door cutscenes `global/script-025/026` in room 34 (seeds 2, 3, 4 and 8; `model.md` §7.12).

  Neither has an override.
- **Confirmation still needed.** The 8.2 harness must confirm each override-path conclusion, using the expected diffs in §4.4.

The classification applies to the state at the end of the plan step, which is what the 8.2 harness compares. The fields are vars, bits, owners, object states and classes, rooms, ego position and the menus that appear.

| class | meaning |
|---|---|
| **SAFE** | The override path ends the step with the same values in every field above, ignoring the scratch vars 100, 141, 142 and 194 and cosmetic animation frames. |
| **STATE-DIFF** | Some field differs, ego position included. Each entry says why the route still works, or needs `no_skip`. |
| **BANNED** | The override path skips a var 19 write. It needs `no_skip`. |
| **NO-OVERRIDE** | The cutscene has no `beginOverride`, so Esc does nothing. |

"Menus inside" marks an override whose skipped region contains a dialogue menu. Its state may be SAFE, but the step's `choose` list must change.

## 2. Engine facts

- **Cutscene start.** `o5_cutscene` (`script_v5.cpp:940`) calls `beginCutscene` (`script.cpp:1632`).
  - It pushes a level (`vm.cutSceneStackPointer`) and clears that level's override pointer.
  - It runs `VAR_CUTSCENE_START_SCRIPT`, which is global 18 in MI1. Global 18 does `UserputSoftOff` and `freezeScripts(127)` (`global/script-018.txt [0007]`, `[0070]`).
  - `freezeScripts` skips freeze-resistant scripts (`script.cpp:928`), the `F` flag in descumm's `startScript(…,F)`.
- **Override start.** `o5_beginOverride` (`script_v5.cpp:2010`) calls `beginOverride` (`script.cpp:1711`).
  - It stores the address of the `goto` that follows in `cutScenePtr[sp]`.
  - It then steps over that `goto`, so the normal path runs on into the override region.
  - The same opcode with argument 0 is `endOverride` (`script.cpp:1728`), which clears the pointer. descumm prints both as `beginOverride()`/`endOverride()`.
- **Esc.** `processKeyboard` handles Esc at `input.cpp:1421` to `1426` and calls `abortCutscene` (`script.cpp:1689`).
  - If `cutScenePtr[sp]` is non-zero, the owning script resumes at the stored `goto`, so it jumps to the override target. `VAR_OVERRIDE` (var 5) becomes 1 and the pointer is cleared.
  - If the pointer is zero, Esc does nothing. That covers both a cutscene without an override and a nested override-less cutscene on top of one with an override.
  - Many targets begin `if (VAR_OVERRIDE) { fix-up }`. The fix-up re-establishes the state that the skipped region would have set.
- **End of cutscene.** `endCutscene` (`script.cpp:1650`) clears the current level's pointer (`script.cpp:1665` to `1666`). It then runs global 19, which does `UserputSoftOn` and `freezeScripts(0)`.
- **The first frame always runs.** `processInput` runs before `runAllScripts` in each frame (`scumm.cpp:3173`, `scumm.cpp:3227`).
  - So the code from `beginOverride` up to the first yield (`breakHere`, `delay`, `Wait*`) runs on both paths.
  - The earliest Esc lands on the next frame. Every entry in §5 accounts for this first-frame code.
- **Esc as a key.** Esc also sets `_mouseAndKeyboardStat = VAR_CUTSCENEEXIT_KEY` (27, `global/script-001.txt [0092]`).
  - When userput is on, that value reaches verb-key matching and the input script (`verbs.cpp:590` to `615`).
  - Every route override is active only while userput is off: inside a cutscene, after a menu choice (`global/script-014.txt [00E2]`), or after an explicit `UserputOff` (`global/script-119.txt [0036]`, `room-053-foyer/local-217.txt [000D]`).
  - The map chart is the exception (§6).
- **Text skip.** `.` (`VAR_TALKSTOP_KEY` = 46, `global/script-001.txt [07E9]`) only sets `_talkDelay = 0` (`input.cpp:1416` to `1419`). The key never reaches a script.
  - It ends the current line, or the current part of a `wait()` line.
  - It does not shorten `delay()`.
- **The frame length is var 19.** `delta = VAR(VAR_TIMER_NEXT)` (`scumm.cpp:2818`), `VAR_TIMER_NEXT` = 19. Each frame lasts var 19 jiffies.
- **Stopped scripts.** `stopScript` aborts with an error if the stopped script still owns an active override (`script.cpp:273`). No fix-up on the route stops such a script.

## 3. Var 19 (the logo speed glitch family)

### 3.1 Every var 19 write in a full route run

With `--debugflags=VARS` the seed 1 replay logs `writeVar(19, …)` exactly six times:

| # | value | writer | when |
|---|---:|---|---|
| 1 | 0 | `global/script-001.txt [006A]` | boot |
| 2 | 6 | `global/script-001.txt [07B3]` | boot |
| 3 | 5 | `room-010-logo/local-204.txt [0000]` | logo (pre-segment) |
| 4 | 6 | `room-010-logo/local-204.txt [0097]` | logo end (pre-segment) |
| 5 | 1 | `room-051-circus-te/local-208.txt [0005]` | A11, circus |
| 6 | 6 | `room-051-circus-te/local-208.txt [005F]` | A11, restores Local[4] |

These are the only direct writers in `data/scripts`. The others are not on the route or in its alternatives:

- `global/script-044.txt [00E2]`/`[011C]` is Part III (room 15, `room-015-fork/obj-0169-rock-on-top-of-note.txt [016B]`) and has no override.
- `global/script-100.txt [000A]`/`[0153]` is swordfighting via `global/script-074.txt [0041]`, has no cutscene, and is out of scope.
- `room-052-circus-gr/local-200.txt [0020]` and `room-059-stans/local-202.txt [001D]` only read var 19.

### 3.2 The circus pair cannot be skipped

`room-051-circus-te/local-207.txt` starts `local-208` at `[03CE]`, then waits for it to end (`[03D2]` to `[03D6]`). While it runs, no override is active:

- the previous override ended at `[01F2]` (`endOverride`), and its cutscene at `[01F7]`;
- the next override starts at `[0438]`, after the wait.

So `cutScenePtr` is 0 and an Esc there is a no-op (§2). No room change can kill 208 mid-way either: userput is off from `[03B0]`, and the verb script is 14. 208 is a 14-frame animation that always restores var 19 at `[005F]`.

**Verdict: not BANNED.** The bridge must still inject Esc only while `cutScenePtr[sp] != 0`, never blindly.

### 3.3 How the logo glitch works (BANNED, pre-segment)

1. `global/script-001.txt [07B3]` sets var 19 = 6 before it starts the intro, `global/script-152.txt`, at `[07C5]`.
2. Global 152 opens `cutscene([])` (`[0000]`), sets `beginOverride` at `[004F]` with target `[08C8]`, then starts the logo script `room-010-logo/local-204.txt` (`[0058]`).
3. Local 204 sets var 19 = 5 at `[0000]` and restores 6 only at its very end, `[0097]`.
4. An Esc while 204 runs jumps 152 to `[08C8]`: `InitCharset`, `stopSound`, `endCutscene`, and 152 ends.
5. Global 1 then loads the next room (`[07EE]`). That kills local 204 before `[0097]`, so every later frame lasts 5 jiffies instead of 6.

This is the only override in the whole run whose path skips a var 19 write. It is pre-segment, and the bridge never injects keys before `tick0` (`rules/glitchless.md`, banned item 2).

## 4. Requirements for Task 8.2

### 4.1 Trigger

- **When to inject Esc.** Inject Esc on the first frame on which `vm.cutScenePtr[vm.cutSceneStackPointer] != 0`, at any stack level, level 0 included. This is exactly what `abortCutscene` tests (`script.cpp:1693`).
  - A trigger of `cutSceneStackPointer > 0` would never skip the five level-0 route overrides in §0.4. A human's Esc does skip them.
  - It would also make the shortened `choose` lists of §4.3 wrong.
- **Two-stage overrides.** Two of them, `global/script-065.txt [0005]` then `[01B9]`, and the circus `local-207 [1041]` then `[1127]`, need two Esc presses on two different frames.
- **Precedence.** One key goes through the key path per frame. When an override is active and a line is showing, inject Esc, because it removes the whole region. Inject `.` only on frames with no active override.
- **Optional extra guard.** Inject Esc only while `_userPut <= 0`. Every route override already satisfies this (§2). It keeps key 27 away from input scripts, as with the map chart (§6).

### 4.2 First frame is a hard requirement

The idol-room override needs the earliest Esc:

1. `room-053-foyer/local-206.txt [008A]` picks up the vase 630.
2. Only the normal path of `local-210.txt [006A]`/`[006E]` removes it again.
3. The fix-up (`local-210.txt [0436]` to `[04C8]`) never touches 630.

An Esc between those two points leaves 630 in a cell before the idol-room items. That changes "the slot before the pot" for the variants `pick-up-pot-not-first-<idol item>` (`model.md` §4.3). An Esc on the first frame lands before local 206 even starts, which is at `local-210 [003E]`.

### 4.3 `choose` changes when cutscene skip is on

| action | current `choose` | with skips | why |
|---|---|---|---|
| A50 `steal-idol` | `could have it`, `Uh`, `Um`, `Blfft` | `could have it` | The last three menus are inside the level-0 override `global/script-119.txt [0000]`→`[08A1]` (`[01BB]`, `[038E]`, `[05D2]`). Their branches set nothing (`[0294]` to `[02BC]`, `[046A]` to `[0492]`, `[06B3]` to `[06DB]`). |
| A51 `walk-past-fester-to-underwater` | `Buzz off` | none | The menu `[006E]` to `[01FF]` is inside the level-0 override `room-053-foyer/local-217.txt [000F]`→`[034B]`. Its replies set nothing on the route; the sword handover `[030F]` to `[0336]` is redone by the fix-up `[0364]` to `[0370]`. |

**As implemented (Task 8.2).** The templates keep both lists through the C4 key `override_choose` (`docs/plan.md`): `steal-idol` has `choose = ["could have it"]` and `override_choose = ["Uh", "Um", "Blfft"]`, and `walk-past-fester-to-underwater` has `override_choose = ["Buzz off"]`. The bridge expects the `override_choose` entries only when cutscene skips are off or the step is `no_skip`. One compiled plan thus replays strictly in both modes.

All other `choose` lists of the plan answer menus that lie outside override regions (§5). The menus actually answered on seed 1, from `choice` records, are:

- A11: `ahem`, `I'll do it`, `Of course`;
- A12: `nibboB`;
- A21: `barber`, `swell gift`;
- A30: `About this shovel`, `I want it`, `breath mint`;
- A43: `stiff upper lip`;
- A50: `She said I could have it!`, `Uh`, `Um`, `Blfft`;
- A51: `Buzz off`.

### 4.4 Expected diffs for the skip-safety harness

The harness compares each step with and without skips. The differences below are expected and do not matter to the route, so the harness should allowlist them rather than turn them into `no_skip`.

| action | field | normal | skip | why it is irrelevant (readers) |
|---|---|---|---|---|
| A7 | Bit[561] | 1 | 0 | Set only by `room-070-hellcliff/entry.txt [0000]`, which the override path never loads. It has no reader anywhere in `data/scripts`; the indirect bit reads `Bit[5+…]` in global 2 and `Bit[521+k]`, k ≤ 7, in global 165/166 cannot reach 561. |
| A7 | Var[274] | 1 | unchanged | Set by `room-070-hellcliff/entry.txt [003D]`. It is read only in Part III/IV (`room-039-hellmaze/local-212.txt [0005]`, `room-025-village/obj-0293-head-of-the-navigator.txt [00B2]`) and is re-set by `room-065-hellhall/entry.txt [0011]` and room 70's entry. |
| A7 | actors 6, 9 | in room 72 (LeChuck and ghost costumes) | left as they were | Every later scene re-inits the actors it uses: `room-053-foyer/local-216.txt [0000]`, `local-212.txt [0002]`, `global/script-065.txt [0019]`. The route never re-enters the bar. |
| A24, tie `talk-to-prisoner` only | ego position | (95,140) | at the cell, after `local-202 [0025]` | The next sentence that moves ego, a Walk to 400, starts wherever ego stands. |
| A38 | owner of 630 (vase) | 14 | 15 | Readers: `room-053-foyer/local-206.txt [0000]`/`[0044]` and `local-210.txt [0056]` (this cutscene only, which cannot replay: 632 is locked), `room-041-kitchen/local-215.txt [001B]`, and `global/script-122.txt [0164]`. None is on the route. It stays outside the inventory on both paths. |
| A38 | names of 646 and 649; verb 60 | renamed; "Hypnotize" defined | DOBJ names; no verb 60 | Names are cosmetic, and the compiler resolves names from the boot-time `objects.json`. Verb 60 is not a standard or dialogue verb, and steal-idol redefines it (`local-211.txt [00B7]`). |
| A50 | class 6 of 641 | set (`local-211.txt [0050]`) | clear | 641 is hidden (owner 14) on both paths. Names of 641, 648 and 650 and verb 60 are cosmetic, as above. |
| A51 | Var[194] | 123 (`Buzz off`) | 120 | Scratch: every menu resets it (`… Var[194] = 0`). |
| A52 | ego position | (ego x + 15, 121) (`room-083-cu-dock/local-201.txt [0706]`/`[0714]`) | (185,121) (`[0781]`) | A53 is a Walk to 904. |
| A52 | ego palette entry 12 | 255 (`[006D]`) | default | Cosmetic. |
| all | scratch Var[100], Var[141], Var[142], Var[194]; RNG-driven values | | | §7.2 |

Bits, owners and the inventory order must otherwise be identical at every `step_end`. Var 19 must be identical everywhere (§3).

**Measured (Task 8.2).** `tests/integration/test_skip_safety.py` compares bits, inventory, the owner of every object, the room and var 19, with skips against without, at every `step_end` on seeds 1 to 3. It finds exactly three differences:
- Bit[561], from the LeChuck exit on (A7, above);
- the owner of 630, from `enter-idol-room` on (A38, above);
- Bit[324], from the store on. This is RNG drift, not an override. Whether the storekeeper is away at a store entry is drawn at `room-030-store/entry.txt [002B]`; `local-204.txt [02F4]` sets the bit when he comes back and catches ego. Its readers only pick a line (`local-200.txt [017D]`, `local-204.txt [02EF]`, `local-212.txt [00ED]`).

The inventory is identical at every step, so the first-frame Esc of §4.2 lands in time.

## 5. Route cutscenes, in plan order

Steps not listed run no cutscene:

- A2, A3, A8 to A10, A13 to A19 (the map, forest and petal steps);
- A23, A25, A26, A28, A31, A33, A36, A39, A40, A42, A45, A46, A48, A49, A53 to A65.

Measured step lengths are means over seeds 1 to 8 without skips. They are an upper bound on what the skips can save.

**A1 `open-bar-door`**
- `room-033-dock/obj-0428-door.txt [0015]` starts `global/script-025.txt`. Its `cutscene([2])` at `[001F]` sets the door state and ends at `[003C]`, all in one frame. **NO-OVERRIDE.**

**A4 `walk-into-kitchen`**
- During the `until` wait, the cook opens 316 through `global/script-025.txt [001F]`. **NO-OVERRIDE.**
- Walk to 316 (`room-028-bar/obj-0316-door.txt [004B]`) starts `room-028-bar/local-218.txt`: `cutscene([2])` `[0000]`, the walk to (660,120), `endCutscene` `[0014]`, `loadRoomWithEgo` `[0017]`. **NO-OVERRIDE.**

**A5 `use-meat-with-pot`, A27, A29, A35, A37, A51 (the Open)**
- The sentence script's reach animation `global/script-002.txt [0385]` to `[0397]` (`cutscene([2])`, `delay(20)`). **NO-OVERRIDE.** A27, A35 and A37 also run the door script `global/script-025.txt [001F]`. **NO-OVERRIDE.**

**A6 `walk kitchen bar-right`**
- On seeds 1, 3, 5 and 6, the cook closes 316 through `global/script-026.txt [0016]`. **NO-OVERRIDE.**

**A7 `walk-out-of-bar-from-right-meanwhile` (traced as `walk bar-right dock`): LeChuck. STATE-DIFF, no `no_skip`. Mean 9,696 ticks.**
- **Start.**
  - The first Walk to 315 sets `Bit[446] = 1` and starts global 120 (`room-028-bar/obj-0315-door.txt [0080]` to `[008A]`).
  - `global/script-120.txt [0000]` opens `cutscene([3])`, then `loadRoom(0)` `[0005]` runs the bar's exit script on both paths.
  - The override is at `[000E]` with target `[052D]`. The first frame shows "Meanwhile" (`[0013]`) and yields at `[002B]`.
- **Skipped by the override.**
  - `loadRoom(70)` `[002F]`, whose entry sets `Bit[561]` and `Var[274]` (§4.4);
  - `loadRoom(72)` `[00B3]`, whose entry does nothing because ego is not in 72;
  - LeChuck and the ghost, actors 9 and 6, being initialised and walked in room 72 (`[00B5]` to `[0524]`);
  - sound 100.
- **Both paths then run** `[052D]` `InitCharset(2)`, `[0535]` `endCutscene`, and `[0538]` `loadRoomWithEgo(428,33,-1,-1)`.
- **Route state.** Bit[446], room 33 and the dock arrival are the same. The extra fields are listed in §4.4. No var 19 write is involved.

**A11 `walk-into-tent-with-pot`. SAFE. Mean 9,104 ticks for compiled step 11.**
- **Arrival.** `room-052-circus-gr/obj-0621-circus-tent.txt [0014]` loads room 51. The entry walk-in is `room-051-circus-te/entry.txt [0036]` to `[0047]`. **NO-OVERRIDE.**
- **`local-207 [0010]` `cutscene([2])`: SAFE.**
  - Override `[001F]` jumps to `[01F2]`.
  - It skips only the brothers' argument, `[0024]` to `[01F0]` (`print` + `WaitForMessage`).
  - The target runs `endOverride`, then starts the freeze-resistant background chatter `local-205` at `[01F4]`, then `endCutscene` at `[01F7]`.
- **Menu `ahem`** at `[0213]` to `[03A3]` is outside the override. Then comes the var 19 window `[03CE]` to `[03D6]` (§3.2).
- **`local-207 [0438]`: SAFE.**
  - This is a level-0 override. It jumps to `[0882]`, skipping the "once in a lifetime" pitch `[043D]` to `[0880]` (lines, ego animation frames, `delay(60)`).
  - `Bit[71] = 1` `[088D]` is after the target, so it is set on both paths.
- **Menus `I'll do it`** (`[08AB]`) **and `Of course`** (`[0B49]`) are outside overrides. `Bit[72] = 1` is at `[0AA8]`.
- **`local-207 [0BCB]`: SAFE.**
  - Level-0 override, taken on the branch `Var[194] == 120`. It jumps to `[0D45]`, skipping 5 lines.
  - The target sets `VAR_VERB_SCRIPT = 200` (`[0D4C]`), which the helmet step waits for.

**A12 `walk-out-of-tent-after-helmet-meat`. SAFE. Mean 6,120 ticks.**
- **Helmet.** `room-051-circus-te/local-200.txt [0039]` to `[008A]`, then `Bit[103] = 1` at `[008B]`. **NO-OVERRIDE.**
- **`local-207 [0D61]` `cutscene([])`: SAFE.**
  - The lines `[0D63]`/`[0D86]` and the walks to the cannon run before the override.
  - Override `[0DE0]` jumps to `[0E5D]`. It skips 3 lines and `startScript(202)` (`[0E5A]`), which would walk ego to (327,121) and print "ECHO".
  - The target stops 202 if `VAR_OVERRIDE` is set. Then `[0E75]` walks ego to (318,122) on both paths.
- **Cannon.** `local-203 [0000]` to `[00C7]`, which sets the pot 567 to owner 0 at `[00C3]`. **NO-OVERRIDE.**
- **Menu `nibboB`** at `[0F40]` comes after `endCutscene` `[0F07]`.
- **`local-207 [1041]`: SAFE.**
  - Level-0 override with target `[1107]`. The first frame already prints "He's all right!" and starts sound 113 (`[1046]`/`[105E]`).
  - The payout `startObject(488,250,[478,0])` `[110E]` and `startScript(206,[],F)` `[111B]` come after the target.
- **`local-207 [1125]` `cutscene([])`: SAFE.**
  - Override `[1127]` jumps to `[1136]`. It skips only the wait for `local-206`, the brothers' "basic theory" lines, which write no state.
  - `[1136]` (ego `Init`, `Costume(1)`), `[113D]` `endCutscene` and `[114D]` `startObject(617,11)` run on both paths.
  - Object 617's `loadRoomWithEgo(621,52,78,87)` (`obj-0617-outside.txt [000C]`) runs in the same frame, so the room change kills 206 at once.

**A20 `walk dock low-street`**
- The walk-in `room-035-low-stree/entry.txt [00DE]` to `[00F0]`. **NO-OVERRIDE.**

**A21 `buy-map`. SAFE. Mean 3,750 ticks.**
- `obj-0441-citizen-of-mle.txt [0015]` starts `room-035-low-stree/local-218.txt`.
- "Excuse me, but do you have a cousin named Sven?" `cutscene([])` `[0398]` to `[03CF]`. **NO-OVERRIDE.**
- Menu `barber` `[04AA]`, then "Close enough." `[05B8]`, then `Bit[475] = 1` at `[06AA]`.
- **`cutscene([2])` `[06AF]`: SAFE.** Override `[06B4]` jumps to `[0792]`, skipping the map pitch `[06B9]` to `[0790]` (lines and delays). The target runs `print(255," ")` and `endCutscene` `[0797]`.
- Menu `swell gift` `[080C]`, then `pickupObject(442)` `[0936]`, `Bit[65] = 1` `[093E]` and the payment `[0945]`. All of these are outside overrides.

**A22 `walk low-street high-street-town`**
- `room-035-low-stree/obj-0451-archway.txt [000C]` to `[001E]`. **NO-OVERRIDE.**

**A24 `give-meat-to-prisoner` (current plan). SAFE.** This override path is static only; the traces ran the tie `talk-to-prisoner`.
- Give 566 (`obj-0405-prisoner.txt [006F]`, verb 80) starts `room-031-jail/local-203.txt`. It walks to the cell, `[0082]` to `[00B7]`. **NO-OVERRIDE.**
- 566 matches none of the handled items, so the script reaches `[029D]` `cutscene([])`.
- Override `[02A3]` jumps to `[0343]` (`endCutscene`, which clears the pointer: `script.cpp:1666`). It skips only "I don't want anything but my freedom!" and "…and maybe a breath mint." (`[02A8]`, `[02DC]`).
- On both paths, `[0351]` then sets `Bit[420] = 1`, and ego says "Man! Talk about bad breath!" (`[035A]`, text skip only). The meat stays in the inventory.

**A24 tie `talk-to-prisoner`. STATE-DIFF (ego position only), no `no_skip`. Mean 1,770 ticks.**
- `obj-0405-prisoner.txt [001A]` sets `Bit[420] = 1` before `local-202` starts (`[001F]`).
- The walk to the cell, `local-202 [001E]` to `[004F]`. **NO-OVERRIDE.**
- **Halitosis, `cutscene([2])` `[18DE]`.**
  - Override `[18E3]` jumps to `[19C0]`. It skips 4 lines and ego's walk to (95,140) (`[194B]`).
  - The target runs `endCutscene`, `[19C4]` restores the verbs, and `[19D8]` starts `local-200`.
  - Bit[420], class 6 of 405 and the open cell are unchanged. Only ego's position differs (§4.4).

**A30 `pay-for-shovel-and-mints`. SAFE. Mean 5,040 ticks (4,854 to 5,544).**
- **Catch.** Walk to 387 (`obj-0387-door.txt [0098]`) starts `room-030-store/local-204.txt`, `cutscene([1])` `[004E]` to `[042A]`.
  - The storekeeper walks in if he is away, catches ego (`Bit[324]` `[009C]`) and asks for money. `Bit[309] = 1` `[0415]`.
  - **NO-OVERRIDE.**
- **Menus.** The dialogue `local-211` builds the topics; the `I want it` menu is in `local-206 [0030]` (`Var[105]`, `Bit[479]`). Both are outside overrides.
- **Shovel, `cutscene([2])` `[0910]`: SAFE.** Override `[0915]` jumps to `[09BD]` and skips one line. The payment `[09C3]` and `Bit[99] = 1` `[09CE]` follow the target.
- **Mints, `cutscene([2])` `[1B23]`: SAFE.** Override `[1B28]` jumps to `[1B7F]` and skips one line. `Bit[312]` `[1BB1]`, `pickupObject(395)` `[1BC0]` and the payment `[1BC8]` follow the target.
- **Browse, `cutscene([2])` `[1DD5]`: SAFE.**
  - Override `[1DDA]` jumps to `[1ECF]`.
  - The variant draw `getRandomNr(4)` `[1DDF]` runs in the first frame on both paths. Only the line is skipped.
  - The target restores the verbs and starts `local-212` (`[1EE4]`).

**A32, A47 `walk high-street-town high-street-mansion`**
- `room-034-high-stre/obj-0436-archway.txt [001E]` to `[003C]`. **NO-OVERRIDE.**

**A41 `walk high-street-mansion high-street-town`**
- `room-034-high-stre/obj-0435-town.txt [000C]` to `[003F]`. **NO-OVERRIDE.**

**A34 `give-meat-to-poodles`. Mean 636 ticks.**
- `obj-0467-deadly-piranha-poodles.txt [00D5]` starts `room-036-mansion-e/local-201.txt`, `cutscene([2])` `[0010]` to `[01F8]`.
- It sets `Bit[15]` at `[0087]` and makes 465 touchable at `[00CE]`. It contains only `delay`s and a `print(254)` notice.
- **NO-OVERRIDE.**

**A38 `enter-idol-room`. STATE-DIFF, no `no_skip`; first-frame Esc required (§4.2). Mean 10,078 ticks.**
- **Start.**
  - `obj-0632-door.txt [0024]` starts `room-053-foyer/local-210.txt`: `cutscene([])` `[0000]`, `Bit[481] = 1` `[0002]`, override `[0007]` with target `[042F]`.
  - In the first frame, `startScript(205)` `[000C]` starts ego's walk to the door (205 then closes 632 through global 26, which is a no-op after the fix-up). The frame yields at `[000F]`.
- **Normal path, skipped by Esc.** Every effect below runs only on the normal path:
  - Fester (`local-216`) runs;
  - `local-206` picks up 630 at `[008A]`, which `local-210 [006A]`/`[006E]` sets to owner 0 and then 14;
  - 632 becomes locked (`[004B]`);
  - verb 60 "Hypnotize" is defined (`[0084]`);
  - the fake sentences of `local-218` write Var[107] to Var[110] (cleared by global 11 on both paths);
  - the renames 646 `[0164]` and 649 `[02D5]` to `[0393]`;
  - the pickups: 643 `[01FB]`, 641 through `local-204 [0047]`, 642 `[0267]`, 640 `[02B8]`, and 644 `[03F2]`, which is removed again at `[0413]`;
  - 637 becomes touchable (`[022F]`).
- **Fix-up `[0436]` to `[04C8]`** (when `VAR_OVERRIDE` is set):
  - stops 220, 218, 219, 216, 206, 203, 204 and 213, none of which owns an override;
  - starts 209 (stops 207 and 208, sends actor 12 to room 0), 11 and 12;
  - closes and locks 632 (`[0467]`/`[046B]`) and sets 644 to owner 0;
  - picks up whichever of 643, 641, 642 and 640 is missing, in that order (`[0476]` to `[04B2]`). That is the normal pickup order, so the inventory cells match;
  - makes 637 touchable (`[04B6]`), draws 636, sets the box flags, and sends actor 8 to room 0.
- **Both paths then run** `local-214` (`[04CC]`): ego `Init`, put at (540,28) `[001A]`, and the line "That should hold him for a while!".
- **Route state.** Identical: Bit[481], 632 state 0 and locked, 637 touchable, 640 to 643 owned in the same order, 644 gone, ego at (540,28). Only 630's owner, two object names and verb 60 differ (§4.4).

**A43 `give-mints-to-prisoner`. SAFE. Mean 1,632 ticks.**
- **Walk.** Give (`obj-0405-prisoner.txt [006F]`) starts `room-031-jail/local-203.txt`, whose walk is `[0082]` to `[00B7]`. **NO-OVERRIDE.**
- **Mints.** `setClass(405,[6])` `[00BF]` clears the bad breath before the cutscene.
- **"Grog-O-Mint", `cutscene([1])` `[00C6]`: SAFE.** Override `[00D8]` jumps to `[0111]`, skipping one line. Then `chainScript(202)` `[0112]`.
- **"So, have you come to release me?"** `local-202 [0066]` to `[0091]`. **NO-OVERRIDE.** `Bit[476]` is set at `[0061]`, before it.
- **Menu `stiff upper lip`** (choice 127) is outside overrides.
- **"Thanks a lot.", `cutscene([2])` `[138B]`: SAFE.** Override `[1390]` jumps to `[13A8]`, then `goto 19C4`.

**A44 `give-repellent-to-prisoner`. Mean 1,080 ticks.**
- `local-203 [0082]` to `[00B7]`, then `cutscene([])` `[0128]` to `[01E4]`. Inside it, 640 goes to owner 0 and then 14 (`[012A]`/`[012E]`), Otis speaks two lines, and ego picks up the cake with `pickupObject(420)` `[01E0]`.
- **NO-OVERRIDE.** Only text skip applies.

**A50 `steal-idol`. SAFE; menus inside, so `choose` must change (§4.3). Mean 18,706 ticks.**
- **`room-053-foyer/local-211.txt` `cutscene([])` `[0000]`.** Started by `obj-0637-gaping-hole.txt [0021]`. **STATE-DIFF**, cosmetic (§4.4).
  - `local-213` runs before the override: the walk to (318,135) and "I've got the file^". It is not skippable.
  - Override `[000F]` jumps to `[019E]`, and the first frame yields at `[0014]`.
  - The normal path runs the gag. It sets 641's class and name, then owner 0/14 (`[0050]` to `[008C]`); defines verb 60 "Throw" (`[00B7]`); sets 642 to owner 0/14 (`[00E3]`/`[00E7]`); picks up 635 at `[016C]` with state 0; sets 420 to owner 0/14 (`[0174]`/`[0178]`); and sets 633 to state 0 (`[018C]`).
  - The fix-up `[01A3]` to `[01E4]` picks up 635 if missing (state 0), sets 641, 642 and 420 to owner 0 and then 14, and sets 633 to state 0.
  - Removed cells close up and 635 is appended, so the final inventory order is the same (`model.md` §4.3).
  - Both paths then run `local-215` (ego at (460,120), "Phew!…") and `endCutscene` `[01F7]`.
- **`local-212` `cutscene([])` `[0000]`: SAFE at step end.**
  - Override `[0015]` jumps to `[0242]`. In the first frame `startScript(25,[639])` `[001A]` opens 639, and the frame yields at `[0020]`.
  - The normal path closes 639 again at `[0031]`; the override path leaves it open. `setState(639,1)` at `[04C0]` then equalises both paths.
  - The fix-up puts ego at (325,131) and Fester at (375,131), where the normal path's walks also end.
- **Menu `could have it`** (`[02BE]` to `[0453]`, sets `Var[273]`) comes after `endCutscene` `[0294]`, so it is outside overrides.
- **`local-212` `cutscene([1])` `[04BB]`: SAFE.**
  - Override `[04D3]` jumps to `[0857]` and yields at `[04D8]`.
  - It skips the governor's lines. The fix-up `[085C]` to `[087A]` places the governor at ego x − 25 and sends Fester to room 0, as the normal path does.
- **`global/script-119.txt`, level-0 override `[0000]` → `[08A1]`: SAFE, menus inside.**
  - The first frame runs `[0005]` to `[0036]`: saves the verb script in Local[0], `loadRoom(23)`, puts ego in room 23, `UserputOff`.
  - The normal path holds the Elaine close-up and the three menus. Their branches are empty.
  - Return to room 53. The normal path does it with `[07C2]` and `[07E4]`. The fix-up `[08BE]` to `[08D7]` puts ego in 53 at (365,131) and calls `actorFollowCamera`, which switches the room (`camera.cpp:69` to `70`).
  - The governor goes to room 0, and global 26 closes 639 on both paths (`[087C]`/`[08E8]`).
  - Both paths print "I really wish I knew how to talk to women." `[08F4]` and restore userput and the verb script (`[092E]` to `[0937]`).
  - Object 277 in the close-up is reset by `room-023-cu-gov/entry.txt [0000]` on every entry.

**A51 `walk-past-fester-to-underwater`. SAFE; menu inside, so `choose` must change (§4.3). Mean 4,752 ticks.**
- **Fester: `room-053-foyer/local-217.txt`** (started by `obj-0633-door.txt [0024]`, which requires owning 635).
  - It has no `cutscene()`. `UserputOff` `[000D]` comes first, then the level-0 override `[000F]` with target `[034B]`.
  - The first frame opens 633 (`[0014]`) and places Fester at (89,94), then yields at `[002E]`.
  - Skipped: ego's walk, global 144 (the random misspelling of ego's name, two RNG draws), Fester's lines and the `Buzz off` menu.
  - The fix-up `[0350]` to `[0370]` turns verbs 120 to 128 off and confiscates the sword if ego owns it.
  - Both paths run `[0379]` to `[0391]`: `setOwnerOf(635,0)`, `Var[277] = 1`, `loadRoomWithEgo(904,83)`.
- **`global/script-065.txt` `cutscene([1])` `[0000]`: SAFE.** Started by `room-083-cu-dock/entry.txt [001D]`.
  - Override `[0005]` jumps to `[01AC]`. The first frame runs `[000A]` to `[0043]`: costumes, Fester at (176,121), ego costume 60 at (136,121). It yields at `[0049]`.
  - It skips Fester's speech. `[01AC]` resets one animation frame.
  - A second override `[01B9]` jumps to `[0232]` and skips the throw animation, sound 16 and "Hmmm…". It needs a second Esc.
  - `[023A]` to `[025C]` put ego in room 42 at (150,130) and send Fester to room 0. Room 42's entry sets ego's costume 38 anyway (`room-042-underwate/entry.txt [0017]`).

**A52 `walk-up-ladder-taking-idol`. STATE-DIFF (ego position), no `no_skip`. Mean 12,738 ticks.**
- **Underwater, NO-OVERRIDE, both cutscenes.**
  - `obj-0578-fabulous-idol.txt [0027]` starts `room-042-underwate/local-203.txt [0000]` to `[009D]`, which picks up 578 at `[0024]`.
  - Then `local-200.txt [003F]` to `[009C]` runs `startScript(71,[2])` `[0041]` (Bit[85], goal) and `Var[277] = 4` `[007D]`.
  - Both are NO-OVERRIDE, so the goal bit is set before anything skippable.
- **Elaine on the dock.**
  - `room-083-cu-dock/entry.txt [008E]` starts `local-201.txt`: `cutscene([1])` `[0000]`, override `[0017]` with target `[0775]`.
  - The first frame runs `[001C]` to `[0053]`: Elaine (actor 10) is set up, and ego gets costume 66 at (242,152). It yields at `[0057]`.
  - The fix-up `[077A]` to `[0791]` sets ego to costume 1, `FollowBoxes`, (185,121). Both paths send Elaine to room 0 (`[079A]`).
  - Ego's position and palette differ (§4.4). Global 143, the music, is not started on the override path.

**A66 `dig-treasure`. SAFE. Mean 3,690 ticks.**
- `obj-0749-x.txt [00A6]` starts `room-064-treasure/local-200.txt`: `cutscene([1])` `[0000]`, override `[0005]` with target `[01A7]`.
- The normal path sets 751 and 753 to state 1 and back. Both paths run `[01B6]` to `[021D]`: 751 and 753 at state 0, ego at (392,108), `pickupObject(752)`, and `startScript(71,[3])` `[0214]`, which sets the goal Bit[86].
- The goal fires after the skipped region, so the skip shortens the measured time directly.

**Before the segment (never skipped, listed for completeness)**
- `global/script-152.txt [004F]`: the intro and logo. **BANNED** (§3.3).
- `room-038-lookout/local-203.txt [0005]`: the opening at the lookout.
- `room-096-part1/local-200.txt [0004]`: the "Part One" card, a level-0 override.

## 6. Alternatives in `model.md` §5

Modelled alternatives (the planner chooses):

| alternative | cutscenes | class |
|---|---|---|
| `drug-meat-with-petal` | none (`global/script-182.txt [0017]`) | none |
| `use-meat-on-table-with-petal`, `pick-up-meat`, `pick-up-pot-*`, `pick-up-stewed-meat-*` | the reach animation `global/script-002.txt [0385]` | NO-OVERRIDE |
| `put-petal-in-stew`, `put-meat-in-stew` | none: `room-041-kitchen` has cutscenes only in `local-202` (plank) and `local-215` (barrel) | none |
| guard variants `pick-up-pot-not-first-<idol item>`; `walk-out-of-tent-after-helmet-<g>` | the idol room (A38) and the circus (A11/A12) | as above. The idol-item guards rely on the first-frame Esc of §4.2. |
| mints timing: `pay-for-shovel`, `pay-for-shovel-files-topic`, `pay-for-shovel-and-mints-files-topic` | `room-030-store/local-211.txt` overrides `[0915]`, `[1B28]`, `[1DDA]`; the files topic `[1C2C]` to `[1DB0]` has no override | SAFE |
| forest gate 215 → 203, and the whole forest graph | none on the paths. The refusal `room-058-damnfores/local-200.txt [0000]` and the sign scripts `local-203`/`local-204` are NO-OVERRIDE. | none |

Excluded alternatives (Task 8.1b may re-admit those excluded only on action count):

| alternative | cutscenes and overrides | class |
|---|---|---|
| Storekeeper as guide (`treasure.md` §7) | **Topic 122**, `room-030-store/local-211.txt [0AB3]` `cutscene([2])`, override `[0AB8]` → `[0ED0]`. The normal path sets `Var[199] = 1` (`[0AC9]`) only when `!Var[199] && !Bit[102]`, i.e. topic unlocked by the paid sword, and `Var[247]++` (`[0D71]`) only on the third ask. `Bit[102]`, `Bit[326]` and `startScript(67,[34])` all come after the target (`[0ED2]` to `[1131]`). | **STATE-DIFF**: with the sword unlock, Var[199] stays 0. It is read by the store topic list `[00FA]` (the `Bit[98]` topic still shows) and by the pirate leaders (`room-028-bar/local-220.txt [0141]`, `[0958]`). Neither is needed by this route, so no `no_skip`. |
| | **Pirate leaders unlock**, `room-028-bar/local-220.txt [0E37]` (level 0) → `[1036]`. The target itself sets `Var[199] = 1`. | SAFE |
| | **First pirate-leaders talk**, `local-220 [03DA]`/`[03DF]` → `[0906]`. `Bit[412]` is set before it, `Var[197]` after (`[0910]`). | SAFE |
| | **Following**, `global/script-067.txt [030B]` (forest 209) | NO-OVERRIDE |
| Talk to the storekeeper | `obj-0394-storekeeper.txt [0015]` → `local-211`: the same overrides as A30 | SAFE |
| Ring the bell | `obj-0399-bell.txt [0018]` → `room-030-store/local-207.txt` `cutscene([])` `[0000]`, override `[0037]` → `[015A]`. The normal path sets `Bit[454] = 1` (`[0041]`), which only selects the line next time. The fix-up adds `setClass(11,[22])` `[016F]`. Both paths: `Bit[326] = 0`, `setClass(394,[32])`. | STATE-DIFF (Bit[454], cosmetic), no `no_skip` |
| Use meat with poodles | `room-036-mansion-e/local-201.txt [0010]` (as A34) | NO-OVERRIDE |
| Use pot with meat | `global/script-002.txt [0385]` | NO-OVERRIDE |
| The minutes deal | `room-035-low-stree/local-216.txt`: `Bit[129] = 1` `[0564]`, then the level-0 override `[056B]` → `[07D0]` (the pitch). The menu `two pieces` `[0882]` and `pickupObject(464)`/+2 `[093A]`/`[093E]` come after it. | SAFE |
| Provoking the cook | `room-028-bar/local-214.txt [0000]` to `[004E]` | NO-OVERRIDE |
| Circus without the pot, "Er… no" | `room-051-circus-te/local-207.txt [0C72]` (level 0) → `[111E]`. Both paths go to `[111E]`; `Bit[72]` is set at `[0AA8]`, before it. | SAFE |
| Repellent to Otis before the mints | `room-031-jail/local-203.txt [029D]` `cutscene([])`, override `[02A3]` → `[0343]` (lines only). `Bit[420] = 1` `[0351]` comes after. | SAFE |
| Re-reading the map | `global/script-123.txt [017B]` `cutscene([])`, override `[0245]` → `[0259]`. Esc ends the chart's wait for a click, which removes `model.md`'s "waits for a click" exclusion. But `UserputSoftOn` at `[024F]` means Esc's key 27 can also reach input script 66, so gate Esc on userput ≤ 0 (§4.1) before modelling it. | SAFE (state) |
| Fish routes, give pot to a Fettucini brother, boot params | not modelled, or banned | n/a |

Task 8.1b added split actions to `model.md` §4.1 and §4.2 while this note was being written. Their cutscenes are:

- **`provoke-cook`, `walk-to-kitchen-door-provoking-cook`.** `room-028-bar/local-214.txt [0000]` to `[004E]` is **NO-OVERRIDE**.
  - It shows the cook's warning, one message of two parts joined by `wait()` (`print(255)` + `WaitForMessage` `[000F]`/`[0039]`), and only then starts `local-212` at `[004B]`. `local-212` waits 600 jiffies (`[0000]`, `[0004]`) before it brings the cook out.
  - A `.` on that line starts the 600-jiffy timer sooner, which moves the cook's door window earlier. That is a timing change only, and the `until` guard of `walk-into-kitchen-after-provoking-cook` absorbs it.
- **`walk-into-kitchen-after-provoking-cook`.** As A4: `local-218`, **NO-OVERRIDE**.
- **`walk-out-of-bar-from-{left,right}[-meanwhile]`.** The `-meanwhile` variants are A7's global 120, **STATE-DIFF**. The others run no cutscene: `obj-0315-door.txt [0090]` goes straight to `loadRoomWithEgo`.
- **`open-store-door-from-inside`.** `room-030-store/obj-0387-door.txt [003E]` to `[0060]` (`cutscene([2])`, door script 25) is **NO-OVERRIDE**.
- **`walk-follow-guide-to-*`, `walk-forest-gate-215-203-with-guide`.** `global/script-067.txt [030B]` is **NO-OVERRIDE**. The topic-122 override above is **STATE-DIFF**.
- **`walk-up-ladder-taking-idol-last`.** The same scripts as A52. The goal bit is set by `room-042-underwate/local-200.txt [0041]`, before any override, so the Elaine override is after the goal and does not affect the measured time.
- **`walk-forest-gate-215-*-open`.** No cutscene.

## 7. Text skip

### 7.1 Where `.` saves the most

Lines inside an override region are already removed by Esc, so `.` only matters for lines outside one. These are the route steps with the most such lines, in rough order of saving:

| action | lines that only `.` can shorten |
|---|---|
| A11, A12 circus | The brothers' background chatter `room-051-circus-te/local-205.txt [000C]` to `[02BE]` while menu 1 is open, and the echo of every chosen line (`local-209.txt [000C]`). "Now we can do the trick." / "Step right over here, son." (`local-207 [0D63]`/`[0D86]`, before the override). "It works!" … "Are you OK?" (`[0E99]` to `[0F05]`, after `endOverride` `[0E69]`). "Ah, that will work as a helmet!" (`local-200 [0056]`). "I'm Bobbin. Are you my mother?" (`[0FEF]`). |
| A30 store | The catch (`room-030-store/local-204.txt [00AB]` to `[03E9]`, NO-OVERRIDE), "Waddya want?" (`local-211 [002D]`), "You're telling me!" (`[1B04]`), "What else do you want?" (`[1DB6]`), the choice echoes and "I think I'd just like to browse for now." (`[0408]`). |
| A50 steal-idol | "I've got the file^" (`room-053-foyer/local-213.txt [003F]`), "Phew! … At least I got the idol." (`local-215.txt [0063]`), "Well, let's hear your explanation." (`local-212 [026C]`), "Ha!" (`[04A3]`), "I really wish I knew how to talk to women." (`global/script-119.txt [08F4]`). |
| A21 buy-map | "…cousin named Sven?" (`room-035-low-stree/local-218.txt [039A]`), "Close enough." (`[05B8]`), "There ya go…" (`[0908]`), "Now get lost." (`[0950]`), and the echoes. |
| A24, A43, A44 jail | "Man! Talk about bad breath!" (`room-031-jail/local-203.txt [035A]`), "So, have you come to release me?" (`local-202.txt [006B]`), and the whole repellent scene (`local-203.txt [0141]`, `[0178]`/`[018F]`, NO-OVERRIDE). |
| A38 idol room | "That should hold him for a while! …" (`room-053-foyer/local-214.txt [0030]`). |

Lines timed by `delay()` instead of `WaitForMessage` gain nothing from `.`. Examples are the circus pitch `local-207 [043D]` to `[06D8]`, the map pitch, global 120, and the poodles notice `room-036-mansion-e/local-201.txt [00E1]` to `[019D]`. The pitches and global 120 are inside overrides anyway.

### 7.2 Can skipping a line change the outcome?

- **No line sets state "on completion".**
  - Every state write is an ordinary statement that runs whether or not the line before it was cut short. `.` only zeroes `_talkDelay` (`input.cpp:1416` to `1419`).
  - For example, the state write `pickupObject(442)` at `local-218 [0936]` runs right after its line starts, before the `WaitForMessage`.
  - No route script branches on `VAR_HAVE_MSG` or `VAR_TALK_ACTOR`. The only such tests are in `global/script-057.txt [14EC]` (gym) and `room-071-gh-room`, both off the route.
- **No route line gates a timer that matters.**
  - The cook's cycle is drawn at bar entry, `room-028-bar/local-211.txt [000D]`. No route action before the kitchen (A1 to A3) shows a line or an override. The bar's ambient scripts `local-208`/`local-209`/`local-210` print nothing.
  - So the kitchen-door race (`model.md` §7.1) is the same with skips on.
  - The underwater drowning timer (`room-042-underwate/local-205.txt [0000]`, 28800 jiffies) and the guide's give-up timer (`global/script-067.txt [035A]`) can only benefit from going faster.
- **The RNG stream shifts.** Both key kinds change how many frames pass, and random draws are tied to frames:
  - global 159 draws `getRandomNr(232)` on every frame it is not frozen (`global/script-159.txt [0000]`);
  - every menu loop draws once per frame, e.g. `room-030-store/local-211.txt [043D]`;
  - some skipped regions contain draws of their own, such as global 144 (A51) and the idol room's `local-218 [000B]`.

  Every later random value therefore differs from the unskipped replay of the same seed. The maximum talk speed also shortens the pre-segment opening, which shifts the RNG before `tick0`. These risks were verified only with skips off (seeds 1 to 5) and must be re-verified with skips on:
  - the storekeeper away on 1 store entry in 4 (`room-030-store/entry.txt [002B]`), and the store's line variants;
  - a citizen closing 437 between `open-store-door` and `walk-into-store` (`model.md` §7.12);
  - a wandering pirate on the 4th map entry (`model.md` §7.2).

  This is what the many-seed measurement of Phase 8 (Option 2) is for. A plan that fails on any seed is rejected (Task 8.4).
- **Menus are not affected.** A menu is answered through a verb click, which `.` never touches, and background chatter during a menu (circus `local-205`) is freeze-resistant and writes nothing.

## 8. Summary table

"choose" marks steps whose `choose` list must change when skipping (§4.3).

| step / action | cutscenes (start → override target) | class | `no_skip` | citation |
|---|---|---|---|---|
| A1 `open-bar-door` | G25 door `[001F]` | NO-OVERRIDE | no | `global/script-025.txt [001F]` |
| A4 `walk-into-kitchen` | G25 (cook); L218 walk-in `[0000]` | NO-OVERRIDE | no | `room-028-bar/local-218.txt [0000]` |
| A5, A27, A29, A35, A37 | G2 reach `[0385]`; G25 doors | NO-OVERRIDE | no | `global/script-002.txt [0385]` |
| A6 `walk kitchen bar-right` | G26 (cook, seed-dependent) | NO-OVERRIDE | no | `global/script-026.txt [0016]` |
| A7 `walk-out-of-bar-from-right-meanwhile` | G120 `[0000]` → `[052D]` | STATE-DIFF (Bit[561], Var[274], actors 6/9) | no | `global/script-120.txt [000E]` |
| A11 `walk-into-tent-with-pot` | entry `[0036]` | NO-OVERRIDE | no | `room-051-circus-te/entry.txt [0036]` |
| | L207 `[0010]` → `[01F2]` | SAFE | no | `room-051-circus-te/local-207.txt [001F]` |
| | L207 `[0438]` → `[0882]` (level 0) | SAFE | no | `local-207.txt [0438]` |
| | L207 `[0BCB]` → `[0D45]` (level 0) | SAFE | no | `local-207.txt [0BCB]` |
| | L208, var 19 1 → 6 | not skippable | no | `local-208.txt [0005]`, `[005F]`; `local-207.txt [03CE]` to `[03D6]` |
| A12 `walk-out-of-tent-after-helmet-meat` | L200 helmet `[0039]`; L203 cannon `[0000]` | NO-OVERRIDE | no | `local-200.txt [0039]`, `local-203.txt [0000]` |
| | L207 `[0D61]`/`[0DE0]` → `[0E5D]` | SAFE | no | `local-207.txt [0DE0]` |
| | L207 `[1041]` → `[1107]` (level 0) | SAFE | no | `local-207.txt [1041]` |
| | L207 `[1125]`/`[1127]` → `[1136]` | SAFE | no | `local-207.txt [1127]` |
| A20 `walk dock low-street` | entry walk-in `[00DE]` | NO-OVERRIDE | no | `room-035-low-stree/entry.txt [00DE]` |
| A21 `buy-map` | L218 `[0398]` | NO-OVERRIDE | no | `room-035-low-stree/local-218.txt [0398]` |
| | L218 `[06AF]`/`[06B4]` → `[0792]` | SAFE | no | `local-218.txt [06B4]` |
| A22 `walk low-street high-street-town` | obj 451 `[000C]` | NO-OVERRIDE | no | `room-035-low-stree/obj-0451-archway.txt [000C]` |
| A24 `give-meat-to-prisoner` | L203 walk `[0082]` | NO-OVERRIDE | no | `room-031-jail/local-203.txt [0082]` |
| | L203 `[029D]`/`[02A3]` → `[0343]` | SAFE | no | `local-203.txt [02A3]`, `[0351]` |
| A24 tie `talk-to-prisoner` | L202 `[001E]` | NO-OVERRIDE | no | `room-031-jail/local-202.txt [001E]` |
| | L202 `[18DE]`/`[18E3]` → `[19C0]` | STATE-DIFF (ego position) | no | `local-202.txt [18E3]` |
| A30 `pay-for-shovel-and-mints` | L204 catch `[004E]` | NO-OVERRIDE | no | `room-030-store/local-204.txt [004E]` |
| | L211 `[0910]`/`[0915]` → `[09BD]` | SAFE | no | `room-030-store/local-211.txt [0915]` |
| | L211 `[1B23]`/`[1B28]` → `[1B7F]` | SAFE | no | `local-211.txt [1B28]` |
| | L211 `[1DD5]`/`[1DDA]` → `[1ECF]` | SAFE | no | `local-211.txt [1DDA]` |
| A32, A47, A41 (high street walks) | obj 436 `[001E]`; obj 435 `[000C]` | NO-OVERRIDE | no | `room-034-high-stre/obj-0436-archway.txt [001E]`, `obj-0435-town.txt [000C]` |
| A34 `give-meat-to-poodles` | L201 `[0010]` | NO-OVERRIDE | no | `room-036-mansion-e/local-201.txt [0010]` |
| A38 `enter-idol-room` | L210 `[0000]`/`[0007]` → `[042F]` | STATE-DIFF (630 owner, names, verb 60); first-frame Esc required | no | `room-053-foyer/local-210.txt [0007]`, `[0436]` |
| A43 `give-mints-to-prisoner` | L203 `[0082]`; L202 `[0066]` | NO-OVERRIDE | no | `room-031-jail/local-203.txt [0082]`, `local-202.txt [0066]` |
| | L203 `[00C6]`/`[00D8]` → `[0111]` | SAFE | no | `local-203.txt [00D8]` |
| | L202 `[138B]`/`[1390]` → `[13A8]` | SAFE | no | `local-202.txt [1390]` |
| A44 `give-repellent-to-prisoner` | L203 `[0082]`, `[0128]` | NO-OVERRIDE | no | `local-203.txt [0128]` |
| A50 `steal-idol` | L211 `[0000]`/`[000F]` → `[019E]` | STATE-DIFF (641 class, names, verb 60) | no | `room-053-foyer/local-211.txt [000F]` |
| | L212 `[0000]`/`[0015]` → `[0242]` | SAFE at step end | no | `local-212.txt [0015]` |
| | L212 `[04BB]`/`[04D3]` → `[0857]` | SAFE | no | `local-212.txt [04D3]` |
| | G119 `[0000]` → `[08A1]` (level 0) | SAFE; **choose: drop `Uh`, `Um`, `Blfft`** | no | `global/script-119.txt [0000]`, `[01BB]`, `[038E]`, `[05D2]` |
| A51 `walk-past-fester-to-underwater` | L217 `[000F]` → `[034B]` (level 0) | SAFE; **choose: drop `Buzz off`** | no | `room-053-foyer/local-217.txt [000F]`, `[006E]` |
| | G65 `[0000]`/`[0005]` → `[01AC]`; `[01B9]` → `[0232]` | SAFE (two Escs) | no | `global/script-065.txt [0005]`, `[01B9]` |
| A52 `walk-up-ladder-taking-idol` | L203 `[0000]`; L200 `[003F]` (goal Bit[85]) | NO-OVERRIDE | no | `room-042-underwate/local-203.txt [0000]`, `local-200.txt [003F]` |
| | L201 (room 83) `[0000]`/`[0017]` → `[0775]` | STATE-DIFF (ego position, palette) | no | `room-083-cu-dock/local-201.txt [0017]` |
| A66 `dig-treasure` | L200 `[0000]`/`[0005]` → `[01A7]` | SAFE | no | `room-064-treasure/local-200.txt [0005]` |
| pre-segment | G152 `[004F]` → `[08C8]` | **BANNED** (never injected) | n/a | `global/script-152.txt [004F]`, `room-010-logo/local-204.txt [0000]`/`[0097]` |
| pre-segment | lookout `[0005]`; Part One card `[0004]` | not injected | n/a | `room-038-lookout/local-203.txt [0005]`, `room-096-part1/local-200.txt [0004]` |
