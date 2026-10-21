# Research notes

This folder is the project's lab notebook. Everything the paper claims should be traceable to an entry here
or to an experiment in `experiments/`.

| File | What goes in it |
|---|---|
| `log/YYYY-MM-DD-topic.md` | Dated notebook entries, one per working session or experiment. Append, never rewrite. |
| `decisions.md` | Design and research decisions, with the reason in the decider's own words. |
| `baselines.md` | Reference runs and how each was made. |
| `open-questions.md` | Research questions we have not answered yet. |

## Notebook entry template

```
# YYYY-MM-DD: short title

Question: what we wanted to find out.
Prediction: what we expected, written before running anything.
Method: what we ran (commit hash, exact command, settings, seed).
Result: the numbers, with paths to the raw outputs.
Surprises: anything we did not expect.
Decision: what we do next because of this.
```

Failed attempts and negative results get entries too. They are part of the paper.
