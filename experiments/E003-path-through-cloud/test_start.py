"""Tests for plan step 2: the start (start.py), the Part I end (end_part1.py) and the game end (end.py).

usage: uv run pytest experiments/E003-path-through-cloud/test_start.py -q

No engine runs here: the start tests read ``results/start-part1.jsonl``, which ``start.py`` writes.
The game-end rule runs on hand-made units, written as descumm lines and read by ir.py's own reader,
and once over the real IR. Tests may name MI1 numbers as fixtures (C2 covers the system, not tests).
"""

import ast
import collections
import json
import re
import subprocess
import sys
from pathlib import Path

import pytest

import end
import end_part1
import ir
import start
from facts import facts_of

HERE = Path(__file__).resolve().parent


def unit(key: tuple, text: str, room: int | None = None) -> ir.Unit:
    u = ir.Unit(key, room, "test.txt", "")
    ir._read(u, text)
    assert not [n for n in u.notes if n[0] == "unparsed"], u.notes
    return u


def kinds(text: str, key=("global", 50)) -> list[str]:
    import collections
    u = unit(key, text)
    return sorted(lp["kind"] for lp in end.loops(key, u, collections.Counter()))


# --- start ---------------------------------------------------------------------------------------

@pytest.fixture(scope="module")
def starts():
    if not start.OUT.exists():
        pytest.skip("results/start-part1.jsonl not written yet (run start.py)")
    return start.load()


def test_start_rows_are_the_tuning_seeds(starts):
    assert sorted(starts) == list(start.SEEDS) == [1, 2, 3, 4, 5]
    assert not set(starts) & start.FORBIDDEN


def test_start_is_a_sentence_idle_from_boot_with_no_input(starts):
    for seed, r in starts.items():
        assert r["stop"]["stop"] == "idle" and r["stop"]["idle"] == "sentence", r["stop"]
        assert r["dump"]["seed"] == seed and r["dump"]["boot_param"] == 0
        assert r["facts"]["var:39"] == 0  # VAR_DEBUGMODE: no debug flag
        assert set(r["trace"]) <= start.TRACE_TYPES, r["trace"]
        assert r["facts"]["cutscene"] == 0 and r["stop"]["up"] > 0
        # the control stop comes earlier and is not the start
        assert r["control_stop"]["frame"] < r["stop"]["frame"]


def test_start_facts_are_facts_of_keys(starts):
    for r in starts.values():
        assert r["facts"]["room"] == r["stop"]["room"] and r["facts"]["ego"] == r["dump"]["ego"]
        assert all(start.fact_kind(k) in {"var", "bit", "owner", "state", "class", "room", "ego", "script", "local",
                                          "objscript", "cutscene", "verb"} or k.startswith(("at:", "frozen:", "esc:"))
                   for k in r["facts"])
        assert not any(k in r["facts"] for k in ("var:11", "var:12", "var:13", "var:14", "var:46", "var:47"))  # clocks


def test_trace_whitelist_holds_no_input_record():
    inputs = {"input", "click", "choice", "interrupt", "skip", "rng_set"}  # the bridge's input records
    assert not inputs & start.TRACE_TYPES


def test_start_facts_are_from_this_facts_py(starts):
    for r in starts.values():
        assert r["facts_py_sha256"] == start.facts_sha256(), "facts.py changed since start.py ran: rerun start.py"


def test_starts_differ_only_in_keys_a_live_script_redraws(starts):
    rows = list(starts.values())
    diff = start.differing(rows)
    assert diff, "the seeds should differ somewhere"
    for k in diff:
        assert all(k in r["random"] and r["random_sites"][k] for r in rows), (k, diff[k])
    assert len({r["stop"]["frame"] for r in rows}) == 1


def test_random_list_is_current_and_facts_keep_their_values(starts):
    units = ir.load()
    for r in starts.values():
        sites, counts = start.live_draws(r["facts"], units)
        assert sorted(sites) == r["random"] and sites == r["random_sites"], "ir.py changed: rerun start.py"
        assert counts == r["random_counts"]
        assert all(k in r["facts"] for k in r["random"] if k.startswith("var:"))  # kept as facts_of gives them


def test_config_refuses_held_out_seeds_and_keeps_inputs_off(tmp_path):
    for s in (31, 35, 836557856):
        with pytest.raises(ValueError):
            start.config(s, tmp_path)
    cfg = start.config(1, tmp_path)
    assert cfg.boot_param == 0 and cfg.seed == 1 and cfg.release == "dos-cd"
    assert not cfg.skip_text and not cfg.skip_cutscenes
    assert cfg.raw_inputs is None and cfg.plan is None and cfg.start_frame is None and not cfg.interrupts
    assert not cfg.extra_args and not cfg.game_settings  # no debug flag, the pinned ini (enhancements=0)
    from speedrun import engine
    argv = engine.build_argv(cfg)
    assert "--boot-param=0" in argv and not any(a.startswith(("-d", "--debug")) for a in argv)
    assert "enhancements=0" in engine._ini_text(cfg)


def test_fact_kind():
    assert start.fact_kind("at:local:202:1") == "at:local"
    assert start.fact_kind("frozen:obj:5") == "frozen:obj"
    assert start.fact_kind("class:3:7") == "class" and start.fact_kind("room") == "room"


def test_random_keys_follow_data_and_one_if():
    a = unit(("global", 50), "[0000] (16) Var[300] = getRandomNr(5);\n[0005] (1A) Var[301] = 7;\n"
                             "[000A] (A0) stopObjectCode();\n")
    b = unit(("global", 51), "[0000] (9A) Local[1] = Var[300];\n[0005] (AC) Var[302] = (Local[1] + 1);\n"
                             "[000C] (48) if (Var[300] == 1) /*0016*/ {\n[0011] (1A)   Bit[40] = 1;\n"
                             "[0016] (**) }\n[0016] (1A) Var[303] = Var[301];\n[001B] (A0) stopObjectCode();\n")
    rnd, indexed = start.random_keys({a.key: a, b.key: b})
    assert rnd == {"var:300": "data", "var:302": "data", "bit:40": "control"} and indexed == 0


def test_random_keys_keep_the_result_register_in_its_script():
    a = unit(("global", 50), "[0000] (16) VAR_RESULT = getRandomNr(5);\n[0005] (48) if (VAR_RESULT == 1) /*000F*/ {\n"
                             "[000A] (1A)   Var[310] = 1;\n[000F] (**) }\n[000F] (A0) stopObjectCode();\n")
    b = unit(("global", 51), "[0000] (48) if (VAR_RESULT == 1) /*000A*/ {\n[0005] (1A)   Var[311] = 1;\n"
                             "[000A] (**) }\n[000A] (A0) stopObjectCode();\n")
    rnd, _ = start.random_keys({a.key: a, b.key: b})
    assert rnd == {"var:0": "data", "var:310": "control"}  # not var:311: b reads its own var 0


def test_live_draws_follow_the_slot_from_where_it_resumes():
    loop = unit(("global", 60), "[0000] (16) Var[300] = getRandomNr(5);\n[0005] (80) breakHere();\n"
                                "[0006] (18) goto 0000;\n")
    once = unit(("global", 61), "[0000] (16) Var[301] = getRandomNr(5);\n[0005] (80) breakHere();\n"
                                "[0006] (80) breakHere();\n[0007] (18) goto 0006;\n")
    units = {loop.key: loop, once.key: once}
    facts = {"script:60": 1, "at:script:60:6": 1, "script:61": 1, "at:script:61:7": 1}
    sites, counts = start.live_draws(facts, units)
    assert sites == {"var:300": ["test.txt:1 [0000]"]} and counts["live slots"] == 2  # 61 drew once, before
    sites, counts = start.live_draws({"script:61": 1}, units)  # no at: key: read whole, counted
    assert sites == {"var:301": ["test.txt:1 [0000]"]} and counts["read whole: no at: key"] == 1


def test_live_draws_follow_constant_starts_and_count_the_rest():
    caller = unit(("global", 60), "[0000] (0A) startScript(61,[]);\n[0003] (0A) startScript(Var[100],[]);\n"
                                  "[0006] (40) cutscene([1]);\n[0009] (80) breakHere();\n[000A] (18) goto 0000;\n")
    callee = unit(("global", 61), "[0000] (16) Var[301] = getRandomNr(5);\n[0005] (A0) stopObjectCode();\n")
    sites, counts = start.live_draws({"script:60": 1, "at:script:60:10": 1}, {caller.key: caller, callee.key: callee})
    assert sites == {"var:301": ["test.txt:1 [0000]"]}
    assert counts["scripts started with a constant target (followed)"] == 1
    assert counts["starts with a target read at run time (not followed)"] == 1
    assert counts["ops that run a hook script (not followed)"] == 1


def test_facts_of_matches_what_start_stores():
    state = {"vars": [0, 1], "bits_set": [], "owners": {}, "states": {}, "classes": {}, "room": 3, "ego": 1,
             "slots": [], "cutscene": {"depth": 0, "levels": [{"override": None}]}, "verbs": []}
    assert facts_of(state) == {"var:0": 0, "var:1": 1, "room": 3, "ego": 1, "cutscene": 0}


# --- Part I end ----------------------------------------------------------------------------------

def test_end_part1_is_key_97_to_99():
    key = json.loads(end_part1.KEY.read_text())["key"]
    goals = end_part1.part_end(key)
    assert [g["key_entry"] for g in goals] == [97, 98, 99]
    assert [(g["fact"], g["op"], g["value"]) for g in goals] == [("owner:901", "==", 1), ("owner:902", "==", 1),
                                                                ("owner:903", "==", 1)]
    if end_part1.OUT.exists():
        doc = json.loads(end_part1.OUT.read_text())
        assert doc["goal_all"] == {"owner:901": 1, "owner:902": 1, "owner:903": 1} and doc["goal_not"] == {}
        assert doc["key_entries"] == [97, 98, 99]


def test_end_part1_json_carries_no_route_data():
    if not end_part1.OUT.exists():
        pytest.skip("results/end-part1.json not written yet (run end_part1.py)")
    doc = json.loads(end_part1.OUT.read_text())
    assert set(doc) == {"source", "key_sha256", "disclosed", "goal_all", "goal_not", "key_entries"}
    text = end_part1.OUT.read_text()
    assert not any(w in text for w in ('"move"', '"move_name"', '"means"', "S3#", "Guybrush"))


def test_key_facts_map_to_facts_keys():
    assert end_part1.as_fact(["owner", 5, 1]) == ("owner:5", "==", 1)
    assert end_part1.as_fact(["class", 420, -6]) == ("class:420:6", "==", 0)
    assert end_part1.as_fact(["class", 420, 6]) == ("class:420:6", "==", 1)
    assert end_part1.as_fact(["owner_not", 568, 1]) == ("owner:568", "!=", 1)
    assert end_part1.as_fact(["script", 7, 1]) == ("script:7", "==", 1)
    synthetic = [{"part": p, "fact": ["bit", n, 1], "move": "", "move_name": "", "means": ""}
                 for n, p in enumerate("I I I I II I II".split())]
    assert [g["key_entry"] for g in end_part1.part_end(synthetic)] == [3, 4, 6]


# --- game end ------------------------------------------------------------------------------------

ENDLESS = """[0000] (40) cutscene([1]);
[0003] (14) print(254,[Text("the end")]);
[0008] (2E) delay(60);
[000C] (18) goto 0003;
[000F] (C0) endCutscene();
[0010] (A0) stopObjectCode();
"""
WAIT, RUNTIME = end.WAIT, end.CLOSED_RUNTIME


def test_endless_loop_in_an_open_cutscene_matches():
    assert kinds(ENDLESS) == ["match"]
    u = unit(("global", 50), ENDLESS)
    matches, counts = end.game_ends({u.key: u})
    assert len(matches) == 1 and matches[0]["goal_any"] == ["at:script:50:12"]  # after the delay: the goto
    assert matches[0]["after_loop_never_runs"] == ["test.txt:5 [000F]", "test.txt:6 [0010]"]
    assert counts["kinds"]["match"] == 1 and counts["foreign_pops"] == []
    assert matches[0]["goal_detectable"] == {"script:50": 1} and matches[0]["delay_before_loop"] == 0
    assert matches[0]["nested_runs_before_loop"][0].startswith("test.txt:1 [0000] cutscene")


def test_an_exit_before_the_loop_makes_the_running_fact_no_goal():
    text = ENDLESS.replace("[0000] (40) cutscene([1]);", "[0000] (40) cutscene([1]);\n[0002] (28) unless (Var[100]) goto 0010;")
    u = unit(("global", 50), text)
    matches, _ = end.game_ends({u.key: u})
    assert [m["goal_detectable"] for m in matches] == [None] and matches[0]["goal_any"] == ["at:script:50:12"]
    assert matches[0]["must_reach_from"] == ["test.txt:3 [0003]"]


def test_a_local_match_names_its_room():
    u = unit(("local", 7, 201), ENDLESS, room=7)
    matches, _ = end.game_ends({u.key: u})
    assert matches[0]["goal_detectable"] == {"local:201": 1, "var:22": 7}


def test_loop_with_an_exit_is_a_wait_loop():
    text = ENDLESS.replace("[000C] (18) goto 0003;", "[000C] (28) unless (Var[100]) goto 0003;")
    assert kinds(text) == [WAIT]


def test_endless_loop_with_no_cutscene_is_ambient():
    assert kinds(ENDLESS.replace("[0000] (40) cutscene([1]);", "[0000] (1A) Var[100] = 1;")) == \
        ["closed, no cutscene open (ambient)"]


def test_cutscene_open_on_one_path_only():
    text = """[0000] (28) if (Var[100]) /*0008*/ {
[0005] (40)   cutscene([1]);
[0008] (**) }
[0008] (2E) delay(60);
[000C] (18) goto 0008;
"""
    assert kinds(text) == ["closed, cutscene open on some paths only"]


def test_endless_loop_with_no_yield_is_a_hang():
    text = ENDLESS.replace("[0008] (2E) delay(60);", "[0008] (1A) Var[100] = 2;")
    assert kinds(text) == ["closed, no yield (a hang)"]


def test_override_that_leaves_the_loop():
    text = """[0000] (40) cutscene([1]);
[0003] (58) beginOverride();
[0005] (18) goto 0011;
[0008] (14) print(254,[Text("the end")]);
[000D] (80) breakHere();
[000E] (18) goto 0008;
[0011] (C0) endCutscene();
[0012] (A0) stopObjectCode();
"""
    assert kinds(text) == ["closed, cutscene open, Esc leaves it"]
    assert kinds(text.replace("[0003] (58) beginOverride();\n[0005] (18) goto 0011;\n",
                              "[0003] (58) beginOverride();\n[0005] (18) goto 0011;\n[0008] (58) endOverride();\n")
                 .replace("[0008] (14)", "[0009] (14)").replace("goto 0008", "goto 0009")) == ["match"]


def test_a_loop_that_turns_input_on_is_no_end():
    for op in ("UserputOn", "UserputSoftOn"):
        text = ENDLESS.replace("[0003] (14) print(254,[Text(\"the end\")]);", f"[0003] (2C) {op}();")
        assert kinds(text) == ["closed, cutscene open, the loop turns input on"]
    assert kinds(ENDLESS.replace("[0003] (14) print(254,[Text(\"the end\")]);", "[0003] (2C) UserputOff();")) == ["match"]


def test_a_restart_in_the_loop_is_a_way_out():
    assert kinds(ENDLESS.replace("[0003] (14) print(254,[Text(\"the end\")]);", "[0003] (98) systemOps(1);")) == [WAIT]
    assert kinds(ENDLESS.replace("[0003] (14) print(254,[Text(\"the end\")]);", "[0003] (98) systemOps(3);")) == ["match"]


def test_a_stop_reaches_only_its_own_slot_kind():
    other = ENDLESS.replace("[0003] (14) print(254,[Text(\"the end\")]);", "[0003] (0A) startScript(77,[]);")
    assert kinds(other) == ["match"]
    assert kinds(other.replace("startScript(77,[])", "startScript(50,[])")) == [WAIT]
    assert kinds(other.replace("startScript(77,[])", "startScript(Var[100],[])")) == [RUNTIME]  # may be itself
    obj = other.replace("startScript(77,[])", "startObject(9,250,[])")
    assert kinds(obj, ("obj", 8)) == ["match"] and kinds(obj, ("obj", 9)) == [WAIT]
    # an object op never stops a global or local slot, and stopScript or doSentence(STOP) never an object's
    assert kinds(obj.replace("startObject(9,250,[])", "startObject(Var[100],250,[])")) == ["match"]
    assert kinds(obj.replace("startObject(9,250,[])", "startObject(Var[100],250,[])"), ("obj", 9)) == [RUNTIME]
    stop = other.replace("[0003] (0A) startScript(77,[]);", "[0003] (62) stopScript(Var[100]);")
    assert kinds(stop) == [RUNTIME] and kinds(stop, ("obj", 9)) == ["match"]
    sentence = other.replace("[0003] (0A) startScript(77,[]);", "[0003] (19) doSentence(STOP);")
    assert kinds(sentence) == [RUNTIME] and kinds(sentence, ("obj", 9)) == ["match"]
    # entry and exit code runs under kScriptNumENCD/EXCD: no stop by number reaches it, its room's included
    assert kinds(other.replace("startScript(77,[])", "startScript(Var[100],[])"), ("entry", 5)) == ["match"]
    assert kinds(other.replace("[0003] (0A) startScript(77,[]);", "[0003] (62) stopScript(5);"), ("entry", 5)) == ["match"]
    assert kinds(stop, ("exit", 5)) == ["match"] and kinds(obj, ("entry", 9)) == ["match"]


def test_room_change_ends_a_local_loop_not_a_global_one_and_an_object_one_at_run_time():
    text = ENDLESS.replace("[0003] (14) print(254,[Text(\"the end\")]);", "[0003] (72) loadRoom(5);")
    assert kinds(text, ("global", 50)) == ["match"]
    assert kinds(text, ("local", 5, 200)) == [WAIT]
    assert kinds(text, ("obj", 9)) == [RUNTIME]  # an inventory slot survives the room change


def test_an_endcutscene_at_own_depth_zero_pops_another_level_and_goes_on():
    text = """[0000] (C0) endCutscene();
[0001] (40) cutscene([1]);
[0004] (80) breakHere();
[0005] (18) goto 0004;
"""
    u = unit(("global", 50), text)
    matches, counts = end.game_ends({u.key: u})
    assert counts["foreign_pops"] == ["test.txt:1 [0000]"] and len(matches) == 1  # the path is not cut there


def test_mi1_has_one_game_end_global_130():
    units = ir.load()
    matches, counts = end.game_ends(units)
    assert [tuple(m["unit"]) for m in matches] == [("global", 130)]
    m = matches[0]
    assert m["goal_any"] == ["at:script:130:2597", "at:script:130:2612", "at:script:130:2616", "at:script:130:2631"]
    assert m["goal_detectable"] == {"script:130": 1} and m["must_reach_from"] == ["global/script-130.txt:8 [0000]"]
    assert m["stop_sites"] == [] and len(m["start_sites"]) == 3 and m["input_ops_before_loop"] == []
    k = counts["kinds"]
    assert sum(k.values()) == 698 and k["match"] == 1 and k[WAIT] == 158 and k["closed, no yield (a hang)"] == 0
    assert k["closed, no cutscene open (ambient)"] == 94 and k[RUNTIME] == 5 and k["other loop"] == 440
    assert counts["foreign_pops"] == []  # the checked assumption: no script pops a level it did not push
    assert any(w.startswith("global/script-023.txt [0000]") for w in counts["near"]["closed, no cutscene open (ambient)"])


def test_system_modules_import_no_e001_search_or_scriptindex():
    code = ("import sys; sys.path.insert(0, %r); import start, end; "
            "bad = [m for m in sys.modules if m in ('speedrun.search', 'speedrun.scriptindex') or 'E001' in m "
            "or m in ('answer_key', 'groundtruth', 'end_part1')]; print(bad)") % str(HERE)
    out = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True, check=True).stdout
    assert out.strip() == "[]"


# C2: every integer literal in the system's step-2 code other than 0 and 1 is a subscript, or sits in a
# module-level UPPERCASE constant whose line cites ScummVM (file.cpp:N, file.h:N) or plan.md, or is one of
# these named budgets. And none of the MI1 numbers this step found appears at all (fixtures).
CITE = re.compile(r"#.*(\w+\.(cpp|h):\d+|plan\.md)")
BUDGETS = {"MAX_FRAME": "a run budget; a stop on it is reported", "MAX_TICKS": "the bridge's tick budget, reported",
           "LOG_SHOWN": "how many places the log lists; the JSON has them all"}
MI1_NUMBERS = {23, 33, 38, 100, 130, 159, 202, 901, 902, 903, 2597, 2612, 2616, 2631}


@pytest.mark.parametrize("name", ["start.py", "end.py"])
def test_system_code_names_no_game_number(name):
    src = (HERE / name).read_text()
    tree = ast.parse(src)
    lines = src.splitlines()
    parent = {c: n for n in ast.walk(tree) for c in ast.iter_child_nodes(n)}
    top = {}
    for st in tree.body:
        for n in ast.walk(st):
            top[n] = st
    bad = []
    for n in ast.walk(tree):
        if not (isinstance(n, ast.Constant) and type(n.value) is int):
            continue
        assert n.value not in MI1_NUMBERS, (name, n.lineno, n.value)
        if n.value in (0, 1):
            continue
        p = parent[n]
        while isinstance(p, (ast.Slice, ast.UnaryOp)):
            p = parent[p]
        if isinstance(p, ast.Subscript) and n is not p.value:
            continue
        st = top[n]
        names = [t.id for t in getattr(st, "targets", []) if isinstance(t, ast.Name)]
        if names and all(x.isupper() for x in names):
            text = "\n".join(lines[st.lineno - 1:st.end_lineno])
            if CITE.search(text) or any(x in BUDGETS for x in names):
                continue
        bad.append((n.lineno, n.value))
    assert not bad, f"{name}: integer literals with no cite: {bad}"


@pytest.mark.parametrize("name", ["start.py", "end.py"])
def test_system_strings_name_no_game_number(name):
    tree = ast.parse((HERE / name).read_text())
    docs = {id(n.body[0].value) for n in ast.walk(tree) if isinstance(n, (ast.Module, ast.FunctionDef, ast.ClassDef))
            and n.body and isinstance(n.body[0], ast.Expr) and isinstance(n.body[0].value, ast.Constant)}
    found = [(n.lineno, n.value) for n in ast.walk(tree) if isinstance(n, ast.Constant) and isinstance(n.value, str)
             and id(n) not in docs and {int(t) for t in re.findall(r"(?<![\w.:-])\d+(?![\w.:-])", n.value)} & MI1_NUMBERS]
    assert not found, found


def test_end_reads_no_route_or_e001():
    text = (HERE / "end.py").read_text() + (HERE / "start.py").read_text()
    assert "E001" not in text and "routes/" not in text and "steps.toml" not in text
