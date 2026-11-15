# E017: from C4 to C5, with the plan limit raised under the per-reading lock

## Question

play9 (E016) reached C4: room 49 holding object 946. It stopped there after about 110 min of run wall with no new
rung. Does play9's archive reach C5 (room 42 holding 946) when the plan limit is raised and nothing else changes?

## Baseline

- **play9's archive:** 4,176 entries, all verified at the rungs and in a sample.
  - The 137 entries holding 946 are all on the opening branch that leads to room 42 (bit:401=0): 58 in room 49, 3
    in 63 and 76 in 69.
  - play8 reached room 42 from that branch, without 946, through a room-49 conversation. A model plan from the
    conversation's end state (bit:403=1, a 141-frame plan) came first, then a free try from its checkpoint.
- **The plan limit:** in play9, 3 of 2,341 found plans took more than 30 s. One is j00110's 152 s replan, which took
  the conversation deepest (var:416=2).
  - (12) then capped every later plan at 30 s, and 3 replans timed out at 30-31 s. No new deep conversation state
    appeared after the cap.
  - The cap was set because one long search held every worker. Since (9), each reading's planner has its own lock
    (`FMI_PLANNER_LOCK=reading`), so a long search holds only the tries on its own reading.

## Method

- Resume play9's archive (a copy: `e017-archives/play10`), with E016's final set and one change: `FMI_PLAN_TIME=300`
  and `FMI_PLAN_DEEPEN=60`.
- No other change to code or switches. A defect found during the run is recorded, not fixed (no patching in E017).
- The run is scored by E016's outside ladder.
- Where:
  - on the cloud VM (n2-custom-30, b = 10, 2 readings in the background, as play9), if the budget allows;
  - otherwise on the laptop with b = 4.
- **The wall limit:** 3 h. **The stop rule:** 40 min of run wall with no new rung, counted across resumes; a resume
  does not restart the count.

## Predictions (before running)

- P(C5 within the 3 h) 25-40%.
- P(a new conversation-depth entry holding 946, var:416 != 0) 50-70%.
- Plans over 30 s in the run: 3-15. Any single try blocked over 5 min by a plan: 0, by (9).
- If C5 is reached, C6 (946 used on the suit in room 42, the step G3 realized offline) follows within 30 min: 50-70%.

## Success threshold

- **Yes:** C5, verified from boot.
- **Partly:** a new conversation-depth entry holding 946, but no C5.
- **No:** neither.

## Stop rule

- The run stops at the wall limit, after 40 min without a new rung, or at the disk floor.
- If the result is "no", the next step is to go back to the design: the retrospective's open question of what the
  search is missing, not another limit.

## Cost

- VM: about 3.5 h at $1.535/h, about $5.40, which needs the cap raised (the ledger has $0.84 left).
- Laptop instead: free, at b = 4, while the machine has memory to spare.

## Result (2026-11-05): no; C4 again, and the raised limit bought no plan

- **The run:** VM, b = 10, from play9's archive as play10, 01:41-02:23 UTC.
  - It made 1,126 tries and 507 new entries (4,683 in all).
  - It stopped on the 40-minute rule: no new rung, furthest C4.
  - No entry holds 946 at conversation depth (var:416 != 0).
- **Place selection spread the tries as designed:** 90-98 tries per (room, holds 946) place and kind.
- **The places holding 946 yielded almost nothing:**

  | Start place | New entries |
  |---|---:|
  | room 49 holding 946 (191 tries) | 3 |
  | room 63 holding 946 | 4 |
  | room 69 holding 946 | 0 |
  | room 49 without 946 | 482 |

- **Model tries from room 49 holding 946 made no plan probes:** the planner was given no target there.
- **The raised limit:** 10 searches ran longer than 30 s (4 at about 300 s, 6 at about 60 s), and all 10 ended in a
  planner timeout. No search over 30 s found a plan.
- **Stall shares over the archive's whole run** (play9 and play10): interface, no yield 66.7%; model, no yield 15.0%;
  no plan 10.5%; productive 0.5%.
- **Predictions:**

  | Prediction | Result |
  |---|---|
  | P(C5) 25-40% | no (the likelier side) |
  | P(a conversation-depth entry holding 946) 50-70% | no (the less likely side, a miss) |
  | Plans over 30 s: 3-15 | 10, all timeouts |
  | No try blocked over 5 min by another's plan | not measured |

- **Success threshold:** no.
- **Cost:** about 0.85 h of VM (the ledger shows $29.95 of the $36.00 cap).

**What it shows:** the limit was not the block. From "room 49 holding 946" the model chooses no target that leads into
the conversation, and free tries do not cross a conversation 4-5 inputs deep. In play8 the crossing came from a model
plan for bit:403=1 at the conversation's end state. play10 never reached that state while holding 946, so it never had
that target. By the stop rule, the next step is design: what the search has to see to enter a conversation with an
object in hand, which is the retrospective's open question.
