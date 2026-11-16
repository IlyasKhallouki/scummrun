"""Time any raw-input movie with the October controller (evaluation only): one rule for both routes.
args: GAME_DIR MOVIE OUT TALKSPEED SEED START_FRAME|none GOAL_JSON"""
import json, sys
from pathlib import Path
from speedrun import releases
from speedrun.control import Controller
from speedrun.engine import EngineConfig
game, movie, out, talk, seed, sf, goal = sys.argv[1:8]
out = Path(out); out.mkdir(parents=True, exist_ok=True)
rel = releases.get("dos-cd")
cfg = EngineConfig(out_dir=out, game_path=Path(game), release="dos-cd", timer_next="any",
                   start_frame=None if sf == "none" else int(sf), max_ticks=10**7, seed=int(seed),
                   raw_inputs=Path(movie), skip_text=False, skip_cutscenes=False, game_settings={"talkspeed": talk})
last = max(json.loads(l)["frame"] for l in Path(movie).read_text().splitlines())
with Controller(cfg, timeout_s=3600) as c:
    b = c.start()
    st = b.run(until=json.loads(goal), max_frame=last + 8000)
    print(json.dumps({k: st.get(k) for k in ("stop", "frame", "tick", "room", "qf", "qf0")}))
    b.quit()
