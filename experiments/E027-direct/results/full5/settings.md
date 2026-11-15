# full5: the run's settings

Reconstructed on 2026-11-15 from the code and scripts the run used (chain.py at commit 2eab709, env.sh, the VM's
run script `vm/full5-run.sh`), because chain.py did not record its commands then (it does now: each leg's
`cmd.txt`). The search's code is unchanged since commit be09fea (`git log be09fea..HEAD -- experiments/E011-speed/rs`
is empty).

## Machine and binaries

- VM: Google Cloud n2-custom-30-49152 (30 vCPU, 48 GB), standard provisioning, us-central1-a; 2026-11-15 14:04-19:38 UTC.
- Search: `direct` built on the VM from the worktree at be09fea (`cargo build --profile fast -p fmi-direct`), copied to
  `bench/bin/high8`. The laptop's build of the same commit: sha256 2778e57ed5cdc9e9e3302de476d0eaf4eb2f1b94df8d5a3f97e77acb7e0f4dab.
- Engine: ScummVM v2026.3.0 with the bridge, `svm-e027` (patches 0001-0011) plus patch 0013 on the VM; 0013 acts only
  through the setting `speedrun_skip_early`, which Monkey Island's runs do not set. The gate (below) gave the
  laptop's digests on this engine.
- Seed 1, boot parameter 0 (the host's defaults).

## Game data

The DOS CD release (ScummVM id `monkey`, CD, DOS, English). Not in the repository. The hashes are of the laptop's copy, from
which the VM's was copied.

| file | bytes | sha256 (first 16) | md5 |
|---|---|---|---|
| MONKEY.000 | 8,955 | 8f40364323a755b1 | 2d1e891fe52df707c30185e52c50cd92 |
| MONKEY.001 | 4,779,713 | d9c4098404e73fde | d074150de681d49ff350e8ad60ee4abd |
| track2.flac | 9,322,204 | 6fec19076fdd2604 |  |
| track3.flac | 7,597,091 | db1eb23f78acb4ca |  |
| track4.flac | 7,387,023 | e45251a435785b88 |  |
| track5.flac | 12,097,541 | 013304433f75498e |  |
| track6.flac | 11,255,439 | 2e62dd8d226b08b7 |  |
| track7.flac | 919,605 | a199b9a634108adc |  |
| track8.flac | 5,642,125 | 8cac00dd250f0a00 |  |
| track9.flac | 11,198,059 | 9cb079262ef76fd7 |  |
| track10.flac | 12,204,058 | ea5b5c0041d49ad9 |  |
| track11.flac | 7,056,942 | f9c934602604cf2b |  |
| track12.flac | 12,424,392 | a577d3b1b0196723 |  |
| track13.flac | 9,586,564 | 56a7d616ea86f6ba |  |
| track14.flac | 1,368,356 | 06d7a721f9c47f5c |  |
| track15.flac | 11,338,002 | 4f2a1078c67532f4 |  |
| track16.flac | 11,765,365 | 4659b8bd7ce48768 |  |
| track17.flac | 16,815,678 | 16a0f9d5f42d949f |  |
| track18.flac | 12,585,859 | f8cc9844924cb88b |  |
| track19.flac | 13,804,620 | d184ac79f8c1d080 |  |
| track20.flac | 1,399,079 | 97cc6a16fe273285 |  |
| track21.flac | 1,242,239 | e79e7e90a0417175 |  |
| track22.flac | 12,087,755 | b951a27a90fbc4f0 |  |
| track23.flac | 11,880,529 | 623f2c101d666ae2 |  |
| track24.flac | 7,610,300 | b5e1857dc8589d69 |  |
| track25.flac | 9,927,216 | 0ee0e9827b60282e |  |

Scripts' dump (`scripts/dump-scripts.sh`: scummrp and descumm on the game's files): 1752 files, digest
5591886cbf7124811161095ebe9ad669361e1ff897d478da49048cabec79863e (sha256 over each file's relative path and sha256, sorted). Not in the repository.

## The guide and the evaluation files

- `--guide`: `guide-part1.json` (sha256 8e57c4111b76524d53bd81b5b45a95bd36aa3d47031d9b5dfa3ea4b614bee400), committed as `experiments/E027-direct/guide/guide-part1.json`.
  The search reads only its 805 key names (`main.rs`, `--guide`). `guide-end.json` (`fmi guide map --goal end=1`, the
  game's own end, no game number given) has the same 805 key names; it is the default from 2026-11-15 on.
- `--rungs`: `rungs-part1.json` (sha256 5e8ceb630ff6f19722863a774e29e6fce30968412b552a1efa13f7e51702d3b2); `--watch` and `--until`: below. Rungs and watches are read only
  to report; `--until bit:565=1` ends the run.

## Each leg's command

```
/var/tmp/fast-mi/direct/bench/bin/high8 /var/tmp/fast-mi/direct/full5/legNN --minutes <hours left> --stall 30 --workers 8 --slices 8 --batch 1 --at-once 1 --guide /home/ilyask/.cache/fast-mi-e026/size-cost/guide-part1.json --rungs /var/tmp/fast-mi/wt-direct/experiments/E011-speed/rs/fmi-checks/rungs/rungs-part1.json --watch room=20,room=12,room=69,room=39,room=70,room=75,room=83,room=59,owner:901=1,owner:902=1,owner:903=1,owner:761=1,owner:823=1,owner:733=1 --until bit:565=1 --lines gains --flag-lines --shares --go --motion-once --count-lines --stay-lines --born-first --walk-born --runs-on --counts-world --ends --said-flags --high-lines --arrivals
```

| leg | added to the command |
|---|---|
| leg00 | `(from the boot)` |
| leg01 | `--from /var/tmp/fast-mi/direct/full5/leg00/top.jsonl 15923 --edges /var/tmp/fast-mi/direct/full5/leg00/edges.jsonl --toggles /var/tmp/fast-mi/direct/full5/toggles-00.txt` |
| leg02 | `--from /var/tmp/fast-mi/direct/full5/leg01/top.jsonl 11189 --edges /var/tmp/fast-mi/direct/full5/leg01/edges.jsonl --toggles /var/tmp/fast-mi/direct/full5/toggles-01.txt` |
| leg03 | `--from /var/tmp/fast-mi/direct/full5/leg02/top.jsonl 22320 --edges /var/tmp/fast-mi/direct/full5/leg02/edges.jsonl --toggles /var/tmp/fast-mi/direct/full5/toggles-02.txt` |

keep.py watched the chain with `KEEP_HOURS=11 KEEP_STALL=30 KEEP_WORKERS=8` and started no continuation.

## The environment

`env.sh`'s `ON` and `MI` lists, as given to `direct` (the host passes on only the engine settings it reads; see the
study guide's chapter 09):

```
FMI_ENGINE_CUTS=1
FMI_PLAY_BATCH=1
FMI_HIT_CACHE=1
FMI_STONE_KEEP=1
FMI_PLAN_FAST=1
FMI_PLAN_LAMA=rust
FMI_READER_PREP=1
FMI_IDLE_CHANGES=1
FMI_PLAYER_DONE=1
FMI_TRY_MEMO=1
FMI_PLANNER_KEEP=1
FMI_PLAY_KEEP=1
FMI_PLAN_DEAD=1
FMI_PLAN_DEEPEN=60
FMI_WAIT=1
FMI_TARGET_PER_KEY=1
FMI_DELAY_FORK=1
FMI_AFFORD=1
FMI_SELECT=progress
SPEEDRUN_REPORT_RESTART=1
FMI_BOXES=1
FMI_READER_LIB=1
SPEEDRUN_IDLE=handler
SPEEDRUN_END_SITE=1
FMI_GOAL_END=1
FMI_READ_EVENTS=1
FMI_EGO_VALUES=1
FMI_MERGE_DEDUP=1
FMI_READ_CUT=1
FMI_STAG_SWEEP=1
FMI_VERBS_VAR_ON=1
FMI_NOVEL_VERBS=1
FMI_NOVEL_GUIDE=1
SPEEDRUN_INPUT=1
FMI_REFUSE=1
FMI_STALLS=1
FMI_FREE=0.25
FMI_FREE_SHIFT=1
FMI_READ_BOUND=300
FMI_READ_BOUND_BG=0
FMI_READ_WIDEN=1
FMI_STAG_CELLS=1
FMI_ARGS_LATER=1
FMI_READ_SOUND=1
FMI_OFFER_SCREEN=1
FMI_GOAL_FACT=1
FMI_SCOPE_GROW=room
FMI_REALIZE=1
FMI_NOGOOD=1
FMI_FREE_T=archive
FMI_GOAL_SITE=verb
FMI_FRONTIER=1
FMI_NOVEL_PLACE=1
FMI_SELECT_PLACE=1
FMI_PLAN_TIME=300
FMI_REACH_ZEROS=1
FMI_PLANNER_LOCK=reading
FMI_TARGET_PLACE=1
FMI_REVEAL=1
FMI_PLACE_RARITY=1
FMI_FREE_SAME=1
FMI_SKIP_SAME=1
FMI_FREE_NO_STOPS=1
FMI_REALIZE_HINTS=1
FMI_READ_HELD=1
FMI_READ_SHARE=1
FMI_IMUSE_TICKS=1
FMI_DIFFERS_DEAD=1
FMI_SKIP_SOUNDRESULT=1
FMI_READ_MAX_RSS_GB=40
FMI_TEMP_SAVES=1
FMI_PLACE_CTX=1
FMI_REFINE_WIDEN=3
FMI_SLOT_LOCALS=1
FMI_PLAN_FOCUS=1
FMI_READ_INPUT_FACTS=1
FMI_FRONTIER_ATOMS=1
FMI_READ_EXITS=1
FMI_READ_PARENTS=1
FMI_GOAL_UNPLANNED=1
FMI_READ_INVENTORY_SUMMARY=1
FMI_FRONTIER_FRESH=0
FMI_SELECT_ADVANCED=1
FMI_READ_OBJ_ROOM=1
FMI_SELECT_CONTINUE=1
FMI_READ_MERGE=1
FMI_READ_B_ARGS=1
FMI_READ_INV_B=1
FMI_READ_RENAME=1
FMI_BAN_TRY=1
FMI_READ_TRIM=1
FMI_REFINE_CHECKS=1
FMI_REALIZE_QUEUED=1
FMI_READ_B_SWAP=1
FMI_READ_QUEUE=1
FMI_READ_STRING_SUMMARY=1
FMI_READ_SWITCHES=value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost,wait_both
FMI_SELECT_FURTHEST=1
FMI_MEM_FILES=1
FMI_GAME_ID=monkey
FMI_GAME_DIR=/var/tmp/fast-mi/wt-direct/game/dos-cd
FMI_GAME_SCRIPTS=/var/tmp/fast-mi/wt-direct/data/scripts-dos-cd
```

