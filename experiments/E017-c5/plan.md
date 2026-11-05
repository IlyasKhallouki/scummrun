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
