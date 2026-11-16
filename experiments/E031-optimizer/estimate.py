#!/usr/bin/env python3
"""E031: the calculated estimate of how fast the optimizer can make full5's route.

Plain Python 3, standard library only.  Run:  python3 estimate.py
Every number carries a tag:  M measured (source), D derived (formula),
A assumed (range and reason), MR a measured range that is drawn, not assumed.

The model in one line:   T = R - G
  R  the route's time when its structure is brought toward the hand no-slide
     route (re-timing, intro, re-routing, in-visit execution, frame length,
     draws, the ending).  At its best R equals the no-slide hand run, 28:01.4.
  G  what is gained beyond that run (luck, tuning, walking, the item slide and
     its skips).  With every hand-known gain G is the gap to the slide run.
R and G share two stages: the delay scan (p_luck) and the walk stage (p_walkopt).
Given those two, R and G are independent: they are drawn separately within each of
the four strata and every draw of one is combined with every draw of the other, which
resolves very small odds; the strata are then mixed by their probabilities.
Parameters are drawn once as columns; a pinned parameter replaces its column,
so all pinned runs use the same random numbers.
"""
import bisect
import math
import random
import statistics
import time

T_START = time.time()
SEED = 31
N_MAIN = 150000       # samples of R (each stratum) and of G in the main run
N_ENDS = 30000        # samples for the all-pessimistic / all-optimistic / flat runs
N_SENS = 6000         # samples per pinned run of the sensitivity tables
G_BIN = 0.1           # seconds: G is binned this finely when the pairs are combined

# --------------------------------------------------------------------------
# 0. Parameter registry: every number of the model, with its tag
# --------------------------------------------------------------------------
REG = {}


class Par(object):
    __slots__ = ("name", "tag", "lo", "c", "hi", "good", "text", "meas", "side")

    def __init__(self, name, tag, lo, c, hi, text, good=None, meas=None, side=None):
        self.name, self.tag, self.lo, self.c, self.hi = name, tag, lo, c, hi
        self.text, self.good, self.meas, self.side = text, good, meas, side
        REG[name] = self

    def end(self, optimistic):
        return self.hi if (self.good == "hi") == optimistic else self.lo


def M(name, v, text):
    Par(name, "M", v, v, v, text)
    return v


def D(name, v, text):
    Par(name, "D", v, v, v, text)
    return v


def MR(name, lo, c, hi, text):
    Par(name, "MR", lo, c, hi, text)


def A(name, lo, c, hi, good, side, text, meas):
    Par(name, "A", lo, c, hi, text, good, meas, side)


# ---- clock, rule, anchors -------------------------------------------------
HZ = M("clock_hz", 239.992156448, "quarter-frames per second; trace boot record, scripts/anypercent_verify.py")
T_NOSLIDE = M("hand_noslide_s", 1681.413, "28:01.4, runs/anypercent/verify-b1/verify.json (CD track 6 present)")
T_SLIDE6 = M("hand_slide_track6_s", 1391.320, "23:11.320, runs/anypercent/m2-verify/verify.json")
T_SLIDE = M("hand_slide_s", 1261.483, "21:01.483, runs/anypercent/m2n-verify/verify.json (track 6 removed)")
T_RECORD = M("human_record_s", 1312.4, "21:52.4, docs/anypercent/overnight.md")
THRESH = [("60:00", 3600.0), ("30:00", 1800.0), ("28:01.4", T_NOSLIDE),
          ("23:11.3", T_SLIDE6), ("21:01.483", T_SLIDE)]

# --------------------------------------------------------------------------
# 1. Segments (M: <tstruct>/segments.json, rep4.py).  Seconds.
#    F full5, N hand no-slide (28:01.4), L hand slide (21:01.483)
#    each: (total, waiting, walking, cutscene+text, acting)
#    DN: against N: (excess waiting, detour, detour low, detour high,
#        detour walking, detour cutscene+text, detour acting)
#    WIN: full5 waiting that lies inside the extra visits (rep4.py)
# --------------------------------------------------------------------------
SEG = [
    # id     label                               F                                      N                                    L                                   DN                                                       WIN
    ("S00", "boot or logo key -> dock",          (263.13, 0.0, 0.1, 262.6, 0.3),        (0.42, 0.0, 0.1, 0.2, 0.2),          (0.42, 0.0, 0.1, 0.2, 0.2),         (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),                    0.0),
    ("S01", "-> rungs P1+P2",                    (968.54, 551.2, 280.0, 127.0, 10.0),   (82.74, 15.2, 60.2, 6.0, 1.5),       (82.74, 15.2, 60.2, 6.0, 1.5),      (536.05, 317.74, 285.11, 351.81, 214.23, 95.52, 7.99),  458.0),
    ("S02", "-> rungs P3+P4+P6",                 (2176.0, 1257.1, 556.4, 335.6, 26.5),  (214.4, 4.7, 135.0, 69.8, 4.8),      (214.4, 4.7, 135.0, 69.8, 4.8),     (1252.47, 608.95, 476.72, 729.62, 372.49, 218.31, 18.16), 876.8),
    ("S03", "-> rung P5",                        (275.81, 170.0, 82.3, 20.2, 3.3),      (49.33, 0.7, 28.0, 19.8, 0.7),       (49.33, 0.7, 28.0, 19.8, 0.7),      (169.34, 60.35, 55.9, 64.8, 45.7, 12.45, 2.2),          137.8),
    ("S04", "-> rung P7",                        (1202.77, 622.8, 218.0, 353.2, 8.7),   (91.84, 15.3, 44.1, 30.9, 1.6),      (80.59, 4.1, 44.2, 30.8, 1.6),      (607.49, 539.88, 514.12, 559.95, 195.65, 336.65, 7.57), 591.5),
    ("S05", "-> rung P8",                        (813.06, 439.4, 244.2, 117.7, 11.7),   (106.25, 11.5, 39.4, 54.1, 1.3),     (103.34, 10.6, 37.4, 54.1, 1.3),    (427.91, 279.41, 267.71, 291.11, 180.46, 89.6, 9.35),   382.4),
    ("S06", "-> rung P9",                        (47.1, 12.9, 24.8, 8.9, 0.5),          (20.0, 0.3, 16.0, 3.6, 0.2),         (19.17, 0.3, 15.2, 3.6, 0.2),       (12.65, 23.0, 22.2, 23.8, 16.7, 5.95, 0.35),            4.4),
    ("S07", "-> rungs P10-P13",                  (1137.01, 539.8, 355.3, 227.2, 14.6),  (218.42, 3.3, 146.4, 65.5, 3.2),     (226.33, 3.8, 152.1, 66.9, 3.4),    (536.57, 263.06, 212.41, 307.41, 178.93, 76.9, 7.23),   182.9),
    ("S08", "-> Part II begins",                 (31.46, 5.1, 18.5, 7.3, 0.3),          (35.0, 0.3, 14.1, 20.2, 0.3),        (23.16, 0.0, 5.4, 17.5, 0.0),       (4.85, 21.1, 21.1, 21.1, 18.3, 2.6, 0.2),               3.7),
    ("S09", "-> Part III begins (Part II)",      (509.02, 345.6, 108.8, 39.6, 15.0),    (74.25, 4.9, 37.3, 25.1, 7.0),       (64.92, 1.8, 40.1, 17.4, 5.7),      (340.69, 86.7, 54.9, 118.8, 58.36, 20.31, 8.03),        180.4),
    ("S10", "-> object 884 owned",               (376.31, 178.7, 129.2, 59.7, 8.8),     (16.75, 0.4, 12.1, 3.7, 0.7),        (14.58, 0.3, 11.1, 2.8, 0.5),       (178.29, 177.27, 175.41, 179.11, 113.64, 55.85, 7.78),  159.1),
    ("S11", "-> object 269 owned",               (2132.37, 958.1, 925.4, 207.2, 41.5),  (298.06, 10.0, 199.0, 80.0, 9.0),    (108.89, 2.3, 80.5, 22.8, 3.3),     (948.13, 774.24, 616.22, 923.23, 631.38, 114.53, 28.33), 586.4),
    ("S12", "-> room 39 entered",                (122.5, 39.4, 65.9, 14.7, 2.4),        (69.17, 0.3, 55.4, 11.9, 1.5),       (61.17, 0.2, 49.2, 10.5, 1.3),      (39.15, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),                  0.0),
    ("S13", "-> room 70 entered",                (103.0, 74.5, 25.2, 1.8, 1.5),         (46.5, 15.9, 24.4, 5.1, 1.1),        (6.58, 1.0, 4.6, 0.4, 0.6),         (58.59, 10.53, 4.5, 22.0, 9.63, 0.27, 0.63),            26.7),
    ("S14", "-> owner 823",                      (450.22, 269.1, 111.2, 61.4, 8.5),     (84.44, 6.3, 42.2, 32.4, 3.5),       (64.02, 0.9, 34.8, 25.8, 2.6),      (262.78, 77.03, 61.1, 93.6, 59.26, 14.07, 3.71),        124.5),
    ("S15", "-> last part's card",               (46.6, 9.0, 20.5, 16.2, 0.8),          (27.25, 0.4, 16.3, 9.8, 0.6),        (26.17, 0.2, 15.5, 9.8, 0.6),       (8.58, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),                   0.0),
    ("S16", "-> end flag (bit 565), Part IV",    (310.29, 53.6, 23.3, 231.6, 1.7),      (206.25, 1.0, 14.2, 189.9, 1.1),     (75.24, 0.8, 13.5, 59.9, 1.0),      (52.6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),                   0.0),
    ("S17", "end flag -> final fade",            None,                                  (40.27, 0.0, 0.0, 40.3, 0.0),        (40.36, 0.0, 0.0, 40.3, 0.0),       None,                                                   0.0),
]
MID = SEG[1:17]            # S01..S16, the part the optimizer works on

CARD_N = M("card_noslide_s", 129.92, "the last part's card in the no-slide run (segments.json card_s); waits for CD track 6")
CARD_F = M("card_full5_s", 130.0, "the same card in full5 (also the first part's card in its intro)")
CARD_OFF = M("card_no_track6_s", 0.167, "the card with track 6 missing, m2n-verify card_s")
TRACK6 = M("track6_s", 129.84, "23:11.320 - 21:01.483, the trace shift of 31,160 qf")
S00_SKIP = M("s00_intro_skipped_s", 0.42, "logo key to dock in both hand routes (three keys, frames 46-51)")
S00_PLAY = M("s00_intro_played_s", 167.7, "full5's S00 from the logo room's own end (frame 961): next room 37.5 s + first card 130.0 s")
S17_N = M("s17_ending_s", 40.27, "end flag to final fade in the no-slide run, all cutscene and text")

# ---- totals over S01..S16 (D: sums of the rows above) ----------------------
FW = sum(s[2][1] for s in MID)
NW = sum(s[3][1] for s in MID)
WIN = sum(s[6] for s in MID)                       # waiting inside extra visits
WMX = FW - WIN - NW                                 # excess waiting inside matched visits
FK = sum(s[2][2] for s in MID)
FC = sum(s[2][3] for s in MID) - CARD_F             # card taken out, added back as a constant
FA = sum(s[2][4] for s in MID)
NK = sum(s[3][2] for s in MID)
NC = sum(s[3][3] for s in MID) - CARD_N
NA = sum(s[3][4] for s in MID)
DK = sum(s[5][4] for s in MID)
DC = sum(s[5][5] for s in MID)
DA = sum(s[5][6] for s in MID)
DTOT = sum(s[5][1] for s in MID)
D_LO = sum(s[5][2] for s in MID) / DTOT
D_HI = sum(s[5][3] for s in MID) / DTOT
N_CLASSED = sum(sum(s[3][1:5]) for s in SEG)
UNCL = T_NOSLIDE - N_CLASSED                        # time outside the main loop, not classed
N_WALK_ALL = sum(s[3][2] for s in SEG)
D("wait_in_extra_share", WIN / FW, "waiting inside extra visits / all waiting = %.1f / %.1f (rep4.py)" % (WIN, FW))
MR("detour_scale", D_LO, 1.0, D_HI, "which visits count as extra: shortest (%.0f s) to longest (%.0f s), centre %.0f s (rep2.py)" % (D_LO * DTOT, D_HI * DTOT, DTOT))

# ---- waiting pools (M: rep3.py) --------------------------------------------
W_PROBE = M("wait_after_probe_s", 2828.2, "idle after a probed trial, 399 stops (the search's 60 + 6 frames)")
W_CALM = M("wait_calm_s", 550.7, "calm after plain trials")
W_INNER = M("wait_between_clicks_s", 1254.3, "idle between the clicks of one trial")
W_XWAIT = M("wait_explicit_s", 649.2 + 243.9, "24 wait-then-act trials + 9 pure wait trials (50, 200, 800 frames)")
W_ALL = W_PROBE + W_CALM + W_INNER + W_XWAIT
R_ART = D("r_artefact", 1.0, "share of probe and calm idle removed: 1, these frames are the search's own, the game is idle (bridge)")

# ---- re-routing pools (M: the graph report over nodes.jsonl) ----------------
STEPS_WALK = M("frames_roomchange_nogain", 34181.0, "403 room-change steps with no gain on the end path (re-counted from path.pkl: 403 steps, 34,181 frames)")
DIRECT_WALK = M("frames_direct_walks", 13280.0, "the same travel by entry-keyed direct walks measured in the tree (12,988-16,238 by admitted transitions)")
DROP_POOL = M("frames_inroom_left", 16808.0, "279 in-room steps with no gain, in rooms left without progress (padding removed)")
DROP_SIGHT = M("frames_inroom_sight", 2822.0, "of those, 72 steps that only walk or look")
TPF = M("ticks_per_busy_frame", 6.098, "full5 after the dock (rep5.py)")
BUSY_RC = M("busy_s_roomchange_nogain", 2457.4, "busy seconds in those 403 steps: 24,474 of their 34,181 frames are busy, 9,707 idle (frame log joined with path.pkl; audit)")
B_WALK = D("busy_share_walk_steps", 24474.0 / 34181.0, "24,474 busy / 34,181 frames (audit; the first version took only 6 calm frames a step as idle, 0.929, and so counted 4,456 idle frames that re-timing already removes)")
SEC_PER_FRAME = TPF * 4.0 / HZ
S_WALK = D("pool_direct_walks_s", BUSY_RC * (1.0 - DIRECT_WALK / STEPS_WALK),
           "2,457.4 busy s x (1 - 13,280/34,181): direct walks are tree steps with the same idle share. If they had no idle at all the pool would be about 1,124 s")
Q_SIGHT = D("q_sight_drop", 1.0, "share of walk-or-look steps a drop test passes: 1, they change no world atom (position and time are re-timing's job)")

# ---- frame length ------------------------------------------------------------
FL_F = M("frame_length_full5", 6.0, "ticks a frame in full5 (var 19 at its segment start)")
FL_R = M("frame_length_refs", 5.0, "ticks a frame in both hand routes (the logo glitch)")
SK = D("scale_walk", FL_R / FL_F, "walking at frame length 5 against 6: 785 ticks against 942 on a measured pair, exactly 1 tick a frame (t_logo.py)")
LOGO_WIN = M("logo_window_frames", 47.0, "Esc on boot frames 46-92 leaves the frame length at 5 (t_logo.py)")
LOGO_SCAN = M("logo_scan_frames", 960.0, "frames of the logo room; only Esc of 8 input kinds works")
COV_LOGO = D("cov_logo", 1.0, "scan tries Esc at every logo frame: 47 of 960 tries hit, so at least one hit is certain")
SIG_LOGO = D("sig_logo", 1.0, "signal: the frame-length variable in Status differs from the next frame on; payoff delay 0 frames")

# ---- draws ---------------------------------------------------------------------
KEEPER_AWAY = M("keeper_away_share", 20.0 / 61.0, "store entry frames with the keeper away: 20 of 61 (t_credit.py, t_slide.py)")
N_DELAY = D("delay_scan_frames", 13.0, "delays 0..12 tried at a draw, as the reference's look-ahead")
COV_KEEPER = D("cov_keeper", 1.0 - (1.0 - KEEPER_AWAY) ** N_DELAY, "1 - (1 - 20/61)^13: some scanned entry frame has the keeper away")
MR("q_delay", 0.375, 0.59, 0.83, "a 13-delay scan re-solves one draw point. The first version (7/8) counted d = 0, the reference's own timing, as a working delay. "
   "Without it: low, fixed continuation, a working delay in 1..12 at 2 of 6 points (fights 485-488: none; exits: 5 and 7-10), (2+1)/(6+2); "
   "centre, the last fight draw alone, 2 of 30 delays, 1-(28/30)^13; high, one catacomb exit, 1-(3/4)^6.1 (fight3.json, late.json)")
EXIT_VALUES = M("exit_values", 4.0, "values the pointed-at exit takes over 31 delays (t_late.py)")
CAT_GAIN = D("catacomb_luck_s", SEG[13][3][0] - SEG[13][4][0], "S13: no-slide 46.50 s - slide 6.58 s; the slide run used delay look-ahead there")
MR("rng_values_in_scan", 13 * 12 / 31.0, 13 * 14.5 / 31.0, 13 * 17 / 31.0, "distinct random states in a 13-delay scan: 12 to 17 per 31 delays (t_late.py)")
MR("fight_luck_s", 2.0, 4.5, SEG[4][3][0] - SEG[4][4][0], "fights: 4.5 s from look-ahead (overnight.md); S04 differs by 11.25 s between the hand runs")

# ---- what separates the two hand routes, segment by segment (D) ---------------
MR("setup_slide_s", 20.0, 23.0, 26.0, "slide setup cost: store detour 13.6 + extra talk about 7 + one more item 2.4")
MR("oars_s", 17.0, 18.0, 19.0, "oars skip, from the two runs' goal times")
MR("monkey_s", 90.0, 92.5, 95.0, "monkey skip, from the two runs' goal times")
FEATHER = M("feather_s", 13.8, "feather skip between the two verified runs")
HEAD = M("head_necklace_s", 6.5, "head variant and necklace skip between the two verified runs")
MR("feather_net_s", 0.0, 5.0, FEATHER, "feather skip less the regain it needs (up to 19.9 s from the reading to rest)")
PART2 = D("part2_slide_s", (SEG[8][3][0] - SEG[8][4][0]) + (SEG[9][3][0] - SEG[9][4][0]), "S08 + S09: no-slide - slide")
ORDER_POOL = D("s11_order_tuning_s", (SEG[11][3][0] - SEG[11][4][0]) - REG["monkey_s"].c - REG["oars_s"].c, "S11: no-slide - slide - monkey - oars")
TUNE_FIXED = D("tuning_pool_fixed_s",
               sum(SEG[i][3][0] - SEG[i][4][0] for i in (5, 6, 10, 12, 15))
               + (SEG[16][3][0] - SEG[16][4][0] - TRACK6)
               + (SEG[7][3][0] - SEG[7][4][0] + REG["setup_slide_s"].c)
               + (SEG[14][3][0] - SEG[14][4][0] - FEATHER - HEAD),
               "no-slide - slide in S05, S06, S10, S12, S15; S16 less track 6; S07 plus the setup; S14 less feather and head")

# ---- item slide family: measured windows (M: scratchpad/gf/out/*.json) --------
FLOOR_HIT = M("slide_floor_hit", 11.0 / 240.0, "floor points that form the slide: 11 of 240 grid points (17 of 42 in the right part)")
COV_PAIR = D("cov_slide_pair", 1.0, "9 verbs x 9 slots tried: holds the 2 working pairs of 72")
COV_FRAME = D("cov_slide_frame", 1.0, "25 start frames tried: hold the 5-frame window measured on full5's own stop (20 frames at slow text)")
SIG_SLIDE = D("sig_slide", 1.0, "signal: input counter above rest; 0 such frames on full5 before its end flag, so no threshold")
N_KEEP = M("carry_readings", 6.0, "deliberate readings that carry the slide through the end of Part I (85 events, 27 needed, 13 frame-exact)")
C12_OARS = D("cov_oars_clicks12", 1.0, "14 frames x 128 points holds 52 hits; 14 x 104 holds 84 hits")
C4_OARS = D("cov_oars_click4", 1.0, "61 frames tried, 12 hit")
KEEP_SPACE = D("keep_space_tries", 72.0 * 33 * 23 * 31 / 4.0, "tries per hit to hold a reading across a dialogue if all four inputs were scanned jointly and blind: 72 pairs x 33 x 23 x 31 frames / 4. "
               "Not used: the hold is found step by step on the counter, the same mechanism as the carry's keep-alive readings (sig_keep); about 72 x 33 + 23 + 31 tries")
COV_FEATHER = D("cov_feather", 1.0, "9 verbs x 61 x 61 frames is under 34,000 tries; the click is 2 of 61 frames, any pixel; ordinary progress shows it")
MONKEY_REST = D("monkey_rest_tries", (61 * 80 / (4.0 * 63)) * (61 / 3.0) * (61 / 13.0), "tries per hit for the three inputs after the monkey reading: 1,847, within any budget")

# ---- walking beyond the hand route (E025) -------------------------------------
MR("walk_gain_rate", 0.028, 0.060, 0.072, "re-click gain as a share of player-walk ticks: 2.8% from full5's own steps, 6.0% rule and 7.2% beam in E025's probes")

# ---- E024 anchors (used as reasons, and in the cross-checks) -------------------
M("e024_drops_kept", 3.0 / 110.0, "E024 drop round 1: 3 of 110 candidates kept, 546 ticks (3.9%)")
M("e024_retime_ratio", 14152.0 / 182260.0, "E024 forward pass on the Fate of Atlantis demo: 182,260 -> 14,152 ticks")
M("e025_survival", 0.25, "E025: before a draw 1 local gain in 4 survived")

# ---- assumed parameters: R side (structure) ------------------------------------
A("p_retime", 0.90, 0.97, 0.995, "hi", "R", "the re-timing pass ends with a whole verified route. E024 did it on another game (182,260 -> 14,152 ticks); draws are the risk",
  "run the re-timer with nothing removed, then on the first 100 input groups (plan stage 0 and G4): minutes")
A("r_inner", 0.80, 0.95, 0.99, "hi", "R", "share of idle between one trial's clicks removed. 264 s of the 1,254 s is certain (581 click pairs at gaps of 3 and 9 frames)",
  "the same first pass: idle frames left per group")
A("r_xwait", 0.60, 0.85, 0.97, "hi", "R", "share of the 33 explicit waits removed. They come in 50, 200, 800 frames; one stop reached the same state 806 frames sooner without its wait",
  "padding ladder on the 33 wait trials alone: 33 x 4 forks, seconds")
A("p_exec", 0.50, 0.75, 0.90, "hi", "R", "a trial can be made again by its name from another place and state (the executor). E024's pass stopped at step 5 of 12 on Monkey Island walking; E025's fixes are designed, not built",
  "make 30 of the route's trials again after a direct walk from another room: 30 forks, under a minute once the executor exists")
A("v_walk", 0.60, 0.90, 1.15, "hi", "R", "share of the direct-walk pool realised. Admitted transitions give 0.86-1.01 of it, best measured walk costs 1.31; some walks will not exist at the later facts",
  "check each of the route's direct walks once in the engine (about 200 forks of under 1 s)")
A("p_drop", 0.60, 0.85, 0.95, "hi", "R", "drop tests with re-timing of the rest work at scale. E024's drops ran but cost the square of the step count",
  "time one drop round on 50 steps; project to the route (G4)")
A("q_atom", 0.20, 0.50, 0.85, "hi", "R", "share of the 207 no-news steps (279 - 72) that changed some world atom and can still be dropped. Unknown: doors and states may be needed",
  "50 single drop tests on such steps: 50 x about 3.6 s = 3 minutes")
MR("b_inroom", 0.653, 0.678, 0.702, "busy share of the droppable in-room steps' frames once their idle tails (padding) are removed: 0.653 over all 377 in-room no-gain steps, "
   "0.702 over the 205 in visits with no gain (frame log joined with path.pkl; audit). The first version assumed 0.3-0.6 (centre 0.4), applying the idle share of whole steps to frames already stripped of padding")
A("p_plan", 0.20, 0.45, 0.70, "hi", "R", "the re-ordering stage exists and verifies. The tree shows independence for 2% of pairs; precedence must be learned by engine tests",
  "single drop tests give direct dependencies (15 CPU-minutes); count how many progress visits may move")
A("y_need", 0.05, 0.30, 0.65, "hi", "R", "share of the remaining detour pool removed by drop tests on progress trials the end does not need. Ceiling 0.65: the 101 of 257 progress trials that set only keys no script tests hold 394 s busy; floor: E024 kept 3 of 110 drops",
  "257 single drop tests of the progress trials: about 15 CPU-minutes")
A("y_order", 0.05, 0.30, 0.90, "hi", "R", "share of what then remains removed by re-ordering, when that stage works. The tree bounds re-ordering at 3,000-7,000 frames (305-711 s); AE+PNGS improved 60 of 270 plans",
  "nearest-neighbour tour over the kept progress visits with measured walks, checked on one Part: minutes")
A("phi_fb", 0.05, 0.18, 0.45, "hi", "R", "detour share removed without the executor, by loop cuts of raw inputs and corridors of the frozen search. The first plan put these at 583 s (167-1,500) and 250 s at 35%; E024 kept 3 of 110",
  "cut the 45 no-gain bounces by raw inputs and re-time: 45 tests")
A("e_walk", 0.30, 0.60, 1.00, "hi", "R", "share of full5's extra walking inside matched visits removed (exit pixels, re-clicks, approach clicks). Needs the walk stage (p_walkopt): 0 without it. "
  "High end 1 = the hand route's quality (the first version capped it at 0.9, so even the best case could not reach the hand route)",
  "E025's direction rule on 20 of full5's walks: about 20 s")
A("e_cut", 0.10, 0.40, 1.00, "hi", "R", "share of full5's extra cutscene, text and acting time inside matched visits removed (shorter dialogue paths, in-visit drops). Needs the executor and drops: 0 without them. "
  "High end 1 = the hand route's quality (was 0.8)",
  "drop tests on dialogue-line clicks in the 10 longest matched visits")
A("u_frame", 0.05, 0.35, 0.90, "hi", "R", "share of non-walking busy time that is paced by frames, not ticks; that time scales by 1 - u x (1 - 5/6). The whole-route saving on the reference is 157 s (walking frames only) to 240 s (all frames)",
  "re-time the first 50 groups twice, with and without the logo key, and compare ticks by class: two short passes")
A("p_logo_surv", 0.50, 0.80, 0.95, "hi", "R", "the route can be re-timed to its end at frame length 5 (tick-paced waits inside a group take more frames; the first plan gave a raw-input re-timer 40%)",
  "the same two short passes, then one whole re-time at frame length 5")
A("p_intro", 0.90, 0.97, 0.995, "hi", "R", "the boot scan also finds the keys that skip the rest of the intro. One Esc at frame 46 was shown to work under the search's settings",
  "greedy Esc scan from the logo key to first control: under 2,700 forks")
A("p_end", 0.90, 0.97, 0.995, "hi", "R", "the two closing menus are answered and the final fade is reached. They come with input on inside a cutscene, where the frozen search never clicks",
  "from full5's end stop, click each menu line: a few forks")
A("iota", 0.0, 0.08, 0.25, "lo", "R", "interactions: share added to whatever excess over the hand route is left, because gains do not add up (E025: 1 local gain in 4 survived before a draw)",
  "compare the sum of single-stage gains with the combined verified route after stages 1 and 2")
A("p_adapt", 0.70, 0.90, 0.98, "hi", "R", "after re-timing, the early safe opening is made again with the count of sentences the new draw needs (20 of 20 keeper-away entries opened)",
  "re-time up to the store, scan 13 entry delays, repeat the handle sentence until the object is owned")
A("pen_credit", 40.0, 58.0, 80.0, "lo", "R", "seconds lost if the early safe opening is lost. 58 s from one runner's two runs on another release, not measured here",
  "none cheap: needs a route without it")
A("q_search", 0.50, 0.80, 0.95, "hi", "R", "the frozen search solves a draw-ruled stretch again from a new random state, started with --from. It solved each once on seed 1",
  "one corridor run of the frozen binary at the catacombs from a shifted stop: 6-15 minutes")
A("n_dp", 7.0, 11.0, 19.0, "lo", "R", "draw points on the route that a re-timed route must solve again (fight choices, catacomb exits; the store entry is Credit Early's own term, so it is not counted twice)",
  "three twin forks with a changed random state per group mark them: about 80 s for the route")
A("pen_draw", 30.0, 120.0, 400.0, "lo", "R", "seconds lost for each draw point not solved again, its stretch left as full5 played it (S13's excess is 96 s, one fight room holds 248 s); multiplied by the expected count of failed points",
  "none before the pass; the pass reports it")

# ---- assumed parameters: G side (gains beyond the no-slide hand route) ----------
A("p_luck", 0.60, 0.85, 0.95, "hi", "G", "the delay scan at draws exists and verifies (twins to mark draws are built in fmi-shorten; the scan is not). Shared with R: without it draw points and the store entry are re-solved by the frozen search alone",
  "delay scan 0..12 with one group of look-ahead at the six catacomb exits of the re-timed route")
A("e_tune", 0.10, 0.40, 1.00, "hi", "G", "share of the tuning gap between the two hand runs that the optimizer closes outside the slide's skips (early inputs gave 4.66 s on 248 goals). High end 1 = the hand tuning (was 0.8)",
  "after stage 1, compare segment times with the references in S05, S07, S12")
A("t_share", 0.30, 0.50, 0.70, "hi", "G", "share of S11's 78.7 s of 'route order and tuning' that needs no skip; the rest needs the monkey skip",
  "none cheap: the two hand runs differ in both")
A("p_walkopt", 0.40, 0.60, 0.85, "hi", "G", "the walk stage (re-clicks, waypoints) gets built and its gains verify from boot. E025 was planned and never built. Shared with R: e_walk needs the same stage",
  "E025's direction rule on 20 walks after the last draw, verified from boot")
A("h_new", 0.40, 0.70, 0.90, "hi", "G", "share of walk gains the hand route does not already hold (it re-clicks on 11 of 532 moves, sweeps 2, hand-steps 7)",
  "run the direction rule on 20 walks of the reference replay (evaluation only)")
A("sv_draw", 0.25, 0.60, 1.00, "hi", "G", "share of walk gains that survive later draws: 1 in 4 before a draw (E025), all after the last draw",
  "count the route's walking time after its last draw point")
A("p_head", 0.10, 0.30, 0.50, "hi", "G", "the head variant and necklace skip: timing only, no state signal; 25 of 77 standing points and a 2-frame window; found only by comparing arrival frames",
  "standing-point scan (77 points) before that sentence on the re-timed route")
A("cov_floor", 1.0 - (1.0 - 11.0 / 240.0) ** 7, 1.0 - (1.0 - 11.0 / 240.0) ** 40, 1.0, "hi", "G",
  "coverage of the floor click that forms the slide: 1-(1-11/240)^n for n = 7, 40 or all 240 grid points per visit",
  "the staged scan at the store visits: about 29,000 tries a visit, under 10 CPU-minutes")
A("p_ts", 0.10, 0.30, 0.60, "hi", "G", "the route between the slide's forming and Part III can be played with readings that last (slow text chosen, no dialogue menu that cannot be bridged)",
  "persistence profile: continue the route from the slide state and count the frames and dialogues it survives: one replay")
A("sig_keep", 0.40, 0.70, 0.90, "hi", "G", "each of the 6 keep-alive readings is found by a stepwise scan that sees the counter stay raised (1 event in 4 moved by a frame left the route in a wrong room)",
  "the same profile, with a 72 x 30 scan at the first point where the slide dies")
A("p_part2", 0.20, 0.50, 0.80, "hi", "G", "with the slide alive, a window scan finds the Part II uses (input inside cutscenes)",
  "only after a carry exists")
A("c3_oars", 0.50, 0.80, 0.95, "hi", "G", "coverage of the third oars click (19-frame window; its click point was not scanned)",
  "grid scan of that click on the reference replay (evaluation only): one launch")
A("s_step", 0.30, 0.60, 0.85, "hi", "G", "each oars click shows a signal the scan rewards (ego absent with input on, a position jump, a room out of order), so the four clicks are found one at a time",
  "only after a carry exists")
A("cov_in_monkey", 0.10, 0.28, 0.60, "hi", "G", "the monkey reading is in the scan's input set: 40 pruned inputs of 72 pairs, times 0.5 for the inventory page it needs",
  "list the pruned input set at that stop and check the pair is in it (evaluation only)")
A("cov_fr_monkey", 0.50, 0.75, 1.00, "hi", "G", "the scan hits the 2-frame window: 1 if every frame is tried, 0.5 if every fourth",
  "a design choice of the scan; cost is 4x")
A("s1_monkey", 0.20, 0.50, 0.80, "hi", "G", "'a timed window held open longer than in its twin' is rewarded as a step, so the hold is kept and the next three inputs searched",
  "only after a carry exists")
A("p_order", 0.20, 0.50, 0.80, "hi", "G", "the route order around the skips is re-made once the monkey skip exists",
  "only after the skip exists")
A("p_regain", 0.10, 0.35, 0.60, "hi", "G", "the slide is formed again at the village by the same scan (not probed; the hand way needs 4 frame-exact text skips)",
  "run the store's staged scan at the village visits: about 10 CPU-minutes")
RHO = 0.0   # common-factor correlation of the success draws within R and within G (table 9b varies it)


def cen(name):
    return REG[name].c


# --------------------------------------------------------------------------
# 2. Calculated probabilities and shares (used by the tables and the samplers)
# --------------------------------------------------------------------------
# Parameters that are the probability of one success draw.  Such a parameter
# enters only through its mean: drawing it from its range and then drawing the
# success gives exactly the same distribution, so the samplers use the mean.
PTYPE = set(["p_retime", "p_exec", "p_drop", "p_plan", "p_logo_surv", "p_intro", "p_end", "p_adapt",
             "p_luck", "p_walkopt", "p_head", "cov_floor", "p_ts", "p_part2", "c3_oars", "s_step",
             "cov_in_monkey", "cov_fr_monkey", "s1_monkey", "p_order", "p_regain"])


def f_wait(r_inner, r_xwait):
    return ((W_PROBE + W_CALM) * R_ART + W_INNER * r_inner + W_XWAIT * r_xwait) / W_ALL


def x_drop(q_atom):
    return (DROP_SIGHT * Q_SIGHT + (DROP_POOL - DROP_SIGHT) * q_atom) / DROP_POOL


def s_drop(b_inroom):
    return DROP_POOL * b_inroom * SEC_PER_FRAME


def p_logo(p_surv):
    return COV_LOGO * SIG_LOGO * p_surv


def p_credit(p_adapt, scan=True, q_search=None):
    """Keeper away at a re-timed store entry: the delay scan tries 13 entry frames; without
    the scan only the entry frame itself, then the frozen search (q_search)."""
    cov = COV_KEEPER if scan else 1.0 - (1.0 - KEEPER_AWAY) * (1.0 - q_search)
    return cov * p_adapt


def p_draws(q_search, n_dp, q_delay, scan=True):
    return (1.0 - (1.0 - q_search) * (1.0 - (q_delay if scan else 0.0))) ** n_dp


def n_failed(q_search, n_dp, q_delay, scan=True):
    """Expected count of draw points not solved again, given that at least one is not."""
    pf = (1.0 - q_search) * (1.0 - (q_delay if scan else 0.0))
    pd = (1.0 - pf) ** n_dp
    return n_dp * pf / (1.0 - pd) if pd < 1.0 else 1.0


def p_form(cov_floor):
    return COV_PAIR * COV_FRAME * SIG_SLIDE * cov_floor


def p_carry(p_ts, sig_keep):
    return p_ts * sig_keep ** N_KEEP


def p_oars(c3, s_step):
    return C12_OARS * c3 * C4_OARS * s_step


def p_monkey(cov_in, cov_fr, s1):
    return cov_in * cov_fr * s1


def p_feather(p_regain, sig_keep):
    return p_regain * sig_keep * COV_FEATHER


ND = statistics.NormalDist()


def _clamp(p):
    return min(max(p, 1e-12), 1.0 - 1e-12)


def corr_all(ps, rho):
    """P(every success draw succeeds) when the draws share one common factor (Gaussian copula, correlation rho)."""
    if rho <= 0.0:
        out = 1.0
        for p in ps:
            out *= p
        return out
    a = [ND.inv_cdf(_clamp(p)) for p in ps]
    s, w, dz, tot = math.sqrt(rho), math.sqrt(1.0 - rho), 0.01, 0.0
    for k in range(-800, 801):
        z = k * dz
        v = ND.pdf(z) * dz
        for ai in a:
            v *= ND.cdf((ai - s * z) / w)
        tot += v
    return tot


def corr_u(us, uz, rho):
    """Uniforms that share the common factor uz: same margins, pairwise normal correlation rho."""
    s, w, z = math.sqrt(rho), math.sqrt(1.0 - rho), ND.inv_cdf(_clamp(uz))
    return [ND.cdf(s * z + w * ND.inv_cdf(_clamp(u))) for u in us]


def cov_exit(n_rng):
    return 1.0 - (1.0 - 1.0 / EXIT_VALUES) ** n_rng


# --------------------------------------------------------------------------
# 3. The structure side R
# --------------------------------------------------------------------------
EXK = FK * SK - NK                 # full5's excess walking at frame length 5 (D)
DKS = DK * SK                      # the detour's walking at frame length 5 (D)


def _time(phi, f, e_w, e_c, logo, intro, pen, pk, exc, pc, exa, pa, sc, it):
    """Seconds with track 6 present.  Busy time is handled per class at frame
    length 5: N is at 5 already; what full5 adds is the part inside extra
    visits (pk, pc, pa: removed by re-routing) and the part inside matched
    visits (removed by execution).  If the logo glitch is not found
    everything is scaled back to frame length 6."""
    q1 = 1.0 - phi
    xk = (EXK - pk) * (1.0 - e_w) + pk * q1
    xca = (exc - pc + exa - pa) * (1.0 - e_c) + (pc + pa) * q1
    if logo:
        r = NK + NC + NA + (xk + xca) * it + S17_N
    else:
        r = (NK + xk * it) / SK + (NC + NA + xca * it + S17_N) / sc
    return r + (S00_SKIP if intro else S00_PLAY) + NW + (WIN * q1 + WMX) * (1.0 - f) * it + CARD_N + UNCL + pen


def no_track6(r, intro):
    return r - CARD_N + CARD_OFF - (0.0 if intro else CARD_F - CARD_OFF)


def route_R(f, phi, e_walk, e_cut, u, ds, iota, logo, intro, pen=0.0):
    """The same as a plain function of the removal shares: (track 6, no track 6)."""
    sc = 1.0 - u * (1.0 - SK)
    pk = min(DKS * ds, EXK)
    exc, exa = FC * sc - NC, FA * sc - NA
    pc, pa = min(DC * ds * sc, exc), min(DA * ds * sc, exa)
    r = _time(phi, f, e_walk, e_cut, logo, intro, pen, pk, exc, pc, exa, pa, sc, 1.0 + iota)
    return r, no_track6(r, intro)


# --------------------------------------------------------------------------
# 4. Samplers.  Parameters are drawn once as columns; a pinned parameter
#    replaces its column, so every pinned run uses the same random numbers.
# --------------------------------------------------------------------------
def spec(name, pin, flat):
    p = REG[name]
    if pin is not None and name in pin:
        return (pin[name], pin[name], 0.0, 0.5)
    span = p.hi - p.lo
    if span <= 0.0:
        return (p.lo, p.lo, 0.0, 0.5)
    if flat and p.tag == "A":                   # a flat range instead of a triangle
        return (p.lo, p.hi, span, -1.0)
    return (p.lo, p.hi, span, (p.c - p.lo) / span)


def pmean(name, pin, flat):
    p = REG[name]
    if pin is not None and name in pin:
        return pin[name]
    return (p.lo + p.hi) / 2.0 if flat else (p.lo + p.c + p.hi) / 3.0


def column(name, n, rnd, pin, flat):
    lo, hi, span, cf = spec(name, pin, flat)
    us = [rnd() for _ in range(n)]              # always drawn, so the streams stay aligned
    if span == 0.0:
        return [lo] * n
    if cf < 0.0:
        return [lo + span * u for u in us]
    a, b, sq = span * math.sqrt(cf), span * math.sqrt(1.0 - cf), math.sqrt
    return [lo + a * sq(u) if u < cf else hi - b * sq(1.0 - u) for u in us]


R_DRAWN = ["r_inner", "r_xwait", "v_walk", "q_atom", "b_inroom", "y_need", "y_order", "phi_fb", "e_walk",
           "e_cut", "u_frame", "iota", "pen_credit", "q_search", "n_dp", "pen_draw", "detour_scale", "q_delay"]
G_DRAWN = ["e_tune", "t_share", "h_new", "sv_draw", "sig_keep", "rng_values_in_scan", "fight_luck_s",
           "setup_slide_s", "oars_s", "monkey_s", "feather_net_s", "walk_gain_rate"]
R_UNI, G_UNI = 10, 8                            # uniforms per sample: the success draws, then the common factor


def strata(pin=None, flat=False):
    """Two stages serve both sides: the delay scan (p_luck: R's draw points and store entry, G's luck)
    and the walk stage (p_walkopt: R's e_walk, G's walking).  R and G are independent only given
    these two, so every result is a mixture over the four strata (scan, walk)."""
    pl, pw = pmean("p_luck", pin, flat), pmean("p_walkopt", pin, flat)
    return [((L, W), (pl if L else 1.0 - pl) * (pw if W else 1.0 - pw)) for L in (True, False) for W in (True, False)]


def draws(names, n_uni, n, seed, flat=False):
    rnd = random.Random(seed).random
    cols = dict((nm, column(nm, n, rnd, None, flat)) for nm in names)
    cols["_u"] = [[rnd() for _ in range(n)] for _ in range(n_uni)]
    cols["_n"] = n
    return cols


def pinned(cols, names, pin):
    out = []
    for nm in names:
        out.append([pin[nm]] * cols["_n"] if (pin is not None and nm in pin) else cols[nm])
    return out + cols["_u"]


def run_R(cols, pin=None, flat=False, L=True, W=True, rho=RHO):
    """R for every draw, twice: with every success draw succeeding (the 'all
    found' stratum; the draw's weight is the probability of that), and with
    the success draws as they fall (plain).  L: the delay scan exists; W: the walk stage exists.
    Returns (forced track 6, forced no track 6, weight, plain track 6, plain no track 6, all-found flag,
    no-re-timer flag)."""
    p_ret, p_ex, p_dr, p_pl = [pmean(k, pin, flat) for k in ("p_retime", "p_exec", "p_drop", "p_plan")]
    p_lg = p_logo(pmean("p_logo_surv", pin, flat))
    p_in, p_en = pmean("p_intro", pin, flat), pmean("p_end", pin, flat)
    p_cr = p_credit(pmean("p_adapt", pin, flat), L, pmean("q_search", pin, flat))
    pw = corr_all([p_ret, p_ex, p_dr, p_pl, p_lg, p_in, p_en, p_cr], rho)
    fw0, fw1, fw2 = (W_PROBE + W_CALM) * R_ART / W_ALL, W_INNER / W_ALL, W_XWAIT / W_ALL
    sd0 = DROP_POOL * SEC_PER_FRAME
    xd0, xd1 = DROP_SIGHT * Q_SIGHT / DROP_POOL, (DROP_POOL - DROP_SIGHT) / DROP_POOL
    INF = float("inf")
    fp, fr, fw, pp, pr, pa_, nr = [], [], [], [], [], [], []
    for (r_in, r_xw, v, q_at, b_in, y_nd, y_or, phi_fb, e_w, e_c, u, iota, pen_c, q_s, n_dp, pen_d, ds, q_d,
         u1, u2, u3, u4, u5, u6, u7, u8, u9, uz) in zip(*pinned(cols, R_DRAWN, pin)):
        if rho > 0.0:
            u1, u2, u3, u4, u5, u6, u7, u8 = corr_u((u1, u2, u3, u4, u5, u6, u7, u8), uz, rho)
        pd = p_draws(q_s, n_dp, q_d, L)
        pen_d *= n_failed(q_s, n_dp, q_d, L)
        if not W:
            e_w = 0.0                           # in-visit walking needs the walk stage
        sc = 1.0 - u * (1.0 - SK)
        it = 1.0 + iota
        pool = DTOT * ds
        pk = DKS * ds
        if pk > EXK:
            pk = EXK
        exc = FC * sc - NC
        pc = DC * ds * sc
        if pc > exc:
            pc = exc
        exa = FA * sc - NA
        pa = DA * ds * sc
        if pa > exa:
            pa = exa
        f = fw0 + fw1 * r_in + fw2 * r_xw
        rem_w = S_WALK * v
        sd = sd0 * b_in
        rem_d = sd * (xd0 + xd1 * q_at)
        p3 = pool - S_WALK - sd
        if p3 < 0.0:
            p3 = 0.0
        # every success draw succeeds
        phi = (rem_w + rem_d + p3 * (y_nd + (1.0 - y_nd) * y_or)) / pool
        if phi > 1.0:
            phi = 1.0
        r = _time(phi, f, e_w, e_c, True, True, 0.0, pk, exc, pc, exa, pa, sc, it)
        fp.append(r)
        fr.append(r - CARD_N + CARD_OFF)
        fw.append(pw * pd)
        # the success draws as they fall
        ret, ex, dr, plan, logo = u1 < p_ret, u2 < p_ex, u3 < p_dr, u4 < p_pl, u5 < p_lg
        intro, end, cred, drw = u6 < p_in, u7 < p_en, u8 < p_cr, u9 < pd
        if ret and ex and dr and plan and logo and intro and end and cred and drw:
            pp.append(r)
            pr.append(r - CARD_N + CARD_OFF)
            pa_.append(True)
            nr.append(False)
            continue
        pa_.append(False)
        nr.append(not ret)
        if not end:                             # the closing menus are not answered: no Any% time
            pp.append(INF)
            pr.append(INF)
            continue
        if not ret:                             # no re-timer: the route stays as full5 played it
            intro = False
            r = _time(0.0, 0.0, 0.0, 0.0, False, False, 0.0, pk, exc, pc, exa, pa, sc, 1.0)
        else:
            if ex:
                rem = rem_w
                if dr:
                    rem += rem_d + p3 * (y_nd + ((1.0 - y_nd) * y_or if plan else 0.0))
                phi = rem / pool
                if phi > 1.0:
                    phi = 1.0
            else:                               # no executor: loop cuts and corridors only
                phi = phi_fb
            e_cv = e_c if (ex and dr) else 0.0  # in-visit drops and dialogue paths need the executor and drops
            r = _time(phi, f, e_w, e_cv, logo, intro, (0.0 if cred else pen_c) + (0.0 if drw else pen_d),
                      pk, exc, pc, exa, pa, sc, it)
        pp.append(r)
        pr.append(no_track6(r, intro))
    return fp, fr, fw, pp, pr, pa_, nr


def run_G(cols, pin=None, flat=False, L=True, W=True, rho=RHO):
    """G for every draw.  The rare step (the slide carried into Part III) is
    not drawn when rho = 0: both outcomes are returned with its probability.
    With rho > 0 it is drawn with the others on the common factor (probability 0 or 1).
    L: the delay scan exists (luck); W: the walk stage exists (walking beyond the hand route).
    Returns (G without the carry, G with it, probability of the carry)."""
    pm = lambda k: pmean(k, pin, flat)
    p_hd, p_p2, p_or, p_rg = pm("p_head"), pm("p_part2"), pm("p_order"), pm("p_regain")
    p_oa = p_oars(pm("c3_oars"), pm("s_step"))
    p_mo = p_monkey(pm("cov_in_monkey"), pm("cov_fr_monkey"), pm("s1_monkey"))
    pi0 = p_form(pm("cov_floor")) * pm("p_ts")
    inv_e = 1.0 - 1.0 / EXIT_VALUES
    g0s, g1s, pis = [], [], []
    for (e_tu, t_sh, h_new, sv, sg, n_rng, fight, setup, oars, monkey, feath, wrate,
         u1, u2, u3, u4, u5, u6, u7, uz) in zip(*pinned(cols, G_DRAWN, pin)):
        if rho > 0.0:
            u1, u2, u3, u4, u5, u6, u7 = corr_u((u1, u2, u3, u4, u5, u6, u7), uz, rho)
        base = e_tu * (TUNE_FIXED + t_sh * ORDER_POOL)
        if L:
            base += CAT_GAIN * (1.0 - inv_e ** n_rng) + fight
        if W:
            base += N_WALK_ALL * wrate * h_new * sv
        if u1 < p_hd:
            base += HEAD
        if u2 < p_feather(p_rg, sg):
            base += feath
        chain = -setup
        if u3 < p_p2:
            chain += PART2
        if u4 < p_oa:
            chain += oars
        if u5 < p_mo:
            chain += monkey
            if u6 < p_or:
                chain += (1.0 - t_sh) * ORDER_POOL
        g0s.append(base)
        g1s.append(base + chain if chain > 0.0 else base)     # the slide is used only if it pays
        pi = pi0 * sg ** N_KEEP
        pis.append((1.0 if u7 < pi else 0.0) if rho > 0.0 else pi)
    return g0s, g1s, pis


# --------------------------------------------------------------------------
# 5. Combining R and G
# --------------------------------------------------------------------------
class CDF(object):
    """F(r) = P(R < r): the 'all found' stratum (weighted) plus the plain
    draws in which something was not found."""

    def __init__(self, R, variant):
        rs, ws = R[variant], R[2]
        order = sorted(range(len(rs)), key=rs.__getitem__)
        self.fx = [rs[i] for i in order]
        cum, c = [0.0], 0.0
        for i in order:
            c += ws[i]
            cum.append(c)
        self.fc = cum
        pr, pa = R[3 + variant], R[5]
        self.px = sorted(pr[i] for i in range(len(pr)) if not pa[i])
        self.n = float(len(rs))

    def __call__(self, r):
        return (self.fc[bisect.bisect_left(self.fx, r)] + bisect.bisect_left(self.px, r)) / self.n


def bin_G(G):
    """G as a list of (seconds, probability): both carry outcomes, binned."""
    g0s, g1s, pis = G
    acc = {}
    inv = 1.0 / len(g0s)
    for g0, g1, pi in zip(g0s, g1s, pis):
        k0, k1 = int(g0 / G_BIN + 0.5), int(g1 / G_BIN + 0.5)
        acc[k0] = acc.get(k0, 0.0) + (1.0 - pi) * inv
        acc[k1] = acc.get(k1, 0.0) + pi * inv
    return [(k * G_BIN, w) for k, w in acc.items()]


def odds(cdfs, gbs, st, tau, extra=0.0):
    """P(R - G - extra < tau): within each stratum over all pairs of draws, mixed over the strata.
    Draws without a re-timer keep G in this sum; they lie above 3 hours, so no line moves."""
    return sum(ps * sum(w * cdfs[k](tau + g + extra) for g, w in gbs[k]) for k, ps in st)


def paired(Rd, Gd, st, variant, seed):
    """One T per plain draw of R, its stratum and G's carry drawn: for the percentiles.
    Without a re-timer the gains are not made (G = 0)."""
    rnd = random.Random(seed).random
    keys, cum, c = [k for k, _ in st], [], 0.0
    for _, ps in st:
        c += ps
        cum.append(c)
    n = min(len(Rd[keys[0]][3]), len(Gd[keys[0]][0]))
    ts = []
    for i in range(n):
        a, b = rnd(), rnd()
        k = keys[min(bisect.bisect_right(cum, a * c), len(keys) - 1)]
        R, G = Rd[k], Gd[k]
        g = 0.0 if R[6][i] else (G[1][i] if b < G[2][i] else G[0][i])
        ts.append(R[3 + variant][i] - g)
    ts.sort()
    return ts


def q(ts, p):
    return ts[min(len(ts) - 1, int(p * len(ts)))]


def RS(cols, pin=None, flat=False, rho=RHO):
    return dict((k, run_R(cols, pin, flat, k[0], k[1], rho)) for k, _ in strata(pin, flat))


def GS(cols, pin=None, flat=False, rho=RHO):
    return dict((k, run_G(cols, pin, flat, k[0], k[1], rho)) for k, _ in strata(pin, flat))


def summarise(Rd, Gd, st, seed):
    res = {"gb": dict((k, bin_G(Gd[k])) for k, _ in st), "st": st}
    for variant, name in ((0, "present"), (1, "removed")):
        cdfs = dict((k, CDF(Rd[k], variant)) for k, _ in st)
        ts = paired(Rd, Gd, st, variant, seed + 9)
        res[name] = {"p10": q(ts, 0.10), "p50": q(ts, 0.50), "p90": q(ts, 0.90),
                     "odds": [(lab, odds(cdfs, res["gb"], st, tau)) for lab, tau in THRESH], "cdf": cdfs, "ts": ts}
    return res


# --------------------------------------------------------------------------
# 6. Printing
# --------------------------------------------------------------------------
def mmss(s):
    if s == float("inf"):
        return "no end"
    sign = "-" if s < 0 else ""
    s = abs(s)
    h, rem = divmod(s, 3600.0)
    m, sec = divmod(rem, 60.0)
    if h:
        return "%s%d:%02d:%04.1f" % (sign, h, m, sec)
    return "%s%d:%04.1f" % (sign, m, sec)


def pct(p):
    """A probability as a percentage, three significant digits, never tidied."""
    if p <= 0.0:
        return "0"
    if p >= 1.0:
        return "100%"
    if p >= 0.9995:
        return "%.3f%%" % (100 * p)
    return "%.3g%%" % (100 * p)


def hdr(title):
    print()
    print("=" * 100)
    print(title)
    print("=" * 100)


def odds_line(r):
    return "   ".join("%s %s" % (l, pct(p)) for l, p in r["odds"])


def main():
    print("E031 estimate.  Seed %d.  Seconds on the Any%% clock (quarter-frames / %.9f)." % (SEED, HZ))
    print("Rule: start at the first key in the logo room (or the room's end if no key), end at the first frame of the final fade.")
    print("Model: T = R - G.  R: the route brought toward the hand no-slide run.  G: gains beyond that run.")

    # ---------------- table 1: segments ----------------
    hdr("1. SEGMENTS (M).  F = full5, N = hand no-slide (28:01.4), L = hand slide (21:01.483).  Seconds.")
    print("%-4s %-30s | %8s %7s %7s %7s %6s | %7s %7s | %8s %17s %7s" % (
        "seg", "to", "F total", "wait", "walk", "cut+txt", "act", "N", "L", "wait+", "detour (lo..hi)", "rest"))
    tot = [0.0] * 10
    for s in SEG:
        sid, lab, F, N, L, Dn, win = s
        if F is None:
            print("%-4s %-30s | %8s %7s %7s %7s %6s | %7.1f %7.1f |" % (sid, lab, "-", "-", "-", "-", "-", N[0], L[0]))
            tot[5] += N[0]
            tot[6] += L[0]
            continue
        if sid == "S00":
            xw, det, lo, hi, rest = 0.0, 0.0, 0.0, 0.0, F[0] - N[0]
        else:
            xw, det, lo, hi = Dn[0], Dn[1], Dn[2], Dn[3]
            rest = F[0] - N[0] - xw - det
        print("%-4s %-30s | %8.1f %7.1f %7.1f %7.1f %6.1f | %7.1f %7.1f | %8.1f %7.1f (%4.0f..%4.0f) %7.1f" % (
            sid, lab, F[0], F[1], F[2], F[3], F[4], N[0], L[0], xw, det, lo, hi, rest))
        for i, v in enumerate((F[0], F[1], F[2], F[3], F[4], N[0], L[0], xw, det, rest)):
            tot[i] += v
    print("%-4s %-30s | %8.1f %7.1f %7.1f %7.1f %6.1f | %7.1f %7.1f | %8.1f %7.1f (%4.0f..%4.0f) %7.1f" % (
        "sum", "", tot[0], tot[1], tot[2], tot[3], tot[4], tot[5], tot[6], tot[7], tot[8], D_LO * DTOT, D_HI * DTOT, tot[9]))
    print("F runs from boot to the end flag (S00 from boot; from the logo room's own end it is %.1f s)." % S00_PLAY)
    print("%.2f s of N lies outside the main loop and is not classed.  The 23:11.320 route is L plus %.2f s in S16." % (UNCL, TRACK6))
    print("wait+ = F's waiting above N's.  detour = F's busy time in visits beyond N's count for that room and segment.")
    print("rest = the remainder: F's play inside the visits both make, against N's (it holds the frame length, 6 against 5).")
    print("Waiting inside the extra visits: %.1f s of %.1f s (%.0f%%)." % (WIN, FW, 100 * WIN / FW))

    # ---------------- table 2: what separates the two hand routes ----------------
    hdr("2. WHAT SEPARATES THE TWO HAND ROUTES (D: N - L by segment, split with the glitch report's goal times)")
    C = dict((k, cen(k)) for k in REG)
    rows = [("S04", "luck at the fights (delay look-ahead)", SEG[4][3][0] - SEG[4][4][0]),
            ("S05,S06,S10,S12,S15,S16", "tuning", sum(SEG[i][3][0] - SEG[i][4][0] for i in (5, 6, 10, 12, 15)) + (SEG[16][3][0] - SEG[16][4][0] - TRACK6)),
            ("S07", "tuning (N - L + setup %.1f)" % C["setup_slide_s"], SEG[7][3][0] - SEG[7][4][0] + C["setup_slide_s"]),
            ("S07", "slide setup cost", -C["setup_slide_s"]),
            ("S08,S09", "slide uses at the end of Part I and in Part II", PART2),
            ("S11", "monkey skip", C["monkey_s"]), ("S11", "oars skip", C["oars_s"]),
            ("S11", "route order and tuning", ORDER_POOL),
            ("S13", "luck at the catacomb exits (delay look-ahead)", CAT_GAIN),
            ("S14", "feather skip", FEATHER), ("S14", "head variant and necklace skip", HEAD),
            ("S14", "tuning", SEG[14][3][0] - SEG[14][4][0] - FEATHER - HEAD),
            ("S16", "CD track 6 removed (a files condition, not a find)", TRACK6)]
    ssum = 0.0
    for a, b, c in rows:
        print("  %-26s %-52s %8.2f s" % (a, b, c))
        ssum += c
    print("  %-26s %-52s %8.2f s   (28:01.4 - 21:01.483 = %.2f s)" % ("", "sum", ssum, T_NOSLIDE - T_SLIDE))

    # ---------------- table 3: parameters ----------------
    hdr("3. PARAMETERS.  M measured, D derived, MR measured range (drawn), A assumed (drawn; low, centre, high)")
    for tag in ("M", "D", "MR", "A"):
        for p in REG.values():
            if p.tag != tag:
                continue
            if p.lo == p.hi:
                val = "%.6g" % p.c
            else:
                val = "%.4g, %.4g, %.4g" % (p.lo, p.c, p.hi)
            print("  %-2s %-24s %-24s %s" % (p.tag, p.name, val, p.text))
    C = dict((k, cen(k)) for k in REG)
    fc = f_wait(C["r_inner"], C["r_xwait"])
    flo = f_wait(REG["r_inner"].lo, REG["r_xwait"].lo)
    fhi = f_wait(REG["r_inner"].hi, REG["r_xwait"].hi)
    xc = x_drop(C["q_atom"])
    sdc = s_drop(C["b_inroom"])
    pool3 = DTOT - S_WALK - sdc
    print("  Derived shares at the centre: f_wait = %.4f; direct-walk pool %.0f s = %.3f of the detour; drop pool %.0f s = %.3f;" % (
        fc, S_WALK, S_WALK / DTOT, sdc, sdc / DTOT))
    print("  the remaining %.3f (%.0f s) is the pool of unneeded progress and order.  x_drop = %.3f." % (pool3 / DTOT, pool3, xc))
    print("  A parameter that is the probability of one success draw (%d of them) enters through the mean of its range." % len(PTYPE))

    # ---------------- table 4: mechanisms ----------------
    pf, pcar = p_form(C["cov_floor"]), p_carry(C["p_ts"], C["sig_keep"])
    po, pm = p_oars(C["c3_oars"], C["s_step"]), p_monkey(C["cov_in_monkey"], C["cov_fr_monkey"], C["s1_monkey"])
    pfe = p_feather(C["p_regain"], C["sig_keep"])
    ce = cov_exit(C["rng_values_in_scan"])
    hdr("4. MECHANISMS.  Probability (calculated at the centre), seconds removed if it succeeds (low, centre, high), needs")
    fmt = "  %-37s %-9s %7s %7s %7s  %s"
    print(fmt % ("mechanism", "p", "low", "centre", "high", "probability from / needs"))
    w_art = (W_PROBE + W_CALM) * R_ART
    mech = [
        ("true end (bit 565, frame 108,830)", 1.0, 614.8, 614.8, 614.8, "M: 694,701 - 657,813 ticks; already in F"),
        ("end menus to the final fade", C["p_end"], -S17_N, -S17_N, -S17_N, "A p_end; adds S17; without it there is no Any% time"),
        ("re-timing: probe and calm idle", C["p_retime"], w_art, w_art, w_art, "A p_retime; D r_artefact"),
        ("re-timing: input spacing in a trial", C["p_retime"], W_INNER * REG["r_inner"].lo, W_INNER * C["r_inner"], W_INNER * REG["r_inner"].hi, "A r_inner; needs re-timing"),
        ("re-timing: explicit waits", C["p_retime"], W_XWAIT * REG["r_xwait"].lo, W_XWAIT * C["r_xwait"], W_XWAIT * REG["r_xwait"].hi, "A r_xwait; needs re-timing"),
        ("intro skip (S00)", C["p_intro"], S00_PLAY - S00_SKIP, S00_PLAY - S00_SKIP, S00_PLAY - S00_SKIP, "A p_intro; %.1f s if track 6 is removed" % (S00_PLAY - S00_SKIP - CARD_F + CARD_OFF)),
        ("re-routing: direct walks", C["p_retime"] * C["p_exec"], S_WALK * REG["v_walk"].lo, S_WALK * C["v_walk"], S_WALK * REG["v_walk"].hi, "needs re-timing; A p_exec; pool %.3f of the detour (D) x v_walk" % (S_WALK / DTOT)),
        ("re-routing: drops of no-gain steps", C["p_retime"] * C["p_exec"] * C["p_drop"], s_drop(REG["b_inroom"].lo) * x_drop(REG["q_atom"].lo), sdc * xc, s_drop(REG["b_inroom"].hi) * x_drop(REG["q_atom"].hi), "needs the executor; A p_drop; pool %.3f x x_drop" % (sdc / DTOT)),
        ("re-routing: unneeded progress", C["p_retime"] * C["p_exec"] * C["p_drop"], pool3 * REG["y_need"].lo, pool3 * C["y_need"], pool3 * REG["y_need"].hi, "needs drops; pool %.3f x y_need" % (pool3 / DTOT)),
        ("re-routing: order", C["p_retime"] * C["p_exec"] * C["p_drop"] * C["p_plan"], pool3 * (1 - C["y_need"]) * REG["y_order"].lo, pool3 * (1 - C["y_need"]) * C["y_order"], pool3 * (1 - C["y_need"]) * REG["y_order"].hi, "needs drops; A p_plan; what is left x y_order"),
        ("re-routing without the executor", C["p_retime"] * (1 - C["p_exec"]), DTOT * REG["phi_fb"].lo, DTOT * C["phi_fb"], DTOT * REG["phi_fb"].hi, "only if the executor fails: A phi_fb x the detour"),
        ("execution: text", 1.0, 0.0, 0.0, 0.0, "M: 1,819 of 1,834 skips already on the first frame"),
        ("execution: walking in matched visits", C["p_retime"] * C["p_walkopt"], None, None, None, "A e_walk x rest (table 5, column in-visit); needs the walk stage"),
        ("execution: cutscene, text, acting", C["p_retime"] * C["p_exec"] * C["p_drop"], None, None, None, "A e_cut x rest (table 5, column in-visit); needs the executor and drops"),
        ("logo speed glitch", C["p_retime"] * p_logo(C["p_logo_surv"]), None, None, None, "47 of 960 scanned frames hit: cov 1; signal 1; x A p_logo_surv (table 5)"),
        ("Credit Early kept after re-timing", p_credit(C["p_adapt"]), 0.0, 0.0, 0.0, "with the delay scan 1-(1-20/61)^13 = %.4f x A p_adapt (without it %.3f); if lost: +%.0f s" % (
            COV_KEEPER, p_credit(C["p_adapt"], False, C["q_search"]), C["pen_credit"])),
        ("draw points solved again", p_draws(C["q_search"], C["n_dp"], C["q_delay"]), 0.0, 0.0, 0.0, "(1-(1-q_search)(1-q_delay))^n_dp, q_delay %.2f (without the delay scan %s); +%.0f s per lost point" % (
            C["q_delay"], pct(p_draws(C["q_search"], C["n_dp"], C["q_delay"], False)), C["pen_draw"])),
        ("luck: catacomb exits (S13)", C["p_luck"], CAT_GAIN * cov_exit(REG["rng_values_in_scan"].lo), CAT_GAIN * ce, CAT_GAIN * cov_exit(REG["rng_values_in_scan"].hi), "A p_luck (the delay scan); 1-(3/4)^n over %.1f random states = %.3f of %.1f s" % (C["rng_values_in_scan"], ce, CAT_GAIN)),
        ("luck: fights (S04)", C["p_luck"], REG["fight_luck_s"].lo, C["fight_luck_s"], REG["fight_luck_s"].hi, "the same pass"),
        ("tuning toward the slide run", 1.0, REG["e_tune"].lo * (TUNE_FIXED + REG["t_share"].lo * ORDER_POOL), C["e_tune"] * (TUNE_FIXED + C["t_share"] * ORDER_POOL), REG["e_tune"].hi * (TUNE_FIXED + REG["t_share"].hi * ORDER_POOL), "A e_tune x (%.1f + t_share x %.1f)" % (TUNE_FIXED, ORDER_POOL)),
        ("walking beyond the hand route", C["p_walkopt"], N_WALK_ALL * REG["walk_gain_rate"].lo * REG["h_new"].lo * REG["sv_draw"].lo, N_WALK_ALL * C["walk_gain_rate"] * C["h_new"] * C["sv_draw"], N_WALK_ALL * REG["walk_gain_rate"].hi * REG["h_new"].hi * REG["sv_draw"].hi, "A p_walkopt; %.0f s walking x rate x h_new x sv_draw" % N_WALK_ALL),
        ("head variant and necklace (S14)", C["p_head"], HEAD, HEAD, HEAD, "A p_head (timing only, no state signal)"),
        ("item slide formed (store)", pf, 0.0, 0.0, 0.0, "cov_floor x pair 1 x frame 1 x signal 1; no gain by itself"),
        ("slide carried into Part III", pf * pcar, -REG["setup_slide_s"].hi, -C["setup_slide_s"], -REG["setup_slide_s"].lo, "needs formed; p_ts x sig_keep^6 = %.4f; pays the setup" % pcar),
        ("slide uses in S08, S09", pf * pcar * C["p_part2"], PART2, PART2, PART2, "needs carried; A p_part2"),
        ("oars skip (S11)", pf * pcar * po, REG["oars_s"].lo, C["oars_s"], REG["oars_s"].hi, "needs carried; 1 x c3_oars x 1 x s_step = %.3f" % po),
        ("monkey skip (S11)", pf * pcar * pm, REG["monkey_s"].lo, C["monkey_s"], REG["monkey_s"].hi, "needs carried; cov_in x cov_fr x s1 = %.3f" % pm),
        ("route order around the skips (S11)", pf * pcar * pm * C["p_order"], (1 - REG["t_share"].hi) * ORDER_POOL, (1 - C["t_share"]) * ORDER_POOL, (1 - REG["t_share"].lo) * ORDER_POOL, "needs the monkey skip; A p_order"),
        ("slide regained, feather skip (S14)", pfe, REG["feather_net_s"].lo, C["feather_net_s"], REG["feather_net_s"].hi, "p_regain x sig_keep (the hold, found step by step) x 1 = %.3f; its own path" % pfe),
        ("CD track 6 removed", 1.0, TRACK6, TRACK6, TRACK6, "a files condition: the second reported variant"),
    ]
    for name, p, lo, c, hi, why in mech:
        if lo is None:
            print(fmt % (name, pct(p), "", "", "", why))
        else:
            print(fmt % (name, pct(p), "%.1f" % lo, "%.1f" % c, "%.1f" % hi, why))
    pch = pf * pcar * C["p_part2"] * po * pm * C["p_order"]
    print("  Whole slide chain at the centre (formed, carried, S08-S09 uses, oars, monkey, order): %s." % pct(pch))
    mp = lambda k: pmean(k, None, False)
    print("  The Monte Carlo uses the mean of each probability's range, not its centre: re-timing %s, executor %s, drops %s," % (pct(mp("p_retime")), pct(mp("p_exec")), pct(mp("p_drop"))))
    print("  order %s, logo %s, intro %s, end %s, Credit Early %s.  'Draws solved' is computed in every draw from q_search, n_dp, q_delay." % (
        pct(mp("p_plan")), pct(p_logo(mp("p_logo_surv"))), pct(mp("p_intro")), pct(mp("p_end")), pct(p_credit(mp("p_adapt")))))
    print("  The delay scan (p_luck %s) and the walk stage (p_walkopt %s) serve both R and G: results mix four strata." % (pct(mp("p_luck")), pct(mp("p_walkopt"))))
    print("  Payoff delays the valuation must bridge (M): logo 0 goals; early safe 105 goals, 385 s (kept by ordinary novelty);")
    print("  slide: first large saving 73 goals, 127 s after forming, setup cost first (bridged by the 6 keep-alive readings);")
    print("  oars 3 goals, 8.3 s; monkey 1 goal, 3.9 s; regain 13 goals to rest, 57 goals to the feather skip; feather 1 goal;")
    print("  head 10 goals, 20.9 s; luck within the next step.")

    # ---------------- table 5: per-segment effects at the centre ----------------
    hdr("5. PER-SEGMENT EFFECT AT THE CENTRE: seconds each mechanism removes if it succeeds, and the segment after all of them")
    u = C["u_frame"]
    sc = 1 - u * (1 - SK)
    phi_w = S_WALK * C["v_walk"] / DTOT
    phi_d = sdc * xc / DTOT
    phi_n = pool3 * C["y_need"] / DTOT
    phi_o = pool3 * (1 - C["y_need"]) * C["y_order"] / DTOT
    phi_all = phi_w + phi_d + phi_n + phi_o
    print("  shares of each segment's detour removed: walks %.3f, drops %.3f, needs %.3f, order %.3f (the same in every segment)" % (phi_w, phi_d, phi_n, phi_o))
    print("%-4s | %8s | %7s %7s %7s %7s %7s %8s %7s | %8s | %7s %7s" % (
        "seg", "F", "re-time", "walks", "drops", "needs", "order", "in-visit", "logo", "after", "N", "L"))
    sums = [0.0] * 11
    it = 1 + C["iota"]
    for s in MID:
        sid, lab, F, N, L, Dn, win = s
        g_ret = Dn[0] * fc
        wres = (win * (1 - phi_all) + (F[1] - win - N[1])) * (1 - fc)
        g_exec, x_sum, at6 = 0.0, 0.0, 0.0
        for (Fc_, Nc_, Dc_, sx, e) in ((F[2], N[2], Dn[4], SK, C["e_walk"]), (F[3], N[3], Dn[5], sc, C["e_cut"]), (F[4], N[4], Dn[6], sc, C["e_cut"])):
            is_card = (sid == "S16" and sx == sc and Fc_ > 200)
            cf_, cn_ = (CARD_F, CARD_N) if is_card else (0.0, 0.0)
            rest5 = (Fc_ - cf_ - Dc_) * sx - (Nc_ - cn_)
            x = rest5 * (1 - e) + Dc_ * sx * (1 - phi_all)
            g_exec += rest5 * e / sx
            x_sum += x
            at6 += (Nc_ - cn_ + x) / sx + cn_
        after = N[0] + (x_sum + wres) * it
        g_logo = at6 + N[1] + wres - (N[0] + x_sum + wres)
        ln = (sid, F[0], g_ret, Dn[1] * phi_w, Dn[1] * phi_d, Dn[1] * phi_n, Dn[1] * phi_o, g_exec, g_logo, after, N[0], L[0])
        print("%-4s | %8.1f | %7.1f %7.1f %7.1f %7.1f %7.1f %8.1f %7.1f | %8.1f | %7.1f %7.1f" % ln)
        for i in range(11):
            sums[i] += ln[i + 1]
    print("%-4s | %8.1f | %7.1f %7.1f %7.1f %7.1f %7.1f %8.1f %7.1f | %8.1f | %7.1f %7.1f" % tuple(["sum"] + sums))
    print("  Low and high over the route: re-time %.0f to %.0f s (f_wait %.3f to %.3f); the others as in table 4." % (flo * (FW - NW), fhi * (FW - NW), flo, fhi))
    print("  Removing a visit also removes the waiting left in it (%.0f s at the centre; counted in 'after', not in the columns)." % (WIN * (1 - fc) * phi_all))
    print("  logo = the segment at frame length 6 less the same at 5 (walking x 5/6, other busy time x (1 - u/6), u = %.2f)." % u)
    print("  'after' holds the interaction share (iota = %.2f).  S00 after: %.2f s.  S17 after: %.2f s." % (C["iota"], S00_SKIP, S17_N))
    print("  Negative in-visit values: in that segment full5's matched visits are already faster than N's in one class.")

    # ---------------- table 6: staircase at the centre ----------------
    hdr("6. STAIRCASE AT THE CENTRE (every parameter at its centre; each step if it and all before it succeed)")
    stairs = []
    r0 = route_R(0, 0, 0, 0, u, 1.0, 0.0, False, False)
    cum = C["p_end"]
    stairs.append(("full5 as found, end menus answered", r0, C["p_end"], cum))
    st = [("+ re-timing", dict(f=fc), C["p_retime"]),
          ("+ intro skip", dict(intro=True), C["p_intro"]),
          ("+ direct walks", dict(phi=phi_w), C["p_exec"]),
          ("+ drops of no-gain steps", dict(phi=phi_w + phi_d), C["p_drop"]),
          ("+ drops of unneeded progress", dict(phi=phi_w + phi_d + phi_n), 1.0),
          ("+ order", dict(phi=phi_all), C["p_plan"]),
          ("+ in-visit execution (walk stage)", dict(e_walk=C["e_walk"], e_cut=C["e_cut"]), C["p_walkopt"]),
          ("+ logo speed glitch", dict(logo=True), p_logo(C["p_logo_surv"])),
          ("+ interactions (iota)", dict(iota=C["iota"]), 1.0)]
    args = dict(f=0.0, phi=0.0, e_walk=0.0, e_cut=0.0, u=u, ds=1.0, iota=0.0, logo=False, intro=False)
    for name, ch, p in st:
        args.update(ch)
        cum *= p
        stairs.append((name, route_R(**args), p, cum))
    rc = stairs[-1][1]
    pcd = C["p_luck"] * p_credit(C["p_adapt"]) * p_draws(C["q_search"], C["n_dp"], C["q_delay"])
    cum *= pcd
    stairs.append(("  (delay scan: Credit Early, draws)", rc, pcd, cum))
    gl = CAT_GAIN * ce + C["fight_luck_s"]
    gt = C["e_tune"] * (TUNE_FIXED + C["t_share"] * ORDER_POOL)
    gw = N_WALK_ALL * C["walk_gain_rate"] * C["h_new"] * C["sv_draw"]
    g = 0.0
    for name, dg, p in (("+ luck by delay", gl, 1.0), ("+ tuning", gt, 1.0), ("+ walking beyond the hand route", gw, 1.0),
                        ("+ head and necklace", HEAD, C["p_head"]), ("+ feather path", C["feather_net_s"], pfe)):
        g += dg
        cum *= p
        stairs.append((name, (rc[0] - g, rc[1] - g), p, cum))
    chain = PART2 + C["oars_s"] + C["monkey_s"] + (1 - C["t_share"]) * ORDER_POOL - C["setup_slide_s"]
    stairs.append(("+ whole slide chain", (rc[0] - g - chain, rc[1] - g - chain), pch, cum * pch))
    print("  %-38s %12s %12s %10s %14s" % ("step", "track 6", "no track 6", "p(step)", "p(all so far)"))
    for name, (a, b), p, c in stairs:
        print("  %-38s %12s %12s %10s %14s" % (name, mmss(a), mmss(b), pct(p), pct(c)))
    print("  Anchors: hand no-slide %s, hand slide %s (track 6) and %s (no track 6), human record %s." % (
        mmss(T_NOSLIDE), mmss(T_SLIDE6), mmss(T_SLIDE), mmss(T_RECORD)))

    # ---------------- cross-checks ----------------
    hdr("7. CROSS-CHECKS OF THE MODEL")
    a, b = route_R(1, 1, 1, 1, u, 1.0, 0.0, True, True)
    print("  All removal shares at 1, logo and intro found: R = %.3f s (hand no-slide %.3f); without track 6 %.3f s." % (a, T_NOSLIDE, b))
    allg = (CAT_GAIN + (SEG[4][3][0] - SEG[4][4][0]) + TUNE_FIXED + ORDER_POOL + HEAD + PART2 + C["oars_s"] + C["monkey_s"] + FEATHER - C["setup_slide_s"])
    print("  Every hand-known gain at its hand value: G = %.2f s; R - G without track 6 = %.2f s (hand slide %.3f)." % (allg, b - allg, T_SLIDE))
    a0, _ = route_R(0, 0, 0, 0, u, 1.0, 0.0, False, False)
    f_any = S00_PLAY + sum(s[2][0] for s in MID) + S17_N / sc
    print("  Nothing removed: R = %.1f s; full5 on the same rule (logo room's end to the fade) = %.1f s." % (a0, f_any))
    fixed = route_R(1, (S_WALK + sdc) / DTOT, 0, 0, u, 1.0, 0.0, False, True)[0]
    print("  Fixed order, all idle gone, every droppable no-gain step dropped, frame length 6: %.1f min" % (fixed / 60))
    print("    (the graph report's bound for the same route with its idle left in: 57.5 min; progress trials alone 35.2 min).")
    only_wait = route_R(1, 0, 0, 0, u, 1.0, 0.0, False, True)[0]
    direct = sum(s[2][0] for s in MID) - (FW - NW) + S00_SKIP + S17_N / sc
    print("  Waiting at N's level and the intro skipped, nothing else: %.0f s; straight from table 1: %.0f s" % (only_wait, direct))
    print("    (the measurement report's 5,223 s is against the slide run's waiting and stops at the end flag).")
    pin = dict((p.name, p.c) for p in REG.values() if p.tag in ("A", "MR"))
    sr = run_R(draws(R_DRAWN, R_UNI, 3, 1), pin, False, True, True)
    ref = route_R(fc, phi_all, C["e_walk"], C["e_cut"], u, 1.0, C["iota"], True, True)
    print("  Sampler against the plain function at the centre: %.6f s apart (track 6), %.6f s (no track 6)." % (abs(sr[0][0] - ref[0]), abs(sr[1][0] - ref[1])))
    print("  Table 5's 'after' column plus S00 and S17: %.1f s; the plain function: %.1f s." % (sums[8] + S00_SKIP + S17_N + UNCL, ref[0]))
    print("  Detour removed at the centre with every stage working: %.3f (1 would be the reference's visit count)." % phi_all)

    # ---------------- Monte Carlo ----------------
    hdr("8. MONTE CARLO (%d draws of R in each stratum and %d of G; every R draw is combined with every G draw; seed %d)" % (N_MAIN, N_MAIN, SEED))
    st0 = strata()
    Rd = RS(draws(R_DRAWN, R_UNI, N_MAIN, SEED + 1))
    Gd = GS(draws(G_DRAWN, G_UNI, N_MAIN, SEED + 3))
    res = summarise(Rd, Gd, st0, SEED)
    R, G = Rd[(True, True)], Gd[(True, True)]
    labs = (("present", "CD track 6 present (compare with 28:01.4 and 23:11.3)"), ("removed", "CD track 6 removed (compare with 21:01.483)"))
    for name, lab in labs:
        r = res[name]
        print("  %s" % lab)
        print("    median %s   10th percentile %s   90th percentile %s" % (mmss(r["p50"]), mmss(r["p10"]), mmss(r["p90"])))
        print("    odds of under:  " + odds_line(r))
    ts = res["present"]["ts"]
    n_inf = sum(1 for t in ts if t == float("inf"))
    fin = [t for t in ts if t != float("inf")]
    print("  No Any%% end (closing menus not answered): %s of draws; they count as failing every line.  Mean of the rest: %s (track 6)." % (
        pct(n_inf / float(len(ts))), mmss(sum(fin) / len(fin))))
    allf = sum(1 for a_ in R[5] if a_) / float(len(R[5]))
    print("  With the delay scan and the walk stage built (%s of the weight): every other success draw of R succeeds in %s of plain draws (weight of the 'all found' stratum: %s)." % (
        pct(st0[0][1]), pct(allf), pct(sum(R[2]) / len(R[2]))))
    print("  Mean chance the slide is carried into Part III: %s.  Mean G without it %.1f s, with it %.1f s (both stages built)." % (
        pct(sum(G[2]) / len(G[2])), sum(G[0]) / len(G[0]), sum(G[1]) / len(G[1])))
    best_r, best_g = min(min(Rd[k][1]) for k in Rd), max(max(Gd[k][1]) for k in Gd)
    print("  Best single draws: R without track 6 %s; G %.1f s; that pair would give %s.  A printed 0 means no pair of draws." % (
        mmss(best_r), best_g, mmss(best_r - best_g)))
    print("  Share of plain draws of R by size (track 6 present, before G, strata mixed):")
    cnt = {}
    for k_, ps_ in st0:
        for v in Rd[k_][3]:
            k = "no end" if v == float("inf") else ("under 40 min" if v < 2400 else "40-50" if v < 3000 else "50-60" if v < 3600 else "60-90" if v < 5400 else "over 90 min")
            cnt[k] = cnt.get(k, 0) + ps_ / float(len(Rd[k_][3]))
    print("    " + "   ".join("%s %s" % (k, pct(cnt.get(k, 0))) for k in ("under 40 min", "40-50", "50-60", "60-90", "over 90 min", "no end")))
    main_res = res

    # ---------------- ends and shape ----------------
    hdr("9. ALL ASSUMED PARAMETERS AT THEIR PESSIMISTIC AND OPTIMISTIC ENDS, AND FLAT RANGES (%d draws; success draws and measured ranges still drawn)" % N_ENDS)
    cr_, cg_ = draws(R_DRAWN, R_UNI, N_ENDS, SEED + 101), draws(G_DRAWN, G_UNI, N_ENDS, SEED + 103)
    for lab, good in (("pessimistic", False), ("optimistic", True), ("flat ranges", None)):
        if good is None:
            r = summarise(RS(draws(R_DRAWN, R_UNI, N_ENDS, SEED + 101, True), None, True),
                          GS(draws(G_DRAWN, G_UNI, N_ENDS, SEED + 103, True), None, True), strata(None, True), SEED + 100)
        else:
            pin = dict((p.name, p.end(good)) for p in REG.values() if p.tag == "A")
            r = summarise(RS(cr_, pin), GS(cg_, pin), strata(pin), SEED + 100)
        for name in ("present", "removed"):
            rr = r[name]
            print("  %-11s track 6 %-7s median %-10s 10th %-10s 90th %-10s | under:  %s" % (
                lab, name, mmss(rr["p50"]), mmss(rr["p10"]), mmss(rr["p90"]), odds_line(rr)))
    print("  'flat ranges': every assumed parameter drawn evenly over its range instead of peaked at its centre (a check on the shape).")

    hdr("9b. CORRELATED SUCCESS DRAWS: one common factor among R's success draws and among G's (Gaussian copula; %d draws)" % N_ENDS)
    print("  The main run treats every success draw as independent.  Stages built by one team on one codebase are not:")
    print("  a common factor raises the chance that all succeed and that several fail together.  rho 0 = the main run's assumption.")
    for rho in (0.0, 0.5, 0.9):
        for lab, pin in (("drawn", None), ("optimistic", dict((p.name, p.end(True)) for p in REG.values() if p.tag == "A"))):
            r = summarise(RS(cr_, pin, False, rho), GS(cg_, pin, False, rho), strata(pin), SEED + 100)
            for name in ("present", "removed"):
                rr = r[name]
                print("  rho %.1f %-10s track 6 %-7s median %-10s | under:  %s" % (rho, lab, name, mmss(rr["p50"]), odds_line(rr)))

    # ---------------- sensitivity, others drawn ----------------
    hdr("10. SENSITIVITY A: one assumed parameter pinned at its pessimistic and its optimistic end, all others drawn (%d draws each)" % N_SENS)
    sr_, sg_ = draws(R_DRAWN, R_UNI, N_SENS, SEED + 201), draws(G_DRAWN, G_UNI, N_SENS, SEED + 203)

    def pack(Rd_, Gd_, st_):
        return (Rd_, Gd_, dict((k, CDF(Rd_[k], 0)) for k in Rd_), dict((k, CDF(Rd_[k], 1)) for k in Rd_),
                dict((k, bin_G(Gd_[k])) for k in Gd_), st_)

    def measure(Rd_, Gd_, c0, c1, gb, st_):
        ts_ = paired(Rd_, Gd_, st_, 0, SEED + 209)
        return odds(c1, gb, st_, T_SLIDE), odds(c1, gb, st_, 1800.0), odds(c0, gb, st_, 3600.0), q(ts_, 0.5)

    def pinned_pack(base_, pin, side):
        st_ = strata(pin)
        if side == "R":
            Rd_ = RS(sr_, pin)
            return (Rd_, base_[1], dict((k, CDF(Rd_[k], 0)) for k in Rd_), dict((k, CDF(Rd_[k], 1)) for k in Rd_), base_[4], st_)
        Gd_ = GS(sg_, pin)
        return (base_[0], Gd_, base_[2], base_[3], dict((k, bin_G(Gd_[k])) for k in Gd_), st_)

    base = pack(RS(sr_), GS(sg_), strata())
    b = measure(*base)
    print("  Nothing pinned, at this sample size: under 21:01.483 (no track 6) %s; under 30:00 (no track 6) %s; under 60:00 (track 6) %s; median (track 6) %s." % (
        pct(b[0]), pct(b[1]), pct(b[2]), mmss(b[3])))
    rows = []
    for p in REG.values():
        if p.tag != "A":
            continue
        vals = []
        for good in (False, True):
            vals.append(measure(*pinned_pack(base, {p.name: p.end(good)}, p.side)))
        rows.append((p, vals[0], vals[1]))
    rows.sort(key=lambda r: (-(r[2][0] - r[1][0]), -(r[2][1] - r[1][1])))
    print("  %-14s %-19s | %-22s | %-22s | %-18s | %s" % ("parameter", "pess. -> opt. end", "under 21:01.483, no t6", "under 30:00, no t6", "under 60:00, t6", "median, t6"))
    for p, lo, hi in rows:
        ends = "%.4g -> %.4g" % (p.end(False), p.end(True))
        print("  %-14s %-19s | %-9s -> %-9s | %-9s -> %-9s | %-7s -> %-7s | %s -> %s" % (
            p.name, ends, pct(lo[0]), pct(hi[0]), pct(lo[1]), pct(hi[1]), pct(lo[2]), pct(hi[2]), mmss(lo[3]), mmss(hi[3])))
    print("  Sorted by the move in the odds of under 21:01.483, then by the 30:00 column.  t6 = CD track 6 present.")
    print("  All runs use the same random numbers.  The 30:00 column still carries noise of about 0.02 percentage points")
    print("  (one plain draw is 1/%d = %.4f%%), so only its first rows are real differences." % (N_SENS, 100.0 / N_SENS))
    sens_a = rows

    # ---------------- sensitivity from the optimistic corner ----------------
    hdr("11. SENSITIVITY B: every assumed parameter at its optimistic end, then one moved to its pessimistic end (%d draws each)" % N_SENS)
    opt = dict((p.name, p.end(True)) for p in REG.values() if p.tag == "A")

    def measure_b(Rd_, Gd_, c0, c1, gb, st_):
        return odds(c1, gb, st_, T_SLIDE), odds(c0, gb, st_, T_SLIDE6), odds(c0, gb, st_, T_NOSLIDE)

    cbase = pack(RS(sr_, opt), GS(sg_, opt), strata(opt))
    cb = measure_b(*cbase)
    print("  All optimistic: under 21:01.483 (no track 6) %s; under 23:11.3 (track 6) %s; under 28:01.4 (track 6) %s." % (pct(cb[0]), pct(cb[1]), pct(cb[2])))
    rows = []
    for p in REG.values():
        if p.tag != "A":
            continue
        pin = dict(opt)
        pin[p.name] = p.end(False)
        rows.append((p, measure_b(*pinned_pack(cbase, pin, p.side))))
    rows.sort(key=lambda r: (r[1][0], r[1][1], r[1][2]))
    print("  %-14s %-12s | %-24s | %-22s | %s" % ("parameter", "moved to", "under 21:01.483, no t6", "under 23:11.3, t6", "under 28:01.4, t6"))
    for p, v in rows:
        print("  %-14s %-12.4g | %-10s (x %-9s) | %-22s | %s" % (p.name, p.end(False), pct(v[0]), ("%.3g" % (v[0] / cb[0])) if cb[0] > 0 else "-", pct(v[1]), pct(v[2])))
    print("  Sorted by what is left of the 21:01.483 odds.  x = the factor on the all-optimistic odds.")
    print("  t_share has no fixed good end: a low share helps when the slide chain is found (more of S11 falls to the skips), a high one when it is not.")
    sens_b = rows

    # ---------------- top five ----------------
    hdr("12. TOP FIVE FOR THE 21:01.483 ODDS, AND THE CHEAPEST EARLY MEASUREMENT THAT REPLACES EACH ASSUMPTION")
    top_a = max(hi[0] for p, lo, hi in sens_a)
    if top_a <= 0.0:
        print("  With the others drawn (table 10) the 21:01.483 odds stay at 0 for every single parameter, so the order is taken")
    else:
        print("  With the others drawn (table 10) no single parameter lifts the 21:01.483 odds above %s, so the order is taken" % pct(top_a))
    print("  from table 11: the parameters whose pessimistic end removes most of the all-optimistic odds.")
    for p, v in sens_b[:5]:
        print("  %-14s all-optimistic odds %s -> %s when it is at %.4g." % (p.name, pct(cb[0]), pct(v[0]), p.end(False)))
        print("  %-14s assumes: %s" % ("", p.text))
        print("  %-14s measure: %s" % ("", p.meas))
    print("  For the odds of under 30:00 without track 6 (table 10), the five that move them most:")
    for p, lo, hi in sorted(sens_a, key=lambda r: -(r[2][1] - r[1][1]))[:5]:
        print("  %-14s %s -> %s.  measure: %s" % (p.name, pct(lo[1]), pct(hi[1]), p.meas))
    print("  For the median with track 6 (table 10), the five that move it most:")
    for p, lo, hi in sorted(sens_a, key=lambda r: -(r[1][3] - r[2][3]))[:5]:
        print("  %-14s %s -> %s.  measure: %s" % (p.name, mmss(lo[3]), mmss(hi[3]), p.meas))

    # ---------------- what the model cannot know ----------------
    hdr("13. WHAT THE MODEL CANNOT KNOW: an unknown gain of X seconds, found for certain, taken off every draw")
    cdf_r, cdf_p, gb, sm = main_res["removed"]["cdf"], main_res["present"]["cdf"], main_res["gb"], main_res["st"]
    print("  %8s | %-28s | %-24s | %-24s | %s" % ("X (s)", "under 21:01.483, no track 6", "under 23:11.3, track 6", "under 28:01.4, track 6", "under 30:00, track 6"))
    for X in (0, 60, 120, 180, 300, 450, 600, 900, 1200, 1800):
        print("  %8d | %-28s | %-24s | %-24s | %s" % (X, pct(odds(cdf_r, gb, sm, T_SLIDE, X)), pct(odds(cdf_p, gb, sm, T_SLIDE6, X)),
                                                   pct(odds(cdf_p, gb, sm, T_NOSLIDE, X)), pct(odds(cdf_p, gb, sm, 1800.0, X))))
    ts_r = main_res["removed"]["ts"]
    print("  Unknown gain needed to reach 21:01.483 without track 6: %.0f s at the 10th percentile, %.0f s at the median, %.0f s at the 90th." % (
        q(ts_r, 0.10) - T_SLIDE, q(ts_r, 0.50) - T_SLIDE, q(ts_r, 0.90) - T_SLIDE))
    print("  Unknown glitches are set to zero in every table above.  Interactions are the parameter iota; R and G are independent given the two shared stages.")

    hdr("14. SUMMARY")
    for name, lab in (("present", "CD track 6 present"), ("removed", "CD track 6 removed")):
        r = main_res[name]
        print("  %-19s median %s, 10th-90th percentile %s to %s; under:  %s" % (lab, mmss(r["p50"]), mmss(r["p10"]), mmss(r["p90"]), odds_line(r)))
    print("  Everything found at the centre (table 6): %s with track 6, %s without.  Hand runs: %s, %s, %s." % (
        mmss(stairs[-2][1][0]), mmss(stairs[-2][1][1]), mmss(T_NOSLIDE), mmss(T_SLIDE6), mmss(T_SLIDE)))
    print()
    print("run time %.1f s" % (time.time() - T_START))


if __name__ == "__main__":
    main()
