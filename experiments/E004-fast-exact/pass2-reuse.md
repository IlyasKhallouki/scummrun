# Pass 2 reruns only what a widened value touches (`--pass2-reuse`)

Reduction 2 of E004. Proof before the code, for any SCUMM v5 game. Off by default.

## The claim

A pass reads with fixed domains. A value written outside its domain widens the domain, and the reading starts over.
With the switch, a run of pass 2 that depends on nothing that changed gives the output pass 1 stored, without running.
The files and counters are byte for byte those of the reading without the switch.

## What one run depends on

The machine is deterministic. A run's output (its records, the Builder's counters' increments, the held branches, the
last pins) is a function of these inputs and no others:

1. **The job:** its kind, label, pins, precondition, what it runs and its checks. They are compared whole.
2. **The rerun's guides** (`--reach` only). They are compared whole.
3. **The domain and kind of each fact the run consults.** The machine half gets them only through `info(k)`; the
   Builder through the keys of a path's precondition and writes. These keys are logged. A widened key among them makes
   the run invalid. A domain the run does not consult cannot change its output. Domains of `pos:`, `escpos:`, `saved:`
   never widen. `args:` domains are read in `pin`, which the job's comparison covers. They are read in `record` only for
   a job's own `args:` set, where the outcome is fixed by the job (a job that has the set has it unequal to the domain).
4. **Whether a list number is folded** (`Builder::record` asks `folded`, the list past SIG_LIMIT). Each number a run asks
   about is logged with the answer. A different answer makes the run invalid.
5. **The reached values the run asks about** (`--reach` only). Each question (`meets`, `was_met`) is logged with the
   answer. A different answer makes the run invalid.

The game's code, the scope, the start and the switches do not change between passes.

## The induction

The jobs of pass 2 run in the same order. Assume every run so far gave the same output as in pass 1, or was run for
real. A later run's inputs are checked before it is reused, so whatever came before it is covered by items 1 to 5: if
an earlier difference matters to this run, it shows in those inputs and the run is made for real. If all five agree, the
function gives the same output. So reused and real outputs are the same as the outputs of a reading without the switch,
and the output of each run is handed on in the same place. The files are the same byte for byte.

## Memory

Pass 1's outputs are kept until pass 2 takes them, each at most once. Peak memory grows by pass 1's records. That is
the price of the switch. It is measured.

## The check

On every scope, with `--reach` on and off: packed file, records' lines and counters have the same sha1 with the switch
on and off.
