# taskcore

E003 step 4, the memory work, stage 1. The planner's task as flat integer arrays, the fork fold, exact
reductions, and Fast Downward's SAS file, written streaming. No dependencies beyond std.

```
cargo build --release
taskcore sas    --task kitchen.tc --start s17 --out task.sas [--stats stats.json]
                [--fold strict|every|off] [--reduce all|none|a,b,...] [--product-limit 64]
                [--encoding model|task] [--check-fold N] [--kept kept.json]
                [--start-file start.txt] [--bans bans.txt] [--ops ops.jsonl]   (the replay harness's, below)
taskcore decode (same options) --plan sas_plan --out steps.json
taskcore validate (same options) --names plan.txt --out result.json   (a plan as input action names, run on the reduced task)
taskcore info   --task kitchen.tc
```

Defaults: `--fold strict`, `--reduce all`, `--product-limit 64` (task.py's `PRODUCT_LIMIT`), `--encoding model`.
With `--fold off --reduce none --encoding task` the file is task.py `write_sas`'s, byte for byte. `decode` reads a
plan back to the actions of the input file (a folded action lists its paths). `--check-fold N` tests each fold on N
sampled states per path; `--kept` lists the input actions each reduced action stands for, and the ones (f) replaced
(every member of a replaced action, so every path of a replaced folded action). The packed file comes from
`../export_records.py`.

The pipeline: load; the start check; fold; reductions (a)-(f) in a loop until none changes the task; fold again
(after (d) cut unreachable values a group may cover its box) and reduce again if it folded anything; then the
writer's two passes. Memory is the flat arrays (about 1.5 times the packed file while a reduction rebuilds them)
plus one action's variants; the SAS file is never held.

Sources: `model.rs` (the arrays), `packed.rs` (reading the file), `sas.rs` (the writer: task.py's `_expand` and
`write_sas`, rule for rule; the two encodings; the start check; the operator table), `fold.rs` (the fork fold and
its check), `reduce.rs` (reductions (a)-(f); each has its soundness argument in its doc comment), `bans.rs` (the
start file and the bans).

## The replay harness's options

replay.py plans through taskcore (plan_fd.py `plan_taskcore`): the reading is exported once to the packed file, and
each replan runs `sas` from it with three options. Without them every output is as before, byte for byte (the old
and the new binary on the kitchen and test_taskcore.py's property test).

- `--start-file F`: the start as a file, one line per fact, `key<TAB>value` (the value's text), `key<TAB>` for a
  value the domain lacks (export_records.py `write_start`). It replaces `--start`; the start check reads it as it
  reads a start of the file.
- `--bans F`: replay.py's bans, one per line, `label<TAB>key<TAB>value...` (export_records.py `write_bans`), cut
  out of the input task before the start check, the fold and the reductions: replay.py's `cut`, rule for rule
  (`bans.rs`). Each piece keeps its action's name and members, so plans read back to the input actions. With
  `--fold off --reduce none --encoding task` the file is task.py's `write_sas` of replay.py's `apply_bans`, byte for
  byte (test_replay_taskcore.py: hand-made cases, random tasks, the kitchen). The stats count the bans and pieces.
- `--ops F`: the operator table, JSON lines (`sas::write_ops`): per operator its action, precondition values and
  guessed values with their conditions (task.py's `Op`), per action its name, members and picked guesses; the pick
  and goal operators. A plan reads back from it without a second run (`decode` loads, folds and reduces again).

## What the reductions keep, and the two encodings

The task's cost is the sum of a plan's actions' costs, and a plan must end in a state where every goal set holds.
Each lever keeps this task's optimal cost (`reduce.rs`, `fold.rs`):

| lever | what survives |
|---|---|
| fold, strict and every end | every plan, at the same cost (exact) |
| (a) effects that cannot change the state | every plan |
| (b) facts nothing reads | every plan |
| (c) actions with no effect | every optimal plan |
| (d) forward reachability from the start | every plan; the reduced task is for that start only |
| (e) backward relevance by (fact, value) | every optimal plan, whole |
| (f) duplicates and dominated actions | the optimal cost, and at least one optimal plan |
| PRODUCT_LIMIT | a setting of the writer; the same states |

The file's own optimal cost (what Fast Downward minimizes) is this one only in the **model encoding**, the
default. task.py's encoding (`--encoding task`) differs in two ways, and the levers can change both:

- **metric 0.** task.py writes metric 0 when every action costs 1 (the kitchen). Fast Downward then gives every
  operator cost 1 (root_task.cc:469), pick and goal operators too, so its cost counts the picks. The fold, (b) and
  (f) move actions across PRODUCT_LIMIT and so change how many picks a plan needs; a reduction that drops the last
  action of cost other than 1 turns metric 1 into 0. The model encoding writes metric 1 whenever the file has a pick
  or goal operator (they cost 0 there).
- **the goal-set latch.** A goal of several values is an extra variable `goal` that one operator per choice sets
  to 1, and nothing sets back: a plan can meet the set and leave it. (d) cuts a goal set to the values it can reach,
  which can make it one value and remove the latch. In the model encoding every effect that writes a goal-set fact a
  value outside its set also sets `goal` to 0, under the same conditions.

Operators, their order and their numbers are the same in both encodings, so plans read back the same way.
lama-first searches with unit costs (`cost_type=one`), so the metric does not change its search. The kitchen's goal
is one value, so on the kitchen the two encodings differ only in the metric line.

**The start check.** A start with a value outside a fact's domain: task.py raises OutOfDomain when its file names
that fact (build the task again with that value). taskcore checks the input task, before the fold and the
reductions, and stops with the same error exactly then (`sas::start_misses`). Without it (d) read the start as no
value: it shrank the fact's domain to the written values and dropped a precondition that does not hold at the
start, or dropped the actions that need the fact; and a fold whose paths cover the fact's domain made it unread.

**The fold and PRE_LIMIT.** A sub-group whose folded precondition would pass the writer's PRE_LIMIT (task.py raises
on such an action) stays one action per path, counted (`subgroups_past_pre_limit_left_unfolded`).

**Two writes of one fact.** task.py's writer takes an action with two effects on one fact that can both happen with
different values where it leaves one out as a write of the value the fact has, a value it knows from the action's
precondition set on that fact or the effect's own condition on it (`reduce::double_writes`). Dropping that set
would make the writer raise: (d) keeps such a fact's whole domain where its cut would make that set hold for every
value (counted in `reduce_counts`), and the fold leaves a sub-group unfolded where its union on such a fact is the
whole domain (`subgroups_writing_a_fact_twice_left_unfolded`). build_task writes one effect per fact, so neither
happens on its tasks; the random tests make such actions (test_taskcore.py's `bad` actions).

## The packed file (version 1)

Little-endian u32 words. Strings are UTF-8. A value is written as the string `str(v)` of a domain value (ints,
`above`, `below`), by its id in the string table.

```
header   MAGIC 0x524F4354 ("TCOR"), VERSION 1
body     one record per action, in build_task's order:
           nwords, then nwords words:
           rec, label, kind, end, cont, cost          rec: the record's index (name = label#rec), or 0xFFFFFFFF
                                                     (name = label); label, kind (record kind), end (path end) and
                                                     cont (continuation) are string ids
           npre,  npre  x [fact, n, value x n]       precondition sets, as written (cut to the domain on reading)
           neff,  neff  x effect
         effect: op, fact, payload, nwhen, nwhen x [fact, n, value x n]
           op 0 set:   payload = value               (the domain's label of the written value)
           op 1 guess: payload = n, value x n        (a write of a value the reader cannot work out: one of these)
           op 2 add:   payload = d (i32)             (the fact plus d; see the add tables)
tables   at the offsets the footer gives:
  strings: n, length x n, then the bytes, padded to 4
  facts:   n, per fact: key, size, value x size     (the domain, in its order; fact ids are positions here)
  adds:    n, per table: fact, d, size, then per domain value u: m, value x m   (Domain.add(u, d))
  goal:    n, per entry: fact, m, value x m
  starts:  n, per start: name, then one value per fact (0xFFFFFFFF: the domain lacks the start's value)
footer   body_start, n_actions, strings, facts, adds, goal, starts (word offsets), END_MAGIC 0x444E4554 ("TEND")
```

The body comes first and the tables last, so a writer can stream: it keeps only the string and fact tables
(and, for build_task's fold of identical records, a 16-byte hash per record). Fact ids are given on first sight.
Every set is written in the order of its values' text, so the same reading gives the same bytes in every run
(a Python set of strings iterates in an order that depends on the hash seed).

What the reader does with it is task.py's own reading (`_expand`): a precondition or condition set is cut to the
domain; a written value the domain lacks raises (in a live action, as task.py); an add becomes one conditional
write per current value its condition admits, a guess where the result is several values.

## What stage 2 needs from this format

- The reader can call `Writer.action` per record as it makes them; the format needs nothing at the end that the
  reader does not already have (domains, goal, starts).
- One gap: `args:` domains grow while the reader runs (`Builder._read`'s `args_vals`). The format already
  writes values as strings and domains at the end, so a growing domain is fine, as long as no written value is
  labelled against an unfinished domain (args domains are exact, so their label is the value itself).
- build_task's fold of identical records needs a set of keys over the whole run (now a 16-byte hash each).
- The fork fold groups by (label, kind, end, continuation, cost) and needs the paths of one group together; it
  works on the file, so the reader need not group anything.
- A byte-for-byte target for a Rust reader: the packed file is now the same in every run, so it can be compared
  whole.
- (d) makes the reduced task depend on its start: each replan is a taskcore run from the packed file (about 10 to
  16 s on the kitchen), not a new initial state in one translated file.
