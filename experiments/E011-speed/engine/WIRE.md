# The engine channel F1 and the state block F2, as built

Both ends belong to the `engine` component: the engine side is patch 0002
(`patches/0002-scumm-speedrun-lean.patch`, `engines/scumm/speedrun/speedrun_wire.cpp`), the controller side is
`experiments/E011-speed/rs/fmi-engine` (`wire.rs`, `block.rs`, `host.rs`). This file is what plan.md's "Binary
formats" fixed, plus what plan.md left to this component: the StepReply's layout and delta, the End status, and how a
fork's connection is found.

## Choosing the channel

- `SPEEDRUN_WIRE=bin` selects F1. Unset or `json` keeps the JSON lines protocol of patch 0001 (`speedrun_control.cpp`),
  unchanged. Any other value is a `bad_env` error.
- Every process of a run speaks the same channel: a fork inherits the setting.
- The commands mean what the JSON commands mean, with the same checks and effects. Only the encoding differs, plus two
  messages the JSON protocol has no twin of: `State` (the F2 block) and `Step`.

## Encoding

- **Framing.** u32 payload length (little-endian), u8 message type, then the payload. The length does not count the
  5 header bytes. The engine refuses a payload over 64 MB by closing the connection.
- **Payload.** fmi-core's types (plan.md Appendix A) in declaration order:
  - integers at their Rust width, little-endian; `bool` u8 0 or 1;
  - `Option<T>`: u8 0, or u8 1 then T;
  - `Vec<T>`: u32 count, then the items; `String`: u32 byte length, then UTF-8;
  - a tuple: its items in order; an enum: u8 variant index (declaration order), then its fields.
- **Game text** (room object and verb names, only with `ObsReq::names`): the bytes as the JSON dump's string decodes,
  bytes 0x7F and up being U+007F..U+00FF, as UTF-8.
- **Int casts.** Where the JSON text casts a value to `int` (a run's frame, seq, code0, code1, ops; an explain entry's
  seq, frame, code; a fork_each child's frame and pid), the binary value is that int, widened. The values are therefore
  equal to what the JSON gives.

## Controller to engine

| type | message | payload | reply |
|---|---|---|---|
| 0x81 | Run | `RunSpec` | Stop or End |
| 0x82 | Inputs | `Vec<Event>` | Ok |
| 0x83 | Fork | none | Forked (the child connects on its own) |
| 0x84 | ForkEach | u32 n (1..64) | Stop "fork_each" (forks listed) or End |
| 0x85 | Obs | `ObsReq` | Obs |
| 0x86 | State | none | State |
| 0x87 | Holds | `Vec<Cond>` | Holds |
| 0x88 | At | i32 x, i32 y | At |
| 0x89 | SetRng | u32 (not 0) | Ok |
| 0x8A | Segment | bool | Ok |
| 0x8B | Runs | none | Runs |
| 0x8C | Explain | `Option<ExplainArm>` (None disarms) | Ok |
| 0x8D | ExplainLog | none | Explain |
| 0x8E | StateJson | `ObsReq` | Json |
| 0x8F | Quit | none | none: the process exits at once |
| 0x90 | Step | u32 frames (at least 1), u8 with_runs, u8 with_explain | StepReply or End |

Any message can instead get `Error` (0x0C, `String`), with the JSON protocol's error text where there is one. After an
error the engine waits for the next command, as with JSON.

Checks as the JSON parsers make them:

- **Run** needs a stop (frame, max_frame, control, idle, menu, until or until_any); a frame must be after the current
  one.
- **Inputs:** frame > 0; clicks, right clicks and moves in 0..65535; a key name the C8 parser knows (`speedrunKeyName`,
  shared with it); frames not decreasing within the list, after the current frame and not before the last scheduled
  input.
- **Conditions:** only the forms the JSON protocol can express. `In` needs values and a var, bit, owner, state,
  actor_room or actor_moving kind. `Le` and `Ge` take those kinds or actor_x and actor_y. An actor kind needs an actor
  in 1..12, and a class condition `Eq` with a class in 1..32.

## Engine to controller

| type | message | payload |
|---|---|---|
| 0x01 | Hello | u32 pid, u32 parent (0 for the root), u64 frame, u64 tick |
| 0x02 | Stop | `Status` (`ended` false) |
| 0x03 | End | `Status` (`ended` true), see below |
| 0x04 | Forked | u32 child pid |
| 0x05 | Obs | `Obs` |
| 0x06 | State | the F2 block (the whole payload) |
| 0x07 | Holds | bool |
| 0x08 | At | `At` |
| 0x09 | Runs | `RunsReply` |
| 0x0A | Explain | `ExplainReply` |
| 0x0B | Ok | none |
| 0x0C | Error | `String` |
| 0x0D | Json | `String`: the C7 dump's text |
| 0x0E | StepReply | `Status`, the block field, `Option<RunsReply>`, `Option<ExplainReply>` |

- **Stop** carries `controlStatus()`'s fields. `idle` and `blk` follow its rule: one of the two is set. `ego`, `mv` and
  `walkbox` are set when the ego is in the room. `forks` lists (frame, pid) of the last fork_each's children.
- **End** carries the end record's reason, frame, tick, qf and room, plus `forks` (the children a fork_each made before
  the end, so the controller can quit them). Every other field is left at its default. A forked process exits after
  sending it, as with JSON.
- **Obs** is `stateJson()` (`ObsReq::objects` None) or `stateLiteJson(objects)`, field for field in `Obs`'s order.
  - A lite dump has no inventory and no room objects (`None`), as the JSON lite dump has neither key.
  - Names only with `names` (the JSON always has them).
  - The exact object only with `exact`, on a full dump. It is `exactJson()`, key for key with its types: ints `Int`;
    `seg` a `Bool`; `audio_owed` (`%.17g`) and `screens` (the digest) `Text`; arrays `List`; `cd` a `Map`.
  - Locals, resistant, recursive and a level's data only with SPEEDRUN_DUMP_LOCALS, as in JSON.
- **StateJson** is the JSON text the `state` command sends (without its `{"state": ...}` wrapper). It is the lite dump
  when `objects` is set, else the full dump, with the exact object when SPEEDRUN_DUMP_EXACT is on or `exact` is asked.
- **Runs** and **ExplainLog** clear the engine's buffers, as `runs` and `explain_log` do. With the binary channel the
  engine keeps runs and explain entries as values, not as JSON text.

## Step and its reply

`Step` sets the run's frame stop to now + frames and resumes. At that stop the engine sends one StepReply instead of a
Stop:

1. `Status`, reason "frame".
2. The block field, which is one of:
   - `u8 0`, then u32 length and the whole F2 block;
   - `u8 1`, then the delta from the last block sent on this connection: u32 new length, u32 chunk count, then per
     chunk u32 offset, u32 length and the bytes.
3. `Option<RunsReply>` when with_runs (the buffer is cleared, as Runs).
4. `Option<ExplainReply>` when with_explain (cleared, as ExplainLog).

How the delta works:

- The receiver takes its last block, cuts or pads it with zeros to the new length, and writes the chunks over it. Bytes
  past the old length always lie in a chunk.
- The engine sends the whole block when the delta would be larger, or when the connection has sent no block yet.
- Every block the engine sends, in a State or a StepReply, becomes "the last block" on both sides.
- A new connection (the root's, or a fork's) starts with none, so a fork's first block is whole.

A run that ends during a Step replies End.

## Forks

- Fork: the parent replies Forked(pid). The child connects to the same socket, sends Hello (its pid, the parent's pid,
  frame, tick) and then a Stop "forked" status.
- ForkEach: each child connects as it is made. The parent's Stop "fork_each" comes after the last child, and its
  `forks` names them.
- Connections may arrive in any order. The controller (`fmi-engine`'s `Host`) keeps one listening socket for all the
  engines it starts and matches each connection to its process by the Hello's pid.
- A fork child drops whatever the parent had received but not yet read.

## F2, the state block (version 1)

```text
u32 magic 0x31534D46 ("FMS1"), u32 version = 1
u64 frame, u64 tick, u64 rng                      the bridge's frame and tick; the RNG state (_rnd.getSeed())
i32 room, i32 ego (VAR_EGO's value), i32 cutscene_depth
u32 n_vars;  i32 vars[n_vars]                     _scummVars, every index
u32 n_bits;  u8  bits[(n_bits + 7) / 8]           _bitVars as held: bit n at byte n >> 3, mask 1 << (n & 7)
u32 n_objs;  u8 owner[n_objs]; u8 state[n_objs]; u32 class[n_objs]
                                                  index = object number (0 unused); class bit c-1 = class c
u32 n_slots; { i32 slot, i32 number, i32 where, i32 status, i64 code, i32 frozen }[n_slots]
                                                  live slots (status not dead), slot order; code = scriptCode()
u8  has_override; { i32 where, i32 number, i64 code, u8 stale }
                                                  the top level's override (level min(depth, 4)): present when its
                                                  pointer is set and its slot below NUM_SCRIPT_SLOT, as the dump's
u32 n_verbs; i32 verb_id[n_verbs]                 visible verbs (verbVisible), slot order
```

Its facts (`fmi_engine::block::Block::facts`) are `facts_of` of the full dump at the same stop, pair for pair. They
are built in `Key` order with no text and no full sort: vars, bits, owners, states and classes in index order, then
the slot keys sorted and deduplicated, the cutscene depth, the override and the verbs. G4 checks this at every dump
and every walked frame.

## The lean switches (also patch 0002)

The lean switches are bridge settings, all off by default:

- `SPEEDRUN_LEAN=1` turns on all four.
- Each also alone:
  - `SPEEDRUN_NO_SPLASH`: no splash wait;
  - `SPEEDRUN_NO_PRESENT`: `waitForTimer` skips `updateScreen`;
  - `SPEEDRUN_NO_EVENTS`: `waitForTimer` skips `parseEvents`;
  - `SPEEDRUN_SILENT_AUDIO`: the OPL chip writes zeros with its callback timing, and a FLAC stream returns zeros with
    the decoder's per-call counts and end-of-data rule.

`fmi-engine`'s launcher also writes `joystick_num=-1` into the ini when lean. The Python reference keeps its ini.
`svm-lean.sh` sets the switches for the Python reference's runs, whose launcher drops the caller's SPEEDRUN_*
variables.
