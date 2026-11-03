"""E011 round 2 (CUTS.md, section 3): the pixel-free comparisons of chain.py's exact dumps (G2, G3).

  compare_np.py drop A/dumps.jsonl B/dumps.jsonl
      chain.py compare's comparison of every dump's state, with the exact object's screen digest left out on both
      sides ("screens", the full digest, or "screens_np", the pixel-free one): everything but the pixels
  compare_np.py np A/dumps.jsonl B/dumps.jsonl
      the exact object's "screens_np" alone (the screens' geometry and the palette), which both sides must carry

Prints the frames that differ and, last, "N vs M dumps compared, K differ" as chain.py compare does; exit 1 on a
difference."""
import json
import sys

DIGESTS = ("screens", "screens_np")


def flat(d, pre=""):
    if isinstance(d, dict):
        for k, v in d.items():
            yield from flat(v, f"{pre}{k}.")
    elif isinstance(d, list) and len(d) < 64:
        for i, v in enumerate(d):
            yield from flat(v, f"{pre}{i}.")
    else:
        yield pre.rstrip("."), d


def view(state, mode):
    x = state.get("exact")
    if mode == "np":
        if not isinstance(x, dict) or "screens_np" not in x:
            raise SystemExit("a dump without exact.screens_np")
        return {"screens_np": x["screens_np"]}
    s = dict(state)
    if isinstance(x, dict):
        s["exact"] = {k: v for k, v in x.items() if k not in DIGESTS}
    return s


def main():
    mode, a, b = sys.argv[1:4]
    if mode not in ("drop", "np"):
        raise SystemExit(__doc__)
    A = [json.loads(line) for line in open(a)]
    B = [json.loads(line) for line in open(b)]
    bad = 0
    for x, y in zip(A, B):
        if x["frame"] != y["frame"]:
            print("frame sets differ", x["frame"], y["frame"])
            sys.exit(1)
        vx, vy = view(x["state"], mode), view(y["state"], mode)
        if vx != vy:
            fx, fy = dict(flat(vx)), dict(flat(vy))
            keys = sorted(k for k in set(fx) | set(fy) if fx.get(k) != fy.get(k))
            print(f"frame {x['frame']}: {len(keys)} fields differ: "
                  + "; ".join(f"{k}: {fx.get(k)!r} vs {fy.get(k)!r}" for k in keys[:8]))
            bad += 1
    print(f"{len(A)} vs {len(B)} dumps compared, {bad} differ")
    sys.exit(1 if bad or len(A) != len(B) else 0)


if __name__ == "__main__":
    main()
