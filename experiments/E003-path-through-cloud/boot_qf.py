"""E003: the Part I runner's boot (replay_part1.config: plan.md's settings, no input) on the bridge's run clock: frame by
frame to the first idle stop, the first frame in each room with its qf and var 19, and the frames per var 19 value.

usage (from this folder, with ../../.venv/bin/python, under nice):  boot_qf.py --seed N [--out FILE.json]

An instrument beside runb_qf.py, which does the same for run B's inputs. It names no game number.
"""

import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import replay_part1 as P  # noqa: E402
from speedrun.control import Controller  # noqa: E402


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, required=True)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--work", type=Path, default=P.OUT / "boot-qf")
    a = ap.parse_args()
    a.work.mkdir(parents=True, exist_ok=True)
    rooms, v19 = {}, {}
    with Controller(P.config(a.seed, a.work), timeout_s=600) as c:
        b = c.root
        st = b.status
        while True:
            rooms.setdefault(str(st.get("room")), {"frame": st["frame"], "qf": st.get("qf"), "v19": st.get("v19")})
            v19[str(st.get("v19"))] = v19.get(str(st.get("v19")), 0) + 1
            if "idle" in st and st["frame"] > 1:
                break
            st = b.run(frame=st["frame"] + 1)
            if "end" in st:
                break
    out = {"seed": a.seed, "first_frame_in_room": rooms,
           "first_idle": {"frame": st.get("frame"), "qf": st.get("qf"), "v19": st.get("v19"), "room": st.get("room")},
           "frames_per_v19": v19}
    text = json.dumps(out, indent=1)
    print(text)
    if a.out:
        a.out.write_text(text + "\n")


if __name__ == "__main__":
    main()
