"""Play a route of moves as raw input on the first frame the engine takes each one.

The micro level of the Any% plan (``docs/anypercent/plan.md``). A route is a list
of ``Move``s: a key, a verb on an object (or two), a floor click, or a dialogue
line. The executor drives one ``control.Branch`` and, for each move:

1. waits for the engine's click gate (user input on, no cutscene) when the move
   clicks, or for the move's own ``after`` condition;
2. turns the move into C8 events: the verb's key on the frame before the click
   (a key and a click in the same frame are handled click first, so the click
   would still use the old verb; ``docs/anypercent/findings.md``), then a click
   at a screen point that the engine's own hit test (``Branch.at``) confirms;
3. runs until the move's ``until`` condition holds.

Every event it schedules is appended to ``Executor.movie``: replaying that list
from boot (``SPEEDRUN_INPUTS``) reproduces the run.
"""

from dataclasses import dataclass, field, replace

from speedrun.control import Branch, ControlError

SCREEN_W = 320
MAIN_H = 144  # the room view; the verbs and inventory sit below it
SCREEN_H = 200  # a verbless room (the island maps) shows all of it
DIALOG_FIRST, DIALOG_LAST = 120, 128  # dialogue menu lines are these verb ids (speedrun_state.cpp)
MAX_ACTOR = 12  # sentence objects up to this number are actors (MI1's 13 actor slots, 0 unused)


@dataclass(frozen=True)
class Profile:
    """The interface facts of one release that the executor clicks through."""

    verb_var: int  # the selected verb (global script 4 reads it)
    walk_to: int  # the default verb's id
    inv_first_verb: int  # inventory slots are verbs inv_first_verb..+inv_slots-1 ...
    inv_first_var: int  # ... showing the objects in vars inv_first_var..+inv_slots-1
    inv_slots: int
    inv_up: int  # scroll arrows
    inv_down: int
    verbless_rooms: frozenset[int] = frozenset()  # rooms that hide the verb bar by design (maps)
    # While the mouse is over the inventory, the hover script swaps Walk to / Pick up for Look at
    # and keeps the player's verb in this var, restoring it when the mouse leaves.
    hover_saved_verb_var: int | None = None
    # The inventory view: its top row is this var and a row holds inv_row objects, in
    # findInventory order. None: the executor cannot scroll on this release.
    inv_scroll_var: int | None = None
    inv_row: int = 4


PROFILES = {
    # v4 text inventory (data/scripts-dos-vga/global/script-009.txt [0048]-[0109]): verbs
    # 101-106 show vars 134-139, 107/108 scroll; Walk to is verb 10.
    "dos-vga": Profile(verb_var=107, walk_to=10, inv_first_verb=101, inv_first_var=134, inv_slots=6,
                       inv_up=107, inv_down=108),
    # v5 icon inventory (data/scripts-dos-cd/global/script-009.txt [0063]-[012B]): verbs 200-207
    # show vars 133-140, 208/209 scroll; Walk to is verb 11 (global/script-004.txt [0028]).
    # The island maps have no verb bar and take clicks (VAR_VERB_SCRIPT = 33, the map handler):
    # room 85 (room-085-melee) and Monkey Island's rooms 2-6 (room-002-monkey-1/entry.txt [0085]).
    # The hover swap is global/script-023.txt [004A]-[0086] (var 394); the view shows
    # findInventory(ego, var 118 * 4 + 1 ...) (global/script-009.txt [0063]-[00ED]).
    "dos-cd": Profile(verb_var=107, walk_to=11, inv_first_verb=200, inv_first_var=133, inv_slots=8,
                      inv_up=208, inv_down=209, verbless_rooms=frozenset({2, 3, 4, 5, 6, 85}), hover_saved_verb_var=394,
                      inv_scroll_var=118),
}
PROFILES["mac"] = PROFILES["dos-cd"]  # same scripts (data/scripts and data/scripts-dos-cd)


class RouteError(Exception):
    pass


@dataclass
class Move:
    name: str
    key: str | None = None  # a key press (e.g. "esc"), on the first frame after `after` holds
    verb: int | None = None  # verb id; None with obj = the default Walk to
    obj: int | None = None  # object clicked: in the room (on screen) or in the inventory
    obj2: int | None = None  # second object (Use obj with obj2, Give obj to obj2), clicked a frame later
    point: tuple[int, int] | None = None  # floor click, virtual room coordinates
    choose: int | str | None = None  # dialogue line: its key digit (1-9), or a substring of its text
    after: list[dict] = field(default_factory=list)  # wait for these (C3) before acting
    until: list[dict] = field(default_factory=list)  # the move is done when these hold
    max_frames: int = 3000
    obj_from_var: int | None = None  # obj is this var's value when the move is played
    # {"text": 0|1, "cutscenes": 0|1[, "spare": [global script, ...]]}: switch the automatic '.' and Esc
    # first; "spare" scripts keep their own lines and override (the bridge's textSpared())
    skips: dict | None = None
    delay: int = 0  # frames to wait before acting (RNG manipulation: a draw lands later)
    # A reply that depends on what was said: {"var": N, "map": {"value": line substring}}; the
    # line is looked up from var N when the menu is up (insult fights answer var 240).
    choose_by: dict | None = None
    # Keys pressed while waiting for `until`: [{"key": k, "when": [C3...]}]; each rule presses
    # its key on every frame that follows a stop where `when` holds (a skip the bridge does not
    # do by itself, such as Esc on a screen that waits for a click with input on).
    press: list[dict] = field(default_factory=list)
    # The bridge's automatic '.' and Esc while this move runs ({"text": 0|1, "cutscenes": 0|1}),
    # back to the run's setting once `until` holds: the repeat-press rules (K-ESC, K-DOT) of a
    # stretch that runs with the skips off.
    auto_skips: dict | None = None
    optional: bool = False  # if it fails, play_steered() rolls back and goes on without it
    # "input": click as soon as user input is on, even inside a cutscene. The engine takes such
    # clicks (checkExecVerbs() tests only _userPut), which the item slide relies on.
    gate: str = "control"
    # The y of the floor clicks that walk toward an off-screen object (default: its walk point's
    # y). A clipped target at the walk point's y can snap into a dead-end walk box on the way
    # (room 65's upper ledge, boxes 16/17, for the cave's (973,87)).
    approach_y: int | None = None
    # Right-click the object instead of verb key + left click: script 4 takes the object's default
    # verb (Var[182], set by its verb-90 entry) for a right click (global/script-004.txt [0005]-[0016]),
    # so the sentence starts without the verb key's frame. Only for a verb that is the object's default.
    rclick: bool = False
    # While ``obj`` is off screen, walk toward this room point instead of the clipped walk point.
    # "approach": "search" picks it: the clipped walk point and floor points above and below it are
    # tried on forks, and the one after which the whole move ends first is kept (room 77: the clipped
    # point (319,122) lies on box 8 and sends ego round the lower boxes 2/6/8, 16 frames longer).
    via: tuple[int, int] | None = None
    approach: str | None = None
    # A Walk-to walk that re-clicks where a restarted walk gains ground (_reclick_walk): True plays
    # it until `until` holds, a C3 list until that list holds (the walk's real end, such as the next
    # room, when `until` only marks the walk as started). With rclick, the re-clicks are right clicks:
    # each restarts the object's default-verb sentence and its walk (the church door's Open).
    reclick: bool | list | None = None
    # With reclick: also try a sweep (_reclick_sweep): a floor click every frame, alternating between
    # two points ahead of ego so that every frame starts a fresh walk, then the own click on the frame
    # that reaches the goal first. Kept only when it beats the greedy re-clicks.
    reclick_sweep: bool = False
    # The approach toward an off-screen object holds its floor click while the camera scrolls and
    # renews it only when ego stops or comes within this many px (x) of the held target. 0: renew
    # on every camera step (the default). A renewal is a new walk target, which floors the step on
    # axes moving right or down (engine-mechanics section 2.5): on a line-box path that runs down
    # (room 61) the per-step renewals froze ego's y while x crept at 1 px per frame.
    approach_hold: int = 0
    # Walk phases played before the move's own click, in order: {"click": [x, y]} one floor click
    # (room coordinates, on screen now; an object there takes its Walk to), {"wait": [C3...]},
    # {"step": [dx, dy], "until": [C3...]} a floor click at ego + (dx, dy) on every frame until the
    # conditions hold ("shmoovement", engine-mechanics sections 2.4-2.5: each click is a new walk
    # target, whose first step rounds up on axes moving left or up, so in a scaled box ego climbs
    # 1 px per frame instead of 2 * scale / 255; a slope under 4.5:1 keeps x at the speed clamp).
    shmoove: list[dict] | None = None
    # Never send this move's inputs early (_play_early): its frame decides a random draw later on
    # (the head dialogue's animation delays in room 86 come from getRandomNr, room-086 local 200).
    no_early: bool = False


@dataclass
class MoveResult:
    move: str
    issue_frame: int
    done_frame: int
    qf: int
    skipped: bool = False  # `until` already held, so nothing was sent


class Executor:
    def __init__(self, branch: Branch, release: str = "dos-vga", fork=None):
        self.b = branch
        self.release = release
        self.p = PROFILES[release]
        self.movie: list[dict] = []
        self.results: list[MoveResult] = []
        self._verb_keys: dict[int, str] = {}
        self._fork = fork  # Controller.fork, for play_steered()
        self.skips = {"text": 1, "cutscenes": 1}  # the bridge's automatic skips (EngineConfig default)
        # Early inputs (_play_early): try each click move's inputs 1-2 frames before the executor
        # would send them, on forks, and keep the earliest that still completes the move.
        self.early = False

    # --- scheduling -------------------------------------------------------------------

    def _schedule(self, events: list[dict]) -> None:
        self.b.inputs(events)
        self.movie.extend(events)

    def _run(self, **kw) -> dict:
        st = self.b.run(**kw)
        if "end" in st:
            raise RouteError(f"the run ended: {st['end']}")
        return st

    def _verb_key(self, verb: int) -> str:
        if verb not in self._verb_keys:  # the verb table changes (intro, dialogues): reread it
            for v in self.b.state()["verbs"]:
                if v.get("key") and 32 <= v["key"] < 127:
                    self._verb_keys.setdefault(v["id"], chr(v["key"]))
        if verb not in self._verb_keys:
            raise RouteError(f"verb {verb} has no key in the verb table")
        return self._verb_keys[verb]

    # --- geometry ---------------------------------------------------------------------

    def _xstart(self) -> int:
        """The screen's left edge in room coordinates for the next frame's input. The camera
        moves first: on a room's entry frame the stopped status has the new camera but the old
        screen offset, which the next frame catches up to before it reads input."""
        xs = (self.b.status["cam"] // 8 - SCREEN_W // 16) * 8
        # On a room's entry frame the camera can still hold the last room's position; the screen
        # never starts past the room's right edge (map room 5 after the clearing: camera 232).
        rw = self.b.status.get("room_w")
        return max(0, min(xs, rw - SCREEN_W)) if rw else xs

    def actor_points(self, actor: int, state: dict) -> list[tuple[int, int]]:
        """Screen points on a character (a sentence's object can be an actor, as the monkey
        that takes the bananas): above its feet, checked with the engine's actor hit test."""
        a = next((x for x in state.get("actors", []) if x["id"] == actor), None)
        if a is None:
            return []
        xs = self._xstart()
        lag = xs - self.b.status["xstart"]
        hits = []
        for dy in (20, 35, 10, 50, 5, 1, 0):  # 0-1: the island map's tiny sprites (room 2's monkey)
            p = (a["x"] - xs, a["y"] - a.get("elevation", 0) - dy)
            if 0 <= p[0] < SCREEN_W and 0 <= p[1] < self._main_h() and self.b.at(p[0] + lag, p[1]).get("actor") == actor:
                hits.append(p)
        return hits

    def _main_h(self) -> int:
        """Rows of the room view: the maps hide the verb bar and show the strips below 144 (the
        sea edges that row to the next map, room 6's object 70 at y 192)."""
        return SCREEN_H if self.b.status.get("room") in self.p.verbless_rooms else MAIN_H

    def object_points(self, obj: int, state: dict) -> list[tuple[int, int]]:
        """Screen points that hit ``obj`` now, centre first (empty if it is off screen)."""
        box = next((o for o in state["room_objects"] if o["id"] == obj), None)
        if box is None:
            return self.actor_points(obj, state) if obj <= MAX_ACTOR else []
        xs = self._xstart()
        lag = xs - self.b.status["xstart"]  # Branch.at() still tests with the stopped frame's offset
        x0, x1 = max(box["x"] - xs, 0), min(box["x"] + box["w"] - xs, SCREEN_W) - 1
        y0, y1 = max(box["y"], 0), min(box["y"] + box["h"], self._main_h()) - 1
        if x0 > x1 or y0 > y1:
            return []
        cands = [((x0 + x1) // 2, (y0 + y1) // 2), (x0, (y0 + y1) // 2), (x1, (y0 + y1) // 2),
                 ((x0 + x1) // 2, y0), ((x0 + x1) // 2, y1)]
        hits = []
        for p in cands:
            if p not in hits and self.b.at(p[0] + lag, p[1]).get("obj") == obj:
                hits.append(p)
        return hits

    def inventory_point(self, obj: int, state: dict) -> tuple[int, int] | None:
        """The screen point of ``obj``'s inventory slot, if it is among the slots shown."""
        for k in range(self.p.inv_slots):
            if state["vars"][self.p.inv_first_var + k] == obj:
                verb = next((v for v in state["verbs"] if v["id"] == self.p.inv_first_verb + k), None)
                if verb is not None:
                    return verb["x"] + 4, verb["y"] + 3
        return None

    def target_point(self, move: Move, obj: int, state: dict) -> tuple[int, int]:
        if obj in state["inventory"]:
            point = self.inventory_point(obj, state)
            if point is None:
                raise RouteError(f"{move.name}: inventory object {obj} is scrolled out of view")
            return point
        points = self.object_points(obj, state)
        if not points:
            raise RouteError(f"{move.name}: object {obj} is not clickable on screen now")
        return points[0]

    # --- moves ------------------------------------------------------------------------

    def wait_gate(self, limit: int) -> dict:
        """Run to the first frame a click counts: the engine's gate (user input on, no cutscene)
        with the verb bar or a dialogue menu on screen. Input on alone is not enough: during the
        CD intro the Lookout scene has input on and no verbs, and a click there breaks the verb
        bar for the rest of the game (docs/anypercent/findings.md)."""
        end = self.b.frame + limit
        while True:
            st = self.b.status
            if not (st.get("up", 0) > 0 and st.get("cs", 1) == 0 and st.get("room", 0) != 0):
                st = self._run(control=True, max_frame=end)
                if st["stop"] != "control":
                    raise RouteError(f"no control within {limit} frames (frame {st['frame']}, room {st['room']})")
            if st["room"] in self.p.verbless_rooms or self.b.state()["verbs"]:
                return self.b.status
            if self.b.frame >= end:
                raise RouteError(f"no verbs on screen within {limit} frames (frame {self.b.frame})")
            self._run(frame=self.b.frame + 1)

    def wait_input(self, limit: int, floor: bool = False) -> dict:
        """Run to the first frame user input is on, cutscene or not, with something to click.
        ``floor``: a scene click needs no verbs on screen (a cutscene that saved them away, as the
        deck's 'Monkey Island ahead' pan: the input script still turns the click into a walk)."""
        end = self.b.frame + limit
        while True:
            st = self.b.status
            if st.get("up", 0) > 0 and st.get("room", 0) != 0 and (
                    floor or st["room"] in self.p.verbless_rooms or self.b.state()["verbs"]):
                return st
            if self.b.frame >= end:
                raise RouteError(f"no user input within {limit} frames (frame {self.b.frame})")
            self._run(frame=self.b.frame + 1)

    EARLY_SHIFTS = (2, 1)  # frames an input burst is moved forward, tried in this order
    LAG = 2  # frames the lag snapshot trails the branch (early inputs may land this far back)

    def play(self, move: Move) -> MoveResult:
        if not (self.early and self._fork is not None):
            return self._play(move)
        pre = self._fork(self.b)  # the move's start, to rebuild the lag snapshot from
        mark = len(self.movie)
        try:
            res = self._play_early(move) if self._early_candidate(move) else None
            if res is None:
                res = self._play(move)
                self._set_lag(pre, self.movie[mark:])
                pre = None
            return res
        finally:
            if pre is not None:
                pre.quit()

    @property
    def _lag(self) -> Branch | None:
        """The lag snapshot (_set_lag), owned by the branch: Branch.quit() quits it too."""
        return getattr(self.b, "lag", None)

    def drop_lag(self) -> None:
        lag = getattr(self.b, "lag", None)
        if lag is not None:
            self.b.lag = None
            lag.quit()

    def _set_lag(self, src: Branch, events: list[dict]) -> None:
        """Keep ``src`` (a fork at or before this move's inputs) with ``events`` queued, run to LAG
        frames before the branch: the next move may move its inputs into these frames."""
        src.inputs(sorted((dict(e) for e in events), key=lambda e: e["frame"]))
        target = self.b.frame - self.LAG
        if target > src.frame:
            src.run(frame=target)
        self.drop_lag()
        self.b.lag = src

    @staticmethod
    def _is_input(e: dict) -> bool:
        return "key" in e or "click" in e or "rclick" in e

    def _early_candidate(self, move: Move) -> bool:
        # Click moves and dialogue choices (a menu line's click or digit key is read after the
        # frame's scripts too, so it can land on the frame the menu appears); not keys such as '.' and
        # Esc (processInput reads them before the scripts), press rules or re-clicked walks.
        # Not moves that read their object from a var (the catacomb exits, obj_from_var 164): a shifted
        # exit redraws every later chamber (each chamber draws its layout on entry), so the run's
        # catacomb RNG alignment (the route's delay before the cave, then the lookahead) would be lost.
        return (move.key is None and not move.press and not move.reclick and bool(move.until) and not move.delay
                and move.obj_from_var is None and not move.no_early
                and (move.obj is not None or move.point is not None or move.choose is not None))

    def _play_early(self, move: Move) -> MoveResult | None:
        """The move's inputs, sent 1 or 2 frames before the executor sends them.

        The executor clicks on the frame after it sees the gate open (user input on, no cutscene),
        and presses a verb key one frame before that click. The engine reads a click and a verb key
        after the frame's scripts (scumm.cpp scummLoop: processInput, runAllScripts, then
        checkExecVerbs), so on the frame a cutscene or reach ends the gate is already open when the
        input is read: a verb key there is taken (with the item slide also inside the reach), and a
        click is taken too when the hover script has already run after the cutscene's unfreeze (slot
        order; a slot click needs no hover). Often that frame is the one the previous move's `until`
        first held, so the inputs may go back into the previous move's last LAG frames: the lag
        snapshot (_set_lag) replays them. The move is first played as usual on a fork (its inputs,
        done frame and end state); its input events, shifted earlier by 2 and by 1 frames, are then
        replayed on forks, and the first shift that is a pure shift (`until` first holds exactly that
        many frames sooner, in the same end state) is played here."""
        start = self.b.frame
        base = Executor(self._fork(self.b), self.release, self._fork)
        base.skips, base._verb_keys = dict(self.skips), dict(self._verb_keys)
        try:
            r0 = base._play(move)
            events0 = list(base.movie)
            skips0 = dict(base.skips)
            sig0 = self._early_signature(base.b, move)
        except (RouteError, ControlError, OSError):
            return None
        finally:
            base.b.quit()
        if r0.skipped or not any(self._is_input(e) for e in events0):
            return None
        lag = self._lag
        floor = lag.frame if lag is not None else start
        queued = [e for e in self.movie if e["frame"] > floor and self._is_input(e)] if lag is not None else []
        for s in self.EARLY_SHIFTS:
            cand = [dict(e, frame=e["frame"] - s) if self._is_input(e) else dict(e) for e in events0]
            frames = [e["frame"] for e in cand if self._is_input(e)]
            if min(frames) <= floor:
                continue
            # One key and one mouse event per frame: the earlier move's inputs still queued there.
            kinds = {(e["frame"], "key" if "key" in e else "mouse") for e in queued}
            if any((e["frame"], "key" if "key" in e else "mouse") in kinds for e in cand if self._is_input(e)):
                continue
            src = lag if min(frames) <= start else self.b
            # A pure shift: `until` first holds exactly s frames sooner, in the same state (room, ego,
            # inventory, sentence and cutscene vars, the objects' states) as the usual play.
            if self._early_trial(src, cand, max(frames), move.until, r0.done_frame - s, sig0, move):
                break
        else:
            return None
        done = r0.done_frame - s
        cand = sorted(cand, key=lambda e: e["frame"])
        if src is lag:
            nb = self._fork(lag)
            nb.inputs(cand)
            self.b.quit()  # and its lag snapshot
            self.b = nb
        else:
            self.b.inputs(cand)
        lag_src = self._fork(self.b)  # before running: the lag is rebuilt from here
        self.movie.extend(cand)
        self.movie.sort(key=lambda e: e["frame"])
        self.skips = skips0
        st = self._run(frame=done)
        if not self.b.holds(move.until):
            lag_src.quit()
            raise RouteError(f"{move.name}: early inputs replayed off their trial (frame {st['frame']})")
        self._set_lag(lag_src, [])
        issue = min(e["frame"] for e in cand if self._is_input(e))
        res = MoveResult(move.name, issue, st["frame"], st["qf"])
        self.results.append(res)
        return res

    # Vars a pure time shift changes by itself: the timers (VAR_TIMER and its copies) and scratch 100.
    EARLY_VOLATILE_VARS = frozenset({11, 12, 13, 14, 47, 100})

    @classmethod
    def _early_signature(cls, b: Branch, move: Move) -> tuple:
        """What an early-input variant must leave as the usual play does: room, ego's position, every
        var but the timers, every bit, owner, class and inventory slot, and the state of every object
        outside the room (the room's own objects may animate on the clock) and of the move's objects.
        A shift that crosses the previous move's last frames must keep its effects too (an early 'u'
        on the spyglass's Open frame left the lens unmade, class 6, and the dam blast failed later)."""
        st = b.state()
        v = st["vars"]
        here = {o["id"] for o in st["room_objects"]} - {move.obj, move.obj2}
        states = tuple(sorted((k, x) for k, x in st["states"].items() if int(k) not in here))
        return (st.get("room"), tuple(st.get("ego_pos") or ()), tuple(st["inventory"]),
                tuple(x for i, x in enumerate(v) if i not in cls.EARLY_VOLATILE_VARS), tuple(st["bits_set"]),
                tuple(sorted(st["owners"].items())), tuple(sorted((k, tuple(x)) for k, x in st["classes"].items())),
                states)

    def _early_trial(self, src: Branch, events: list[dict], last: int, until: list[dict], done: int, sig0: tuple,
                     move: Move) -> bool:
        """Whether ``events``, queued on a fork of ``src``, make `until` first hold on frame ``done``
        (not on the frame before, unless that is before the last input) with the usual play's end
        signature."""
        if done < last:
            return False
        trial = self._fork(src)
        try:
            trial.inputs(sorted(events, key=lambda e: e["frame"]))
            if done - 1 > trial.frame:
                trial.run(frame=done - 1)
                if trial.holds(until) and done - 1 >= last:
                    return False
            trial.run(frame=done)
            return trial.holds(until) and self._early_signature(trial, move) == sig0
        except (ControlError, OSError):  # the engine crashed on the shifted inputs (a click inside a reach)
            return False
        finally:
            trial.quit()

    def _play(self, move: Move) -> MoveResult:
        if move.skips is not None:
            self._schedule([{"frame": self.b.frame + 1, "skips": move.skips}])
            self.skips = dict(move.skips)
        if move.delay:
            self._run(frame=self.b.frame + move.delay)
        if move.after and not self.b.holds(move.after):
            if move.press:  # the press rules also act while the move waits for its turn
                end = self.b.frame + move.max_frames
                while not self.b.holds(move.after):
                    if self.b.frame >= end:
                        raise RouteError(f"{move.name}: `after` did not hold within {move.max_frames} frames")
                    self._step(move)
            else:
                st = self._run(until=move.after, max_frame=self.b.frame + move.max_frames)
                if st["stop"] != "until":
                    raise RouteError(f"{move.name}: `after` did not hold within {move.max_frames} frames")
        room_only = all(next(iter(c)) == "room" for c in move.until)
        is_walk = move.verb in (None, self.p.walk_to) and move.key is None and move.choose is None
        if move.until and self.b.holds(move.until) and (is_walk or not room_only):
            # Already done (a door the route opens only if it is closed): no input. Checked once
            # `after` holds, since a repeated choice's `until` still holds from the last one. A
            # verb whose `until` names only the room it acts in ("Open the spyglass", until
            # room 15) is not done because ego stands there.
            res = MoveResult(move.name, self.b.frame, self.b.frame, self.b.status["qf"], skipped=True)
            self.results.append(res)
            return res
        if move.obj_from_var is not None:
            move = replace(move, obj=self.b.state()["vars"][move.obj_from_var])
        if move.key is None and move.choose is None and move.obj is None and move.point is None and move.press:
            issue = self.b.frame + 1  # a wait: only the press rules act
        elif move.key is not None:
            f = self.b.frame + 1
            if move.auto_skips is not None:
                self._schedule([{"frame": f, "skips": move.auto_skips}])
            self._schedule([{"frame": f, "key": move.key}])
            issue = f
        elif move.choose is not None:
            issue = self._choose(move)
        else:
            if move.gate == "input":
                self.wait_input(move.max_frames, floor=move.point is not None and move.verb in (None, self.p.walk_to))
            else:
                self.wait_gate(move.max_frames)
            if move.shmoove:
                self._shmoove(move)
            issue = self._click(move)
            if move.reclick and self._fork is not None and move.obj is not None and (
                    move.verb in (None, self.p.walk_to) or move.rclick):
                self._run(frame=issue)
                self._reclick_walk(move, move.until if move.reclick is True else move.reclick)
        if move.reclick and move.until and self.b.holds(move.until):
            st = self.b.status  # the re-clicked walk ended on its goal: no extra frame
        elif move.until and move.obj is not None and move.verb in (None, self.p.walk_to) and not move.press:
            st = self._run_reclicking(move, issue)
        elif move.until and move.press:
            st = self._run_pressing(move, issue)
        elif move.until:
            st = self._run(until=move.until, max_frame=issue + move.max_frames)
            if st["stop"] != "until":
                raise RouteError(f"{move.name}: `until` did not hold within {move.max_frames} frames of frame {issue}")
        else:
            st = self._run(frame=issue)
        if move.auto_skips is not None and move.key is not None:
            self._schedule([{"frame": self.b.frame + 1, "skips": self.skips}])
        res = MoveResult(move.name, issue, st["frame"], st["qf"])
        self.results.append(res)
        return res

    def _shmoove(self, move: Move) -> None:
        """Play ``move.shmoove``'s walk phases before the move's own click (see Move.shmoove)."""
        end = self.b.frame + move.max_frames
        if self._selected_verb(Move(move.name, point=(0, 0)), self.b.state()) != self.p.walk_to:
            self._wait_walk_to(move)
        for ph in move.shmoove:
            if "click" in ph:
                x, y = ph["click"][0] - self._xstart(), ph["click"][1]
                if not (0 <= x < SCREEN_W and 0 <= y < self._main_h()):
                    raise RouteError(f"{move.name}: shmoove point {ph['click']} is off screen (xstart {self._xstart()})")
                f = self.b.frame + 1
                self._schedule([{"frame": f, "click": [x, y]}])
                self._run(frame=f)
            elif "wait" in ph:
                if not self.b.holds(ph["wait"]):
                    st = self._run(until=ph["wait"], max_frame=end)
                    if st["stop"] != "until":
                        raise RouteError(f"{move.name}: shmoove wait {ph['wait']} did not hold by frame {end}")
            elif "step" in ph:
                k = 0
                while not self.b.holds(ph["until"]):
                    if self.b.frame >= end:
                        raise RouteError(f"{move.name}: shmoove {ph} did not end by frame {end}")
                    xs = self._xstart()
                    lag = xs - self.b.status["xstart"]
                    ego = next(a for a in self.b.state()["actors"] if a["id"] == 1)
                    # One px apart on alternate frames: the engine ignores a repeated target.
                    px = max(0, min(SCREEN_W - 1, ego["x"] + ph["step"][0] - xs + k % 2))
                    py = max(0, min(self._main_h() - 1, ego["y"] + ph["step"][1]))
                    f = self.b.frame + 1
                    if not self.b.at(px + lag, py).get("obj"):  # an object there would start its walk-to
                        self._schedule([{"frame": f, "click": [px, py]}])
                    self._run(frame=f)
                    k += 1
            else:
                raise RouteError(f"{move.name}: unknown shmoove phase {ph}")

    def _scroll_to(self, move: Move, obj: int) -> None:
        """Click the inventory arrows, one per frame, until ``obj``'s slot is shown. The arrows
        leave the selected verb alone (global/script-004.txt [02E6]-[0304] chains to script 9)."""
        for _ in range(16):
            state = self.b.state()
            if obj not in state["inventory"] or self.inventory_point(obj, state) is not None:
                return
            if self.p.inv_scroll_var is None:
                raise RouteError(f"{move.name}: inventory object {obj} is scrolled out of view")
            row = state["inventory"].index(obj) // self.p.inv_row
            arrow = self.p.inv_up if row < state["vars"][self.p.inv_scroll_var] else self.p.inv_down
            verb = next((v for v in state["verbs"] if v["id"] == arrow), None)
            if verb is None:
                raise RouteError(f"{move.name}: inventory arrow {arrow} is not on screen")
            f = self.b.frame + 1
            self._schedule([{"frame": f, "click": [verb["x"] + 4, verb["y"] + 3]}])
            self._run(frame=f)
        raise RouteError(f"{move.name}: scrolling never showed inventory object {obj}")

    def _click(self, move: Move) -> int:
        if move.obj is None and move.point is None and move.verb in (self.p.inv_up, self.p.inv_down):
            # An inventory arrow on its own (a scroll inside a reach, where it costs no frame):
            # script 4 only moves the view and keeps the selected verb (global/script-004.txt [02E6]).
            v = next((v for v in self.b.state()["verbs"] if v["id"] == move.verb), None)
            if v is None:
                raise RouteError(f"{move.name}: inventory arrow {move.verb} is not on screen")
            f = self.b.frame + 1
            self._schedule([{"frame": f, "click": [v["x"] + 4, v["y"] + 3]}])
            return f
        verb = move.verb if move.verb is not None else self.p.walk_to
        events: list[dict] = []
        if move.obj is not None:
            self._scroll_to(move, move.obj)  # the second item scrolls after the first click
        state = self.b.state()
        if move.obj2 is not None and move.obj2 not in state["inventory"] and not self.object_points(move.obj2, state):
            # Walk until the second object is on screen before the sentence starts: a floor
            # click in the middle of a two-object sentence would drop it.
            state = self._approach(replace(move, obj=move.obj2), state)
        if move.rclick and (move.obj is None or move.obj2 is not None):
            raise RouteError(f"{move.name}: rclick needs one object")
        if not move.rclick and verb == self.p.walk_to and self._selected_verb(move, state) != verb:
            state = self._wait_walk_to(move)  # Walk to has no key: the sentence's end resets to it
        f = self.b.frame + 1
        if not move.rclick and self._selected_verb(move, state) != verb:
            state = self._wait_verb_visible(move, verb)
            f = self.b.frame + 1
            events.append({"frame": f, "key": self._verb_key(verb)})
            f += 1
        if move.point is not None:
            x, y = move.point[0] - self._xstart(), move.point[1]
            if not (0 <= x < SCREEN_W) and 0 <= y < self._main_h() and move.verb in (None, self.p.walk_to):
                # A floor point beyond the screen's edge: walk toward it until the camera shows it.
                self._approach_point(move)
                f = self.b.frame + 1
                x = move.point[0] - self._xstart()
            if not (0 <= x < SCREEN_W and 0 <= y < self._main_h()):
                raise RouteError(f"{move.name}: point {move.point} is off screen (xstart {self._xstart()})")
            target = (x, y)
        elif move.obj is not None:
            if move.obj not in state["inventory"] and not self.object_points(move.obj, state):
                if move.obj <= MAX_ACTOR and all(o["id"] != move.obj for o in state["room_objects"]):
                    state = self._wait_actor(move)  # an actor walks on screen by itself (the map monkey)
                else:
                    state = self._approach(move, state)
                f = self.b.frame + 1
                if events:
                    events[0]["frame"] = f
                    f += 1
            target = self.target_point(move, move.obj, state)
        else:
            raise RouteError(f"{move.name}: a click move needs obj or point")
        events.append({"frame": f, "rclick" if move.rclick else "click": list(target)})
        if len(events) == 2:
            # The key's frame runs first so the verb is set when the click comes.
            self._schedule(events[:1])
            self._run(frame=events[0]["frame"])
            state = self.b.state()
            if state["vars"][self.p.verb_var] != verb:
                raise RouteError(f"{move.name}: verb key did not select verb {verb}")
            if move.obj is not None:  # the camera may have moved during the key's frame
                events[1]["click"] = list(self.target_point(move, move.obj, state))
            self._schedule(events[1:])
        else:
            self._schedule(events)
        if move.obj2 is not None:
            self._run(frame=f)
            # Two inventory items in rows that are never shown together ("Use flint with compass"):
            # the arrows keep the half-built sentence (global/script-004.txt [02E6]-[02F5]).
            self._scroll_to(move, move.obj2)
            g = self.b.frame + 1
            point = self.target_point(move, move.obj2, self.b.state())
            self._schedule([{"frame": g, "click": list(point)}])
            return g
        return f

    def _run_reclicking(self, move: Move, issue: int, retries: int = 5) -> dict:
        """Wait for a walk to an object; another script can stop ego's walk and kill the sentence
        (the slide's carrier with gate "input"; room 39's pan cutscene when ego crosses the middle,
        doSentence(STOP)), so when ego stands still with input on (and, for gate "control", no
        cutscene open) and `until` does not hold, click the object again."""
        self._run(frame=max(issue, self.b.frame + 1))
        still, cam = 0, None
        while not self.b.holds(move.until):
            if self.b.frame >= issue + move.max_frames:
                raise RouteError(f"{move.name}: `until` did not hold within {move.max_frames} frames of frame {issue}")
            st = self.b.status
            # The camera too: a pan can outlast its cutscene (room 39's panCameraTo after
            # endCutscene), and a click then lands at a moving screen offset.
            ready = st.get("up", 0) > 0 and (move.gate == "input" or st.get("cs", 0) == 0) and st.get("xstart") == cam
            cam = st.get("xstart")
            still = still + 1 if st.get("mv", 0) == 0 and ready else 0
            if still >= 3 and retries > 0:
                retries -= 1
                still = 0
                if move.obj_from_var is not None:  # room 39: the middle marker's pan picks a real cave
                    move = replace(move, obj=self.b.state()["vars"][move.obj_from_var] or move.obj)
                self._click(replace(move, until=[]))
            self._run(frame=self.b.frame + 1)
        return self.b.status

    RECLICK_LIMIT = 600  # frames a re-clicked walk may take
    RECLICK_AHEAD = (16, 40)  # floor candidates: px ahead of ego along its heading ...
    RECLICK_SIDE = (0, 8, -8)  # ... and px to either side
    # Equal finish frames go to the first kind in the order. Both orders are played on forks and
    # the faster kept: a restart on a tie gains a fraction of a pixel that pays off frames later
    # (the cave walk: 3 frames), while elsewhere taking it pushes the greedy choice off its best line.
    RECLICK_ORDERS = (("none", "own", "floor"), ("floor", "own", "none"))

    def _reclick_walk(self, move: Move, goal: list[dict]) -> None:
        """Play a Walk-to walk to ``goal`` with re-clicks where a restarted walk gains ground.

        A new walk starts its step fractions at zero (engines/scumm/actor.cpp calcMovementFactor)
        and every step floors the position (actorWalkStep), so on an axis moving left or up the
        first step of a fresh walk is up to a pixel longer than the running average; in the
        small-scale walk boxes (room 70's path to the ghost ship: 0.5 px a frame upward, room 25's
        jungle path) that is a large share of a step. Moving right or down it is shorter, so a
        restart there loses. The re-clicks are chosen greedily (_reclick_greedy) under each tie order
        on a fork, and the events of the order that reaches ``goal`` first are played here.
        """
        best = None
        for ties in self.RECLICK_ORDERS:
            trial = Executor(self._fork(self.b), self.release, self._fork)
            trial.skips = dict(self.skips)
            try:
                trial._reclick_greedy(move, goal, ties)
                if best is None or trial.b.frame < best[0]:
                    best = (trial.b.frame, list(trial.movie))
            except (RouteError, ControlError):
                pass
            finally:
                trial.b.quit()
        if move.reclick_sweep:
            sweep = self._reclick_sweep(move, goal, best[0] if best is not None else None)
            if sweep is not None and (best is None or sweep[0] < best[0]):
                best = sweep
        if best is None:
            raise RouteError(f"{move.name}: re-clicked walk did not reach {goal}")
        if best[1]:
            self._schedule(best[1])
        st = self._run(until=goal, max_frame=best[0])
        if st["stop"] != "until":
            raise RouteError(f"{move.name}: re-clicked walk replayed off its trial (frame {st['frame']})")

    # (ahead, side) px of the two floor points a sweep alternates between, per variant tried.
    SWEEP_PAIRS = (((40, 0), (40, 8)), ((40, 0), (40, -8)), ((16, 0), (40, 0)))

    def _reclick_sweep(self, move: Move, goal: list[dict], bound: int | None) -> tuple[int, list[dict]] | None:
        """The greedy re-click (_reclick_greedy) judges each frame by the finish frame with no
        further input, so it cannot see a gain that only adds up over many restarts: a fresh walk
        gains at most a pixel's fraction a frame (moving left or up). Here a walker branch clicks the
        floor every frame, alternating between two points ahead along ego's heading (a new target
        each frame, so startWalkActor never ignores it and every step is a fresh one), and on every
        frame a fork tries the move's own click instead and runs to ``goal``. Returns the best
        (finish frame, events) over SWEEP_PAIRS, or None."""
        end = self.b.frame + self.RECLICK_LIMIT
        ck = "rclick" if move.rclick else "click"
        best = None
        for pair in self.SWEEP_PAIRS:
            walker = Executor(self._fork(self.b), self.release, self._fork)
            walker.skips = dict(self.skips)
            try:
                prev, i = None, 0
                while not walker.b.holds(goal) and walker.b.frame < end:
                    limit = min(x for x in (bound, best[0] if best else None, end) if x is not None)
                    if walker.b.frame + 1 >= limit:
                        break  # an own click from here cannot finish sooner
                    st = walker.b.status
                    state = walker.b.state()
                    ego = next((a for a in state.get("actors", []) if a["id"] == 1), None)
                    f = walker.b.frame + 1
                    if ego is not None and st.get("up", 0) > 0 and st.get("cs", 1) == 0:
                        pts = walker.object_points(move.obj, state)
                        if pts:
                            own = [{"frame": f, ck: list(pts[0])}]
                            t = walker._finish_frame(own, None, f, goal, end)
                            if t is not None and (best is None or t < best[0]):
                                best = (t, list(walker.movie) + own)
                        pt = walker._sweep_point(move, state, ego, prev, pair[i % 2])
                        if pt is not None:
                            walker._schedule([{"frame": f, "click": pt}])
                        i += 1
                    prev = (ego["x"], ego["y"]) if ego else None
                    walker._run(frame=f)
            except (RouteError, ControlError):
                pass
            finally:
                walker.b.quit()
        return best

    def _sweep_point(self, move: Move, state: dict, ego: dict, prev: tuple[int, int] | None,
                     offset: tuple[int, int]) -> list[int] | None:
        """The floor point ``offset`` = (ahead, side) px from ego along its heading, on screen and
        hitting no object or actor; None if it does not."""
        ux, uy = self._heading(move, state, ego, prev)
        if ux == uy == 0:
            return None
        xs = self._xstart()
        lag = xs - self.b.status["xstart"]
        k, side = offset
        px = round(ego["x"] + ux * k - uy * side) - xs
        py = round(ego["y"] + uy * k + ux * side)
        if not (0 <= px < SCREEN_W and 0 <= py < self._main_h()):
            return None
        hit = self.b.at(px + lag, py)
        return None if hit.get("obj") or hit.get("actor") else [px, py]

    def _reclick_greedy(self, move: Move, goal: list[dict], ties: tuple[str, ...]) -> None:
        """Each frame with input on and no cutscene, the candidates are: no input, the move's own
        click (the object again), and floor clicks a little ahead of ego along its heading (then the
        own click on the next frame, so the walk still ends on the object). Each is played on a fork
        until ``goal`` holds, and the first to get there is kept (equal frames: ``ties``). Floor
        points are checked with the engine's hit test: no object, no actor."""
        end = self.b.frame + self.RECLICK_LIMIT
        prev = None
        floor_last = False  # the last click was a floor point: the own click must still follow
        while not self.b.holds(goal):
            if self.b.frame >= end:
                raise RouteError(f"{move.name}: re-clicked walk did not reach {goal} within {self.RECLICK_LIMIT} frames")
            st = self.b.status
            state = self.b.state()
            ego = next((a for a in state.get("actors", []) if a["id"] == 1), None)
            f = self.b.frame + 1
            own = None
            if st.get("up", 0) > 0 and st.get("cs", 1) == 0 and ego is not None:
                pts = self.object_points(move.obj, state)
                own = list(pts[0]) if pts else None
            if own is None:
                prev = (ego["x"], ego["y"]) if ego else None
                self._run(frame=f)
                continue
            ck = "rclick" if move.rclick else "click"  # an rclick move re-clicks its own default verb
            cands = [("none", [], floor_last), ("own", [{"frame": f, ck: own}], False)]
            for p in self._reclick_points(move, state, ego, prev):
                cands.append(("floor", [{"frame": f, "click": p}], True))
            best = None  # (finish frame, tie rank, kind, events)
            for kind, evs, follow in cands:
                t = self._finish_frame(evs, own if follow else None, f, goal, end, ck)
                if t is not None and (best is None or (t, ties.index(kind)) < best[:2]):
                    best = (t, ties.index(kind), kind, evs)
            kind, evs = (best[2], best[3]) if best is not None else ("own", [{"frame": f, ck: own}])
            if evs:
                self._schedule(evs)
            floor_last = kind == "floor" or (kind == "none" and floor_last)
            prev = (ego["x"], ego["y"])
            self._run(frame=f)

    def _finish_frame(self, events: list[dict], follow: list[int] | None, f: int, goal: list[dict],
                      end: int, follow_kind: str = "click") -> int | None:
        """On a fork: ``events``, then (if given) the click ``follow`` on the next frame; the frame
        ``goal`` first holds, or None."""
        trial = self._fork(self.b)
        try:
            evs = list(events)
            if follow is not None:
                evs.append({"frame": f + 1, follow_kind: follow})
            trial.inputs(evs)
            st = trial.run(until=goal, max_frame=end)
            return st.get("frame") if st.get("stop") == "until" else None
        except ControlError:
            return None
        finally:
            trial.quit()

    def _heading(self, move: Move, state: dict, ego: dict, prev: tuple[int, int] | None) -> tuple[float, float]:
        """Ego's direction of travel: the last frame's step, or toward the object's walk point."""
        if prev is not None and prev != (ego["x"], ego["y"]):
            dx, dy = ego["x"] - prev[0], ego["y"] - prev[1]
        else:
            box = next((o for o in state["room_objects"] if o["id"] == move.obj), None)
            dx, dy = (box["walk"][0] - ego["x"], box["walk"][1] - ego["y"]) if box else (0, 0)
        n = (dx * dx + dy * dy) ** 0.5
        return (dx / n, dy / n) if n else (0.0, 0.0)

    def _reclick_points(self, move: Move, state: dict, ego: dict, prev: tuple[int, int] | None) -> list[list[int]]:
        """Floor points 16 and 40 px ahead of ego along its heading (the last frame's step, or
        toward the object's walk point), and 8 px to either side."""
        ux, uy = self._heading(move, state, ego, prev)
        if ux == uy == 0:
            return []
        xs = self._xstart()
        lag = xs - self.b.status["xstart"]
        out: list[list[int]] = []
        for k in self.RECLICK_AHEAD:
            for side in self.RECLICK_SIDE:
                px = round(ego["x"] + ux * k - uy * side) - xs
                py = round(ego["y"] + uy * k + ux * side)
                if 0 <= px < SCREEN_W and 0 <= py < self._main_h() and [px, py] not in out:
                    hit = self.b.at(px + lag, py)
                    if not hit.get("obj") and not hit.get("actor"):
                        out.append([px, py])
        return out

    def _step(self, move: Move) -> None:
        """One frame, pressing the first of the move's press rules whose `when` holds."""
        f = self.b.frame + 1
        for rule in move.press:
            if self.b.holds(rule["when"]):
                self._schedule([{"frame": f, "key": rule["key"]}])
                break
        self._run(frame=f)

    def _run_pressing(self, move: Move, issue: int) -> dict:
        self._run(frame=max(issue, self.b.frame + 1))
        while not self.b.holds(move.until):
            if self.b.frame >= issue + move.max_frames:
                raise RouteError(f"{move.name}: `until` did not hold within {move.max_frames} frames of frame {issue}")
            f = self.b.frame + 1
            for rule in move.press:
                if self.b.holds(rule["when"]):
                    self._schedule([{"frame": f, "key": rule["key"]}])
                    break
            self._run(frame=f)
        return self.b.status

    def _wait_walk_to(self, move: Move, limit: int = 120) -> dict:
        for _ in range(limit):
            self._run(frame=self.b.frame + 1)
            state = self.b.state()
            if self._selected_verb(move, state) == self.p.walk_to:
                return state
        raise RouteError(f"{move.name}: the verb did not return to Walk to within {limit} frames")

    def _selected_verb(self, move: Move, state: dict) -> int:
        """The verb a click for ``move`` would use: the hover script's saved verb comes back as
        soon as the mouse leaves the inventory for the room."""
        verb = state["vars"][self.p.verb_var]
        hv = self.p.hover_saved_verb_var
        in_room = move.point is not None or (move.obj is not None and move.obj not in state["inventory"])
        if hv is not None and in_room and state["vars"][hv]:
            return state["vars"][hv]
        return verb

    def _choose(self, move: Move) -> int:
        """Pick a dialogue line on the first frame its menu takes a click: by key digit, or by
        clicking the one visible line whose text contains ``move.choose``."""
        end = self.b.frame + move.max_frames
        while True:
            st = self.b.status
            # gate "input": a menu inside another script's cutscene (room 83's crew lines while
            # Otis's cutscene carries the slide) takes a click as soon as input is on.
            if st.get("up", 0) > 0 and (st.get("cs", 1) == 0 or move.gate == "input"):
                lines = [v for v in self.b.state()["verbs"]
                         if DIALOG_FIRST <= v["id"] <= DIALOG_LAST and v["name"]]
                if lines:
                    break
            if self.b.frame >= end:
                raise RouteError(f"{move.name}: no dialogue menu within {move.max_frames} frames")
            self._step(move)
        f = self.b.frame + 1
        if move.choose_by is not None:
            said = self.b.state()["vars"][move.choose_by["var"]]
            line = move.choose_by["map"].get(str(said))
            if line is None:
                raise RouteError(f"{move.name}: var {move.choose_by['var']} is {said}, which has no planned reply")
            move = replace(move, choose=line)
        if isinstance(move.choose, int):
            self._schedule([{"frame": f, "key": str(move.choose)}])
            return f
        needle = move.choose.lower()
        hits = [v for v in lines if needle in v["name"].lower()]
        if len(hits) != 1:
            shown = "; ".join(repr(v["name"]) for v in lines)
            raise RouteError(f"{move.name}: {len(hits)} visible lines contain {move.choose!r}: {shown}")
        point = (hits[0]["x"] + 2, hits[0]["y"] + 2)
        if self.b.at(*point).get("verb_slot") != hits[0]["slot"]:
            raise RouteError(f"{move.name}: a click at {point} does not hit the line {hits[0]['name']!r}")
        self._schedule([{"frame": f, "click": list(point)}])
        return f

    def _wait_verb_visible(self, move: Move, verb: int) -> dict:
        """Run until ``verb`` is on screen: a verb key does nothing while the verbs are saved
        away (cutscenes, room entries, dialogues)."""
        for _ in range(move.max_frames):
            state = self.b.state()
            if any(v["id"] == verb for v in state["verbs"]):
                return state
            self._run(frame=self.b.frame + 1)
        raise RouteError(f"{move.name}: verb {verb} never became visible")

    def _wait_actor(self, move: Move) -> dict:
        """Run until actor ``move.obj`` can be clicked (room 2's wandering monkey)."""
        for _ in range(move.max_frames):
            self._run(frame=self.b.frame + 1)
            state = self.b.state()
            if self.actor_points(move.obj, state):
                return state
        raise RouteError(f"{move.name}: actor {move.obj} never came on screen")

    def _floor_point(self, x: int, y: int, goal: int | None) -> list[int]:
        """(x, y), or the nearest point inward of it that is floor: a click on another object would be
        a walk to that object (the catacombs' exit strips at the screen edge, room 39's 543/544)."""
        lag = self._xstart() - self.b.status["xstart"]
        step = -1 if x > SCREEN_W // 2 else 1
        for dx in range(0, 64, 4):
            for dy in (0, -4, 4, -8, 8):
                px, py = x + step * dx, y + dy
                if 0 <= px < SCREEN_W and 0 <= py < self._main_h():
                    hit = self.b.at(px + lag, py).get("obj")
                    if not hit or hit == goal:
                        return [px, py]
        return [x, y]

    def _approach(self, move: Move, state: dict) -> dict:
        """Floor-click toward an off-screen object's walk point until the object is on screen."""
        box = next((o for o in state["room_objects"] if o["id"] == move.obj), None)
        if box is None:
            raise RouteError(f"{move.name}: object {move.obj} is not in room {state['room']}")
        if move.approach == "search" and move.via is None and self._fork is not None:
            move = replace(move, via=self._search_via(move, box))
        last, stalls, seen = None, 0, None
        for _ in range(move.max_frames):
            xs = self._xstart()
            wx, wy = move.via if move.via is not None else box["walk"]
            if move.approach_y is not None:
                wy = move.approach_y
            target = self._floor_point(max(0, min(SCREEN_W - 1, wx - xs)), max(0, min(self._main_h() - 1, wy)),
                                       move.obj)
            virtual = (target[0] + xs, target[1])
            # A new click when the clipped target moved (the camera scrolled) or ego stopped.
            renew = virtual != last
            if renew and move.approach_hold and last is not None:
                ego = next((a for a in self.b.state().get("actors", []) if a["id"] == 1), {})
                renew = abs(ego.get("x", last[0]) - last[0]) <= move.approach_hold
            if renew or self.b.status.get("mv", 0) == 0:
                ego = next((a for a in self.b.state().get("actors", []) if a["id"] == 1), {})
                where = (ego.get("x"), ego.get("y"), xs)
                # Only while input is on: a click in a cutscene or a map label does nothing either.
                stalls = stalls + 1 if where == seen and self.b.status.get("up", 0) > 0 else 0
                seen = where
                if stalls >= 3:  # floor clicks get ego no further: no floor path to the object
                    return self._stepping_stone(move, box)
                f = self.b.frame + 1
                if self._selected_verb(Move(move.name, point=(0, 0)), self.b.state()) != self.p.walk_to:
                    self._wait_walk_to(move)
                    f = self.b.frame + 1
                self._schedule([{"frame": f, "click": target}])
                self._run(frame=f)
                last = virtual
            else:
                self._run(frame=self.b.frame + 1)
            state = self.b.state()
            if self.object_points(move.obj, state):
                return state
        raise RouteError(f"{move.name}: object {move.obj} never came on screen")

    def _approach_point(self, move: Move) -> None:
        """Floor-click toward an off-screen floor point (at its own y) until the camera shows it.
        Each click goes to the screen edge on the point's side, the nearest pixel inward that is
        floor; inside a cutscene (the item slide) every scene click is such a walk anyway."""
        px, py = move.point
        last, stalls, seen = None, 0, None
        for _ in range(move.max_frames):
            xs = self._xstart()
            if 0 <= px - xs < SCREEN_W:
                return
            target = self._floor_point(max(0, min(SCREEN_W - 1, px - xs)), py, None)
            virtual = (target[0] + xs, target[1])
            if virtual != last or self.b.status.get("mv", 0) == 0:
                ego = next((a for a in self.b.state().get("actors", []) if a["id"] == 1), {})
                where = (ego.get("x"), ego.get("y"), xs)
                stalls = stalls + 1 if where == seen and self.b.status.get("up", 0) > 0 else 0
                seen = where
                if stalls >= 3:
                    raise RouteError(f"{move.name}: floor clicks toward {move.point} get ego no further")
                f = self.b.frame + 1
                if self._selected_verb(Move(move.name, point=(0, 0)), self.b.state()) != self.p.walk_to:
                    self._wait_walk_to(move)
                    f = self.b.frame + 1
                self._schedule([{"frame": f, "click": target}])
                self._run(frame=f)
                last = virtual
            else:
                self._run(frame=self.b.frame + 1)
        raise RouteError(f"{move.name}: point {move.point} never came on screen")

    def _search_via(self, move: Move, box: dict) -> tuple[int, int] | None:
        """The approach point for ``move`` (room coordinates) after which the whole move ends
        first, tried on forks: the clipped walk point (None, the default) and floor points 4-16 rows
        above and below it. The box path to a floor point depends on its walk box, so a few rows
        can pick another route (room 77's hatch: box 8 via the lower deck, or box 7 straight on)."""
        xs = self._xstart()
        lag = xs - self.b.status["xstart"]
        x = max(0, min(SCREEN_W - 1, box["walk"][0] - xs))
        y0 = max(0, min(self._main_h() - 1, box["walk"][1]))
        cands: list[tuple[int, int] | None] = [None]
        for dy in (-4, -8, -12, -16, 4, 8, 12, 16):
            y = y0 + dy
            if 0 <= y < self._main_h() and not self.b.at(x + lag, y).get("obj"):
                cands.append((x + xs, y))
        best, best_frame = None, None
        for via in cands:
            trial = Executor(self._fork(self.b), self.release, None)
            trial.skips = dict(self.skips)
            try:
                done = trial.play(replace(move, approach=None, via=via, delay=0, skips=None)).done_frame
            except RouteError:
                done = None
            finally:
                trial.b.quit()
            if done is not None and (best_frame is None or done < best_frame):
                best, best_frame = via, done
        return best

    STONE_FRAMES = 300  # a stepping stone must bring the object on screen within this

    def _stepping_stone(self, move: Move, box: dict) -> dict:
        """No floor path leads toward ``box`` (room 39's halves: only the walk-to of the middle
        markers 543/544 crosses, and starts the pan): walk to a visible object whose walk point is
        nearer the goal than ego, tried on a fork first; keep the first after which the goal comes
        on screen in the same room."""
        state = self.b.state()
        room = state["room"]
        ego = next(a for a in state["actors"] if a["id"] == 1)
        gx = box["walk"][0]
        stones = sorted((o for o in state["room_objects"]
                         if o["id"] != move.obj and abs(o["walk"][0] - gx) < abs(ego["x"] - gx)
                         and self.object_points(o["id"], state)), key=lambda o: abs(o["walk"][0] - gx))
        for o in stones if self._fork is not None else []:
            trial = Executor(self._fork(self.b), self.release, self._fork)
            trial.skips = dict(self.skips)
            frames = None
            try:
                trial.play(Move(f"{move.name}: via {o['id']}", verb=self.p.walk_to, obj=o["id"]))
                start = trial.b.frame
                while trial.b.frame - start < self.STONE_FRAMES and trial.b.status.get("room") == room:
                    if trial.object_points(move.obj, trial.b.state()):
                        frames = trial.b.frame - start
                        break
                    trial._run(frame=trial.b.frame + 1)
            except RouteError:
                pass
            trial.b.quit()
            if frames is not None:
                self.play(Move(f"{move.name}: via {o['id']}", verb=self.p.walk_to, obj=o["id"]))
                self._run(frame=self.b.frame + frames)
                state = self.b.state()
                if self.object_points(move.obj, state):
                    return state
        raise RouteError(f"{move.name}: object {move.obj} never came on screen (no floor path, no stepping stone)")

    def play_steered(self, move: Move, max_delay: int = 60, window: int = 900) -> MoveResult:
        """Play ``move``; if its ``until`` never holds, it hinged on a random draw (an insult
        fight's counter, the opponent's insult, the safe combination): retry from a snapshot
        taken before the move with 1..``max_delay`` frames of delay, and keep the first delay
        that works. Every frame spent waiting at a menu or on the floor draws the RNG, so the
        delay moves the draw."""
        if self._fork is None:
            return self.play(move)
        snap = self._fork(self.b)
        mark = len(self.movie), len(self.results)
        try:
            res = self.play(move)
            snap.quit()
            return res
        except RouteError as first:
            failure = first
        if move.optional:
            snap.quit()
            raise failure  # play_group() rolls the whole optional group back
        for d in range(1, max_delay + 1):
            trial = Executor(self._fork(snap), self.release, self._fork)
            trial.skips = dict(self.skips)
            try:
                trial.play(replace(move, delay=move.delay + d, max_frames=min(move.max_frames, window)))
                ok = True
            except RouteError:
                ok = False
            trial.b.quit()
            if ok:
                self.b.quit()
                self.b = snap
                del self.movie[mark[0]:], self.results[mark[1]:]
                return self.play(replace(move, delay=move.delay + d))
        snap.quit()
        raise RouteError(f"{failure} (and with 1..{max_delay} frames of delay)")

    def play_group(self, moves: list[Move]) -> list[MoveResult]:
        """Play consecutive optional moves (a stall block) as one unit: if any of them fails,
        go back to the state before the first and leave the whole block out."""
        if self._fork is None or not all(m.optional for m in moves):
            return [self.play_steered(m) for m in moves]
        snap = self._fork(self.b)
        mark = len(self.movie), len(self.results)
        try:
            out = [self.play_steered(m) for m in moves]
            snap.quit()
            return out
        except RouteError:
            self.b.quit()
            self.b = snap
            del self.movie[mark[0]:], self.results[mark[1]:]
        # A block hinges on frame timing between its moves (a reading must start after the walk
        # it rides on has started): try a few frames of delay on each move in turn.
        for d in range(1, 9):
            for k in range(len(moves)):
                tried = [replace(m, delay=m.delay + d) if j == k else m for j, m in enumerate(moves)]
                trial = Executor(self._fork(self.b), self.release, self._fork)
                trial.skips = dict(self.skips)
                try:
                    for m in tried:
                        trial.play_steered(m)
                    ok = True
                except RouteError:
                    ok = False
                trial.b.quit()
                if ok:
                    return [self.play_steered(m) for m in tried]
        out = [MoveResult(m.name, self.b.frame, self.b.frame, self.b.status["qf"], skipped=True) for m in moves]
        self.results.extend(out)
        return out

    def play_all(self, moves: list[Move]) -> list[MoveResult]:
        for m in moves:
            try:
                self.play(m)
            except ControlError as e:
                raise RouteError(f"{m.name}: {e}") from e
        return self.results
