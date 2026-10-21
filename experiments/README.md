# Experiments

One folder per experiment: `experiments/EXXX-short-name/`, numbered in the order they are planned.

1. Copy `_template/` and fill in `plan.md` **before** running anything: the question, the prediction and
   the threshold that decides the outcome.
2. Run it. Keep the exact commands, the commit hash and the raw outputs (or paths to them) in the folder.
3. Write `report.md`: what happened against the plan, including if it failed.
4. Add a notebook entry in `notes/log/` and, if the result goes in the paper, a row in `paper/claims.md`.

Do not change `plan.md` after the run starts. If the plan was wrong, say so in the report.
