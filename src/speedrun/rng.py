"""ScummVM's random number generator, offline.

``Common::RandomSource`` (``third_party/scummvm/common/random.cpp``) is a
32-bit xorshift: every draw scrambles the state with the shift triplet
(13, 21, 11) and returns ``state * 0xDEADBF03 mod 2^32 mod (max + 1)``. Seed 0
becomes 1. SCUMM's ``getRandomNr(max)`` opcode is one such draw, so a random
outcome in the game is a function of the state the bridge reports (control
status ``rng``) and the number of draws taken since.

The step is a bijection, so it can be run backwards: the seed that puts the
generator in a chosen state after ``n`` draws is ``rewind(state, n)``.
"""

MASK = 0xFFFFFFFF
MULT = 0xDEADBF03


def step(state: int) -> int:
    state ^= state >> 13
    state ^= (state << 21) & MASK
    state ^= state >> 11
    return state


def _unshift_right(y: int, s: int) -> int:
    x = y
    for _ in range(32 // s + 1):
        x = y ^ (x >> s)
    return x


def _unshift_left(y: int, s: int) -> int:
    x = y
    for _ in range(32 // s + 1):
        x = y ^ ((x << s) & MASK)
    return x


def unstep(state: int) -> int:
    state = _unshift_right(state, 11)
    state = _unshift_left(state, 21)
    return _unshift_right(state, 13)


def seed_state(seed: int) -> int:
    """The state ``--random-seed`` gives (``RandomSource::setSeed``)."""
    return (seed & MASK) or 1


def advance(state: int, n: int) -> int:
    for _ in range(n):
        state = step(state)
    return state


def rewind(state: int, n: int) -> int:
    for _ in range(n):
        state = unstep(state)
    return state


def value(state: int, maximum: int) -> int:
    """What a draw returns when it leaves the generator in ``state``."""
    return ((state * MULT) & MASK) % (maximum + 1)


def draws(state: int, n: int, maximum: int) -> list[int]:
    """The next ``n`` draws of ``getRandomNumber(maximum)`` from ``state``."""
    out = []
    for _ in range(n):
        state = step(state)
        out.append(value(state, maximum))
    return out


def distance(start: int, target: int, limit: int) -> int | None:
    """Draws from ``start`` to ``target``, if within ``limit``."""
    s = start
    for n in range(limit + 1):
        if s == target:
            return n
        s = step(s)
    return None

