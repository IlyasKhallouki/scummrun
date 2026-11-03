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

## on

| try | scope | return | idle | read | plan | play | rest | try | stop |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| j00000 | 4 | 0.09 / 0.00 | 0.10 / 0.00 | 108.31 / 325.20 | 22.40 / 37.50 | 0.29 / 0.00 | 0.01 / 0.00 | 131.20 / 46.50 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00001 | 5 | 0.01 / 0.00 | 0.10 / 0.00 | 3.02 / 8.50 | 0.28 / 0.50 | 0.00 / 0.00 | 0.00 / 0.00 | 3.30 / 1.10 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00002 | 5 | 0.01 / 0.00 | 0.10 / 0.00 | 2.31 / 6.50 | 0.37 / 0.70 | 0.02 / 0.00 | 0.00 / 0.00 | 2.80 / 1.30 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00003 | 5 | 0.01 / 0.00 | 0.10 / 0.00 | 1.16 / 2.90 | 0.33 / 0.60 | 0.01 / 0.00 | 0.00 / 0.00 | 1.60 / 1.10 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00004 | 5 | 0.00 / 0.00 | 0.10 / 0.10 | 0.19 / 0.20 | 0.19 / 0.20 | 0.01 / 0.00 | 0.01 / 0.00 | 0.50 / 0.40 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00005 | 5 | 0.00 / 0.00 | 0.00 / 0.00 | 0.05 / 0.00 | 0.19 / 0.10 | 0.01 / 0.00 | 0.00 / 0.10 | 0.20 / 0.20 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00006 | 4 | 0.01 / 0.00 | 0.10 / 0.00 | 8.21 / 18.90 | 14.49 / 17.90 | 0.27 / 0.00 | 0.02 / 0.00 | 23.10 / 18.00 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00007 | 5 | 0.02 / 0.00 | 0.10 / 0.10 | 0.82 / 1.60 | 0.26 / 0.80 | 0.02 / 0.00 | 0.00 / 0.00 | 1.20 / 0.80 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00008 | 4 | 0.00 / 0.00 | 0.10 / 0.10 | 0.02 / 0.00 | 13.29 / 11.20 | 0.29 / 0.00 | 0.10 / 0.00 | 13.80 / 11.30 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00009 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.03 / 0.00 | 11.07 / 9.20 | 0.10 / 0.00 | 0.00 / 0.00 | 11.20 / 9.20 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00010 | 5 | 0.00 / 0.00 | 0.10 / 0.00 | 0.00 / 0.00 | 0.29 / 0.20 | 0.01 / 0.00 | 0.00 / 0.00 | 0.40 / 0.20 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00011 | 4 | 0.00 / 0.00 | 0.10 / 0.00 | 0.02 / 0.00 | 10.32 / 8.60 | 0.26 / 0.00 | 0.00 / 0.00 | 10.60 / 8.60 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00012 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.01 / 0.00 | 0.06 / 0.10 | 0.13 / 0.00 | 0.00 / 0.00 | 0.20 / 0.10 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00013 | 5 | 0.00 / 0.00 | 0.10 / 0.00 | 0.01 / 0.00 | 0.27 / 0.20 | 0.02 / 0.00 | 0.00 / 0.00 | 0.30 / 0.20 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00014 | 4 | 0.00 / 0.00 | 0.10 / 0.00 | 0.01 / 0.00 | 10.57 / 8.90 | 0.32 / 0.00 | 0.00 / 0.10 | 11.00 / 9.00 | refinement: Lemma R's premise R0: the last pass wrote values |

| over | n | try wall median | try CPU median | try wall mean | try CPU mean | try wall max | return wall med | idle wall med | read wall med | plan wall med | play wall med | rest wall med |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 15 | 2.80 | 1.10 | 14.09 | 7.20 | 131.20 | 0.00 | 0.10 | 0.05 | 0.33 | 0.02 | 0.00 |
| ring33 | 7 | 11.20 | 9.20 | 28.73 | 14.67 | 131.20 | 0.00 | 0.10 | 0.02 | 11.07 | 0.27 | 0.00 |
| ring38 | 8 | 0.85 | 0.60 | 1.29 | 0.66 | 3.30 | 0.01 | 0.10 | 0.51 | 0.27 | 0.01 | 0.00 |

