"""Replay a hand-route movie with a chosen engine binary and game dir (evaluation only)."""
import json, sys
from pathlib import Path
sys.path.insert(0, '/home/ilyask/projects/scummrun/scripts')
from speedrun import releases
game = Path(sys.argv[1]); movie = Path(sys.argv[2]); out = Path(sys.argv[3]); talk = sys.argv[4]
skips = (sys.argv[5] == "auto") if len(sys.argv) > 5 else True
goal = json.loads(sys.argv[6]) if len(sys.argv) > 6 else [{"room": 0}, {"var": 293, "eq": 161}]
releases.game_dir = lambda rel, root=None: game
import anypercent_verify as v
out.mkdir(parents=True, exist_ok=True)
print(json.dumps(v.replay(movie, out, goal, talk, skips)))
