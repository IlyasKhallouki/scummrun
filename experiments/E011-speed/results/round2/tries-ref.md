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

## r1-ref

| try | scope | return | idle | read | plan | play | rest | try | stop |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| j00000 | 4 | 0.24 / 0.00 | 0.20 / 0.20 | 90.23 / 297.50 | 65.42 / 29.80 | 1.85 / 1.50 | 0.06 / 37.90 | 158.00 / 366.90 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00001 | 5 | 0.02 / 0.00 | 0.70 / 0.70 | 5.37 / 11.50 | 1.63 / 0.70 | 0.00 / 0.00 | 0.08 / 2.82 | 7.80 / 15.72 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00002 | 5 | 0.02 / 0.00 | 0.30 / 0.20 | 4.59 / 11.60 | 1.39 / 0.50 | 0.02 / 0.00 | 0.00 / 1.64 | 6.30 / 13.94 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00003 | 5 | 0.02 / 0.00 | 0.30 / 0.30 | 4.52 / 12.60 | 1.32 / 0.50 | 0.06 / 0.00 | 0.00 / 0.85 | 6.20 / 14.25 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00004 | 5 | 0.00 / 0.00 | 0.30 / 0.30 | 0.20 / 0.20 | 1.37 / 0.50 | 0.02 / 0.00 | 0.01 / 0.75 | 1.90 / 1.75 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00005 | 5 | 0.00 / 0.00 | 0.00 / 0.00 | 0.09 / 0.10 | 1.27 / 0.50 | 0.02 / 0.00 | 0.02 / 0.77 | 1.40 / 1.37 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00006 | 4 | 0.01 / 0.00 | 0.40 / 0.40 | 19.30 / 58.40 | 41.24 / 17.60 | 0.55 / 0.00 | 0.00 / 23.76 | 61.50 / 100.16 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00007 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 1.65 / 4.80 | 1.60 / 0.50 | 0.15 / 0.00 | 0.00 / 1.13 | 3.60 / 6.63 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00008 | 4 | 0.00 / 0.00 | 0.30 / 0.30 | 0.02 / 0.00 | 43.79 / 16.50 | 0.50 / 0.00 | 0.00 / 27.52 | 44.60 / 44.32 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00009 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.03 / 0.00 | 41.43 / 14.70 | 0.45 / 0.00 | 0.09 / 27.16 | 42.00 / 41.86 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00010 | 5 | 0.01 / 0.00 | 0.30 / 0.30 | 0.02 / 0.00 | 1.45 / 0.50 | 0.14 / 0.00 | 0.00 / 0.94 | 1.90 / 1.74 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00011 | 4 | 0.00 / 0.00 | 0.20 / 0.20 | 0.02 / 0.00 | 40.08 / 16.20 | 0.51 / 0.00 | 0.00 / 24.19 | 40.80 / 40.59 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00012 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.02 / 0.00 | 2.52 / 2.80 | 0.47 / 0.00 | 0.09 / 0.25 | 3.10 / 3.05 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00013 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 0.03 / 0.00 | 1.56 / 0.50 | 0.13 / 0.00 | 0.00 / 1.10 | 1.90 / 1.80 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00014 | 4 | 0.00 / 0.00 | 0.20 / 0.20 | 0.01 / 0.00 | 42.85 / 14.20 | 0.44 / 0.00 | 0.00 / 28.89 | 43.50 / 43.29 | refinement: Lemma R's premise R0: the last pass wrote values |

| over | n | try wall median | try CPU median | try wall mean | try CPU mean | try wall max | return wall med | idle wall med | read wall med | plan wall med | play wall med | rest wall med |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 15 | 6.30 | 14.25 | 28.30 | 46.49 | 158.00 | 0.01 | 0.20 | 0.09 | 1.63 | 0.15 | 0.00 |
| ring33 | 7 | 43.50 | 43.29 | 56.21 | 91.45 | 158.00 | 0.00 | 0.20 | 0.02 | 41.43 | 0.50 | 0.00 |
| ring38 | 8 | 2.75 | 4.21 | 3.88 | 7.15 | 7.80 | 0.01 | 0.30 | 0.92 | 1.42 | 0.04 | 0.00 |

## r2-ref

| try | scope | return | idle | read | plan | play | rest | try | stop |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| j00000 | 4 | 0.12 / 0.00 | 0.10 / 0.10 | 105.43 / 290.00 | 21.41 / 19.80 | 1.06 / 0.70 | 0.08 / 9.43 | 128.20 / 320.03 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00001 | 5 | 0.01 / 0.00 | 0.10 / 0.00 | 4.29 / 9.40 | 0.31 / 0.30 | 0.00 / 0.00 | 0.00 / 0.66 | 4.70 / 10.36 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00002 | 5 | 0.01 / 0.00 | 0.10 / 0.00 | 4.34 / 10.50 | 0.34 / 0.30 | 0.02 / 0.00 | 0.00 / 0.69 | 4.80 / 11.49 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00003 | 5 | 0.01 / 0.00 | 0.10 / 0.10 | 4.49 / 11.10 | 0.27 / 0.30 | 0.04 / 0.00 | 0.00 / 0.54 | 4.90 / 12.04 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00004 | 5 | 0.00 / 0.00 | 0.10 / 0.10 | 0.20 / 0.20 | 0.19 / 0.20 | 0.01 / 0.00 | 0.00 / 0.10 | 0.50 / 0.60 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00005 | 5 | 0.00 / 0.00 | 0.00 / 0.00 | 0.07 / 0.10 | 0.18 / 0.20 | 0.01 / 0.00 | 0.04 / 0.12 | 0.30 / 0.42 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00006 | 4 | 0.00 / 0.00 | 0.10 / 0.00 | 22.34 / 58.90 | 7.60 / 6.50 | 0.32 / 0.00 | 0.04 / 4.78 | 30.40 / 70.18 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00007 | 5 | 0.00 / 0.00 | 0.10 / 0.00 | 1.62 / 4.10 | 0.19 / 0.20 | 0.09 / 0.00 | 0.00 / 0.21 | 2.00 / 4.51 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00008 | 4 | 0.00 / 0.00 | 0.10 / 0.00 | 0.01 / 0.00 | 5.56 / 4.70 | 0.33 / 0.00 | 0.00 / 3.84 | 5.90 / 8.54 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00009 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.02 / 0.00 | 4.91 / 4.20 | 0.27 / 0.00 | 0.00 / 3.20 | 5.20 / 7.40 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00010 | 5 | 0.00 / 0.00 | 0.10 / 0.00 | 0.01 / 0.00 | 0.24 / 0.20 | 0.15 / 0.00 | 0.00 / 0.29 | 0.40 / 0.49 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00011 | 4 | 0.00 / 0.00 | 0.10 / 0.00 | 0.02 / 0.00 | 5.47 / 4.70 | 0.31 / 0.00 | 0.00 / 3.65 | 5.90 / 8.35 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00012 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.01 / 0.00 | 0.08 / 0.10 | 0.21 / 0.00 | 0.00 / 0.18 | 0.30 / 0.28 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00013 | 5 | 0.00 / 0.00 | 0.10 / 0.10 | 0.01 / 0.00 | 0.21 / 0.20 | 0.08 / 0.00 | 0.00 / 0.16 | 0.40 / 0.46 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00014 | 4 | 0.00 / 0.00 | 0.10 / 0.10 | 0.01 / 0.00 | 4.91 / 4.10 | 0.28 / 0.00 | 0.00 / 3.44 | 5.30 / 7.64 | refinement: Lemma R's premise R0: the last pass wrote values |

| over | n | try wall median | try CPU median | try wall mean | try CPU mean | try wall max | return wall med | idle wall med | read wall med | plan wall med | play wall med | rest wall med |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 15 | 4.80 | 7.64 | 13.28 | 30.85 | 128.20 | 0.00 | 0.10 | 0.07 | 0.31 | 0.15 | 0.00 |
| ring33 | 7 | 5.90 | 8.35 | 25.89 | 60.35 | 128.20 | 0.00 | 0.10 | 0.02 | 5.47 | 0.31 | 0.00 |
| ring38 | 8 | 1.25 | 2.55 | 2.25 | 5.05 | 4.90 | 0.00 | 0.10 | 0.91 | 0.22 | 0.03 | 0.00 |

