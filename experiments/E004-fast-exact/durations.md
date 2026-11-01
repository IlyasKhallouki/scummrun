# Duration rules in game time (plan step C2)

Each record of a reading gets a lower bound on the game time before it can happen, in quarter-frames. The bounds feed
the timed search (`timed.md`, plan step C3) and B1's wait check. The proofs come first and hold for any SCUMM v5 game
run under the pinned configuration K (enhancements off). Numbers of one game appear only in the tests and logs.

Behind taskcore's switch `--timed` (off by default). With it off nothing taskcore, the reader or the harness writes
changes.

## The clock and what a bound is

- **The clock.** Q(k) is the bridge's run clock after frame k's wait: the quarter-frames summed in
  `SpeedrunBridge::onWait` (`speedrun_bridge.cpp:415-418`). Every engine wait goes through `waitForTimer`
  (`scumm.cpp:2919-2920`). A frame is one pass of the main loop: `waitForTimer(delta * 4)` (`scumm.cpp:2889`), then
  `scummLoop(delta)` (`:2893`). Fades and room changes wait more, outside frames; they only add to Q.
- **A record's time.** An event runs in the frame of its slot's run. An input runs in the frame its last key or click
  lands. A model run maps every engine run in V to a sequence of records (A1), each at its frame.
- **An event's entry.** An event e resumes slot U at wait point W. Its entry is the record that left U's slot standing
  at W: the slot's previous run, or the run that started U and ran it to W (`runScriptNested`, `script.cpp:344-372`).
  Let j be the entry's frame and k the event's.
- **A bound.** lb(e) is a bound when every engine run in V has Q(k) − Q(j) ≥ lb(e). For an input, the reference is the
  previous input record (D5).

`dmax` and `charinc` are costs.rs's: the most a frame takes off a delay in the scope, and VAR_CHARINC where no unit
writes it. F(r) is costs.rs's frame cost of record r (`--frame-costs`): the frames costs.rs proves its wait takes.

## D1. Frames

A frame k costs q_k = Q(k) − Q(k − 1) ≥ 4 · max(1, V_k) quarter-frames, where V_k is VAR(VAR_TIMER_NEXT) when frame
k's loop pass starts (v5: var 19, `vars.cpp:55`).

*Proof.* The loop reads `delta = VAR(VAR_TIMER_NEXT)` and raises it to 1 (`scumm.cpp:2821`, `:2834-2835`). Under K
the enhancement and other-game branches (`:2836-2884`: v0, Maniac v1, the enhancement rooms of other games, v8) do not
apply. The FM-Towns scroll adjustment gives max(0, δ − a) + ⌊4 · min(a, δ) / 3⌋ ≥ δ. Then `waitForTimer(delta * 4)`
adds 4 · delta to Q before `scummLoop` runs. ∎

**δ_lo**, the least frame value of a scope: max(1, s19) when the scope's write set (`scope.rs` `scope_writes`, which
holds every write the reader can see, by constant or folded index, and windows of unfolded indexed writes) does not hold
var 19, where s19 is var 19 at the start; otherwise max(1, the least value of var 19's domain) when var 19 is a fact of
the task with integer values only, and 1 when it is not. The engine itself writes VAR_TIMER_NEXT nowhere under K (its
other writes are other games' enhancements, `script.cpp:766`, `:786`). So q_k ≥ 4 · δ_lo on every frame of a run
that stays in the scope, and Q(k) − Q(j) ≥ 4 · δ_lo · (k − j).

## D2. One frame between two runs of a slot

k ≥ j + 1 for every event: a slot that stood at W after a run in frame j runs again no earlier than frame j + 1. So
lb(e) = 4 · δ_lo holds for every event.

*Proof.* A slot runs again only from `runAllScripts` (`script.cpp:968-987`): a nested run starts a new slot, and the
resume after a nested call (`:377-384`) continues a run that has not yielded. `runAllScripts` clears `didexec` once,
at its start (`:971-972`), runs only slots with `didexec` false (`:979`), and every opcode a slot executes sets its
`didexec` (`:506-507`), nested runs included. In v5 it runs once a frame (`scumm.cpp:3230-3232`). A slot that yields
in frame j has `didexec` set for the rest of frame j, and slots started after frame j's `runAllScripts` (input,
sentence) next run in frame j + 1's. ∎

## D3. A frame bound in game time

If costs.rs proves k − j ≥ F(e), then Q(k) − Q(j) ≥ 4 · δ_lo · F(e) (D1). This covers breakHere and the WaitFor ops
(F = 1, D2) and the delays (D4). It does not cover a talk wait under the talk-stop key (D6), a recursive unit (D7), or
a slot already waiting at the start (D8).

## D4. Delays

A delay of n jiffies (`o5_delay`; `o5_delayVariable` with a value of at least n) entered in frame j lasts
Q(k) − Q(j) ≥ 4 · max(δ_lo · (⌊n / dmax⌋ + 1), n + 1).

*Proof.* The opcode pauses the slot with delay n and yields (`script_v5.cpp:972-986`). Each later frame f first takes
d_f = min(15, max(1, V_f)) off it (`scumm.cpp:3129-3132`, `decreaseScriptDelay`, `script.cpp:1513-1542`), and the slot
is running again in the first frame k where the sum of the d_f exceeds n. That frame's `runAllScripts` comes after
`decreaseScriptDelay` in `scummLoop`. A freeze only lengthens the wait; a stop ends the slot, and its next run is
another wait. So Σ_{f=j+1..k} d_f ≥ n + 1, and by D1 Q(k) − Q(j) ≥ Σ 4 · max(1, V_f) ≥ 4 · Σ d_f ≥ 4 · (n + 1). The
frames are at least ⌊n / dmax⌋ + 1 (costs.rs), each at least 4 · δ_lo. ∎

From the frame cost alone: costs.rs gives F = ⌊n / dmax⌋ + 1 for the least n it proves, so n ≥ dmax · (F − 1), and
lb = 4 · max(δ_lo · F, dmax · (F − 1) + 1). With one value of var 19 (δ_lo = dmax ≤ 15) this is 4 · δ_lo · F.

## D5. Inputs, and '.'

Let r be an input record that plays c(r) keys and clicks (costs.rs: a sentence its verb key and one click per object,
a walk or a dialog line one click, a skipped cutscene the Esc key), and r' the input record before it in the run.
Then r's frame is at least c(r) frames after r''s, or after the start for the first input: lb = 4 · δ_lo · c(r) from
the previous input.

*Proof.* `processInput` sets one input state a frame (`input.cpp:439-459`): a left or right click, or both as Esc;
`processKeyboard` then puts a key in the same variable (`:1476-1530`), so a key and a click in one frame leave one of
them. `checkExecVerbs` reads that state once a frame (`scumm.cpp:3232`). V plays each input record's keys and clicks
in order, one record after the other, so r's c(r) keys and clicks land in c(r) distinct frames after r''s last
one. ∎

The talk-stop key '.' is not one of them. Its branch only sets `_talkDelay = 0` (`input.cpp:1418-1421`) and leaves the
frame's click as it was, so '.' takes no frame of the chain. The reader has no '.' record: a '.' changes no fact. Its
one effect on time is a talk wait cut short, which D6 accounts for.

## D6. Talk waits

costs.rs's bound for a WaitForMessage after a talk, Σ_parts max(1, ⌈T_j / dmax⌉) + 1 frames, holds only while nothing
else ends or replaces the message. The engine ends it early when:

- the talk-stop key '.' is pressed while VAR(VAR_TALKSTOP_KEY) ≠ 0 (v5: var 57; `input.cpp:965`, `:1418-1421`);
- another talk on text slot 0 starts (`actorTalk`, `actor.cpp:3529-3540`);
- `stopTalk` runs: a room change (`room.cpp:53`), the talking actor put in another room (`actor.cpp:1730-1733`,
  `script_v5.cpp:2166-2168`), or the camera moving while text shows (`camera.cpp:58-59`), which needs no record at all.

**Rule.** A talk wait keeps costs.rs's frames only where the task shows all of: (a) the talk-stop key off for the
whole wait (VAR_TALKSTOP_KEY 0 at the start and not in the scope's write set); (b) no other record fires during the
wait; (c) a camera that cannot move during the wait. Otherwise it is D2's 1 frame.

*Proof.* If none of the ways above happens, the message runs its course as costs.rs proves. (a) rules out the key,
(b) every other talk and every `stopTalk` a script runs (a record would show it), and (c) the camera's. ∎

This version establishes (a) from the reading's start and write set; it does not establish (c), so every talk wait is
1 frame. Under V with '.', a start whose var 57 is not 0 makes every talk wait 1 frame by (a) alone.

## D7. Recursive units

A unit U that can run as two instances has events with lb = 0.

*Proof (why).* The model keeps one place `pos:U` per unit. `runScript` stops a running instance first unless the start
is recursive (`script.cpp:65-66`), so two instances need a recursive start: `o5_startScript` with bit 0x40
(`script_v5.cpp:2971`), or `o5_chainScript` from a recursive slot, which passes its flag on (`:850`; the engine's
copies of chainScript for single games do the same, `scumm.cpp:3651`, `:3755`). The engine's own starts are not
recursive (`script.cpp:906-1686`, `input.cpp:557-1410`, `scumm.cpp:3299`, `actor.cpp:931`, `:3558`), and v5 object
scripts stop the object's running script first (`script.cpp:117-118`, `script_v5.cpp:2889`). With two instances, an event of one can
follow the other's entry, so no positive bound on the time since the last write of `pos:U` holds. lb = 0 is a bound. ∎

The recursive units of a game: the targets of every recursive start site, closed under chainScript from them; a site
whose script number does not fold makes every global and local script recursive.

## D8. A slot already waiting at the start

A slot that stands at W at the start s has waited an unknown part of its wait. From s, its next event e has
lb(e) = cap = 4 · max(δ_lo · (⌊d / dmax⌋ + 1), d + 1) for a slot paused with d jiffies left in the start's dump, and
lb(e) = min(lb(e) from D2-D7, 4 · δ_lo) for any other slot (0 for D7's units, whatever the dump shows).

*Proof.* The dump is taken after frame s's scripts, so D2 gives one frame from s. A paused slot has delay d left, and
D4's proof from frame s gives the cap whatever the code before W says: the delay left is the one the engine counts
down. A unit of D7 has no bound. ∎

These are replay.py's cost caps (`cost_caps_of`, frames F = ⌊d / dmax⌋ + 1, so d ≥ dmax · (F − 1)) in game time:
cap = 4 · max(δ_lo · F, dmax · (F − 1) + 1). The same caps apply at every engine node of C5.

## D9. Walks: ego_at and the walk floor

Let e resume slot U at a wait point W that is a `WaitForActor(A)` whose straight code before it, with no yield between,
is `walkActorToObject(A, X)`; let the event's locals give A the ego's actor number and X the object o. Its entry ran
the walk op in frame g. Let T be o's walk point in the room (`getObjectXYPos`, the object's CDHD walk_x and walk_y).

**ego_at** is a fact of the timed search with values {a point, unknown}: the ego's position at the start (the dump),
T after such an event when (W2) and (W3) below held for its walk, and unknown after any record that may move the ego
(below).

**Lemma (window-search.md Lemma 6, on origin/window2).** If ego_at = p when the walk begins and

- (W1) the ego's walk speed is at most (sx, sy) until it arrives;
- (W2) no record that may move the ego fires between e's entry and e;
- (W3) T lies in a box of the room that `adjustXYToBeInBox` keeps (not invisible, not locked);
- (W4) the room has at most L boxes, so the walk has at most L legs;

then k − g ≥ D = max(⌈|T.x − p.x| / (sx + 1)⌉, ⌈|T.y − p.y| / sy⌉) − (L − 1), and lb(e) = 4 · δ_lo · max(1, D).
The proof is Lemma 6's: one call of `actorWalkStep` moves x by at most sx + 1 and y by at most sy (`actor.cpp:633-682`,
`calcMovementFactor` `:520-576`); a frame holds k moves only if k − 1 of them end a leg exactly (`walkActor`,
`:944-1015`); the first move comes in frame g's `walkActors`, after the scripts (`scumm.cpp:3255-3284`); and the wait
sees the arrival one frame later.

*May move the ego:* every input record (the input, sentence and verb scripts it runs walk and put the ego), every record
of a unit whose code, or the code of a unit it can start, holds a walk, put, stop or animate op on an actor that may be
the ego (VAR_EGO, a local or variable that does not fold, or the ego's number), a room load, or a write of VAR_EGO.

*Premises the search must establish before it uses the rule.* W1 needs the ego's speed at the start, which the state
dump does not give, and every walk-speed op that can reach the ego in the scope. W3 needs the room's box data and the
box flags the scope can set. Until they are established, a walk wait is D2's 1 frame; this version uses D2 there.

## The rule table

| record | lb (quarter-frames) | reference | lemma |
|---|---|---|---|
| event, recursive unit | 0 | its entry | D7 |
| event, talk wait | 4 · δ_lo (frames only under D6's (a)-(c)) | its entry | D6 |
| event, delay or delayVariable | 4 · max(δ_lo · F, dmax · (F − 1) + 1) | its entry | D4 |
| event, walk wait | 4 · δ_lo (D9's floor once W1-W4 are shown) | its entry | D2, D9 |
| any other event | 4 · δ_lo · F | its entry | D2, D3 |
| a slot waiting at the start | the cap if paused, else min(the above, 4 · δ_lo) | the start | D8 |
| input | 4 · δ_lo · c(r) | the previous input, or the start | D5 |

A name that the task's labels share between units of different rooms (`local:N`) names the current room's unit: a room
change ends every local script (`startScene` → `killScriptsAndResources`, `room.cpp:80`, `script.cpp:1083-1105`), so
a live `local:N` slot belongs to the room the task's `room` fact holds. Where the room is not known, the name gets the
least bound of its units.

## The test

B1's wait check (`trace_check.py`, trace-validation.md Lemma W) on every committed run, in game time: each slot run that
resumes at the wait point where its previous run left it is a wait, Q(k) − Q(j) is measured on the run's own clock, and
the wait must be at least its rule's bound. The check counts, per rule, the waits it covered and the least slack. Runs:
the kitchen window runs (from the harness's boot and from run B's S1#7 start), the bar door window run, and run B's
recorded inputs on seed 1.
