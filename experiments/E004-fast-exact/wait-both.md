# A wait the reading cannot test both blocks and passes (`wait_both`)

A reader switch (reader-scope and reader-port `--switches wait_both`, read-rust `--switches`). Proof before the code,
for any SCUMM v5 game. Off by default: off, the reading is the one before, byte for byte.

## What the reader does without it

The machine meets a WaitFor op (o5_wait) inside a run. The reader (machine.rs `Reader::wait`) has two cases:

- the op is the wait the event resumes at (`resume_wait`): it falls through, with the check "`<op>` blocks" on the
  side false. This is the event's own start, the later part of the wait;
- any other WaitFor op: the slot yields there, and the record ends with the slot at the op. The run stops.

So the reader never lets a run go past a WaitFor op it meets. The four subops, and what each reads in the engine:

| subop | o5_wait (script_v5.cpp:3280-3302) blocks when | held by a fact of the reading |
|---|---|---|
| WaitForActor | the actor is moving (`Actor::_moving`) | no: no fact key holds an actor's moving state (scumm_ir `Fact`) |
| WaitForMessage | VAR_HAVE_MSG (var 3) is not 0 | no: the engine writes var 3 every frame (scumm.cpp:3382), domains::ENGINE |
| WaitForCamera | the camera's x / 8 differs from its target's x / 8 | no: neither the camera's position nor its target is a fact |
| WaitForSentence | a sentence is queued and (the top one is not frozen or the sentence script is in use), or none is queued and the sentence script is in use | in part: whether the sentence script is in use is a fact (`script:N`, `objscript:N` for N = VAR_SENTENCE_SCRIPT); the queue is not |

The strict window search found the engine passing such a wait: walk_to(428)'s sentence ran past WaitForActor at
[02E0] of the sentence script because ego was not moving, and went through the door in the same run. No record of the
reading holds that run, so every child of the window failed on it.

## The rule

With the switch, a WaitFor op the run does not resume at takes both branches:

- **blocks**: the slot yields at the op, and the record ends there, as without the switch;
- **passes**: the run goes on at the next instruction in the same run, and the record carries the check "`<op>`
  blocks" on the side false (the check the event that resumes there already carries).

Where the path holds the test, it takes that branch alone. Of the four, only WaitForSentence can be held:

- the sentence script is in use (a fact the path reads, which may fork on its values as any read of a fact): it
  blocks, whatever the queue holds;
- not in use, and the path itself queued a sentence (doSentence pushes at `_sentenceNum`, script.cpp:1164-1165; only
  checkAndRunSentenceScript pops, script.cpp:1193-1194): the top sentence is the path's last one, and the op blocks
  exactly when it is not frozen;
- not in use, and the path queued none: the queue the run began with decides, and the path does not hold it. Both.

## Lemma W (an over-approximation, with the same facts)

*For every engine run of a unit from a decision point, if the reading without the switch holds a path that agrees
with it up to a WaitFor op, the reading with the switch holds a path that agrees with it up to the op's outcome and
past it.*

*Proof.* o5_wait either returns, having written nothing (each case ends in `return` before any write), and the engine
executes the next opcode in the same `executeScript` call; or it rewinds `_scriptPointer` to the op and calls
o5_breakHere, which ends the run with the slot at the op (script_v5.cpp:3308-3309). These are the two branches of the
rule. The blocking branch is the path without the switch. The passing branch starts from the same machine state, which
the op did not change, at the next instruction, which is where the engine goes on. Where the rule keeps one branch, the
path's facts give the engine's outcome: with the sentence script in use both arms of case 4 break; with it not in use
and a sentence the path queued on top, the arm with a queue tests that sentence's freeze count, which the path holds. □

So the switch only adds paths, except at a WaitForSentence the path decides, where it drops the branch the engine
cannot take. The model stays an over-approximation of the engine (A1): every engine run the reading held, it still
holds, and the runs that pass a wait in the same run are added. No fact is added, and none changes meaning.

## What it costs

Each WaitFor op a run meets can now double its paths past that point: a run through k such ops in a row has up to k + 1
records where it had 1. The records, the read CPU and the peak memory are measured on the kitchen, the bar, k = 1 and
k = 2 with `--reach` and the kitchen's switches, with the switch off and on.

## The check

- Off: `reader-port read kitchen --packed` writes the packed file with sha1 279b7a24, as before; the reader's tests pass.
- On: the sentence script's [02E0] has a record that passes it in the same run as walk_to(428) on the kitchen reading,
  and the strict window search runs on that reading from boot.
