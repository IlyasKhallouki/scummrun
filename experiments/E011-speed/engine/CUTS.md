# Round 2: the count-only mixer and no pixel output (patch 0003), and why neither reaches the game state

Round 1's lean build (patch 0002, all four switches) still spends most of a frame on two things the game never reads.
A flat `perf record` of the door chain from boot (fmi-engine verify, entry 5, 3,129 frames, cycles:u) on the round-1
build gives:

| what | share of the engine's user cycles |
|---|---:|
| `BaseCostumeRenderer::byleRLEDecode` (actor pixels) | 25.1% |
| `RateConverter_Impl::convert` (stereo CD track 13.5%, mono OPL 8.2%) | 21.7% |
| `Gdi::drawStripBasicH`, `drawStripBasicV`, `MajMinCodec::decodeLine`, `drawStripComplex` (room and object strips) | 7.5% |
| `ScummEngine::drawStripToScreen` (the text overlay and the copy to the backend) | 3.1% |

The profile priced the two cuts at 131 k cycles (the mixer) and 213 k cycles (the pixels) per frame
(`~/.cache/fast-mi-e011/profile-engine.md`, section 4).

Patch 0003 (`patches/0003-scumm-speedrun-cuts.patch`, on top of 0001 and 0002) adds them as bridge settings, off by
default. Off, the build behaves as round 1's.

- `SPEEDRUN_COUNT_MIXER`: the rate converter keeps every count and every read and writes no sample.
- `SPEEDRUN_NO_PIXELS`: the costume and strip decoders and the copies of finished pixels are skipped.
- `SPEEDRUN_CUTS=1` turns both on. `SPEEDRUN_LEAN=1` still means 0002's four switches.
- `SPEEDRUN_DUMP_NOPIXELS` (observation only, and on whenever `SPEEDRUN_NO_PIXELS` is): the pixel-free exact dump,
  section 3.

The bridge refuses either cut together with `SPEEDRUN_VIDEO`, the one reader of the mixed samples and of the
backend's screen.

The game is Monkey Island 1 CD (SCUMM v5, VGA, DOS), the only game the bridge runs. The argument below follows that
game's code paths and says where another platform's path is left as it was.

## 1. The count-only mixer

**Where samples go.** With `--disable-sdl-audio` the bridge pumps the mixer itself. `pumpAudio()` calls
`MixerImpl::mixCallback(_audioBuf, 512 frames)` (22,050 Hz stereo, the null mixer manager's rate).

`mixCallback` zeroes the buffer, then calls `Channel::mix` for each live channel, which calls
`RateConverter::convert(stream, buf, len, volL, volR)`. The converter reads its stream through `readBuffer` into its own
512-sample cache. It then adds volume-scaled samples (interpolated, for the interpolating case) into `buf`, clamped.

- The CD track: 44,100 Hz stereo FLAC, so the integer-ratio ("simple") converter.
- The AdLib chip: 22,050 Hz mono, so the copying converter.

**Who reads them.** `_audioBuf` is read only by the `SPEEDRUN_VIDEO` capture (the PCM file). No engine code reads a
mixed or converted sample. The SCUMM code asks the mixer only these things:

- whether a handle, an id or a sound type has a live channel: `isSoundHandleActive`, `isSoundIDActive`,
  `hasActiveChannelOfType`;
- whether it is ready, its output rate, and its volumes.

The CD manager asks whether its handle is active. Elapsed times come from `Channel`'s counts (`_samplesConsumed`,
`_samplesDecoded`) and its timestamps.

The music's clock is the chip's timer: `EmulatedChip::readBuffer` calls iMUSE's and the AdLib driver's `onTimer` after
a number of samples (`_nextTick`). That depends on how many samples are read, never on their values.

**What the switch changes.** `RateConverter_Impl::convert` takes a count-only path for each of its three cases (copy,
simple, interpolate). The path keeps all of the following:

- the original loops;
- the same `readBuffer` calls, with the same sizes, in the same order;
- the same updates of `_bufferPos`, `_bufferSize`, `_outPos`, `_outPosFrac`, `_inLastL/R` and `_inCurL/R`;
- the same return value.

It skips only the volume scaling, the interpolation and `clampedAdd` into `buf`. So:

- every stream sees the same calls, and so has the same state: its position, its end of data, the chip's timer
  callbacks and with them iMUSE's and the AdLib driver's state;
- `needsDraining()` (`_bufferSize != 0`) is the same, so `Channel::isFinished` and every channel's lifetime are the
  same, and so are `isSoundHandleActive`, `isSoundIDActive` and `hasActiveChannelOfType`;
- the converter's result is the same, so `Channel::_samplesDecoded` and `getElapsedTime` are the same;
- `buf` keeps the zeros `mixCallback` wrote. With 0002's silent audio every sample the chip and the FLAC stream return
  is 0, so the original path added zeros: `buf` holds the same bytes either way, and so does every converter member.
  With a stream that returns other samples only `buf` differs, and only the capture could see it.

**Where the path takes a batch.** The copying case takes k outputs at once. k is the number of iterations the original
loop runs before its next `_bufferSize == 0` test can succeed: `_bufferSize / step` when `_bufferSize` is a positive
multiple of the step, else every remaining output. The state after the batch is therefore the state after k original
iterations.

In the second case, where `_bufferSize` is negative or not a multiple of the channel count, the original loop never
refills again and reads past its cache, which is undefined behaviour. The count-only path makes the same counter
updates there without the reads.

## 2. No pixel output

**What is skipped.**

1. `ClassicCostumeRenderer::paintCelByleRLE`: the call `byleRLEDecode(compData)` on the PC path. The C64, Amiga and PC
   Engine decoders are left as they were.
2. `Gdi::decompressBitmap`: the calls to the strip decoders (`drawStripBasicH/V`, `drawStripComplex`, `drawStripRaw`,
   `drawStripEGA`, `unkDecode8-11`, `drawStrip3DO`, `drawStripHE`). These still run as before:
   - the read of the code byte;
   - `_paletteMod`, `_decomp_shr` and `_decomp_mask`;
   - the returned `transpStrip`, which depends on the code byte alone.
3. `Gdi::drawBitmap`: `copy8Col`/`clear8Col` of each strip from the back buffer to the front buffer.
4. `Gdi::resetBackground`: the same copy, which restores the background behind actors. Its dirty bounds are still set.
5. `ScummEngine::drawStripToScreen`: compositing the text surface over the strip into `_compositeBuf`, and the
   `copyRectToScreen` to the backend. The function returns after its clipping.

**What each skipped piece writes.**

- **`byleRLEDecode`** writes pixels through `compData.destPtr` into the main virtual screen, front or back buffer. Its
  other writes go to `compData`, a local of `paintCelByleRLE` dropped after the call. It writes no member: `_srcPtr` is
  copied into a local.

  Everything an actor keeps from being drawn is computed before the decode, in `paintCelByleRLECommon` and
  `markAsDirty`, which still run:
  - the draw flag, which becomes `drawLimb`'s result and so `Actor::_needRedraw`;
  - `_drawTop` and `_drawBottom`, which become `Actor::_top` and `_bottom`;
  - `markRectAsDirty`, which sets the strips' gfx usage bits that `getActorFromPos` tests, and the dirty bounds.

  v5 has no pixel hit test: `Actor::actorHitTest` belongs to the AKOS renderer (v7 and HE).
- **The strip decoders** are `const` members of `Gdi`, which has no `mutable` member, and `drawStripComplex`'s
  `MajMinCodec` is a local. So they write only through `dst`.
- **`copy8Col` and `clear8Col`** write the pixels they are given and nothing else.
- **`drawStripToScreen`'s tail** writes `_compositeBuf` and the backend's screen surface. Neither feeds the game:
  - `_compositeBuf` is read only by `drawStripToScreen` itself, and by the cursor code when EGA dithering is on, which
    it is not for this game.
  - The backend surface is read by `moveScreen`, which only moves it, and by the bridge's video capture, which is
    refused.

**Who reads the pixels.** A grep of `engines/scumm` for `getPixels`, `getBackPixels`, `backBuf` and `getBasePtr`,
leaving out the HE, v6+, Mac, FM Towns, NES and PC Engine files, finds these readers:

- the drawing code itself: charset, verbs, bomp, gfx and gfx_gui;
- the cursor grab (`ScummEngine_v6::grabCursor`): v6 only, and its result is the cursor image, which is output;
- the savegame thumbnail (`saveload.cpp`): it goes into a save file and is never read back into the state, and the
  runs save nothing;
- the exact dump's screen hash;
- `o6_getPixel` (`script_v6.cpp`): a v6 opcode, not in v5's opcode table.

So no pixel value feeds a variable, a bit, an object's owner, state or class, a script, an actor's position, box, top
or bottom, the RNG, the clocks or the sound.

**Masks are kept.** `decodeMask` runs as before with the same `transpStrip`, so the z-plane masks hold the same bytes.
They are read only by the costume decoders, which is pixel output again.

## 3. The pixel-free exact dump

The exact object's `screens` field is an FNV-1a 64 hash over three things:

- the four virtual screens' headers (`w`, `h`, `pitch`, `xstart`, `topline`);
- their front and back pixels;
- the palette.

With no pixels drawn, the pixels part would differ for no reason. With `SPEEDRUN_DUMP_NOPIXELS`, the exact object
carries `screens_np` instead: the same hash over the headers and the palette alone. Every other field is unchanged.
The JSON default and the binary channel's blocks have no pixel field.

**How the gates use it.**

- **G2 and G3 (against svm-w3b).** Each new switch is run alone and with all the others, on the lean build.
  - For the count-only mixer, the full exact dumps are compared, with no field left out.
  - With the pixel switch, the comparison leaves out the exact object's screen field on both sides: svm-w3b's
    `screens` and the new build's `screens_np`.
  - A second comparison checks `screens_np` against the new build run with every switch off and
    `SPEEDRUN_DUMP_NOPIXELS`. That run's other fields are compared with svm-w3b's too, so the geometry and the palette
    are tied to the reference.
- **G1** (`fork_exact.py run` on the new build with all six switches). Every engine of the run uses the pixel-free
  dump, so the run compares C, P and the load children on everything but the pixels.
- **G4, G5 and verify from boot.** The binary blocks, the facts and the checkpoints have no pixel field; they compare as
  in round 1.
