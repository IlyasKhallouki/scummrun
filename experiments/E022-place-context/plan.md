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
  live holder in the whole archive of an atom of T (`Progress::first_counts`). A new atom then comes first, as a new
  room does.
- **Result on the archive (offline):** 22574 is on the frontier and ranks 4 of 2,921.
  - A first version used `novel_counts`, which under the place context counts first holders per place. It put 73
    entries ahead of 22574, and new ones would keep arriving.
- **What happened next (the laptop run, 16:37–16:52):**
  - The model selection took 22574 at once (j08840). Its reading, reused from another state, gave only one dialog
    click (line 120). No new atom.
  - Room changes have come from free tries (C1–C6 were model-free). But one try of either kind took 22574 off the
    frontier for both kinds.
  - Then one free try (j08841) gave 36 new atoms at once, and the key order puts fresh places ahead of 22574's place.
- **Change (same switch):**
  - An atom's first holder stays on the frontier until it has had a try of each kind (model and free).
  - The frontier's entries come in id order, the order they were found, so a stream of new atoms cannot starve an
    older one.
- **What happened then (17:00–17:34):**
  - The per-kind rule put old first holders back on the frontier, those lacking a try of one kind. In id order they
    came first: entries 157 to 11993 in 14 jobs, and 22574 still waited.
  - A cargo build beside the run took the laptop under the run guard's 3 GB, and the guard stopped the run.
- **Change (same switch):** the frontier's entries come with the most guide atoms set first, then in found order.
  This is Go-Explore's preference for cells further along: an adventure's progress is state that stays set.
  - 22574 holds 43 set guide atoms. Only 5 entries in the archive hold more, and the old backlog holds 30–37.

## Amendments (11)–(13), 2026-11-07 17:45–19:52 UTC: the voyage, the suit and the dive (C7 reached by single jobs)

Measured on the laptop, one job at a time from the archive's own states:
- the quiz's entry 22574, made by the laptop run itself (j08778, 14:12);
- then each job's last entry.

- **(11) The voyage, `FMI_READ_EXITS=1` (reader `machine.rs`):**
  - The captain sails with `loadRoom(75)` in room 49's dialog. The reader ended every path that changed to a room
    outside the scope as "left the scope" and dropped it, so no model try could plan a room exit, and the room the
    dialog leaves for was never a target.
  - With the switch, such a path writes the room and ends "room", as a change inside the scope does.
  - Result: `room = 75` is a tier-1 target. The model try plans the dialog's last two choices and plays 570 frames.
    It ends in room 42 (the voyage goes through the world map) with `bit:443 = 1`.
- **(12) The suit, `FMI_READ_PARENTS=1` (`scripts/dump_scripts.py`, IR `game.rs`, reader `builder.rs`):**
  - From the ship, the plan's first step (the repair kit on the punctured suit) was never formed: 1,801 children and
    no hit point.
  - The probe `e016 hits` found the cause. The suit is a child of the locker in room 42's object table, and
    findObject touches a child only while its parent is in the child's parent state. The suit needs the locker open
    (state 1).
  - The index knew the parent's index in the room, not its object or state, so the reader made it an assumption, never
    a precondition.
  - Change:
    - The dump reads each room's OBCD order from the data file, which resolves the parent's index to its object, and
      reads the parent state from the CDHD flags (1 when 0x80, else the low four bits).
    - The IR loads both.
    - Under the switch, a room object's placement requires `state:<parent>` = its parent state, up the chain, where
      the reading holds that state.
  - Four objects in the demo have a parent: the suit (locker), the light wand of the dark room 48 (object 585, state
    1), the balloon (594) and the invoice (941).
  - Result: the plan opens the locker, patches the suit with the kit, and Indy puts it on (the ego becomes Sophia).
- **(13) The dive, `FMI_GOAL_UNPLANNED=1` (fmi-play `window.rs`):**
  - From the suited state, the plan's first step (the hoist with Indy in the suit) was refused twice. The engine's run
    did not write var 104, the verb script's repeat-click counter, as the record predicted.
  - The window's summary shows all 588 children reached the goal, room 82, as "goal, not from the plan", and the
    window discarded them.
  - Change: a child whose run holds the goal is the goal's child even though the plan's goal records did not match.
    It is a real run that reached the target, and the records it did not match are the reading's error.
  - Result: the try plays 121 frames and stops with "goal". Its last entry (frame 18157) is in room 82 with
    `bit:443 = 1`. **C7 is reached** by the system's own plans from the conversation's entry, through three single
    jobs. The archive run has not yet recorded it.

## Amendment (14), 2026-11-07 20:00–21:00 UTC: C8 by single jobs, and the dark room's reading

- **C8 (room 48), single jobs from the room-82 entry:**
  - The first dive is a cutscene. Room 82's local 201 shows a shadow by the boat, sets 488's state and loads room
    42, and room 42 dives again by itself.
  - On the second dive, room 82's entry clears class 6 on one cave at random. Walking into any other cave loads room
    48.
  - The model try's targets put `room = 24` (the drowning's room, its tier and depth equal) before `room = 48`. With
    room 24 tried, as the archive's next try from the same entry would have it, the try plans one walk click. Its
    entry (frame 18956) is in room 48.
- **The dark room's reading did not end.**
  - Room 48's reading ran past the 300 s bound, and past 30 minutes with the bound raised.
  - `FMI_READ_SLOW_DEBUG=S` (reader `par.rs`) names jobs slower than S seconds. The slow ones are local 206's events,
    up to 212 s a run.
  - Local 206 places the ladder and starts global 10, the inventory redraw, directly: this game's
    `VAR_INVENTORY_SCRIPT` is 0. The reader does not enumerate the inventory, so `findInventory` is unknown and every
    one of the ten slots forks.
- **Change, `FMI_READ_INVENTORY_SUMMARY=1` (reader `machine.rs` `eng_script`, `scope.rs` `enumerates_inventory`):**
  a start of a global script whose assignment reads `findInventory` is summarized as the inventory hook's run already
  was: not run, its writes guessed.
- **Result:** room 48's reading ends in 2 minutes, with 14 targets. They are the first steps of the dark room: the
  stone box, the wand, the ladder.
- **The end, from the scripts:**
  - The demo ends when the player walks to the metal door (582) once it is open: `systemOps(1)`. The run's end entry
    comes from the engine (`FMI_GOAL_END`), since the reader does not model `systemOps`.
  - The door opens in local 212, which the sentry statue starts when used with object 933 while `Var[209]` is set.
  - `Var[209]` is set by local 200, the light, which the wand starts when used with 933 (local 203, if `Var[444]`;
    130 here).
- **Next:** the run itself, with every switch of amendments (6)–(14). It uses fresh readings: the 75 readings the
  archive had registered on the laptop, and the committed jobs' run folders, moved to `attic-pre-e022-11`.

## Amendment (15), 2026-11-07 21:24 UTC: a changed model tries the frontier again

- **What happened:**
  - The run with amendments (6)–(14) started at 20:56 and made 156 jobs in 21 minutes. It did not return to the
    quiz's entry 22574.
  - That entry had its model try and its free try (j08840, j08857) under the reader before amendment (11), which
    could not plan the voyage.
- **Change, `FMI_FRONTIER_FRESH=1`:** the atom frontier counts only the tries of the current driver. The driver marks
  its first job (`Archive::fresh_from`). With the model changed, the frontier's entries are tried again, the most
  guide atoms set first.

## Amendment (17), 2026-11-08 01:15 UTC: the VM run, and the most advanced fresh cells first

- **The VM run:**
  - Started 00:07 UTC on fast-mi-1 (8 workers, 2.5 h booked, the cap's last $3.12), on its own archive: 33,301
    entries, which hold the conversation's entries.
  - It used fresh readings: the archive's registered readings and the committed jobs' run folders were deleted, and
    the old 16 GB reading cache `play9-run` too, when the disk fell to 4 GB.
  - By 00:35 the run held room-42 entries with `bit:443 = 1` from frame 3714. That is a shorter route than the
    laptop's (10656).
  - Its model tries opened the locker, patched the suit and put it on. They were refuted on the verb script's click
    counter, as in amendment (13)'s case.
  - j09872's entry 33621 (frame 11119) is the suited state before the dive: ego 2, suit on, `bit:443 = 1`.
  - No job took 33621 in 35 minutes. It is no first holder (an older C6 branch suited up) and not the first at its
    place, so it waited in the place order. The jobs went to early room-49 entries.
- **Change, `FMI_SELECT_ADVANCED=1` (archive `advanced_key`):** ahead of the place keys, the cell's tries of the kind,
  then the entry's guide atoms set, most first. Among the cells tried least, the most advanced comes first
  (Go-Explore's preference for cells further along).
- **And `FMI_FRONTIER_FRESH=0` for this run:** the model does not change within it, and every restart under the fresh
  reset put hundreds of untried first holders ahead of the rest.
