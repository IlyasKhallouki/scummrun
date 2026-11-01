"""Unit tests for the Executor's stepping stone on a stone-only fork (``Executor.stone_fork``): with no fork at all the
stone is not tried, as before; with ``stone_fork`` each trial plays on a fork of its own that has no fork, and the stone
the trial kept is played on the executor's own branch.
"""

import pytest

from speedrun.executor import Executor, Move, RouteError


class FakeBranch:
    """A room 640 wide: ego at x 67, the goal 316 off screen, an object 333 on screen whose walk takes ego past the
    point where the camera pans. A walk to 333 pans the camera on this branch."""

    def __init__(self, name="main"):
        self.name, self.f, self.xstart = name, 100, 0
        self.played = []

    @property
    def frame(self):
        return self.f

    @property
    def status(self):
        return {"frame": self.f, "room": 28, "xstart": self.xstart, "up": 1, "mv": 0}

    def state(self):
        return {"room": 28, "actors": [{"id": 1, "x": 67 if not self.xstart else 330, "y": 125}],
                "room_objects": [{"id": 316, "x": 592, "y": 72, "w": 32, "h": 64, "walk": [595, 125]},
                                 {"id": 333, "x": 290, "y": 100, "w": 20, "h": 30, "walk": [330, 137]}]}

    def run(self, frame=None, **kw):
        self.f = frame if frame is not None else self.f + 1
        return self.status

    def quit(self):
        self.played.append("quit")


def _executor(monkeypatch, branch):
    def play(self, move):
        self.b.played.append(move.obj)
        if move.obj == 333:
            self.b.xstart = 288
        return None
    monkeypatch.setattr(Executor, "play", play)
    monkeypatch.setattr(Executor, "object_points",
                        lambda self, obj, state: [(1, 1)] if obj == 333 or self.b.xstart else [])
    monkeypatch.setattr(Executor, "_run", lambda self, **kw: self.b.run(**kw))
    return Executor(branch, "dos-cd", fork=None)


def test_without_a_fork_no_stone_is_tried(monkeypatch):
    b = FakeBranch()
    ex = _executor(monkeypatch, b)
    box = b.state()["room_objects"][0]
    with pytest.raises(RouteError, match="no stepping stone"):
        ex._stepping_stone(Move("walk_to(316)", verb=11, obj=316), box)
    assert b.played == []


def test_a_stone_fork_tries_the_stone_on_a_fork_and_plays_it_here(monkeypatch):
    b = FakeBranch()
    forks = []

    def fork(branch):
        assert branch is b
        child = FakeBranch(f"fork{len(forks)}")
        child.f = branch.f
        forks.append(child)
        return child
    ex = _executor(monkeypatch, b)
    ex.stone_fork = fork
    box = b.state()["room_objects"][0]
    state = ex._stepping_stone(Move("walk_to(316)", verb=11, obj=316), box)
    assert len(forks) == 1 and forks[0].played == [333, "quit"]  # the trial, then its branch closed
    assert b.played == [333] and b.xstart == 288 and state["room"] == 28
