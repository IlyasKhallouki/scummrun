"""Tests for the script IR on real MI1 lines and units.

usage: uv run pytest experiments/E003-path-through-cloud -q

The tests name MI1 files and offsets: they check the parser on this dump, they are not part of the
system that rule C2 covers.
"""

import re

import pytest

import ir
from ir import (Assign, Bit, Concat, Const, Embed, Getter, Index, Jump, List, Local, Not, Op, Start, Str, Sub,
                Truth, Var)


@pytest.fixture(scope="module")
def units():
    return ir.load()


def at(u: ir.Unit, offset: int) -> int:
    return u.at[offset]


def run(u: ir.Unit, i: int, taken: bool) -> list[int]:
    """Offsets visited from instruction i, taking every conditional jump (or none), until the end."""
    seen = []
    while i < len(u.ins) and len(seen) < 50:
        x = u.ins[i]
        seen.append(x.offset)
        if x.ends == "always":
            break
        if isinstance(x.st, Jump) and (x.st.cond is None or taken):
            i = x.target
        else:
            i = x.next
    return seen


def test_else_if_chain(units):
    u = units[("obj", 17)]
    hidden = [x for x in u.ins if x.hidden == "elseif-jump"]
    assert [(x.offset, x.st.to) for x in hidden] == [(0x0021, 0x004E), (0x0036, 0x004E)]
    test = u.ins[at(u, 0x0024)]  # the test sits 3 bytes after the else-if line's offset
    assert test.st == Jump(ir.Cmp("==", Var(4), Const(131)), 0x0039, "elseif")
    assert u.ins[at(u, 0x000F)].target == at(u, 0x0024)  # the first test's false edge lands on it
    zero = u.ins[at(u, 0x004B)]  # "/* goto 004E; */": a 0-byte jump to the next instruction
    assert zero.st.form == "goto0" and zero.target == at(u, 0x004E)
    assert u.ins[at(u, 0x004E)].ends == "room"  # actorFollowCamera can change the room


def test_shared_offset_verbs(units):
    u = units[("obj", 17)]
    assert u.entries == {7: 0, 11: 0}
    u = units[("obj", 26)]
    assert u.entries[8] == u.entries[9] == at(u, 0x0018) and u.entries[90] == at(u, 0x0012)
    u = units[("obj", 566)]  # verb 91 twice in the table: the first event wins
    assert u.entries[91] == at(u, 0x0021) and u.shadowed == [(91, at(u, 0x001B))]


def test_wait_loop(units):
    u = units[("local", 45, 200)]
    wait, test = u.ins[at(u, 0x05AB)], u.ins[at(u, 0x05AC)]
    assert wait.st == Op("breakHere", ()) and wait.yields == "frame"
    assert test.st == Jump(Truth(Var(194)), 0x05AB, "unless") and test.target == at(u, 0x05AB)
    assert test.next == at(u, 0x05B1)


def test_while_loop(units):
    u = units[("local", 45, 200)]
    back = u.ins[at(u, 0x05A8)]  # the jump back, printed as the loop's closing brace
    assert back.hidden == "while-back" and back.target == at(u, 0x0577) and back.next is None
    assert u.ins[at(u, 0x0577)].target == at(u, 0x05AB)
    assert u.ins[at(u, 0x05A6)].yields == "wait"


def test_override_skips_its_jump(units):
    u = units[("local", 45, 200)]
    i = at(u, 0x05B1)
    x = u.ins[i]
    assert x.st == Op("beginOverride", ()) and x.esc == i + 1 and x.next == i + 2
    assert u.ins[i + 1].st == Jump(None, 0x0880, "goto")


def test_give_dispatch(units):
    u = units[("global", 2)]
    assert u.ins[at(u, 0x006B)].st == Op("doSentence", (Const(4), Local(1), Local(2)))
    to_b = u.ins[at(u, 0x009A)].st  # walk to B, then B's entry 80 with A as its argument
    assert to_b == Start("startObject", Const(80), Local(2), (Local(1),), False, False)
    to_a = u.ins[at(u, 0x010D)].st
    assert to_a == Start("startObject", Const(4), Local(1), (Local(2),), False, False)
    hook = u.ins[at(u, 0x0127)].st
    assert hook.script == Var(116) and hook.args == (Local(1), Local(3), Local(2))
    assert u.ins[at(u, 0x0315)].st.flags == ("STOP",) and u.ins[at(u, 0x0315)].ends == "if_self"
    assert u.ins[at(u, 0x0317)].ends == "always"  # stopScript(0)


def test_start_stops_its_target(units):
    """A start without R stops script n, or every script of object o, so a script that restarts itself ends."""
    u = units[("global", 67)]
    assert u.ins[at(u, 0x0353)].st.script == Const(67) and u.ins[at(u, 0x0353)].ends == "if_self"
    u = units[("obj", 420)]
    assert u.ins[at(u, 0x0047)].st == Start("startObject", Const(2), Const(420), (), False, False)
    assert u.ins[at(u, 0x0047)].ends == "if_self"
    u = units[("entry", 1)]
    assert u.ins[at(u, 0x0123)].st.recursive and u.ins[at(u, 0x0123)].ends is None
    chains = [x for u in units.values() for x in u.ins if isinstance(x.st, Start) and x.st.name == "chainScript"]
    assert chains and all(x.ends == "always" for x in chains)


def test_ambiguous_name_from_bytes():
    """descumm prints var 41 as VAR_TALK_ACTOR too: the operand word in the bytes decides."""
    text = "[0000] (1A) VAR_TALK_ACTOR = 0;\n[0005] (00) stopObjectCode();\n"
    for word, n in ((b"\x19\x00", 25), (b"\x29\x00", 41)):
        u = ir.Unit(("global", 0), None, "t.txt", "")
        ir._read(u, text, bytes(8) + b"\x1a" + word + b"\x00\x00\x00")
        assert u.ins[0].st == Assign(Var(n), "=", Const(0)) and ir.show(u.ins[0].st) == "VAR_TALK_ACTOR = 0;"
        assert u.notes == [("ambiguous name from bytes", 1, f"VAR_TALK_ACTOR is var {n}")]
    u = ir.Unit(("global", 0), None, "t.txt", "")
    ir._read(u, text)
    assert u.ins[0].st == Assign(Var(25), "=", Const(0)) and u.notes[0][0] == "ambiguous name, unverified"


def test_indexed_and_folded(units):
    u = units[("global", 57)]
    assert u.ins[at(u, 0x01AC)].st == Assign(Index("bit", 384, Var(100)), "=", Const(1))
    assert ir.parse_statement("Local[0] = Var[133 + Local[2]];") == Assign(Local(0), "=", Index("var", 133, Local(2)))
    u = units[("obj", 26)]
    assert u.ins[at(u, 0x0018)].st.cond == Not(Bit(528))  # Bit[521 + 7]


def test_two_flag_start(units):
    u = units[("entry", 1)]
    st = u.ins[at(u, 0x0123)].st
    assert st == Start("startScript", Const(45), None, (Const(3),), True, True)
    assert ir.show(st) == "startScript(45,[3],F,R);"


def test_global_176_missing_target(units):
    """descumm printed one else for two jumps; the copy check must send its pass branch to the
    stop and its fail branch to the quit message, as the bytes do."""
    u = units[("global", 176)]
    assert [(x.offset, x.st.to, x.hidden) for x in u.ins if isinstance(x.st, Jump) and x.offset >= 0x0058] == [
        (0x0058, 0x005E, "else-chain"), (0x005B, 0x0061, None), (0x005E, 0x00A2, None)]
    inner = at(u, 0x001E)
    assert u.ins[inner].target == at(u, 0x005B)
    passed = run(u, inner, taken=False)  # Var[74] > 1200: prints, then on to the stop
    assert passed[-2:] == [0x005E, 0x00A2]
    failed = run(u, inner, taken=True)
    assert 0x00A0 in failed and failed[:2] == [0x001E, 0x005B]  # systemOps(3)


def test_else_if_target_from_bytes(units):
    u = units[("global", 23)]
    x = u.ins[at(u, 0x000C)]
    if not (ir.ROOT / u.block).exists():
        pytest.skip("no raw blocks under build/")
    assert x.hidden == "elseif-jump" and x.st.to == 0x0075  # the text rule alone says 008B


def test_expressions():
    st = ir.parse_statement("Exprmode Local[7] = ((<VAR_RESULT = getActorWidth(Local[2])> / 2) + 4);")
    width = Embed(Assign(Var(0), "=", Getter("getActorWidth", (Local(2),))))
    assert st == Assign(Local(7), "=", ir.BinOp("+", ir.BinOp("/", width, Const(2)), Const(4)))
    st = ir.parse_statement('VerbOps(Var[100],[SetXY(2,Var[229]),Text("a " + getName(Var[1]) + "."),On()]);')
    assert st.subops[1] == Sub("Text", (Concat((Str("a "), Getter("getName", (Var(1),)), Str("."))),))
    assert ir.parse_statement("drawObject(687setXY(,37,0));") == Op("drawObject", (Const(687),), (Sub("setXY", (Const(37), Const(0))),))
    assert ir.parse_statement("PutCodeInString(39,);").args == (Const(39), Str(""))
    assert ir.parse_cond("classOfIs(Local[2],[133])") == Truth(Getter("classOfIs", (Local(2), List((Const(133),)))))
    assert ir.parse_statement("putActor(VAR_EGO,-1,-1);").args == (Var(1), Const(-1), Const(-1))


def test_self_check(units):
    log = "\n".join(ir.check(units))
    assert "unparsed lines: 0" in log
    assert "lines in no unit 0, in two units 0; statements that print back differently: 0" in log
    assert "target not an instruction offset" not in log and "closer gap" not in log
    assert re.search(r"object entries: 1393 verbs .* = 1400, index.json counts 1400\)", log)
    assert "empty unit: room-088-fighting/entry.txt" in log and "empty unit: room-088-fighting/exit.txt" in log
    if ir.DESCUMM.exists() and (ir.ROOT / units[("global", 1)].block).exists():
        assert "flat descumm: jumps same 7756, units same 1195\n" in log + "\n"
        assert "ambiguous name, unverified" not in log


def test_var_names_match_sources():
    """The name table against descumm's printer and ScummVM's v5 numbers."""
    descumm = ir.ROOT / "third_party/scummvm-tools/engines/scumm/descumm.cpp"
    vars_cpp = ir.ROOT / "third_party/scummvm/engines/scumm/vars.cpp"
    if not descumm.exists() or not vars_cpp.exists():
        pytest.skip("no third_party sources")
    src = descumm.read_text()
    tables = {}
    for m in re.finditer(r"const char \*(var_names\d)\[\] = \{(.*?)\};", src, re.S):
        items = [x.strip() for x in re.sub(r"/\*.*?\*/", "", m[2]).split(",") if x.strip()]
        tables[m[1]] = [None if x == "NULL" else x.strip('"') for x in items]
    printed = {}  # what descumm prints for var n in v5 (get_var: var_names5, then 4, 3, 2)
    for n in range(len(tables["var_names5"])):
        for t in ("var_names5", "var_names4", "var_names3", "var_names2"):
            if n < len(tables[t]) and tables[t][n]:
                printed.setdefault(n, tables[t][n])
    for name, n in ir.VARS.items():
        assert printed[n] == name
    numbers = {}
    for n, name in printed.items():
        numbers.setdefault(name, []).append(n)
    # every printed name means one var, except the ones the IR resolves from the bytes
    assert {name: tuple(ns) for name, ns in numbers.items() if len(ns) > 1} == ir.AMBIGUOUS
    assert all(ir.VARS[name] in ns for name, ns in ir.AMBIGUOUS.items())
    scummvm = {}
    text = vars_cpp.read_text()
    for fn in ("void ScummEngine::setupScummVars() {", "void ScummEngine_v5::setupScummVars() {"):
        body = text[text.index(fn):text.index("\n}\n", text.index(fn))]
        scummvm.update((name, int(n)) for name, n in re.findall(r"\b(VAR_[A-Z0-9_]+) = (\d+);", body))
    for name, n in ir.VARS.items():
        if name != "VAR_RESULT":
            assert scummvm.get(name) == n, name
    assert scummvm["VAR_KEYPRESS"] == ir.VARS["VAR_RESULT"] == 0
