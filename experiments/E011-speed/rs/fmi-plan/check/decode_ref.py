"""The reference's decode: plan_fd.read_ops and plan_fd.decode_ops on an operator table and a plan file, as JSON lines
in decode.rs `steps_json`'s form (values as texts, each condition's values sorted by text).

usage: decode_ref.py OPS_JSONL SAS_PLAN
"""
import json
import sys
from pathlib import Path

E003 = Path(__file__).resolve().parents[4] / "E003-path-through-cloud"
sys.path.insert(0, str(E003))
sys.path.insert(0, str(E003.parents[1] / "src"))
import plan_fd  # noqa: E402

head, ops, acts = plan_fd.read_ops(sys.argv[1])
steps = plan_fd.decode_ops(Path(sys.argv[2]).read_text(), ops, acts)
for s in steps:
    print(json.dumps({"op": s["op"], "action": s["action"], "name": s["name"], "members": s["members"],
                      "chosen": [[k, str(v)] for k, v in s["chosen"]],
                      "guessed": [[k, str(v), [[c, sorted(str(x) for x in vs)] for c, vs in w]]
                                  for k, v, w in s["guessed"]]}))
