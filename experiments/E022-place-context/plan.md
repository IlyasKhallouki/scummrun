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
