"""Evaluation only: replay a reference movie from boot with the per-frame log on, sampling the state every STEP frames.
args: GAME_DIR MOVIE OUT TALKSPEED STEP"""
import json, sys, time
from pathlib import Path
from speedrun import releases
from speedrun.control import Controller
from speedrun.engine import EngineConfig
game, movie, out, talk, step = sys.argv[1:6]
step = int(step)
out = Path(out); out.mkdir(parents=True, exist_ok=True)
goal = [{"room": 0}, {"var": 293, "eq": 161}]
BITS = [72, 65, 98, 19, 304, 51, 89, 88, 76, 565]
cfg = EngineConfig(out_dir=out, game_path=Path(game), release="dos-cd", timer_next="any", start_frame=46,
                   max_ticks=10**7, seed=836557856, raw_inputs=Path(movie), skip_text=True, skip_cutscenes=True,
                   game_settings={"talkspeed": talk}, frame_log=True)
last = max(json.loads(l)["frame"] for l in Path(movie).read_text().splitlines())
t0 = time.time()
with Controller(cfg, timeout_s=3600) as c, open(out / "samples.jsonl", "w") as w:
    b = c.start()
    st = b.status
    n = 0
    while True:
        f = st["frame"]
        s = b.state()
        bits = set(s["bits_set"])
        w.write(json.dumps({"frame": st["frame"], "tick": st.get("tick"), "qf": st.get("qf"), "room": st.get("room"),
                            "inv": s["inventory"], "bits": [x for x in BITS if x in bits], "nbits": len(bits),
                            "v196": s["vars"][196], "v293": s["vars"][293], "v19": s["vars"][19]}) + "\n")
        n += 1
        if b.holds(goal) or f > last + 800 or "end" in st:
            break
        st = b.run(frame=f + step, until=goal)
    print(json.dumps({"stop": st.get("stop"), "frame": st.get("frame"), "tick": st.get("tick"), "qf": st.get("qf"),
                      "qf0": st.get("qf0"), "samples": n, "seconds": round(time.time() - t0, 1)}))
    b.quit()
