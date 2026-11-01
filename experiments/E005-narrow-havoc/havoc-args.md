# A computed start names what its callers can pass (`havoc_args`)

E005's change (B). The rule and its proof come first, for any SCUMM v5 game. MI1 numbers appear only in the tests and
the notes. Off by default; it needs `havoc_cut` (experiments/E004-fast-exact/havoc-cut.md).

## What it replaces

havoc-cut.md: "a number that does not fold to constants names every unit of the scope". havoc_starts narrows a start
by a variable to the variable's values. A start by a local (`startScript(Local[k])`) or by an indexed variable
(`startScript(Var[b + Local[k]])`) still names every unit, and with it the havoc record has an effect on every fact
the scope writes and an event at every wait point of every unit.

## The rule

A static analysis of the game's code, made once per reading (and again when pass 2 widens the domains):

1. **Values.** A local's values at an instruction are a set of up to 64 numbers, a range, or unknown. They come from
   a forward analysis of the unit's control flow (each instruction's `next`, `target` and `esc`):
   - at the unit's start, `Local[k]` is what its launches pass as argument `k` (below);
   - `Local[k] = e` gives it `e`'s values; `+=`, `-=`, `*=`, `/=`, `++`, `--` apply the arithmetic; `setVarRange` on
     locals gives each its item. These are the writes the reader's `written_locals` reads;
   - an engine result in `e` with a known range is that range: `findObject` an object number or 0, `[0, 65535]`
     (object.cpp `findObject`); `actorFromPos` an actor number or 0, `[0, NUM_ACTORS - 1]` (actor.cpp
     `getActorFromPos`); `getRandomNr(n)` `[0, n]` (script_v5.cpp `o5_getRandomNr`). Any other engine result is
     unknown;
   - a jump's condition that compares `Local[k]` with a constant, or tests its truth, narrows `Local[k]` on each edge:
     the fallthrough holds the condition, the jump its negation;
   - an indexed write to a local, an instruction the IR did not parse, or an indexed write to a variable whose index
     may leave `[0, 0x4000 - b)` (the index could reach the local range, `writeVar`'s `0x4000` bit) makes every local
     unknown;
   - a loop is widened after 8 rounds at an instruction (a bound that moves goes to infinity), then 2 rounds narrow it
     again from that fixpoint.
2. **Variables.** A variable's values follow the domain convention havoc_starts reads them by (havoc-cut.md, "Values"
   and "The switch havoc_starts"): a fact's domain when its values are plain, the start value of a variable no code of
   the scope writes, otherwise unknown. That convention reads an indexed write whose index does not fold as a window of
   16 variables from its base (scope.rs `var_windows`); a write past that window is outside the reading's model, here
   as everywhere in the reader.
3. **Launches.** Every start of a script in the game's code (every unit, not only the scope's, since a slot live at
   the start may have been started by code outside it): its number's values (a constant, or this analysis's values for
   a computed one) name the units it starts, as `runScript` finds them (a global below the first local number, else
   the room's local script: the caller's room for room code, any room otherwise). Its arguments' values at the call
   site are what it passes; an argument it does not pass is 0 (`getWordVararg` fills 25 zeros, script_v5.cpp). The
   engine's own starts pass arguments the analysis does not follow: a unit a hook variable can name (VAR_SCROLL_SCRIPT
   to VAR_CUTSCENE_END_SCRIPT, vars.cpp:62-71), and the boot script, have unknown arguments; room entry and exit code
   has none (`initializeLocals(slot, nullptr)`, script.cpp:1016, 1054), so its locals are 0. An object's code has
   unknown arguments (the analysis does not follow `startObject`'s).
4. **Fixpoint.** Steps 1 to 3 depend on each other (a computed start's values come from its unit's locals, which come
   from that unit's launches). The analysis starts from "no launch" and repeats until nothing changes; every step only
   adds values, so it ends.
5. **The start's names.** In a havoc closure, a start whose number does not fold, and that havoc_starts does not name
   by a variable's values, names the scripts of its number's values at that instruction: for `Local[k]`, the local's
   values; for `Var[b + X]`, the values of each variable `b + x` for `x` in `X`'s values. 0 names nothing
   (`runScript(0)` returns, script.cpp:62-63). Where the values are unknown, every unit, as before; READER_TRACE's havoc
   line names the start and the counter `havoc_args: starts not resolved, every unit` counts it.

## Why the record still covers every engine run of the path

havoc-cut.md's item 1 needs every instruction the continuation runs to be in `C(p)`. A start the continuation runs at
instruction `i` of unit `u` runs the script its operand's value names. The claim is that this value is in the set
step 5 gives:

- **The slot's locals.** Every slot the engine runs for a script was started by `runScript`: from `o5_startScript` or
  `o5_chainScript` with the call site's arguments, from a hook variable, from the boot, or for room code from
  `runEntryScript`/`runExitScript` with no arguments. Step 3 lists each kind, so the locals the slot starts with are
  in step 1's start values. A slot that is already live at the reading's start was also started this way, by code of
  the game, which step 3 reads whole.
- **Between the start and `i`.** Only the slot's own instructions write its locals (`writeVar` with the `0x4000` bit
  writes `vm.localvar[_currentScript]`). Step 1 follows every path of the unit's control flow, applies every write
  the IR shows to a local, makes the locals unknown where a write's target is not known, and narrows only by a
  condition that held on that edge. A yield does not change locals. So each path's value at `i` is in the
  analysis's set (abstract interpretation over sets and ranges; widening and narrowing from a fixpoint keep it an
  over-approximation).
- **The number.** A local operand is that value. `Var[b + X]` reads variable `b + x` for `X`'s value `x`, and that
  variable's value is in its domain by the convention every guess of the reader rests on.

So the script the engine starts is one step 5 names, and havoc-cut.md's items 2 to 5 read only `C(p)`.

## B in the exact reader

The exact reader does not treat a computed start as every unit. The machine leaves a start whose number it does not
know out of the path and lists it on the record (machine.rs `start`, `unknown_writes`; builder.rs counts it as
"unknown-target writes left out and listed on the record ... : startScript"). So havoc_args does not change the exact
reader; the Part I readings report that counter.

## Checks

1. Off: the kitchen's packed sha1 279b7a24; the reader's tests.
2. Tests: the value domain's own (join, arithmetic, a condition's narrowing, widening; `args.rs`), and
   `tests/havoc_args.rs` on Part I's scope: a cut in global 73 names global 58 and the room's local 200 and not every
   unit; a cut in global 23 bounds its index by the guards and names no unit for that start; with the switch off, both
   name every unit.
3. The soundness check of plan.md.
