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

## Amendment (18), 2026-11-08 01:50 UTC: the VM run at the cap's end

- **The VM run from 01:13 (amendment (17)):**
  - 42 jobs by 01:43; the ladder stays at C6.
  - `FMI_SELECT_ADVANCED` took the fresh cells with the most guide atoms set. Those are side-branch entries (45 set,
    rooms 49/63 near frame 14,600, without `bit:443`), not the suited entry 33621 (41 set).
  - The count of guide atoms set does not measure progress: side branches gather atoms the goal does not need.
  - The guide gives each atom a depth from the goal: rooms 48 and 82 and `state:582 = 1` at 1, `bit:443`, the kit
    and the suit's class at 2. That is the measure to try next.
- **A single job on the VM from 33621 (frame 11119, suited, `bit:443 = 1`), target `room = 82`:**
  - The plan's first step, the hoist with Indy in the suit, ran in all 588 children. All were "spent": no dive.
  - The laptop's suited state (frame 18036), where the same step dives (amendment (13)), differs in the hose
    animation's state: object 495 is 3 on the VM and 0 on the laptop.
  - Not explained yet. The hose and the compressor are the first suspects. The laptop's chain connected the hose
    (`sentence(8,496,491)`) before suiting up.
- **The booking ends at 02:32 UTC with the cap: $49.50 of $50 committed.** The run stops with the VM, and its archive
  stays on the VM's disk.

## Amendment (19), 2026-11-08 12:45 UTC: why neither run passed C6

- **The laptop run** (amendment (15)):
  - The memory guard killed it at 22:03 on 11-07, seven jobs after j08908 reached room 42 with `bit:443 = 1`
    (entry 24147, frame 10656).
  - None of the seven started from 24147. The killed job, j08912, was a model try from 8296 (room 42 at frame 2776,
    before the quiz); its five reach extensions took the memory.
  - Under `FMI_FRONTIER_FRESH=1`, every untried first holder came ahead of 24147, which was not on the frontier.
    Under the current switches `sel_explain` ranks 24147 first of 3,252 candidates; the run was gone by then.
- **The VM run** (amendments (17), (18)):
  - Its suited entry 33621 waited: its cell already held an older suited entry, and `FMI_SELECT_ADVANCED` ranked
    side branches holding more atoms ahead of it.
  - Taken by hand, its first step (the hoist with Indy in the suit) was refused in all 588 children: "input
    sentence(8,497,500)#92: effect frozen:local:201: the predicted write is not in the run; script:20".
  - The air compressor's loop (room 42's local 201) runs in that state. The dive's cutscene starts global 20, which
    calls `freezeScripts(127)`. The engine spares a slot started with the F flag, and both starts of room 42's local
    201 (the entry and local 213) pass F.
  - The F flags (IR `compute_resistance`) gave object code no room. A start of a local number from an object's
    script counted for every room's local of that number. Objects of rooms 63 (853–855) and 69 (943) start their own
    local 201 without F, so room 42's local 201 had both flags and its F was not known.
  - The reader then counts the slot frozen. The sentence's record predicts `frozen:local:201 = 1`, and the window
    refuses the real run at the click, before the dive.
  - In the laptop's suited state (frame 18036) the compressor was off: nothing to freeze, and the dive played.
- **Reproduced on the laptop:**
  - `e016 cp` (new) records the engine's checkpoint at a movie's frame. The laptop's suited state with the switch
    pulled at 18037 gives a compressor-on state at frame 18500 (`local:201 = 1`, `state:495 = 1`).
  - The engine dives from it: `e016 g3`, the hoist, room 82 by frame 20005.
  - The single job fails as the VM's did: "the same empty window 2 times on sentence(8,497,500)#92 … effect
    frozen:local:201".
- **Change, `FMI_READ_OBJ_ROOM=1` (IR `game.rs`):**
  - In the F flags, a start of a local script from an object's code starts its own room's local, unless the object
    can be in the inventory. That is the case when a constant `pickupObject` or `setOwnerOf` names it, or its own code
    picks up `VAR_ME`.
  - Nine units' flags become definite. Room 42's 201; room 48's 201, 205, 209, 211 and 213; room 49's 201, 209 and
    213. Five of them are in the dark room, the C9 path.
  - Probe: `examples/resist` in the IR crate.
- **A writer bug on the way:** the next try's reach extension (from the dive's cutscene) failed with "185814 action
  lines for 185338 actions". `Files::reset` truncated the actions file before the old writer's buffer went out.
  Fixed, with a test (6f01029).
- **Result:** from the compressor-on state, the job plans the dive and stops "goal" in room 82 (frame 18814,
  5 entries). The state that failed on the VM now passes.
- **Left:** the selection has no measure of progress. The runs ranked untried or atom-rich entries ahead of the
  newest step of the chain. The guide's depths (amendment (18)) are the measure to try next.

## Amendment (20), 2026-11-08 13:40 UTC: the archive follows the chain of model tries

- **Problem (amendment (19)):** the runs never went back to the newest step of the chain. Each model try plans to
  one of the guide's targets and stops at the first step the engine refutes, so the chain C6 → C9 needs a try from
  each try's end. The selection put untried first holders (the frontier) or the cells holding the most atoms first,
  and the chain's ends waited behind thousands of entries.
- **Change, `FMI_SELECT_CONTINUE=1` (archive `continuation`, `continue_key`, `depth_profile`):**
  - The end entry of every model try that played (its attempt ran frames, it committed entries; the entry marked
    `end`, else its last frame) is a continuation until the kind (model, or free) has tried it. A cell's continuation
    stands for its cell.
  - Continuations come first, ahead of the frontier. Among them, the guide atoms each holds set (nonzero) are counted
    by depth from the goal, depth 1 first, most first; then the newest.
- **Measured before the run, play10 (24,184 entries, 2,227 continuations, 1,904 untried by a model try):**
  - Order by depth profile: first 23351, the quiz try's end in room 49 (`bit:443 = 1`); second 24147, the voyage's
    end in room 42. Every other candidate comes after (3,252).
  - Newest first alone put 24149 first, a room-63 side try's end, and it would have followed that chain first.
    Rarity (24147 at rank 74) and frame (464) do not order it either.
- **Tried and dropped:** a guide whose any-value atoms (a room change) are not regressed (`FMI_GUIDE_SPECIFIC`)
  still has 340 atoms at depth 2 of 492, since the specific rooms' writers read most of the game too. The code is not
  kept.
- **The run:** play10 on the laptop, 1 worker, every switch through (20), readings fresh (the registered readings and
  the jobs' run folders, made under the reader before (19), moved to `attic-pre-e022-20`), wall limit 4 h, the
  watcher's stop after 90 minutes without a new rung.
- **Prediction:** the first model try starts from 23351 or 24147; C7 (room 82) within 2 hours; C8 (room 48) within
  3 hours.
- **Stop rule:** the wall limit, the watcher, or a memory guard kill. A kill is the laptop's limit, not the
  selection's result, and is reported as such.

## Amendment (21), 2026-11-08 14:55 UTC: the run's first 35 minutes, and the continuations ordered by their try's target

- **The run of amendment (20), 13:49–14:24 UTC, 12 jobs:**
  - j08916, a model try from 23351, played the voyage: entry 24189, room 42 at frame 10657 with `bit:443 = 1`. As
    predicted.
  - j08918, a model try from 24189 with target room 82, opened the locker, used the kit and connected the hose, then
    waited 2 × 3,600 frames on the suit (`sentence(8,491)`). All 588 children were refused on the sentence script's
    repeat-click test (`global/script-002.txt:8`, `Var[105] == Local[2]`, no fact). Its end, 24197 (frame 17965),
    is the state before the suit.
  - The next model try took 24147 and failed the same way. Then the order went to side tries' ends in rooms 49 and
    63: their depth profile tied (36–37 atoms set at depth 2) and they were newer.
  - The memory guard ended the run at 14:24, in a free try in room 63 (3.1 GB available).
- **The suit, by a single job from 24196 (frame 14365, just before the suit step):** a fresh reading plans the suit and
  the hoist and stops "goal" in room 82 (4 entries). The hoist passed through one of the click's other records
  (`FMI_WINDOW_ALT_DEBUG=1`). The step can pass from that state; the run did not continue there.
- **Tried and dropped:** h_add over the guide's regression graph (the write sites' needs, exported by the guide
  builder) gave 23 for every entry, the boot's included. A writer of room 48 has no needs (the boot script), and the
  end's `owner:582` has no writer.
- **Change, under `FMI_SELECT_CONTINUE=1`:** continuations are ordered by their try's target. A target value no entry
  held before the try comes first (the guide target's tier 1; novelty from the progress store), then the newest.
  - On play10 after the run: first 24241, the end of the second room-42 try (target room 82). The side ends follow at
    ranks 406–407.
- **The run resumes** on play10 under the same settings. The killed run's unfinished job (j08928, room 63) is moved to
  the attic.
- **Prediction:** the first model try starts from 24241 or 24197; C7 within an hour; C8 within 2 hours.

## Amendment (22), 2026-11-08 16:00 UTC: C7 in the run, the order's fixes from an adversarial review, the VM

- **The run of amendment (21), from 15:02 UTC:**
  - j08928, a model try from 24241 (room 42, before the suit) with target room 82, played the suit and the hoist and
    stopped "goal" in room 82: entry 24381 at frame 18157. **C7, by the run itself**, 4 minutes after the restart.
  - j08930, a model try from 24381, aimed at room 42 (the first dive is a cutscene back to the ship) and reached it.
    Its end 24391 (frame 18321) is the furthest state; its waits found the air counter (`var:452`) and the drowning
    (room 24).
  - The next model try went to 24197, a copy of the pre-suit state, since its try's target had been new when it
    was made.
- **Fixes to the order (`FMI_SELECT_CONTINUE`), the review's three confirmed findings and the drowning:**
  - Novelty is read when the order is made. A try counts as having gone for something new while a value it went for
    is held by no row but its own entries: its target's value (any of a list target's values; list targets were
    never counted before), or an atom of T or a room it gave.
  - Only the latest 64 played model tries (`CONT_WINDOW`) make continuations. Before, every played model try of the
    archive's history did, 1,946 still open, and the frontier would never have decided a selection again.
  - A continuation stays open for up to 3 model tries (`CONT_TRIES`) while none of its tries left a live end. From
    the room-82 entry a first try that goes for the drowning (a dead end) no longer ends the chain.
- **Order on play10 after j08932:** first 24391, the end past the first dive's cutscene. 24197 and 24241 are closed.
- **The VM, with $10 more approved (cap $60):** fast-mi-1 on the 48 GB shape for 8 h (the last VM run at 8 workers
  never went under 24 GB free). The run on a copy of the laptop's play10 (the C7 chain), 10 workers, every switch
  through (22), the reading bound 900 s, wall limit to the booking's end.
- **Prediction:** C8 within 1 hour of the start; C9 today, before the booking ends (23:43 UTC).

## Amendment (23), 2026-11-08 16:30 UTC: C8 on the VM; the dark room's memory and stairway

- **C8 at 16:04 UTC, 11 minutes into the VM run:** j08934, a model try from a room-82 entry with target room 48,
  walked into a cave. Its end 24418 is in room 48 at frame 18956, with 64 inputs from boot on its chain.
- **The dark room's readings (a scout from the 11-07 room-48 entry, on the VM with an 18 GB reading cap):**
  - The base reading: 340,034 records, 5.3 GB. Each reach extension: about 365,000 records and 6 GB more. The try
    keeps every reading it made: 11.5, 17.5, 23, 30, then 35 GB by the fifth extension. The scout was stopped at
    6.5 GB available.
  - The reading cap bounds one reading's growth, not the process. A try's memory can reach the guards' floor, where
    the driver's guard ends the whole run (the laptop run's end at 14:24 was this).
  - 307,000 of the base reading's 336,000 actions are room 48's local 205, the ladder climb. It copies the pending
    sentence (verb, objects) into its locals, so each sentence is a list of locals of its own (up to SIG_LIMIT, 1,024
    per wait point), each with a chain of events polling global 126.
  - The plan's first step on the broken stairway (584, local 204) was refused 5 times. Local 204 walks Indy to a
    spot and sets `Var[444]` = 1 only when he ends within 10 pixels: a walk's outcome the scripts do not decide.
- **Changes:**
  - `FMI_REACH_TRY_LIMIT=1` (harness): a try makes at most one reach extension. The next try starts from its end in a
    process of its own.
  - The guards staggered on the VM: the reading stops at 3 GB available, a job's guard at 2 GB (`guard-job.sh`,
    `E008_GUARD`), the driver's guard at 1 GB (`GUARD_MIN_KB`, `guard.sh`).
  - The continuations' novelty does not count the rows below the try's own entries. The free try from the room-48
    end made more room-48 rows, which took the end's claim, and side ends in rooms 63 and 69 filled the 10 workers.
- **The C9 chain, from the scripts:**
  - Open the stone box (585), which shows the wand (586, a child of the box at state 1). Take the wand.
  - Use the wand with 933, which starts local 203. Local 203 lights the room (local 200, `Var[209]`) when `Var[444]`
    is not 0. `Var[444]` is a scratch variable several globals write; it is 130 in the room-48 entry's state. The
    stairway is not needed.
  - A stairway walk that ends short sets `Var[444]` to 0, which closes the light in that branch.
  - Use the sentry statue (587) with 933 while `Var[209]` is set: local 212 opens the metal door (582). Walking to
    the open door is the restart, C9.
- **The run:** restarted twice on the VM (16:18 and 16:28 UTC) for these changes; the uncommitted jobs run again.

## Amendment (24), 2026-11-08 18:40–20:40 UTC: the dark room's reading from gigabytes to megabytes

- **The VM is off until room 48 reads in megabytes** (Ilyas). It ran 15:43–18:16 UTC (2.6 h, $3.16); its run had
  ended at 17:55 on repeated engine failures while my 18 GB scout squeezed its memory. From 15:53 to 17:09 it ran the
  reader of Nov 7: my sync of the reader's sources went to the wrong folder (`experiments/scumm`), so
  `FMI_READ_OBJ_ROOM` and the writer fix were not in its build until 17:09.
- **What multiplied room 48's reading** (a read-only workflow, its findings checked by a second agent; reader-scope on
  the laptop from the idle state, entry 28025 at frame 19203):
  - 285,696 of its 311,734 actions are the ladder climb's events (local 205 after its two WaitForActor, 142,848
    each). Each run goes on into local 208, which tests the dark classes (25) of eight objects and only renames them:
    3 x 2^8 paths a run, then local 211's walk-box test and a re-chain of 205 that copies var:113 (13 values).
  - The machine forks a path on each fact it reads; at a join the siblings stay apart while their assumed values
    differ (the join's signature holds them); each path is a record.
  - Lowering SIG_LIMIT, turning off value_sets and turning on reach_pairs changed nothing (measured).
- **Change, `FMI_READ_MERGE=1` (reader `Builder::merge_records`):** a run's records equal in all but their
  preconditions are merged exactly: one key's values united (the key dropped when they cover its domain), a covered
  record dropped.
  - Room 48: 315,519 records made, 5.2 GB, 642 MB packed → 16,755, 322 MB, 7.3 MB.
  - Room 42: 477,501 records, 1,117 MB → 49,943, 282 MB; reached values equal (519 keys); every record of the reading
    without the switch covered by one with it (same label, effects, end and checks, its precondition covering).
    Rooms 63 and 49: nothing to merge, the readings equal.
- **Tried and dropped:** a union at the machine's joins (paths whose whole state is equal and whose assumptions differ
  in one key). It cut the paths themselves, but lost records: global 59's input loop, 1,050 of room 63's 3,092
  actions uncovered, every union at one join (0x250, `script:2` assumed 0 and 1 by the settled round). Keeping both
  paths' loop histories did not change it. The cause is not found and the code is not kept.
- **`FMI_BAN_TRY=1` (harness):** a step refused twice in a try is banned in every state for the rest of it (the ban by
  movie prefix missed the next plan's start). On the laptop the third plan carried the ban.
- **`FMI_READ_TRIM=1`** (a try's readings released, the heap trimmed) and **`FMI_REACH_TRY_LIMIT`** stay, for memory.

## Amendment (25), 2026-11-08 20:00–20:40 UTC: the sentences the dark room's chain needs

- **Why "open the box" was unsolvable from the idle state** (the workflow's second finding, checked): the box is
  untouchable (`class:585:32 = 1`) until local 208 clears it, which with the light off needs class 27 on the
  stairway (584), set only by local 206, started only by local 204 when it is run for the ladder (583). The input
  "use ladder with stairway" was never built: `b_values` takes B from constants compared with Local[0] in A's own
  entry, and the stairway's Use passes Local[0] to local 204 (`startScript(204,[8,Local[0]])`), which compares it as
  its Local[1]. And a B held in the inventory was never placed there: "use wand with 933" was missing too.
- **Changes:** `FMI_READ_B_ARGS=1` follows Local[0] into a started or chained script's argument; `FMI_READ_INV_B=1`
  places B in the room and in the inventory. Room 48's reading now has `sentence(8,584,583)`, `sentence(8,586,933)`
  (and `[inv]`) and `sentence(8,587,933)`.
- **Open:** with the ladder usable the reading reaches the climb, the light and their scripts; on the laptop it was at
  round 20 after 344 s and did not end in 15 minutes (each such run enumerates its paths before its records merge).
  Measuring it to the end; the path-level cut is the remaining work.
- **Also open:** room-48 tries from the C8 entry (the arrival cutscene) fail on its background scripts (local
  201/202); the idle states after it (254 of 269 room-48 entries) do not have that. The stairway walk's outcome (within
  10 px of a spot) is the engine's.

## Amendment (26), 2026-11-08 21:30–22:30 UTC: the dark room's light reads in seconds

- **Measured (amendment (25)'s open item):** with the ladder usable, room 48's reading from the idle state ran an hour
  and did not end (rounds 34–43 at 200–350 s each, about 110 records a round). The time was one job: the light (local
  203) resumed after its wait. Its run starts local 200, which starts local 208, which tests the dark class (25) of
  eight objects only to pick their names: 2^8 paths times the fade's `Var[211]` values, past the path limit, so the job
  split some 80 ways (beads `Var[229]` 5 values x owner of 933 x six more facts), each part about 1,000 paths and
  2–4 s, and every round a newly reached value ran it again. `Var[229]` (the beads the light spends) has 6 values; it
  does not grow.
- **Change, `FMI_READ_RENAME=1` (reader `machine::rename_test`):** a jump whose test reads classes, states, owners,
  locals and constants alone, whose two arms hold only setObjectName, such tests and forward jumps up to where they
  meet, and where every way runs a setObjectName before any other statement, is read with no fork and nothing assumed.
  setObjectName's one effect the machine keeps is the inventory hook's run, whose writes are the same at every run and
  which no such test reads, so both arms are in the same state at the first rename after the test. In local 208 it
  takes six of the eight dark-class tests (582, 584's 26, 585, 586, 587, 589); 583 and 584's 25 stay forked (an arm
  writes `VAR_RESULT`).
- **Room 48, all switches** (merge, B args, inventory B, rename): the whole reading 36 s wall, 197 MB peak, 8.3 MB
  packed, 16,680 actions. Before: no end in an hour.
- **Gate, off vs on** (each with `FMI_READ_MERGE=1`): rooms 63, 49, 42: every action of either reading covers one of
  the other, reached values equal (room 42: 268 s to 220 s). Room 48 (without the B switches, which make the off
  reading too long): every off action is covered by an on action with the same label, end and continuation, a covering
  precondition, the off checks among its checks and, where they differ, a guessed write holding the off value. The
  on reading is coarser at two joins the machine already had: siblings that no longer differ in a dark class meet
  there, and the join joins their values: the cutscene-end script's data (`Local[0]` of global 21, checks the
  engine settles) and `Var[113]` after the ladder is placed. Reached values equal (271 keys); 14,417 actions to 12,096.
- **Seen, not changed:** the ladder climb's animation (global 125) loops to a bound the actors' positions set, a
  non-fact, so its chain runs to the fold limit: about 6,000 one-job rounds, 7,000 cheap records, a few seconds.
- **The stairway walk:** local 204 places the ladder only when the walk ends within 10 px of (381,129); the scripts
  cannot say whether it does, the engine can. The light does not depend on it: local 203 starts global 84 first, which
  sets `Var[444]` to -1.

## Amendment (27), 2026-11-08 22:00 – 2026-11-09 00:20 UTC: the dark room played through, step by step

Each laptop try from the idle room-48 entry (28025) named one gap; each was fixed, gated and committed:

- **A refuted check on a non-fact (`FMI_REFINE_CHECKS=1`).** The sentence script's first test reads `Var[105]`, the
  last sentence's object, which no reading held; three plans chose the record the engine contradicts and the attempt
  waited 3,600 frames twice. A window's refuted check whose record says `?(not a fact: var:N)` now makes N a fact,
  as a split does.
- **The sentence the player makes.** The ladder's and the beads' Use entries are `doSentence(8,Local[0],VAR_ME)`:
  the player makes "use ladder with stairway" (8,583,584) and the ladder re-issues (8,584,583). `e016 g3` on the
  engine: (8,584,583) no clicks in 60 candidates; (8,583,584) in 6, and local 206 placed the ladder 8 frames on.
  - `FMI_READ_B_SWAP=1`: such an object takes as B each object whose own entry takes it as B (the player's form
    built), and gets no one-object sentence (alone it re-issues (8,0,A), and the interface asks for B first).
  - `FMI_READ_QUEUE=1` with the switch `wait_both`: a doSentence holds its sentence, which runs where the run's own
    step ends when the sentence script's slots are not live (checkAndRunSentenceScript at the frame's end). The
    re-issue happens just before the sentence script's `WaitForActor(VAR_EGO)`; wait_both's passing branch ends the
    script in the same record, and the held sentence runs there. Room 48 reads in 55 s, 200 MB, 22,024 actions.
  - `FMI_REALIZE_QUEUED=1`: a planned sentence the engine's sentence script begins with no input within 8 frames
    is played with no clicks (the slot's locals, or the object's code in the run log).
  - `FMI_REALIZE_SWAP=1` (realizing (v,a,b) by the swapped clicks) works on the engine but the strict window refuses
    the first run's writes; kept off.
- **Steps no clicks make (`FMI_BAN_TRY`).** A window that formed no input bans its label for the try at once (a
  held object's Use waits for B: "use 934" alone stopped try 6); a deviation counts its refuted record's label (try 7
  replayed "look at the archway" and a continuation no record matched 13 times, to the replan limit).
- **Both objects held.** The realizer searches the first verb's whole grid of A's and B's slots: "use wand with
  beads" was beyond 60 candidates; `e016 g3` finds it at 75, and the light (local 203) runs the next frame.
- **Try 8** (before the last fix): ladder placed (frame 23149), box opened (23195), wand taken (23224); the light's
  sentence was not found in 60 candidates.
- **The run:** the VM (n2-custom-30-49152, booked to 04:43 UTC, $6.07) resumes play10 (the C8 archive, 30,006
  entries) with these switches since 23:53 UTC, the new build from 00:15. The 21 GB reading cache of the old switches
  was deleted (its keys hold the switches; none could be reused).

## Amendment (28), 2026-11-09 00:20–01:10 UTC: C9, the demo finished

- **The light's records (`FMI_READ_STRING_SUMMARY=1`).** "Use wand with beads" gives IQ points (global 144), which start
  global 145: a loop over the 154 characters of the points string, each an unknown, so every run giving points passed
  the path limit and was cut. The reading kept only the "points already given" records, and the engine, giving them
  the first time, matched none. A global script that reads a string's characters in a loop is now summarized as the
  inventory hook is (not run, its writes guessed); the light's records start local 203 either way (room 48 from the
  wand held: 7 s, 18,802 records with and without).
- **The run stays at its front (`FMI_SELECT_FURTHEST=1`).** After the restart with the new model the cells tried least
  were the early rooms' (room 69) and the dark room got no job. Selection now keeps the cells within one guide atom of
  the most held (play10: 19 cells, 18 in room 48).
- **C9 on the laptop first** (tries chained by hand from entry 28025: ladder, box and wand in try 8; light, statue and
  door from the wand held, 33 frames; the end by a free try from the open door, 36 moves ending at the door's
  restart, `restart@1:582:48:0067`). The chain's movie, 89 clicks from boot, ends the game at frame 23324 in a fresh
  engine (`c9/movie-c9.jsonl`, `c9/end-from-boot.jsonl`).
- **C9 by the run itself** (the VM, all switches, `FMI_SELECT_FURTHEST=1` from 00:34): the room's entries placed the
  ladder, opened the box, took the wand, lit the room and opened the door, and job j09747, a free try from entry
  33601, walked through the door: the ladder's C9 at 01:05 UTC (entry 33633). Its movie, 97 clicks from boot, ends
  the game at frame 30785, tick 182,260 (50:38 of game time), verified from boot by the bridge's own launch and by a
  standalone one.
- **Shortened, engine-judged (`compress.py`).** Each test replays the whole movie from boot (1.2 s) and keeps a change
  when the game ends at the same site no later: a click dropped, else moved with every later click as early as it can
  go. The laptop's 89 clicks: 23324 frames to 19584 (pass 1: 17 clicks dropped, one wait of 3,550 frames removed), to
  19518 with geometric probes (71 clicks, tick 114,658: 31:51 of game time, from 38:11).
- **Not used:** `e016 g4 --mode chain` re-realized the C0–C8 chain's steps from boot and missed 14 of 23 (walks and
  dialog lines realize does not make), so the chain drifted and stopped in room 49.
