# Baselines

Reference times for The Secret of Monkey Island, Any%. All bot times are RTA from the logo-skip Esc to
the first frame of the final fade, verified by replaying the movie from boot twice (automatic skips and
explicit keys) and getting the same frame.

| Run | Time | Release and setup | How it was made | Evidence |
|---|---:|---|---|---|
| Human record (saruya) | 21:52.4 | DOS CD, ScummVM; "Last Part" card about 6 s | Human play | `docs/research/anypercent-runner-sources.md` |
| Bot, no item slide (B) | 28:01.4 | DOS CD, seed 836557856, talkspeed 255, full audio | Hand goal lists for Parts III-IV, search for inputs; hand route moves for Parts I-II | `runs/anypercent/noslide-p7`, `runs/anypercent/verify-b1` |
| Bot, item slide (m2) | 23:11.3 | DOS CD, seed 836557856, talkspeed 85, full audio | Hand route modelled on saruya's run, driver with lookahead, improvement lanes | `runs/anypercent/m2`, `runs/anypercent/m2-verify` |
| Bot, item slide (m2n) | 21:01.483 | Same as m2 with CD track 6 absent | m2's route; card ends in 0.2 s; one card-only delay removed | `runs/anypercent/m2n`, `runs/anypercent/m2n-verify` |
| Glitchless Part I | 6:11.5 (22,292 mean ticks) | Mac release, 30 held-out seeds | Hand PDDL model, Fast Downward on engine-measured costs | `docs/optimization.md` |
