"""E003 step 1: action kinds, and whether an action is usable in a state, from facts only.

usage: from actions import sentence, walk_to, dialog_line, skip_cutscene, wait, usable
       usable(sentence(verb, a), model.FactOracle(facts)).value   # True, False or None (unknown)

An action is what a player input does (plan.md C1). ``usable`` checks only what the **engine** needs
before the input reaches the scripts; conditions the scripts test are not here. They come from the
model's paths (its assumptions, later its preconditions), so plan step 5's control run can drop
them and keep these.

Kinds and their engine conditions:

* **sentence(verb, A, B)**: input is on (VAR_USERPUT > 0: checkExecVerbs ignores clicks and verb keys
  otherwise, verbs.cpp:590; o5_cursorCommand mirrors _userPut into VAR_USERPUT, script_v5.cpp:935-938,
  its only writer), the verb's button is shown (``verb:V``: verbs.cpp:610 and findVerbAtPos test
  verbid, saveid 0 and curmode 1, the bridge's verbVisible) and every object is reachable;
* **walk_to(A)** is the sentence with the walk verb, which the caller gives (the harness reads it from
  the executor's interface profile, a disclosed C2 exception). The walk verb has no button: a click on
  the room with no verb chosen gives it (the input script's default). So it needs input on and A
  reachable as a room object (a scene click reaches only room objects, through findObject);
* **dialog_line(V)**: a dialog line is a verb, so the action is a left click on verb V, and it is usable
  when checkExecVerbs (verbs.cpp:586-692, the v5 one) takes the click as a click on V. Its tests, in order:
  ``_userPut <= 0`` drops the click (verbs.cpp:590): input on, VAR_USERPUT > 0 as for a sentence (VAR_USERPUT
  is _userPut after every cursor command, script_v5.cpp:935-938; the engine's other writes of _userPut are its
  init, scumm.cpp:2188, save and load, saveload.cpp:1499, and the v3 save-menu exits of Zak, Indy3 and Loom,
  scumm.cpp:3532-3758, none of which a v5 run without saves reaches; a script that wrote var 53 itself would
  break the mirror, and check.py counts such writes in the IR); ``_mouseAndKeyboardStat == 0`` drops it too (590),
  but a click sets it to MBS_LEFT_CLICK (input.cpp:455-456), so the click is the action; a mouse code is not a
  key (593, 665), and in v4 and later a left click gives code 1 (666); ``findVirtScreen(_mouse.y)`` must find a
  virtual screen under the click (675-678); the Macintosh GUI test (680-681) does not apply to DOS; then
  ``findVerbAtPos`` (683-686, 1009-1032) walks the verb slots from the last down and takes the first whose verb is
  visible and enabled (curmode 1, verbid set, saveid 0) and whose rectangle holds the click: ``verb:V`` shown
  is the fact for the first part (the bridge's verbVisible). v5's checkExecVerbs has no cutscene test (the
  ``_currentMode != kModeCutscene`` test at verbs.cpp:872 is ScummEngine_v0's): a cutscene stops clicks in v5
  only through _userPut, which scripts set. Not facts, so not checked and listed in UNCHECKED: the virtual
  screen under the click (IN_SCREEN) and the verb's rectangle under the click with no later visible slot's
  over it (VERB_UNDER). These are kept apart from the object conditions (``CLICK``). The engine runs these
  tests after the click frame's runAllScripts (scumm.cpp:3231-3232); ``usable`` reads them in the state before
  the move, and ``model.Machine.run_verb`` counts the paths whose turns in that frame changed them
  (``Path.input_drift``);
* **skip_cutscene()**: the Esc key, which skips a cutscene when the engine's Esc branch aborts one. MI1's DOS
  release is v5, and ScummEngine_v5 has no processKeyboard of its own (scumm_v5.h:29), so processInput hands the
  key (ASCII 27, no modifier: input.cpp:533-534) to ScummEngine::processKeyboard; with the original GUI off (the
  run's settings; isUsingOriginalGUI, scumm.cpp:4271-4282) no earlier branch takes Esc (input.cpp:1400-1421 are
  F5, F8, space and '.'). Its branch (input.cpp:1423-1429) needs VAR_CUTSCENEEXIT_KEY != 0 (input.cpp:966), and
  then abortCutscene (script.cpp:1689-1709) acts only on the top level of the cutscene stack, and only when an
  override is armed there (``if (offs)``, 1693-1694; the level may be 0, an override needs no cutscene): an
  ``esc:`` fact is 1. The key is read in processInput, before the frame's scripts run (scumm.cpp:3177, 3231),
  so the override must be armed by the end of the frame before the key: the state before the move. What Esc
  does otherwise: with VAR_CUTSCENEEXIT_KEY 0 the branch is not taken and the key reaches checkExecVerbs as
  ASCII 27 (input.cpp:1553-1558); with no override armed abortCutscene does nothing and the key reaches
  checkExecVerbs as VAR_CUTSCENEEXIT_KEY's value (input.cpp:1427-1428). Either way no cutscene is skipped (the
  key may still reach the input script, verbs.cpp:586-664), so the action is not usable. A state that holds no
  ``esc:`` facts makes it unknown, and so does an armed override with VAR_CUTSCENEEXIT_KEY not known (whether
  Esc's branch is taken is then not known). Not facts, so not checked and listed in UNCHECKED (``ESC``): no
  sentence armed or cleared an override between the state's decision point and the key (checkAndRunSentenceScript
  runs after the bridge's decision point, scumm.cpp:3254-3255, and the sentence queue is not a fact: ESC_QUEUE);
  and, for where the key then goes (``model.Machine.run_esc``), no shown verb's key is the key code (verbs.cpp:
  608-615: a verb's key is not a fact: VERB_KEY; check.py's Esc lines give, for each key setting in the IR whose
  value is a variable, whether the plan's closure of that variable's writes from the start value reaches the
  code). An esc: fact names the slot that armed the override: facts.py gives none for an
  override the bridge calls stale, although abortCutscene itself would rewind whatever runs in that slot
  (script.cpp:1695); the bridge's own refusals (input on, its spared scripts, the jump opcode at the pointer) are
  its rules, not the engine's, and are not used;
* **wait()**: always usable.

Reachable object O (findObject, object.cpp:557-597, and the inventory):

* owned by the actor in VAR_EGO: in the inventory (not for walk_to);
* else owned by the room (owner OF_OWNER_ROOM, 0x0F in v5: scumm.cpp:1745), defined in the room
  resource in use (``Game.object_room``; a pseudo room maps to its resource through the map
  o5_pseudoRoom builds, read from the scripts: room.cpp:166-169, script_v5.cpp:2085-2092), and not of
  class Untouchable (kObjectClassUntouchable = 32, object.h:54; findObject skips it, object.cpp:563);
* an actor (an object number below _numActors, object.cpp:1620-1622) is reachable when the click
  finds it: getActorFromPos (actor.cpp:2115-2147) needs it drawn in this room and not Untouchable. Its
  room is not a fact, so an actor that is not Untouchable is unknown, and counted as such;
* not modelled, so not checked, and listed in the check's ``unchecked`` (UNCHECKED) so the harness
  can count the moves that rest on them: findObject also hides an object whose parent's state does
  not match (object.cpp:567-590; the room data's parent and parentstate), and a click finds only an
  object under the point on screen, the first in the room's list (object.cpp:557-597); an
  inventory object is clicked in the inventory slot that shows it, a verb the scripts draw.
"""

from dataclasses import dataclass, field

from model import (CLASS_UNTOUCHABLE, NUM_ACTORS, OF_OWNER_ROOM, VAR_CUTSCENEEXIT_KEY, VAR_EGO, VAR_USERPUT, Game,
                   Oracle, Unknown)

# Engine conditions a check rests on but does not test (module docstring): a reachability check's, and a click on a
# verb's (CLICK: findVirtScreen and findVerbAtPos, verbs.cpp:675-686, 1009-1032)
PARENT, ON_SCREEN, SLOT_SHOWN = "parent state", "on screen, first under the click", "shown in an inventory slot"
IN_SCREEN, VERB_UNDER = "a virtual screen under the click", "the verb's rectangle under the click, no later slot's over it"
CLICK = (IN_SCREEN, VERB_UNDER)
# the Esc key's (ESC): skip_cutscene's check rests on them (module docstring)
ESC_QUEUE = "no sentence armed or cleared an override after the decision point"
VERB_KEY = "no shown verb's key is the Esc key code"
ESC = (ESC_QUEUE, VERB_KEY)
UNCHECKED = (PARENT, ON_SCREEN, SLOT_SHOWN) + CLICK + ESC


@dataclass(frozen=True)
class Action:
    kind: str  # sentence, dialog_line, skip_cutscene, wait
    verb: int | None = None
    a: int | None = None
    b: int | None = None
    walk: bool = False  # a walk-to sentence

    def label(self) -> str:
        if self.kind == "sentence":
            what = f"walk_to({self.a})" if self.walk else f"sentence({self.verb}, {self.a}"
            return what if self.walk else what + (f", {self.b})" if self.b is not None else ")")
        if self.kind == "dialog_line":
            return f"dialog_line({self.verb})"
        return f"{self.kind}()"

    def as_dict(self) -> dict:
        return {"kind": "walk_to" if self.walk else self.kind, "verb": self.verb, "a": self.a, "b": self.b}


def sentence(verb: int, a: int, b: int | None = None) -> Action:
    return Action("sentence", verb, a, b)


def walk_to(walk_verb: int, a: int) -> Action:
    return Action("sentence", walk_verb, a, None, walk=True)


def dialog_line(verb: int) -> Action:
    return Action("dialog_line", verb)


def skip_cutscene() -> Action:
    return Action("skip_cutscene")


def wait() -> Action:
    return Action("wait")


@dataclass
class Check:
    name: str
    holds: bool | None  # None: unknown
    why: str
    unchecked: tuple = ()  # engine conditions this check rests on but does not test (UNCHECKED)


@dataclass
class Usable:
    value: bool | None  # True when every check holds, False when one fails, else None
    checks: list = field(default_factory=list)

    def failing(self) -> list:
        return [c for c in self.checks if c.holds is False]

    def unknown(self) -> list:
        return [c for c in self.checks if c.holds is None]

    def unchecked(self) -> list:
        """(condition, check name) for every engine condition a holding check rests on untested."""
        return [(u, c.name) for c in self.checks if c.holds for u in c.unchecked]


def _get(oracle: Oracle, key: str):
    return oracle.get(key)


def _input_on(oracle: Oracle) -> Check:
    v = _get(oracle, f"var:{VAR_USERPUT}")
    if isinstance(v, Unknown):
        return Check("input on", None, f"VAR_USERPUT {v}")
    return Check("input on", v > 0, f"VAR_USERPUT = {v}")


def _verb_shown(oracle: Oracle, verb: int, click: bool = False) -> Check:
    """``verb:V`` is 1. ``click``: the action is a click on the verb itself (a dialog line), which also rests on
    the CLICK conditions."""
    v = _get(oracle, f"verb:{verb}")
    if isinstance(v, Unknown):
        return Check(f"verb {verb} shown", None, str(v))
    return Check(f"verb {verb} shown", v == 1, "verb:%d = %d" % (verb, v), CLICK if click else ())


def reachable(obj: int, oracle: Oracle, game: Game, room_only: bool = False) -> Check:
    name = f"object {obj} reachable"
    untouchable = _get(oracle, f"class:{obj}:{CLASS_UNTOUCHABLE}")
    if obj < NUM_ACTORS:
        if untouchable == 1:
            return Check(name, False, f"actor {obj} is Untouchable")
        return Check(name, None, f"actor {obj}: getActorFromPos needs it drawn in this room; its room is not a fact")
    ego, owner = _get(oracle, f"var:{VAR_EGO}"), _get(oracle, f"owner:{obj}")
    if isinstance(owner, Unknown) or isinstance(ego, Unknown):
        return Check(name, None, f"owner {owner}, ego {ego}")
    if owner == ego and not room_only:
        return Check(name, True, f"in the inventory of actor {ego}", (SLOT_SHOWN,))
    if owner != OF_OWNER_ROOM:
        return Check(name, False, f"owner {owner}" + (" (walk_to needs a room object)" if owner == ego else ""))
    room = _get(oracle, "room")
    res = game.resource(room)
    home = game.object_room.get(obj)
    if isinstance(res, Unknown):
        return Check(name, None, f"room {room}: {res}")
    if home != res:
        return Check(name, False, f"defined in room {home}; room {room} uses resource {res}")
    if isinstance(untouchable, Unknown):
        return Check(name, None, f"class {CLASS_UNTOUCHABLE} {untouchable}")
    if untouchable:
        return Check(name, False, "Untouchable")
    parent = game.object_parent.get(obj, 0)
    return Check(name, True, f"room object of resource {res}" + (f" (parent {parent}'s state not checked)"
                                                                  if parent else ""),
                 (ON_SCREEN, PARENT) if parent else (ON_SCREEN,))


def _override_armed(oracle: Oracle) -> Check:
    key = _get(oracle, f"var:{VAR_CUTSCENEEXIT_KEY}")
    if not isinstance(key, Unknown) and key == 0:
        return Check("Esc enabled", False, "VAR_CUTSCENEEXIT_KEY = 0")
    if not oracle.tracks("esc"):
        return Check("override armed", None, "the state holds no esc: facts")
    keys = oracle.keys("esc:")
    if keys is None:
        return Check("override armed", None, "the oracle cannot list esc: facts")
    armed = [k for k in keys if _get(oracle, k) == 1]
    if armed and isinstance(key, Unknown):  # no override: not usable whatever the key; one: the key decides
        return Check("Esc enabled", None, f"VAR_CUTSCENEEXIT_KEY {key}: whether Esc reaches abortCutscene is not "
                                          f"known (input.cpp:966, 1423); {', '.join(armed)}")
    return Check("override armed", bool(armed), ", ".join(armed) or "no esc: fact is 1", ESC if armed else ())


def usable(action: Action, oracle: Oracle, game: Game | None = None) -> Usable:
    """The engine's conditions for ``action`` in the state ``oracle`` holds (module docstring)."""
    game = game or Game.load()
    checks = []
    if action.kind == "sentence":
        checks.append(_input_on(oracle))
        if not action.walk:
            checks.append(_verb_shown(oracle, action.verb))
        checks.append(reachable(action.a, oracle, game, room_only=action.walk))
        if action.b is not None:
            checks.append(reachable(action.b, oracle, game))
    elif action.kind == "dialog_line":
        checks += [_input_on(oracle), _verb_shown(oracle, action.verb, click=True)]
    elif action.kind == "skip_cutscene":
        checks.append(_override_armed(oracle))
    elif action.kind != "wait":
        raise ValueError(f"unknown action kind {action.kind}")
    if any(c.holds is False for c in checks):
        return Usable(False, checks)
    return Usable(True if all(c.holds for c in checks) else None, checks)
