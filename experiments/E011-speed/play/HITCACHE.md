# The hit cache (fmi-play, `executor::HitCache`)

The harness's input code asks the engine what a click at a screen point would hit (`at(x, y)`) before every click
it plans: `Executor._floor_point` probes up to 80 points for a floor point, `object_points` up to 5 points per object,
and the stepping stones test every object of the room. In test (a)'s third window that is 227,624 round trips to
the engine (769 per child). Where it reads only the hit object, fmi-play can answer from a cache instead.

**It is on unless `FMI_HIT_CACHE=0`** (`FMI_CHECK_HITS=1` asks the engine for every cached answer too and compares).
Round 1 kept it off because of a gap in the proof (the screen layout, "The gap" below); patch 0004 reports the screen
layout in the binary status and the key holds it, which closes the gap (round 2). Over the JSON protocol the status
has no screen layout, so every hit test is asked there.

## What the hit test reads

The bridge's `at` command (patches/0001, speedrun_control.cpp) finds the virtual screen of y (`findVirtScreen`); on
the main screen it answers `obj = findObject(x + vs->xstart, y - vs->topline)`.

`ScummEngine::findObject` (engines/scumm/object.cpp:557-598), for a v5 game (the version branches for v0-v2, HE
and CMI/DIG do not apply):

```
for (i = 1; i < _numLocalObjects; i++) {
    if ((_objs[i].obj_nr < 1) || getClass(_objs[i].obj_nr, kObjectClassUntouchable))
        continue;
    b = i;
    do {
        a = _objs[b].parentstate;
        b = _objs[b].parent;
        if (b == 0) {
            if (_objs[i].x_pos <= x && _objs[i].width + _objs[i].x_pos > x &&
                _objs[i].y_pos <= y && _objs[i].height + _objs[i].y_pos > y)
                return _objs[i].obj_nr;
            break;
        }
    } while ((_objs[b].state & mask) == a);
}
return 0;
```

For an entry whose `parent` is 0 the loop body tests only the entry's box: `parentstate` and the states are read
only when the chain goes on to a parent.

## The claim

Let a stop's *layout* be: the room, the main screen's xstart (the stop status' `xstart`), the screen layout (the
stop status' `screen`: initScreens' b and h and `_forceBannerVirtScreen`, patch 0004), and the state dump's
`room_objects` list in its order, each entry as (id, x, y, w, h) with whether its object has class 32 (untouchable,
from the dump's `classes`). If every listed entry's parent is 0, then for every screen point the hit test's object
is the same at any two stops with the same layout.

**Proof.** The dump lists `_objs[i]` for i = 1 .. `_numLocalObjects` - 1 in that order, every entry whose obj_nr is
not 0, with `x_pos`, `y_pos`, `width`, `height` and `parent` (speedrun_state.cpp `stateJson`), and every object's
class bits. findObject skips the entries the dump leaves out (obj_nr 0), and for the others, all with parent 0, its
answer is the first entry in table order, not untouchable, whose box holds (x + xstart, y - topline). The table order
of the listed entries is the list's order, so that answer is a function of the layout and of the point, given
`findVirtScreen` and the main screen's topline. `findVirtScreen(y)` (gfx.cpp:463-476) returns the banner screen when
`_forceBannerVirtScreen` is set, else the first of the main, text and verb screens whose [topline, topline + h)
holds y. In this game (v5, DOS) those three are set only by `initScreens(b, h)` (gfx.cpp:340-400; the only other
`initVirtScreen` call, in `initBGBuffers`, is for v7 and up): the main screen is [b, h), the text screen [0, b) and
the verb screen [h, 200), with the main screen's topline b (the NES adjustment does not apply). So equal layouts,
which hold equal (b, h, forced banner), give equal answers at every point, including "not on the main screen" (no
object).

The untouchable class is class 32 here: `getClass` translates it to 24 only for games with `GF_SMALL_HEADER`, and the
CD release ("monkey", "CD", v5) has the flags `GF_AUDIOTRACKS` only (detection_tables.h).

A table with a parent anywhere is not covered: its answer can depend on `parentstate` (not in the dump) and on the
parent's position in the table, which the list does not give. The cache asks the engine for those every time.

## The gap (round 1), closed in round 2

`initScreens(b, h)` runs at boot and again from the scripts' `SetScreen(b, h)` (o5_roomOps; in MI1's scripts in room
entry and exit scripts and in five global scripts), and `_forceBannerVirtScreen` is set around the banners. In round 1
neither was in the state dump nor in the stop status, so the layout could not include them: two stops with equal
layouts and different screens would have shared answers that differ for points whose virtual screen changed, and the
cache was off. Patch 0004 puts `_screenB`, `_screenH` and `_forceBannerVirtScreen` in every binary stop status
(`Status::screen`), the key holds them, and a stop whose status has none (the JSON protocol, an end) is not cached.
On the door chain the screen layout changes at 2638 from (0, 200, false) to (0, 144, false) (play-wire check).

## The cache

- A layout is interned by its whole content (`Layout`: room, xstart, the screen layout, the list), compared for
  equality; no hash stands for it. Each (layout, point) holds the engine's answer the first time it is asked.
- The answer is used only where the input code reads the hit object (`_floor_point`, `object_points`). The actor and
  verb answers (`actor_points`, `verb_point`) are always asked.
- One cache per window search, shared by its children and stone trials, and kept across windows and plans.

## Checks (with the cache on)

- Round 1: test (a) in ready mode on the lean build through fmi-engine with `FMI_CHECK_HITS=1`: 236,870 cached
  answers, each equal to the engine's; the three plans give the reference's checkpoints, inputs and frames (0
  differences).
- In that run every stop's table had parent 0 everywhere (0 uncovered frames); 50 layouts covered every stop of the
  four windows, and the engine was asked 173 times instead of about 237,000.
- Round 2 (the key with the screen layout, patch 0004's engine, `FMI_CHECK_HITS=1`): test (a) in ready mode 236,870
  cached answers and in strict mode 328,573, each equal to the engine's; 50 layouts, 173 answers asked of the engine;
  the same children, checkpoints, inputs and frames as the reference's (play's tests 3 and 4).
