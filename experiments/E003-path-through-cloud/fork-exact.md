# Fork exactness under load (plan step A0)

The window search (A4) and the exact frame search (C5) treat a forked child as the run in which the input lands on
that frame. This note states when that holds, for any SCUMM v5 game under the bridge, and how `fork_exact.py` tests
it. It was written before the code.

## Setting

- E is the patched engine under a pinned configuration K: the bridge on (`SPEEDRUN_OUT`), `--disable-sdl-audio` so
  the bridge pumps the mixer from `onFrameBegin` (`pumpAudio`), `SPEEDRUN_FAST`, no save or load (the bridge stops a
  run that asks for one), a fixed seed and a fixed music driver.
- I is a raw input schedule (C8 events on fixed frames, `SPEEDRUN_INPUTS`).
- A stop is the decision point of a frame (`SpeedrunBridge::onDecisionPoint`, after `checkExecVerbs`).
- P is E booted with (K, I) and stopped at frame n. C is the child of P's `fork` command at that stop. F is a fresh E
  booted with (K, I) that never forks.
- The game state is everything the next frames' game logic reads: the engine's memory, the open data file, the CD
  and sound streams, the RNG.

## Claim

For every frame m ≥ n, C's game state at the stop of frame m equals F's. So does P's, after any number of forks.

## Premises

- **G1. One thread carries the game.** Under K every write to the game state happens on the main thread:
  - the frame loop and the scripts;
  - the mixer callback, which the bridge calls itself (`pumpAudio`), and with it the music driver's timer when the
    driver is an emulated chip (the OPL emulator runs its timer callbacks inside its audio stream) or none;
  - the CD music timer and the speech timer, which the bridge advances in `onWait` (`advanceTimers`), since
    `startCDTimer` and `startSpeechTimer` return before installing a timer proc when the bridge is on.

  G1 fails for drivers that install a `TimerManager` proc: hardware MIDI (`audio/mpu401.cpp`), a real OPL chip
  (`audio/chip.cpp`, RealChip) and MT-32 emulation (`audio/softsynth/mt32.cpp`). So K must pin an emulated chip or
  none. The boot record's `music_type` names it.
- **G2. No wall clock in the game state.** With `SPEEDRUN_FAST`, `waitForTimer` does not sleep and a frame's length
  is var 19 (`VAR_TIMER_NEXT`), not a clock reading. `getMillis` is read for pacing and for the play-time counter.
  The premise is that no clock value reaches anything a script reads. Under load, F and C run at different wall
  speeds, so a leak shows up as a difference.
- **G3. Determinism.** E's next state is a function of its game state and I. Verify from boot has shown this on runs
  with no fork.

## Proof

1. `fork()` copies the whole address space. The child keeps only the thread that called `fork`. File descriptors are
   shared with the parent, with their offsets. The PID differs.
2. By G1 the threads the child lacks write nothing the next frames read. In P they stay, and by G1 they write nothing
   there either.
3. The RNG (`ScummEngine::_rnd`, a 32-bit xorshift state) is engine memory, so the child has P's.
4. Shared files.
   - The CD tracks are decoded from memory (`SeekableAudioStream::_loadStreamFilesInMemory`, set by the bridge), so
     no track file offset is shared.
   - Sound and speech data files get a new handle for each sound (`sound.cpp`), so none is open across the fork.
   - The data file is the one shared descriptor that game logic reads. The bridge closes it in P just before `fork`
     (`closeRoom`). Lemma F1 shows that this close is invisible.
5. By G2 and G3, from equal game states and the same I, F and C (and P) make the same next state. By induction over
   frames they agree at every stop after the first room open that follows the fork. Before that open, nothing reads
   what `closeRoom` changed (F1).

**Lemma F1.** Take a state S and the state S' that `closeRoom` makes from it. After the next call `openRoom(r)`, the
two agree on every member game logic reads.

- *What `closeRoom` changes* (`resource.cpp:139-145`):
  - `_lastLoadedRoom`, which becomes -1;
  - the room offset table, where `deleteRoomOffsets` sets every valid entry to 0;
  - the data file, which is closed.
- *Who reads them.* `_lastLoadedRoom` and the room offset table are read only in `openRoom`, `closeRoom` and
  `deleteRoomOffsets` (`resource.cpp:73-151`). The data file is read only after an `openRoom`.
- *From S.* `openRoom(r)` returns at once if `_lastLoadedRoom == r`. Otherwise, if r's entry is valid and nonzero, it
  sets `_fileOffset` to it. Otherwise it opens r's disk file and reads its offset table (`readRoomsOffsets`).
- *From S'.* `_lastLoadedRoom` is -1 and r's entry is 0, so `openRoom(r)` opens r's disk file and reads its offset
  table. These are the same bytes S's table was read from when S last opened that file, provided the file has not
  changed. So `_fileOffset` and the table entries come out equal, and `_lastLoadedRoom` is r in both.
- *The file position.* The position differs between the two. Every read of resource data seeks to an absolute
  position (`_fileOffset` plus the resource's offset) before it reads, so the position is never read.

  This is the step the test checks most directly. P and C both continue from S', and F continues from S.

## What the test sees

The test compares a projection of the game state, not all of it.

- **The state dump** (C7, `stateJson`), with `SPEEDRUN_DUMP_LOCALS`:
  - vars, bits, owners, states and classes;
  - the room's objects and the visible verbs;
  - the live slots, with their offsets, delays, freeze counts and locals;
  - the cutscene stack;
  - the room's visible actors.
- **The stop status:** the frame, ticks and quarter-frames, the RNG, room, userput, cursor, cutscene depth, var 19,
  ego's position, walk box and moving state, the camera and the idle rule.
- **The new switch `SPEEDRUN_DUMP_EXACT`.** The dump gains one object, `exact`:
  - the RNG state;
  - the bridge clock: quarter-frames, and the segment start's tick and quarter-frames;
  - the raw input cursor and the mouse and button state;
  - the audio pump's counters;
  - the CD music timer and its quarter-frame phase, the music timer, the current CD sound, the CD manager's status
    and `pollCD`;
  - the speech timer;
  - every sound `isSoundRunning` calls running, with its status;
  - the sound and MIDI queues;
  - the camera;
  - a 64-bit FNV-1a digest of the four virtual screens (front and back buffers, and their scroll offsets) and the
    palette. This covers costume frames and anything else that is drawn.

Not seen: what never reaches these within the span. That includes walk-path internals of actors not drawn, iMuse's
internals other than sound status, and resource contents such as arrays and strings, until a script reads them into
a var. A full save-stream digest would cover these too. C5 needs one, with its own lemma, because the serializer is
not shown free of side effects in save mode.

**Lemma D (the switch changes nothing else).**

- The switch is read in `readEnv` and in `stateJson` only. `stateJson` is a `const` member function, and it runs only
  for the control socket's `state` command and the two state files.
- Each new field is one of these:
  - a member read (the bridge's own members; `ScummEngine` members, as the bridge is its friend; `Sound` and `SoundCD`
    members, as the bridge is `Sound`'s friend);
  - a call to a `const` member function: `RandomSource::getSeed`, `Sound::isSoundRunning`, `Sound::pollCD`,
    `AudioCDManager::getStatus`, `SoundCD::getMusicTimer`;
  - a read of a virtual screen's or the palette's memory.
- The calls take and release the mixer's and iMuse's mutexes. That returns them to their state, and under G1 no
  other thread waits on them.
- The JSON text is heap memory that game logic never reads.
- With the switch off, the dump is byte for byte what it was.

`fork_exact.py check-dump` tests this:

- three engines in lockstep from boot, one frame at a time: the shared build, the new build with the switch off,
  and the new build with it on;
- at every frame, the stop status and the dump are compared, the switch-on dump with its `exact` object taken out;
- at the end, the three `trace.jsonl` files are compared.
