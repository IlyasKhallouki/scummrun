# Claims

Every number or claim in the paper, with what supports it. Status: `measured` (from a reproducible
experiment or verified run), `probe` (seen once, needs a proper experiment), `pending`.

| Claim | Value | Evidence | Status |
|---|---|---|---|
| Hand-routed item-slide run, full audio | 1391.320 s (23:11.3), frame 16378 | `runs/anypercent/m2-verify/verify.json` | measured |
| Same route, CD track 6 absent | 1261.483 s (21:01.483), frame 14820 | `runs/anypercent/m2n-verify/verify.json` | measured |
| No-slide run with hand goal lists | 1681.413 s (28:01.4) | `runs/anypercent/verify-b1/verify.json` | measured |
| Glitchless Part I, time-optimal plan | 22,292 mean ticks over 30 held-out seeds | `docs/optimization.md` | measured |
| Esc sweep finds the logo-skip window blind | frames 46-92 | autonomy study sandbox | probe |
| Backward chaining recovers hand goals | about 28% (Part II: 0 of 23) | autonomy study sandbox | probe |
