# E022: a place's context, so information found elsewhere makes a room's interactions new again

## Problem (E021's result)

- **What E021 left:** after reading the book, 4,344 entries held the quiz's answers, drawn into `var:239` and
  `var:240`, but none re-entered room 49's quiz.
- **Why:**
  - Targets and novelty are per place (room, objects ego owns).
  - The quiz's atoms (`var:176 = 11`, `bit:403`, `bit:410`, `bit:408`, `var:416`) were held at that place by the
    entries from before the book.
  - So they were neither offered as targets nor novel.

## Prior work (research workflow wf_80dc4253-a1c, 2026-11-05)

- **Novelty partitioned by a context:**
  - BFWS's partition functions, novelty given #g/#r (Lipovetzky & Geffner 2017).
  - Go-Explore's cells with domain knowledge (Ecoffet et al. 2021).
  - IJON's state counters.
- **The rule they share:** a partition by a small count keeps the number of partitions bounded, whereas a partition by
  values or by sets of keys explodes.

## Change (`FMI_PLACE_CTX=1`, off by default; fmi-explore `ctx.rs`)

- **A room's input keys:** the vars and bits that its own code (entry, exit, local scripts, its objects' verbs) tests in
  a jump's condition. Left out are keys its own code writes, the engine's variables (`scumm_ir::vars::VARS`), and any
  key the archive has seen cleared from a parent's state to its child's (`note_edge`).
  - The cleared rule matters: in play9, `var:176` (the dialog topic) is cleared 16 times, `var:165` 205 times and
    `var:52` 275 times, while `bit:403`, `bit:410` and `var:416` never are. The room's own code writes those three
    anyway.
- **A state's context:** the set of its room's input keys that it holds nonzero. A nonempty set is appended to the
  place's text in both `progress::place_text` (novelty, `by_place`, the try's `Novel`) and `Archive::place_of_row`
  (job.json held atoms, place counts, rarity).
- **Count against set (amendment (1), below):** a count, capped at 3, saturated, so the set replaced it.
- **Test:** `fmi-explore/tests/ctx.rs`. An input key moved from 0 opens a context. After an edge clears it, it opens
  none. With the switch off, the text is unchanged.

## Gates (offline, on E021's archive pulled to the laptop)

- **G0:** room 49's input keys include `var:239` and `var:240`, and the cleared ones include `var:176`.
- **G1:** live places grow by at most 2×, and the book-read room-49 entries are at a place with context ≥ 1.
- **G2:** for a book-read room-49 entry (22115), job.json "held" lacks the quiz's atoms (`var:176 = 11`, `bit:403`,
  `bit:410`, `var:416`). With the switch off, "held" contains them.

## Run, prediction, stop rule

- **Run:** play10's archive on the VM with E021's `env.sh` plus `FMI_PLACE_CTX=1`.
- **Prediction:** an entry in the quiz after the book within 30 minutes, `bit:443 = 1` within 60, and C7 within 90.
- **Stop rule:** the watcher's 90 minutes with no new rung.

## Gates, measured 2026-11-05 on E021's archive pulled to the laptop (example `ctx_gate`)

- **G0 passes.**
  - Room 49 has 10 input keys: `bit:22`, `bit:424`, `bit:444`, `bit:446`, `var:176`, `var:224`, `var:238`,
    `var:239`, `var:240` and `var:444`.
  - Seen cleared: `bit:22`, `var:176` (the dialog topic) and `var:444`.
- **Amendment (1): the count saturates.**
  - Most room-49 entries hold 3 or more input keys (`bit:424`, `var:224`, `var:238`, ...), so with the count both the
    book-read entries and the earlier ones are at `#3`. 9,722 live entries share one place, and G2 fails.
  - The context is now the set of keys.
- **G1 passes with the set:** 13 live places (12 without contexts). The 3,199 book-read room-49 entries have their own
  context (`...+var:239+var:240`).
- **G2 passes for the quiz's entry atom.**
  - In entry 22115's place, job.json "held" no longer has `var:176 = 11`, so entering the conversation after the book
    is novel and a target again.
  - It still holds `bit:403`, `bit:408`, `bit:410` and `var:416 = 2/3`. The book-read entries inherited these sticky
    flags from earlier conversations, so they are not what re-entering the quiz gives. That is `bit:443 = 1`, which a
    reading inside the quiz targets (E021, entry 1296).

## Amendment (2), 2026-11-05 23:30 UTC: place counts and the dialog's refinement

- **The first prediction is met:** an entry in the quiz after the book 6 minutes into the run (22356: room 49,
  `var:239 = 250`, `var:240 = 2`, local 208 waiting at 0x04BE). 6 such entries by 23:22.
- **Selection:** a place's count resets only on new T atoms or rooms archive-wide. The book-read context carried
  E021's 4,640 unproductive tries, so the selection worked the pre-book context. Under `FMI_PLACE_CTX=1` a try now
  also resets its place's count when its new entries add an atom no earlier entry at their place held
  (`Archive::place_key`, the log replayed in order). Restarted at 23:24.
- **The dialog's refinement:**
  - Every model try from the quiz entries stopped on "Lemma R's premise R0: the last pass wrote values of var [165]
    outside the domain". `var:165` is the dialog choice, written 120-124 by the dialog and seeded only with its start
    value.
  - New: `FMI_REFINE_WIDEN=N` (harness.rs). The out-of-domain values seed the domain through starts that hold them
    (`Rereads::add_start`, `Wiring` keeps them), and the scope is read again, at most N times. Test:
    `widen_tests`. The run sets N = 3.

## Amendment (3), 2026-11-05 23:55 UTC: the entries that first reach something at a place are selected first

- **What happened after amendment (2):** in 22 minutes no try started from the 6 quiz entries. The selection did take
  the book-read context, but in that place they compete with 3,500 entries in fresh cells. Among fresh cells the
  order falls to the entry's novelty count, which counts the atoms the entry is the least live holder of archive-wide.
- **Change, under `FMI_PLACE_CTX=1` (progress.rs):** `Progress` keeps the least live holder of each atom per place,
  the place's text with its context (`first_place`). `novel_counts` counts those instead. So an entry that is the first
  at its place to hold an atom, as the quiz entries are with `var:176 = 11` after the book, comes first among its
  place's fresh cells (Go-Explore: newly found cells first).
- **Check:** fmi-explore's tests all pass. With the switch off the counts are unchanged.

## Amendment (4), 2026-11-06 00:05 UTC: the place-first count goes ahead of the cell's counts, for a cell's first 3 tries

- **What happened after amendment (3):** the quiz entries' cells had had tries before (`chosen ≥ 1`), so they sorted
  behind the place's thousands of fresh cells. The novelty count only broke ties among equal counts.
- **Change, `Archive::place_new_key`:** right after the place's key comes minus the entry's place-first atoms, while its
  cell has had fewer than 3 tries of the kind (model or free), in the model's and the free selection, with and without
  the frontier.

## Amendment (5), 2026-11-06 00:40 UTC: the place-first count uses the guide's atoms

- **What happened after amendment (4):** in 30 minutes still no try started from the 6 quiz entries. The first-holder
  count used T, the novelty atoms, and the dialog topic `var:176` is not in T (the code map found room 49's tested
  keys without it).
- **Change:** `place_new_key` counts the guide's atoms (`Row.held`, the atoms job.json's "held" and the targets use)
  that an entry is the first live row at its place to hold. It is cached by the number of rows.

## Amendment (6), 2026-11-06 01:15 UTC: a fresh reading takes the live slots' locals (F10), and so do its extensions

- **What was found on the laptop, rerunning play10's j09542 (entry 22356, local 208 waiting at the conversation's
  start, 0x04BE):**
  - The room-49 reading reaches the quiz's turns (events at 0x0DB4 and 0x10E6) but not `bit:443`, and local 208 has no
    lists of locals.
  - The slot runs at the reading's start, and `read_fresh` passed no `--slot-locals`. Its locals were unknown through
    every turn, so the counter never passed the test at 0x1166.
- **Change, `FMI_SLOT_LOCALS=1`:**
  - tryjob's `SLOT_LOCALS` provider, which fmi sets from fmi-play's `dumped_slots`, gives the live slots' locals to
    `read_fresh`.
  - `Wiring` reads the reading's own `slot-locals.jsonl`, so its reach extensions and refinements read with the same
    locals. Before this, the extension dropped them and the planner answered "--goal: no fact bit:443".
- **Result on the same job:** the reading reaches `bit:443 = 1` (reached 0, 1), which becomes the try's first target
  (tier 1). Local 208 has 27 lists.
- **Open:** the planner proves `bit:443 = 1` unsolvable on the reach extension from the entry ("6 values not reached"
  before it). That is the next diagnosis.
- **The run:** stopped at 01:15 so the booking is not spent while this is diagnosed offline. Still C6.

## Amendment (7), 2026-11-07 11:40 UTC: replans read the replan state's slots, and a plan tries the goal's families first

Measured on the laptop, rerunning play10's j09542 (entry 22356) with the target `bit:443 = 1`.

- **Replans did not get their slots' lists.** Extensions and refinements read only with the original start's slot
  locals. At the replan state (frame 2593), local 208's locals matched no list, so its args fell to 0 and the planner
  ran out of its 300 s.
  - **Fix:** `Rereads::add_slot_locals`. Before an extension or a refinement, the harness gives the reader the
    current state's live slots, deduplicated with the ones it has.
  - **Result (q5):** the replan's slots all match but one ("args from the slot's dumped locals": 4). The planner
    still ran out of time.
- **Why it still ran out.** Plan 0 picked dialog line 120, but at frame 2593 only lines 121–124 are on, so the replan
  has to go through line 124 to open line 120.
  - The plan exists. A breadth-first search over the dialog's own actions (the clicks, global 17's events, local
    208's events) finds 16 steps after 394 states.
  - On the whole reading, LAMA, FD and the Rust GBFS each ran out of 900 s.
  - Room 49's reading holds 11,272 events of local 209 and 4,800 of global 59, against 898 of local 208. The
    relaxed plan reaches var 165 through an args-0 event of global 17 in 3 rounds, so the heuristic sees the goal
    as near everywhere.
  - With every label outside the dialog banned, LAMA and GBFS each find a 20-step plan in 0.1 s.
- **Change, `FMI_PLAN_FOCUS=1` (fmi-explore `focus.rs`).** This is partial grounding (Gnad et al., AAAI 2019): plan
  on a subset of the actions first.
  - Actions fall into families: an event's slot, or a label with its arguments dropped.
  - The subset starts with the families that write a goal value. Each round adds the smallest family that writes a
    precondition value the subset does not reach under relaxation, until it reaches the goal.
  - The harness bans every other label and plans for at most `FMI_PLAN_FOCUS_S` (30 s), then plans on the whole
    reading as before.
  - A plan on the subset is a plan on the reading.
  - The relaxed start is the planner's own: every fact of the domains, 0 where the state lacks it. The first version
    seeded only the facts the state named, and found no subset in the harness.
- **Result (q8):** plan 1 focuses on 931 actions (the dialog lines, local 208, global 17, the boat's events) and finds
  20 steps in 0.1 s. The try plays 154 frames.

## Amendment (8), 2026-11-07 12:35 UTC: a replan whose live slot matches no list extends the reading

- **What happened (q8):** after 154 frames, the replan at frame 2746 had local 208 at 0x1A4C and global 86, whose
  locals matched no list. The planner proved no plan.
- **Change:** `args_of_slots` returns the unmatched slots. The harness extends the reading from that start with the
  live slots' locals, as for a start value the reading did not reach, under the same limit.
- **Result (q9):** extensions at frames 2746, 2874 and 3032; the try plays 440 frames. At 3032 local 208 waits at
  turn 2 (0x10E6) with its counter still 0: turn 1 was answered wrong, and no plan exists from there.

## Amendment (9), 2026-11-07 12:45 UTC: the scope rooms' input vars are facts of a fresh reading

- **What happened (q9):**
  - The quiz compares the dialog choice with var 239, the distance the book draws. The state has var 239 = 250.
  - var 239 was not a fact of the reading, so either answer looked right to the planner.
  - The refinement at the wrong answer's deviation could not name the variable ("the frame before is not kept").
- **Change, `FMI_READ_INPUT_FACTS=1`:** a fresh reading takes as fact vars the input vars of its scope's rooms
  (`ctx.rs`: the vars the room's code tests and does not write, the engine's aside). Room 49's are 176, 224, 238,
  239, 240 and 444. A var the room never writes keeps its start value through the reading, so its tests read that
  value.
- **Result (q10):**
  - Probes on var 444 are proved unsolvable at once.
  - The `bit:443 = 1` attempt plays 545 frames, with 9 plans (all focused, 0–25 steps, 0 s each) and 4 extensions.
  - It stops with "goal, not from the plan: it held at a plan's start".
  - Entry 14 (frame 3137) holds `bit:443 = 1`. The quiz is passed from the conversation's start with no hand input.

## Amendment (10), 2026-11-07 16:00 UTC: a new atom joins the frontier as a new room does

- **What happened (the laptop run from play10, W = 3 then 2, with amendments (6)–(9)):**
  - At 14:12, job j08778 started from entry 22307 (the conversation's first menu, reached by the run itself). It
    passed the quiz in 66 s, and its entry 22574 is the archive's only holder of `bit:443 = 1`.
  - No job started from 22574 in the next 1.5 h.
  - The selection's report (`Archive::explain_selection`, example `sel_explain`) ranks 22574 at 724 of 2,921
    candidates, and the place count decides it.
  - `bit:443` is written by room 49's own code, so it is not in the place's context. The place's count reset when
    j08778 gave the atom. Its next 15 tries went to other entries there that first held more of the guide's atoms at
    the place (up to 6, against 22574's 1), and gave nothing. The place now waits behind every place with 14.
- **Change, `FMI_FRONTIER_ATOMS=1`:** the frontier (E016 F) also holds each entry not chosen yet that is the least
  live holder of an atom of T. A new atom then comes first, as a new room does.
- **Result on the archive (offline):** 22574 is on the frontier and ranks 74 of 2,921. The 73 ahead of it are the
  other untried first holders of atoms of T.
