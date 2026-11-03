"""Fixtures for the harness's start rules (the explore component's part of the planning loop): replay.task_start,
replay.replan_start with replay.prediction, replay.view, Harness._actor_rooms_start, export_records.write_start and
Harness._same_input's key, on recorded readings and recorded entries, their outputs from the frozen reference, for
``fmi check harness`` to compare against.

usage (from the repo root): .venv/bin/python experiments/E011-speed/explore/harness_fixtures.py [--out DIR]

* readings: every recorded reading whose packed file is on disk (packed_reading.load_reading); its task facts' domains
  are written next to the fixtures (domains/*.json: "domains": key -> [values, low, high], "below"/"above" as text, as
  the reading's ``domains`` gives them, and reading.json's "live_locals");
* the engine's facts: the entries of recorded jobs (job.json's checkpoint), each against two readings;
* first plan: task_start, the actors' rooms not known, the start file's text, the input key, the view;
* replan: the plan's prediction made from another entry's start on the same reading (the PREDICTED facts labelled by
  their domains, ``Harness._planned``'s rule), its args: numbers moved by a seeded draw over their domains; then
  replan_start on prediction(view, pred), the actors' rooms from pred, the start file's text, the input key, counts.

Out: DIR/harness-check.jsonl. A test tool: it imports the frozen reference and edits none of its files.
"""

import argparse
import hashlib
import json
import random
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
E003 = ROOT / "experiments" / "E003-path-through-cloud"
sys.path.insert(0, str(E003))
sys.path.insert(0, str(ROOT / "src"))
sys.path.insert(0, str(HERE))

import export_records  # noqa: E402
import packed_reading as PR  # noqa: E402
import replay  # noqa: E402
from step_fixtures import jobs, reached_files  # noqa: E402

HOME = Path.home()


def lab(v):
    return v if v is None or isinstance(v, (int, str)) else str(v)


def dom_json(d) -> list:
    return [[lab(v) for v in d.values], d.low, d.high]


def input_key(frame: int, n_bans: int, start: dict) -> str:
    """Harness._same_input's key."""
    return hashlib.sha1(json.dumps([frame, n_bans, sorted((k, str(v)) for k, v in start.items())]).encode()) \
        .hexdigest()[:16]


def actor_rooms(base_facts, start: dict, pred: dict, first: bool) -> dict:
    """Harness._actor_rooms_start."""
    out = dict(start)
    for k in base_facts:
        if k.startswith(replay.ACTOR_ROOM):
            v = None if first else pred.get(k)
            out[k] = v if isinstance(v, int) and not isinstance(v, bool) else replay.ACTOR_ROOM_NOT_KNOWN
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(HOME / ".cache/fast-mi-e011/explore/fixtures/harness"))
    a = ap.parse_args()
    out = Path(a.out)
    (out / "domains").mkdir(parents=True, exist_ok=True)
    unob = replay.UNOBSERVED + (replay.ACTOR_ROOM,)
    predp = replay.PREDICTED + (replay.ACTOR_ROOM,)
    # readings other builders' tests make and remove (readio's cache tests) are left out: the fixtures name files
    readings = sorted({p.with_name("reading.json") for p in reached_files() if p.with_name("reading.tc").exists()
                       and "cache-test" not in str(p)})
    js = jobs()
    cases = []
    rng = random.Random(11)
    for i, rp in enumerate(readings):
        try:
            rd = PR.load_reading(rp)
        except Exception as e:  # noqa: BLE001
            print(f"{rp}: not loaded ({type(e).__name__}: {e})", file=sys.stderr)
            continue
        doms = rd.task.facts
        df = out / "domains" / (hashlib.sha1(str(rp).encode()).hexdigest()[:16] + ".json")
        live = rd.doc.get("live_locals")
        df.write_text(json.dumps({"domains": {k: dom_json(d) for k, d in sorted(doms.items())}, "live_locals": live}) + "\n")
        picks = js[i % 5::max(1, len(js) // 8)][:8]
        for n, jd in enumerate(picks):
            job = json.loads((jd / "job.json").read_text())
            obs = job["entry"]["checkpoint"][1]
            frame = job["entry"]["frame"]
            b = PR.Points(doms, live)
            st = replay.task_start(b, obs, rd.task)
            st = actor_rooms(doms, st, {}, True)
            now = replay.view(b, doms, obs, unob)
            cases.append({"kind": "first", "reading": str(rp), "domains": str(df), "job_dir": str(jd), "frame": frame,
                          "start_text": "".join(f"{k}\t{export_records._label_text(doms[k], st.get(k, 0))}\n"
                                                for k in sorted(doms)),
                          "input_key": input_key(frame, 0, st),
                          "view": {k: lab(v) for k, v in sorted(now.items())},
                          "counts": dict(b.counts)})
            # a replan: the prediction from another entry's start on this reading
            other = json.loads((picks[(n + 1) % len(picks)] / "job.json").read_text())["entry"]["checkpoint"][1]
            b2 = PR.Points(doms, live)
            st2 = actor_rooms(doms, replay.task_start(b2, other, rd.task), {}, True)
            pred = {k: doms[k].label(st2.get(k, 0)) for k in doms if k.startswith(predp)}
            for k in list(pred):
                if k.startswith("args:") and rng.random() < 0.5:
                    pred[k] = rng.choice([v for v in doms[k].values if isinstance(v, int)] or [0])
                if k.startswith("pos:") and rng.random() < 0.2:  # the slot where the plan put it, now and then
                    pred[k] = st.get(k)
            b3 = PR.Points(doms, live)
            now3 = replay.view(b3, doms, obs, unob)
            start, counts = replay.replan_start(b3, obs, replay.prediction(now3, pred), rd.task)
            start = actor_rooms(doms, start, pred, False)
            cases.append({"kind": "replan", "reading": str(rp), "domains": str(df), "job_dir": str(jd), "frame": frame,
                          "pred": {k: lab(v) for k, v in sorted(pred.items())},
                          "start_text": "".join(f"{k}\t{export_records._label_text(doms[k], start.get(k, 0))}\n"
                                                for k in sorted(doms)),
                          "input_key": input_key(frame, 0, start), "counts": dict(counts)})
        rd.close()
    (out / "harness-check.jsonl").write_text("".join(json.dumps(c) + "\n" for c in cases))
    print(f"{len(cases)} cases from {len(readings)} readings and {len(js)} jobs -> {out / 'harness-check.jsonl'}")


if __name__ == "__main__":
    main()
