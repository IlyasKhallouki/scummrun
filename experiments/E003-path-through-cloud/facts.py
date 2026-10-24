"""E003: the engine's state as a flat fact map, shared by the ground truth and the model check.

usage: from facts import facts_of, diff, patch

``facts_of(state)`` turns the bridge's C7 state dump (``Branch.state()``,
``speedrun_state.cpp`` stateJson) into ``{key: int}``. A key that is absent is 0.

* ``var:N``: global variable N, every one but the engine's clocks: VAR_TMR_1-3, VAR_MUSIC_TIMER,
  VAR_TIMER and VAR_TIMER_TOTAL (vars.cpp:47-50, 79-80, setupScummVars), which scummLoop and
  Sound::updateMusicTimer advance every frame. Timers are not facts (plan.md, step 1);
* ``bit:N``: 1 for every set bit variable;
* ``owner:O`` and ``state:O``: every global object's owner and state;
* ``class:O:C``: 1 for every class C object O has (the dump lists classes 1-32, as getClass reads them);
* ``room``: the current room; ``ego``: the actor in VAR_EGO;
* ``script:N``: 1 for every live global script (slot ``where`` == WIO_GLOBAL);
* ``local:N``: 1 for every live room-local script N (WIO_LOCAL). The engine runs only the current
  room's locals and kills them on a room change, so the room is its own fact and not part of the
  key. In a pseudo-room the locals are those of the room resource it maps to;
* ``objscript:O``: 1 for every live object script, O the object (WIO_INVENTORY, WIO_ROOM,
  WIO_FLOBJECT: runObjectScript stores the object as the slot's number). A room's entry and exit
  code also run in a WIO_ROOM slot, under the numbers kScriptNumENCD/EXCD; they are not objects and
  are left out;
* ``at:<unit>:W``: 1 for every live slot, W the instruction its script resumes at, from the
  slot's ``code`` (speedrun_state.cpp scriptCode): its PC minus where the engine started the
  script, which is the ``[XXXX]`` offset in the decompiled script (data/scripts-dos-cd/index.json
  "offset_bases"). The unit is ``script:N`` for a global script, ``local:N`` for a room-local
  script, ``obj:O`` for an object script, ``entry`` and ``exit`` for the room's entry and exit
  code (``at:entry:W``, ``at:exit:W``). The start is the resource header for a global script
  (script.cpp:73, so W is its PC minus ``HEADER``), _localScriptOffsets for a local one
  (script.cpp:79), _ENCD_offs and _EXCD_offs for the room code (script.cpp:995-997, 1046-1048),
  and the OBCD's offset plus the VERB block's offset in it for an object (script.cpp:127-142,
  210-214, 257): the event table's base. W is where the script last stopped, in one of three
  ways. After a breakHere or a delay it is the instruction after it. A script blocked in o5_wait
  re-runs that opcode every frame, so W is the wait itself. A script that ran another one nested
  (startScript, startObject, or any opcode that runs a script on the spot: a room change,
  cutscene, the inventory hook ...) waits right after that call when the callee froze it:
  runScriptNested saved its PC before the call (script.cpp:348) and resumes it only if it is
  unchanged and not frozen (script.cpp:377-390). It stays there, frozen or not, until it next
  runs. (Esc's abortCutscene moves a PC to an override jump, script.cpp:1699, but the script
  runs on from there in the same frame: processInput comes before runAllScripts, scumm.cpp:3177,
  3231.) Instances of one unit at one W share one key: the number of slots there is not a fact.
  Local, entry and exit keys carry no room, as ``local:N``; in a pseudo-room the code is that of
  the room resource it maps to
  (VAR_ROOM_RESOURCE, var 22, vars.cpp:58, room.cpp:171-172). A slot whose ``code`` is -1 (the
  bridge could not work out its start) gets no ``at:`` key. ``at:exit`` cannot occur at a
  decision point: runExitScript's one caller (room.cpp:78) kills every room slot right after it
  (room.cpp:80, script.cpp:1089-1096), so the exit code is never live when the state is read;
* ``frozen:<unit>``: 1 for every live slot whose freeze count is above 0 (the dump's ``frozen``,
  ScriptSlot::freezeCount). The unit is written as in ``at:``. freezeScripts adds 1 to the count of
  every live slot but the running one (and, below flag 0x80, but the freeze-resistant ones) and sets
  the frozen bit; unfreezeScripts takes 1 off every frozen slot and clears the bit at 0
  (script.cpp:914-966); beginCutscene's caller and an Esc's override slot go to 0 (script.cpp:937-940,
  1700-1701), and runScript and runObjectScript start a slot at 0 (script.cpp:100, 147). A frozen slot
  does not run (runAllScripts needs status ssRunning exactly, script.cpp:979), its caller is not
  resumed after a nested call (script.cpp:377-390), and a frozen sentence script does not hold back
  the next sentence (script.cpp:1186-1191). The count itself is not a fact, only whether it is above 0. Slots of one
  unit share the key, as in ``at:``;
* ``cutscene``: the depth of the cutscene stack (vm.cutSceneStackPointer: beginCutscene pushes,
  endCutscene pops, script.cpp:1632-1687). 0 is no open cutscene;
* ``esc:<unit>:W``: 1 for the override armed at the top level of the cutscene stack, the one
  abortCutscene resumes (script.cpp:1689-1709). The unit is written as in ``at:``; W is the jump
  after the beginOverride, where the script goes on with VAR_OVERRIDE = 1 (ir.py ``esc``). The Esc
  key calls abortCutscene only while VAR_CUTSCENEEXIT_KEY (var 24, vars.cpp:60) is non-zero
  (input.cpp:966, 1423-1424); the key does not test that, ``var:24`` is its own fact. An override
  whose slot is dead or changed hands (the dump's ``stale``: abortCutscene would rewind an
  unrelated script) gives no key. ``esc:exit`` cannot occur, as ``at:exit``;
* ``verb:V``: 1 for every verb V the dump lists. The dump lists only visible verbs (verbid set,
  saveid 0, curmode 1: speedrun_state.cpp verbVisible, the engine's own hit test).

"Live" is any slot status but ssDead, as isScriptRunning() counts it (script.cpp), so frozen and
paused scripts are running. The WIO_* values, script numbers, variable indices and header size are
ScummVM constants (scumm.h, script.h, vars.cpp); nothing here names a game's object, script, room
or variable.
"""

WIO_INVENTORY, WIO_ROOM, WIO_GLOBAL, WIO_LOCAL, WIO_FLOBJECT = 0, 1, 2, 3, 4  # scumm.h:213-217
ENCD, EXCD = 10002, 10001  # kScriptNumENCD, kScriptNumEXCD (script.h:121-122): the room's entry and exit code
ROOM_CODE = {ENCD, EXCD}
CLOCKS = {11, 12, 13, 14, 46, 47}  # VAR_TMR_1-3, VAR_MUSIC_TIMER, VAR_TIMER, VAR_TIMER_TOTAL (vars.cpp:47-50, 79-80)
HEADER = 8  # _resourceHeaderSize (scumm.h:1153): a global script's PC starts after its resource header


def unit_of(where: int, number: int) -> str | None:
    """The ``at:``/``esc:`` unit of a slot: script:N, local:N, obj:O, entry or exit (None for a
    where the engine does not use)."""
    if where == WIO_GLOBAL:
        return f"script:{number}"
    if where == WIO_LOCAL:
        return f"local:{number}"
    if where == WIO_ROOM and number in ROOM_CODE:
        return "entry" if number == ENCD else "exit"
    if where in (WIO_INVENTORY, WIO_ROOM, WIO_FLOBJECT):
        return f"obj:{number}"
    return None


def facts_of(state: dict) -> dict[str, int]:
    f = {f"var:{n}": v for n, v in enumerate(state["vars"]) if n not in CLOCKS}
    f.update({f"bit:{n}": 1 for n in state["bits_set"]})
    f.update({f"owner:{o}": v for o, v in state["owners"].items()})
    f.update({f"state:{o}": v for o, v in state["states"].items()})
    for o, classes in state["classes"].items():
        f.update({f"class:{o}:{c}": 1 for c in classes})
    f["room"] = state["room"]
    if state.get("ego") is not None:
        f["ego"] = state["ego"]
    for s in state["slots"]:  # the dump lists live slots only
        if s["where"] == WIO_GLOBAL:
            f[f"script:{s['number']}"] = 1
        elif s["where"] == WIO_LOCAL:
            f[f"local:{s['number']}"] = 1
        elif s["where"] in (WIO_INVENTORY, WIO_ROOM, WIO_FLOBJECT) and s["number"] not in ROOM_CODE:
            f[f"objscript:{s['number']}"] = 1
        unit = unit_of(s["where"], s["number"])
        if unit is not None and s["code"] >= 0:
            f[f"at:{unit}:{s['code']}"] = 1
        if unit is not None and s["frozen"] > 0:
            f[f"frozen:{unit}"] = 1
    cutscene = state["cutscene"]
    f["cutscene"] = cutscene["depth"]
    top = cutscene["levels"][-1]["override"]  # levels run from 0 to the depth
    if top is not None and not top["stale"] and top["code"] >= 0:
        unit = unit_of(top["where"], top["number"])
        if unit is not None:
            f[f"esc:{unit}:{top['code']}"] = 1
    f.update({f"verb:{v['id']}": 1 for v in state["verbs"]})
    return f


def diff(a: dict[str, int], b: dict[str, int]) -> dict[str, tuple[int, int]]:
    """{key: (old, new)} for every key whose value differs from ``a`` to ``b``; absent is 0."""
    return {k: (a.get(k, 0), b.get(k, 0)) for k in a.keys() | b.keys() if a.get(k, 0) != b.get(k, 0)}


def patch(a: dict[str, int], d: dict) -> dict[str, int]:
    """``a`` with a ``diff`` applied, so ``diff(patch(a, diff(a, b)), b)`` is empty. A new value of 0
    removes the key (absent is 0), so a 0/1 key (bit, class, script, local, objscript, at, frozen, esc,
    verb) left in the map holds; ``var:``, ``owner:``, ``state:`` and ``cutscene`` keys from ``facts_of``
    can still be 0."""
    out = dict(a)
    for k, (_, new) in d.items():
        if new:
            out[k] = new
        else:
            out.pop(k, None)
    return out
