from speedrun import rng


def test_step_matches_common_random():
    # common/random.cpp scrambleSeed() with (13, 21, 11), from seed 1: the boot status of a
    # --random-seed=1 run reports 2098177 after its one draw.
    assert rng.step(1) == 2098177


def test_unstep_inverts_step():
    for s in (1, 2098177, 0xDEADBEEF, 0x80000000, 836557856):
        assert rng.unstep(rng.step(s)) == s


def test_rewind_inverts_advance():
    s = rng.seed_state(1013804604)
    assert rng.rewind(rng.advance(s, 3184), 3184) == s


def test_seed_zero_becomes_one():
    assert rng.seed_state(0) == 1


def test_draws_are_values_of_the_stepped_states():
    s = 12345
    out = rng.draws(s, 3, 3)
    assert out == [rng.value(rng.advance(s, i), 3) for i in (1, 2, 3)]
    assert all(0 <= v <= 3 for v in out)


def test_distance():
    s = 99
    assert rng.distance(s, rng.advance(s, 17), 100) == 17
    assert rng.distance(s, rng.advance(s, 17), 10) is None
