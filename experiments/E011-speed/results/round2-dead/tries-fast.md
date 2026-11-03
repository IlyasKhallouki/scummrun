# E011 B5: the 15 tries by part

Wall / CPU seconds. ring(33): 4-room scope; ring(38): 5-room.

## b0

| try | scope | return | idle | read | plan | play | rest | try | stop |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| j00000 | 4 | 2.01 / 1.30 | 6.50 / 6.30 | 296.90 / 716.90 | 292.70 / 277.80 | 38.10 / 54.50 | 0.69 / 0.00 | 636.90 / 1035.30 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00001 | 5 | 2.69 / 2.00 | 17.70 / 15.70 | 23.75 / 34.00 | 5.73 / 5.50 | 0.02 / 0.00 | 0.11 / 0.00 | 50.00 / 52.60 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00002 | 5 | 2.44 / 1.80 | 9.80 / 9.70 | 13.67 / 27.70 | 4.57 / 4.60 | 0.16 / 0.10 | 0.16 / 0.00 | 30.80 / 40.90 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00003 | 5 | 3.98 / 3.30 | 15.30 / 15.10 | 14.46 / 31.70 | 5.76 / 5.70 | 0.68 / 0.80 | 0.12 / 0.00 | 40.30 / 51.80 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00004 | 5 | 2.72 / 2.10 | 10.20 / 10.00 | 7.86 / 14.00 | 5.09 / 5.10 | 0.20 / 0.20 | 0.13 / 0.00 | 26.20 / 28.10 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00005 | 5 | 2.02 / 1.30 | 0.00 / 0.00 | 3.39 / 7.60 | 4.79 / 4.70 | 0.16 / 0.10 | 0.04 / 0.10 | 10.40 / 13.80 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00006 | 4 | 2.55 / 1.90 | 13.80 / 13.50 | 63.92 / 150.80 | 145.32 / 143.40 | 5.46 / 6.20 | 0.25 / 0.00 | 231.30 / 311.20 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00007 | 5 | 2.41 / 1.80 | 12.70 / 12.30 | 8.40 / 14.90 | 10.97 / 10.80 | 3.97 / 4.50 | 0.15 / 0.00 | 38.60 / 39.10 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00008 | 4 | 3.52 / 2.90 | 18.60 / 17.20 | 140.29 / 236.00 | 205.40 / 199.10 | 12.05 / 12.60 | 0.54 / 0.00 | 380.40 / 461.40 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00009 | 4 | 2.50 / 1.80 | 0.00 / 0.00 | 69.81 / 169.30 | 115.30 / 113.00 | 6.48 / 7.40 | 0.11 / 0.00 | 194.20 / 290.60 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00010 | 5 | 2.55 / 1.90 | 8.20 / 8.10 | 4.16 / 9.10 | 5.68 / 5.60 | 1.89 / 2.50 | 0.12 / 0.00 | 22.60 / 24.10 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00011 | 4 | 3.03 / 2.40 | 14.60 / 13.90 | 121.86 / 231.80 | 160.20 / 157.60 | 7.29 / 8.40 | 0.32 / 0.00 | 307.30 / 408.90 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00012 | 4 | 4.17 / 3.40 | 0.00 / 0.00 | 62.92 / 153.50 | 155.42 / 151.00 | 5.49 / 6.20 | 0.40 / 0.00 | 228.40 / 313.80 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00013 | 5 | 2.70 / 2.00 | 8.90 / 8.80 | 6.16 / 12.90 | 7.02 / 6.90 | 2.36 / 3.00 | 0.06 / 0.00 | 27.20 / 30.10 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00014 | 4 | 2.60 / 1.90 | 9.20 / 9.10 | 58.12 / 136.90 | 105.80 / 104.70 | 4.85 / 5.40 | 0.23 / 0.00 | 180.80 / 254.70 | refinement: Lemma R's premise R0: the last pass wrote values |

| over | n | try wall median | try CPU median | try wall mean | try CPU mean | try wall max | return wall med | idle wall med | read wall med | plan wall med | play wall med | rest wall med |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 15 | 50.00 | 52.60 | 160.36 | 223.76 | 636.90 | 2.60 | 9.80 | 23.75 | 10.97 | 3.97 | 0.15 |
| ring33 | 7 | 231.30 | 313.80 | 308.47 | 439.41 | 636.90 | 2.60 | 9.20 | 69.81 | 155.42 | 6.48 | 0.32 |
| ring38 | 8 | 29.00 | 34.60 | 30.76 | 35.06 | 50.00 | 2.62 | 10.00 | 8.13 | 5.71 | 0.44 | 0.12 |

## r1-fast

| try | scope | return | idle | read | plan | play | rest | try | stop |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| j00000 | 4 | 0.32 / 0.00 | 0.30 / 0.20 | 94.67 / 310.20 | 19.73 / 20.10 | 0.60 / 0.00 | 0.00 / 2.50 | 115.60 / 333.00 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00001 | 5 | 0.02 / 0.00 | 0.30 / 0.40 | 3.14 / 8.10 | 0.66 / 0.70 | 0.00 / 0.00 | 0.08 / 0.29 | 4.20 / 9.49 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00002 | 5 | 0.02 / 0.00 | 0.30 / 0.30 | 3.96 / 11.50 | 0.52 / 0.50 | 0.02 / 0.00 | 0.00 / 0.22 | 4.80 / 12.52 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00003 | 5 | 0.01 / 0.00 | 0.30 / 0.20 | 3.95 / 11.20 | 0.63 / 0.60 | 0.02 / 0.00 | 0.00 / 0.31 | 4.80 / 12.31 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00004 | 5 | 0.00 / 0.00 | 0.30 / 0.20 | 0.21 / 0.20 | 0.47 / 0.50 | 0.02 / 0.00 | 0.00 / 0.00 | 1.00 / 0.88 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00005 | 5 | 0.00 / 0.00 | 0.00 / 0.00 | 0.06 / 0.10 | 0.47 / 0.50 | 0.02 / 0.00 | 0.05 / 0.00 | 0.60 / 0.60 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00006 | 4 | 0.01 / 0.00 | 0.30 / 0.30 | 20.02 / 60.60 | 16.12 / 16.20 | 0.42 / 0.00 | 0.03 / 0.00 | 36.90 / 77.07 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00007 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 1.67 / 4.60 | 0.52 / 0.60 | 0.02 / 0.00 | 0.08 / 0.00 | 2.50 / 5.35 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00008 | 4 | 0.00 / 0.00 | 0.20 / 0.20 | 18.46 / 57.80 | 16.99 / 17.10 | 0.36 / 0.00 | 0.00 / 0.05 | 36.00 / 75.15 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00009 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.01 / 0.00 | 15.58 / 15.80 | 0.41 / 0.00 | 0.00 / 0.13 | 16.00 / 15.93 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00010 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 0.01 / 0.00 | 0.47 / 0.50 | 0.02 / 0.00 | 0.09 / 0.00 | 0.80 / 0.64 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00011 | 4 | 0.00 / 0.00 | 0.20 / 0.20 | 0.03 / 0.00 | 16.51 / 16.60 | 0.37 / 0.00 | 0.00 / 0.02 | 17.10 / 16.82 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00012 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.01 / 0.00 | 2.48 / 2.70 | 0.31 / 0.00 | 0.10 / 0.12 | 2.90 / 2.82 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00013 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 0.02 / 0.00 | 0.57 / 0.50 | 0.02 / 0.00 | 0.00 / 0.00 | 0.80 / 0.67 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00014 | 4 | 0.00 / 0.00 | 0.20 / 0.20 | 0.01 / 0.00 | 15.41 / 15.60 | 0.48 / 0.00 | 0.00 / 0.04 | 16.10 / 15.84 | refinement: Lemma R's premise R0: the last pass wrote values |

| over | n | try wall median | try CPU median | try wall mean | try CPU mean | try wall max | return wall med | idle wall med | read wall med | plan wall med | play wall med | rest wall med |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 15 | 4.80 | 12.31 | 17.34 | 38.61 | 115.60 | 0.01 | 0.20 | 0.21 | 0.66 | 0.02 | 0.00 |
| ring33 | 7 | 17.10 | 16.82 | 34.37 | 76.66 | 115.60 | 0.00 | 0.20 | 0.03 | 16.12 | 0.41 | 0.00 |
| ring38 | 8 | 1.75 | 3.11 | 2.44 | 5.31 | 4.80 | 0.01 | 0.25 | 0.94 | 0.52 | 0.02 | 0.03 |

## r2-fast

| try | scope | return | idle | read | plan | play | rest | try | stop |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| j00000 | 4 | 0.12 / 0.00 | 0.10 / 0.00 | 105.95 / 295.00 | 14.77 / 13.90 | 0.28 / 0.00 | 0.08 / 6.35 | 121.30 / 315.25 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00001 | 5 | 0.01 / 0.00 | 0.10 / 0.00 | 3.47 / 8.00 | 0.33 / 0.30 | 0.00 / 0.00 | 0.00 / 0.57 | 3.80 / 8.87 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00002 | 5 | 0.01 / 0.00 | 0.10 / 0.00 | 4.00 / 10.40 | 0.39 / 0.30 | 0.01 / 0.00 | 0.00 / 0.54 | 4.50 / 11.24 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00003 | 5 | 0.01 / 0.00 | 0.10 / 0.10 | 4.05 / 10.20 | 0.24 / 0.30 | 0.01 / 0.00 | 0.09 / 0.36 | 4.50 / 10.96 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00004 | 5 | 0.00 / 0.00 | 0.10 / 0.00 | 0.18 / 0.20 | 0.19 / 0.20 | 0.01 / 0.00 | 0.02 / 0.12 | 0.50 / 0.52 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00005 | 5 | 0.00 / 0.00 | 0.00 / 0.00 | 0.05 / 0.10 | 0.19 / 0.10 | 0.01 / 0.00 | 0.00 / 0.12 | 0.20 / 0.32 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00006 | 4 | 0.01 / 0.00 | 0.10 / 0.00 | 20.93 / 56.70 | 8.43 / 7.30 | 0.21 / 0.00 | 0.02 / 4.17 | 29.70 / 68.17 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00007 | 5 | 0.00 / 0.00 | 0.10 / 0.10 | 1.47 / 3.80 | 0.21 / 0.20 | 0.02 / 0.00 | 0.00 / 0.05 | 1.80 / 4.15 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00008 | 4 | 0.00 / 0.00 | 0.10 / 0.00 | 0.01 / 0.00 | 5.60 / 4.80 | 0.19 / 0.00 | 0.00 / 3.51 | 5.90 / 8.31 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00009 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.01 / 0.00 | 5.51 / 4.70 | 0.08 / 0.00 | 0.00 / 3.19 | 5.60 / 7.89 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00010 | 5 | 0.00 / 0.00 | 0.10 / 0.00 | 0.00 / 0.00 | 0.18 / 0.20 | 0.02 / 0.00 | 0.00 / 0.08 | 0.20 / 0.28 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00011 | 4 | 0.00 / 0.00 | 0.10 / 0.00 | 0.00 / 0.00 | 5.48 / 4.70 | 0.22 / 0.00 | 0.00 / 3.29 | 5.80 / 7.99 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00012 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.01 / 0.00 | 0.01 / 0.00 | 0.08 / 0.00 | 0.00 / 0.07 | 0.10 / 0.07 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00013 | 5 | 0.00 / 0.00 | 0.10 / 0.00 | 0.01 / 0.00 | 0.18 / 0.10 | 0.01 / 0.00 | 0.00 / 0.18 | 0.30 / 0.28 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00014 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.01 / 0.00 | 5.55 / 4.70 | 0.24 / 0.00 | 0.00 / 3.39 | 5.80 / 8.09 | refinement: Lemma R's premise R0: the last pass wrote values |

| over | n | try wall median | try CPU median | try wall mean | try CPU mean | try wall max | return wall med | idle wall med | read wall med | plan wall med | play wall med | rest wall med |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 15 | 4.50 | 7.99 | 12.67 | 30.16 | 121.30 | 0.00 | 0.10 | 0.05 | 0.33 | 0.02 | 0.00 |
| ring33 | 7 | 5.80 | 8.09 | 24.89 | 59.40 | 121.30 | 0.00 | 0.10 | 0.01 | 5.55 | 0.21 | 0.00 |
| ring38 | 8 | 1.15 | 2.33 | 1.98 | 4.58 | 4.50 | 0.00 | 0.10 | 0.82 | 0.20 | 0.01 | 0.00 |

## d-fast

| try | scope | return | idle | read | plan | play | rest | try | stop |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| j00000 | 4 | 0.11 / 0.00 | 0.10 / 0.10 | 120.58 / 332.50 | 18.00 / 16.90 | 0.32 / 0.00 | 0.00 / 7.63 | 139.10 / 357.13 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00001 | 5 | 0.01 / 0.00 | 0.10 / 0.10 | 3.39 / 8.20 | 0.41 / 0.40 | 0.00 / 0.00 | 0.00 / 0.40 | 3.90 / 9.10 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00002 | 5 | 0.01 / 0.00 | 0.10 / 0.10 | 3.78 / 9.60 | 0.40 / 0.40 | 0.02 / 0.00 | 0.00 / 0.42 | 4.30 / 10.52 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00003 | 5 | 0.01 / 0.00 | 0.10 / 0.00 | 4.06 / 10.40 | 0.33 / 0.30 | 0.01 / 0.00 | 0.00 / 0.44 | 4.50 / 11.14 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00004 | 5 | 0.00 / 0.00 | 0.10 / 0.00 | 0.20 / 0.20 | 0.19 / 0.20 | 0.01 / 0.00 | 0.00 / 0.13 | 0.50 / 0.53 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00005 | 5 | 0.00 / 0.00 | 0.00 / 0.00 | 0.07 / 0.10 | 0.19 / 0.20 | 0.01 / 0.00 | 0.03 / 0.06 | 0.30 / 0.36 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00006 | 4 | 0.01 / 0.00 | 0.10 / 0.10 | 20.01 / 53.60 | 9.41 / 8.50 | 0.26 / 0.00 | 0.01 / 3.61 | 29.80 / 65.81 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00007 | 5 | 0.00 / 0.00 | 0.10 / 0.00 | 1.58 / 4.10 | 0.21 / 0.20 | 0.01 / 0.00 | 0.00 / 0.14 | 1.90 / 4.44 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00008 | 4 | 0.00 / 0.00 | 0.10 / 0.10 | 0.01 / 0.00 | 5.58 / 4.80 | 0.21 / 0.00 | 0.00 / 3.30 | 5.90 / 8.20 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00009 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.01 / 0.00 | 6.01 / 5.20 | 0.08 / 0.00 | 0.00 / 3.04 | 6.10 / 8.24 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00010 | 5 | 0.00 / 0.00 | 0.10 / 0.10 | 0.00 / 0.00 | 0.19 / 0.20 | 0.01 / 0.00 | 0.00 / 0.03 | 0.30 / 0.33 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00011 | 4 | 0.00 / 0.00 | 0.10 / 0.00 | 0.01 / 0.00 | 5.59 / 4.80 | 0.20 / 0.00 | 0.00 / 3.26 | 5.80 / 8.06 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00012 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.01 / 0.00 | 0.00 / 0.10 | 0.10 / 0.00 | 0.09 / 0.00 | 0.20 / 0.10 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00013 | 5 | 0.00 / 0.00 | 0.10 / 0.10 | 0.01 / 0.00 | 0.17 / 0.20 | 0.02 / 0.00 | 0.00 / 0.04 | 0.30 / 0.34 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00014 | 4 | 0.00 / 0.00 | 0.10 / 0.00 | 0.01 / 0.00 | 6.27 / 5.40 | 0.22 / 0.00 | 0.00 / 3.45 | 6.60 / 8.85 | refinement: Lemma R's premise R0: the last pass wrote values |

| over | n | try wall median | try CPU median | try wall mean | try CPU mean | try wall max | return wall med | idle wall med | read wall med | plan wall med | play wall med | rest wall med |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 15 | 4.30 | 8.20 | 13.97 | 32.88 | 139.10 | 0.00 | 0.10 | 0.07 | 0.40 | 0.02 | 0.00 |
| ring33 | 7 | 6.10 | 8.24 | 27.64 | 65.20 | 139.10 | 0.00 | 0.10 | 0.01 | 6.01 | 0.21 | 0.00 |
| ring38 | 8 | 1.20 | 2.49 | 2.00 | 4.59 | 4.50 | 0.00 | 0.10 | 0.89 | 0.20 | 0.01 | 0.00 |

