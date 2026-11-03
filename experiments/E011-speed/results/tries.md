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

## b4

| try | scope | return | idle | read | plan | play | rest | try | stop |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| j00000 | 4 | 2.05 / 1.40 | 8.40 / 8.30 | 265.32 / 653.40 | 186.17 / 184.70 | 21.51 / 32.90 | 0.45 / 0.00 | 483.90 / 870.64 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00001 | 5 | 2.43 / 1.70 | 8.80 / 8.60 | 11.67 / 23.60 | 5.21 / 5.10 | 0.02 / 0.00 | 0.17 / 0.00 | 28.30 / 37.82 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00002 | 5 | 2.12 / 1.50 | 7.90 / 7.80 | 11.34 / 25.50 | 4.41 / 4.40 | 0.15 / 0.10 | 0.08 / 0.00 | 26.00 / 37.99 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00003 | 5 | 2.84 / 2.20 | 9.70 / 9.60 | 11.78 / 25.80 | 4.59 / 4.50 | 0.43 / 0.50 | 0.06 / 0.00 | 29.40 / 40.78 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00004 | 5 | 2.20 / 1.60 | 9.00 / 8.80 | 4.15 / 9.30 | 5.21 / 5.20 | 0.17 / 0.10 | 0.07 / 0.00 | 20.80 / 23.41 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00005 | 5 | 2.03 / 1.40 | 0.00 / 0.00 | 4.07 / 9.50 | 5.49 / 5.40 | 0.16 / 0.10 | 0.05 / 1.06 | 11.80 / 17.46 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00006 | 4 | 2.36 / 1.70 | 7.70 / 7.60 | 69.65 / 170.30 | 105.35 / 104.50 | 6.84 / 7.60 | 0.20 / 0.00 | 192.10 / 290.79 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00007 | 5 | 2.42 / 1.80 | 8.30 / 8.10 | 4.46 / 10.40 | 5.26 / 5.20 | 1.71 / 2.30 | 0.05 / 0.00 | 22.20 / 25.67 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00008 | 4 | 2.07 / 1.40 | 8.50 / 8.30 | 63.63 / 152.70 | 105.61 / 104.80 | 5.08 / 5.70 | 0.21 / 0.00 | 185.10 / 272.12 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00009 | 4 | 2.56 / 1.90 | 0.00 / 0.00 | 70.92 / 172.80 | 125.87 / 124.50 | 6.93 / 7.80 | 0.22 / 1.48 | 206.50 / 308.48 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00010 | 5 | 2.22 / 1.50 | 7.90 / 7.80 | 4.51 / 10.80 | 5.83 / 5.70 | 1.78 / 2.30 | 0.06 / 0.00 | 22.30 / 26.12 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00011 | 4 | 2.32 / 1.70 | 8.20 / 8.00 | 57.70 / 139.40 | 102.91 / 102.20 | 4.96 / 5.60 | 0.11 / 0.00 | 176.20 / 255.95 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00012 | 4 | 2.08 / 1.40 | 0.00 / 0.00 | 56.09 / 135.50 | 100.79 / 100.10 | 4.63 / 5.20 | 0.21 / 1.55 | 163.80 / 243.75 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00013 | 5 | 2.12 / 1.50 | 9.50 / 9.30 | 5.12 / 11.90 | 6.32 / 6.20 | 2.10 / 2.70 | 0.04 / 0.00 | 25.20 / 29.02 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00014 | 4 | 2.59 / 2.00 | 9.30 / 9.10 | 57.23 / 140.30 | 111.66 / 110.60 | 5.21 / 6.00 | 0.21 / 0.00 | 186.20 / 266.72 | refinement: Lemma R's premise R0: the last pass wrote values |

| over | n | try wall median | try CPU median | try wall mean | try CPU mean | try wall max | return wall med | idle wall med | read wall med | plan wall med | play wall med | rest wall med |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 15 | 29.40 | 40.78 | 118.65 | 183.11 | 483.90 | 2.22 | 8.30 | 11.78 | 6.32 | 2.10 | 0.11 |
| ring33 | 7 | 186.20 | 272.12 | 227.69 | 358.35 | 483.90 | 2.32 | 8.20 | 63.63 | 105.61 | 5.21 | 0.21 |
| ring38 | 8 | 23.75 | 27.57 | 23.25 | 29.78 | 29.40 | 2.21 | 8.55 | 4.81 | 5.24 | 0.30 | 0.06 |

## a-ref

| try | scope | return | idle | read | plan | play | rest | try | stop |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| j00000 | 4 | 0.27 / 0.00 | 0.20 / 0.20 | 97.48 / 313.90 | 70.90 / 30.70 | 1.92 / 1.50 | 0.03 / 43.88 | 170.80 / 390.18 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00001 | 5 | 0.29 / 0.00 | 0.30 / 0.20 | 3.55 / 8.80 | 1.15 / 0.40 | 0.00 / 0.00 | 0.00 / 1.45 | 5.20 / 10.85 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00002 | 5 | 0.26 / 0.00 | 0.30 / 0.20 | 3.22 / 9.30 | 1.26 / 0.60 | 0.02 / 0.00 | 0.04 / 1.31 | 5.10 / 11.41 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00003 | 5 | 0.28 / 0.00 | 0.20 / 0.20 | 3.30 / 9.20 | 1.26 / 0.50 | 0.04 / 0.00 | 0.02 / 1.35 | 5.10 / 11.25 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00004 | 5 | 0.27 / 0.00 | 0.30 / 0.20 | 1.65 / 4.20 | 1.28 / 0.40 | 0.02 / 0.00 | 0.00 / 1.32 | 3.50 / 6.12 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00005 | 5 | 0.31 / 0.00 | 0.00 / 0.00 | 1.63 / 4.40 | 1.08 / 0.40 | 0.02 / 0.00 | 0.06 / 1.11 | 3.10 / 5.91 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00006 | 4 | 0.30 / 0.00 | 0.20 / 0.20 | 20.65 / 59.20 | 36.62 / 13.90 | 0.42 / 0.00 | 0.01 / 23.59 | 58.20 / 96.89 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00007 | 5 | 0.31 / 0.00 | 0.20 / 0.20 | 1.83 / 4.50 | 1.30 / 0.50 | 0.11 / 0.00 | 0.05 / 1.27 | 3.80 / 6.47 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00008 | 4 | 0.26 / 0.00 | 0.30 / 0.20 | 22.72 / 65.70 | 41.64 / 14.20 | 0.47 / 0.00 | 0.01 / 28.42 | 65.40 / 108.52 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00009 | 4 | 0.32 / 0.00 | 0.00 / 0.00 | 18.82 / 56.50 | 43.68 / 15.70 | 0.43 / 0.00 | 0.05 / 28.89 | 63.30 / 101.09 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00010 | 5 | 0.28 / 0.00 | 0.30 / 0.30 | 1.58 / 4.10 | 1.43 / 0.60 | 0.12 / 0.00 | 0.00 / 1.16 | 3.70 / 6.16 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00011 | 4 | 0.26 / 0.00 | 0.20 / 0.20 | 16.53 / 51.00 | 37.55 / 14.80 | 0.42 / 0.00 | 0.04 / 23.52 | 55.00 / 89.52 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00012 | 4 | 0.25 / 0.00 | 0.00 / 0.00 | 18.07 / 54.80 | 36.41 / 14.70 | 0.43 / 0.00 | 0.04 / 22.72 | 55.20 / 92.22 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00013 | 5 | 0.34 / 0.00 | 0.30 / 0.40 | 1.79 / 4.90 | 1.83 / 0.60 | 0.14 / 0.00 | 0.10 / 1.63 | 4.50 / 7.53 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00014 | 4 | 0.25 / 0.00 | 0.20 / 0.20 | 21.01 / 62.30 | 41.66 / 14.10 | 0.54 / 0.00 | 0.04 / 28.51 | 63.70 / 105.11 | refinement: Lemma R's premise R0: the last pass wrote values |

| over | n | try wall median | try CPU median | try wall mean | try CPU mean | try wall max | return wall med | idle wall med | read wall med | plan wall med | play wall med | rest wall med |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 15 | 5.20 | 11.41 | 37.71 | 69.95 | 170.80 | 0.28 | 0.20 | 3.55 | 1.83 | 0.14 | 0.04 |
| ring33 | 7 | 63.30 | 101.09 | 75.94 | 140.50 | 170.80 | 0.26 | 0.20 | 20.65 | 41.64 | 0.43 | 0.04 |
| ring38 | 8 | 4.15 | 7.00 | 4.25 | 8.21 | 5.20 | 0.29 | 0.30 | 1.81 | 1.27 | 0.03 | 0.03 |

## a-fast

| try | scope | return | idle | read | plan | play | rest | try | stop |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| j00000 | 4 | 0.38 / 0.00 | 0.30 / 0.30 | 110.57 / 348.80 | 21.27 / 21.30 | 0.66 / 0.00 | 0.00 / 3.91 | 133.10 / 374.31 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00001 | 5 | 0.23 / 0.00 | 0.20 / 0.20 | 2.95 / 7.50 | 0.65 / 0.60 | 0.00 / 0.00 | 0.07 / 0.60 | 4.10 / 8.90 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00002 | 5 | 0.30 / 0.00 | 0.30 / 0.30 | 3.30 / 9.10 | 0.49 / 0.50 | 0.01 / 0.00 | 0.00 / 0.64 | 4.40 / 10.54 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00003 | 5 | 0.24 / 0.00 | 0.20 / 0.20 | 3.17 / 8.80 | 0.41 / 0.40 | 0.02 / 0.00 | 0.06 / 0.63 | 4.10 / 10.03 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00004 | 5 | 0.26 / 0.00 | 0.20 / 0.20 | 1.68 / 4.00 | 0.47 / 0.50 | 0.02 / 0.00 | 0.07 / 0.31 | 2.70 / 5.01 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00005 | 5 | 0.27 / 0.00 | 0.00 / 0.00 | 1.33 / 3.40 | 0.39 / 0.40 | 0.02 / 0.00 | 0.09 / 0.48 | 2.10 / 4.28 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00006 | 4 | 0.25 / 0.00 | 0.20 / 0.20 | 20.48 / 61.60 | 17.36 / 17.20 | 0.37 / 0.00 | 0.14 / 0.71 | 38.80 / 79.71 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00007 | 5 | 0.27 / 0.00 | 0.20 / 0.20 | 1.55 / 4.10 | 0.47 / 0.50 | 0.02 / 0.00 | 0.09 / 0.31 | 2.60 / 5.11 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00008 | 4 | 0.42 / 0.00 | 0.30 / 0.30 | 20.30 / 61.00 | 17.59 / 17.70 | 0.65 / 0.00 | 0.04 / 0.90 | 39.30 / 79.90 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00009 | 4 | 0.30 / 0.00 | 0.00 / 0.00 | 17.77 / 54.70 | 15.89 / 15.90 | 0.38 / 0.00 | 0.06 / 0.65 | 34.40 / 71.25 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00010 | 5 | 0.30 / 0.00 | 0.30 / 0.20 | 1.47 / 4.10 | 0.42 / 0.40 | 0.02 / 0.00 | 0.00 / 0.37 | 2.50 / 5.07 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00011 | 4 | 0.26 / 0.00 | 0.20 / 0.20 | 16.80 / 52.20 | 14.56 / 14.50 | 0.35 / 0.00 | 0.03 / 0.57 | 32.20 / 67.47 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00012 | 4 | 0.30 / 0.00 | 0.00 / 0.00 | 21.13 / 61.70 | 15.45 / 15.60 | 0.34 / 0.00 | 0.00 / 0.68 | 37.20 / 77.98 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00013 | 5 | 0.26 / 0.00 | 0.20 / 0.20 | 1.77 / 4.80 | 0.53 / 0.50 | 0.02 / 0.00 | 0.02 / 0.33 | 2.80 / 5.83 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00014 | 4 | 0.37 / 0.00 | 0.30 / 0.30 | 20.57 / 61.30 | 16.67 / 16.70 | 0.32 / 0.00 | 0.07 / 0.79 | 38.30 / 79.09 | refinement: Lemma R's premise R0: the last pass wrote values |

| over | n | try wall median | try CPU median | try wall mean | try CPU mean | try wall max | return wall med | idle wall med | read wall med | plan wall med | play wall med | rest wall med |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 15 | 4.40 | 10.54 | 25.24 | 58.97 | 133.10 | 0.27 | 0.20 | 3.30 | 0.65 | 0.02 | 0.06 |
| ring33 | 7 | 38.30 | 79.09 | 50.47 | 118.53 | 133.10 | 0.30 | 0.20 | 20.48 | 16.67 | 0.37 | 0.04 |
| ring38 | 8 | 2.75 | 5.47 | 3.16 | 6.85 | 4.40 | 0.27 | 0.20 | 1.73 | 0.47 | 0.02 | 0.06 |

## b-ref

| try | scope | return | idle | read | plan | play | rest | try | stop |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| j00000 | 4 | 0.38 / 0.00 | 0.30 / 0.30 | 101.62 / 326.50 | 66.29 / 30.60 | 1.79 / 1.50 | 0.00 / 38.53 | 170.30 / 397.43 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00001 | 5 | 0.01 / 0.00 | 0.30 / 0.30 | 3.01 / 7.70 | 0.99 / 0.50 | 0.00 / 0.00 | 0.00 / 0.77 | 4.30 / 9.27 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00002 | 5 | 0.02 / 0.00 | 0.20 / 0.20 | 2.25 / 6.10 | 1.23 / 0.50 | 0.02 / 0.00 | 0.08 / 0.98 | 3.80 / 7.78 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00003 | 5 | 0.02 / 0.00 | 0.30 / 0.30 | 1.75 / 3.30 | 1.11 / 0.50 | 0.04 / 0.00 | 0.08 / 0.78 | 3.30 / 4.88 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00004 | 5 | 0.00 / 0.00 | 0.20 / 0.20 | 0.14 / 0.10 | 1.08 / 0.40 | 0.01 / 0.00 | 0.00 / 0.63 | 1.40 / 1.33 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00005 | 5 | 0.00 / 0.00 | 0.00 / 0.00 | 0.05 / 0.00 | 1.27 / 0.50 | 0.02 / 0.00 | 0.00 / 0.80 | 1.30 / 1.30 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00006 | 4 | 0.01 / 0.00 | 0.30 / 0.20 | 7.38 / 15.80 | 39.60 / 17.90 | 0.44 / 0.00 | 0.00 / 21.71 | 47.70 / 55.61 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00007 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 0.76 / 1.40 | 1.34 / 0.40 | 0.12 / 0.00 | 0.07 / 0.96 | 2.50 / 2.96 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00008 | 4 | 0.00 / 0.00 | 0.20 / 0.20 | 0.03 / 0.00 | 46.40 / 16.50 | 0.58 / 0.00 | 0.00 / 30.18 | 47.20 / 46.88 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00009 | 4 | 0.01 / 0.00 | 0.00 / 0.00 | 0.04 / 0.00 | 41.83 / 15.90 | 0.55 / 0.00 | 0.07 / 26.23 | 42.50 / 42.13 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00010 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 0.03 / 0.00 | 1.36 / 0.50 | 0.12 / 0.00 | 0.08 / 0.91 | 1.80 / 1.61 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00011 | 4 | 0.00 / 0.00 | 0.20 / 0.20 | 0.02 / 0.00 | 37.16 / 14.10 | 0.43 / 0.00 | 0.00 / 23.13 | 37.80 / 37.43 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00012 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.03 / 0.00 | 3.21 / 3.40 | 0.47 / 0.00 | 0.00 / 0.28 | 3.70 / 3.68 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00013 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 0.03 / 0.00 | 1.75 / 0.50 | 0.13 / 0.00 | 0.00 / 1.28 | 2.10 / 1.98 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00014 | 4 | 0.01 / 0.00 | 0.40 / 0.40 | 0.02 / 0.00 | 43.49 / 14.90 | 0.70 / 0.00 | 0.00 / 28.79 | 44.60 / 44.09 | refinement: Lemma R's premise R0: the last pass wrote values |

| over | n | try wall median | try CPU median | try wall mean | try CPU mean | try wall max | return wall med | idle wall med | read wall med | plan wall med | play wall med | rest wall med |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 15 | 3.80 | 7.78 | 27.62 | 43.89 | 170.30 | 0.01 | 0.20 | 0.05 | 1.75 | 0.13 | 0.00 |
| ring33 | 7 | 44.60 | 44.09 | 56.26 | 89.61 | 170.30 | 0.01 | 0.20 | 0.03 | 41.83 | 0.55 | 0.00 |
| ring38 | 8 | 2.30 | 2.47 | 2.56 | 3.89 | 4.30 | 0.01 | 0.20 | 0.45 | 1.25 | 0.03 | 0.03 |

## b-fast

| try | scope | return | idle | read | plan | play | rest | try | stop |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| j00000 | 4 | 0.27 / 0.00 | 0.20 / 0.20 | 97.17 / 319.60 | 16.88 / 17.10 | 0.45 / 0.00 | 0.03 / 2.53 | 115.00 / 339.43 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00001 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 3.04 / 8.10 | 0.56 / 0.60 | 0.00 / 0.00 | 0.00 / 0.20 | 3.80 / 9.10 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00002 | 5 | 0.02 / 0.00 | 0.30 / 0.30 | 3.07 / 7.70 | 0.41 / 0.50 | 0.02 / 0.00 | 0.00 / 0.10 | 3.80 / 8.60 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00003 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 1.45 / 2.70 | 0.43 / 0.50 | 0.02 / 0.00 | 0.00 / 0.12 | 2.10 / 3.52 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00004 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 0.14 / 0.10 | 0.38 / 0.40 | 0.02 / 0.00 | 0.05 / 0.03 | 0.80 / 0.73 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00005 | 5 | 0.00 / 0.00 | 0.00 / 0.00 | 0.05 / 0.10 | 0.39 / 0.40 | 0.01 / 0.00 | 0.05 / 0.01 | 0.50 / 0.51 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00006 | 4 | 0.01 / 0.00 | 0.20 / 0.20 | 9.53 / 21.00 | 15.59 / 15.50 | 0.34 / 0.00 | 0.00 / 0.08 | 25.60 / 36.78 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00007 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 0.67 / 1.40 | 0.52 / 0.50 | 0.02 / 0.00 | 0.00 / 0.00 | 1.40 / 2.00 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00008 | 4 | 0.01 / 0.00 | 0.40 / 0.30 | 11.15 / 24.00 | 22.31 / 22.10 | 0.85 / 0.00 | 0.00 / 0.14 | 34.70 / 46.54 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00009 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.03 / 0.00 | 17.03 / 17.00 | 0.45 / 0.00 | 0.00 / 0.20 | 17.50 / 17.20 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00010 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 0.02 / 0.00 | 0.47 / 0.50 | 0.02 / 0.00 | 0.00 / 0.00 | 0.70 / 0.59 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00011 | 4 | 0.00 / 0.00 | 0.20 / 0.20 | 0.03 / 0.00 | 12.87 / 12.90 | 0.40 / 0.00 | 0.00 / 0.00 | 13.50 / 13.07 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00012 | 4 | 0.00 / 0.00 | 0.00 / 0.00 | 0.03 / 0.00 | 3.23 / 3.30 | 0.35 / 0.00 | 0.00 / 0.10 | 3.60 / 3.40 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00013 | 5 | 0.01 / 0.00 | 0.20 / 0.20 | 0.03 / 0.00 | 0.47 / 0.50 | 0.02 / 0.00 | 0.07 / 0.00 | 0.80 / 0.66 | refinement: Lemma R's premise R0: the last pass wrote values |
| j00014 | 4 | 0.01 / 0.00 | 0.30 / 0.30 | 0.02 / 0.00 | 16.03 / 16.10 | 0.35 / 0.00 | 0.09 / 0.00 | 16.80 / 16.31 | refinement: Lemma R's premise R0: the last pass wrote values |

| over | n | try wall median | try CPU median | try wall mean | try CPU mean | try wall max | return wall med | idle wall med | read wall med | plan wall med | play wall med | rest wall med |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 15 | 3.80 | 8.60 | 16.04 | 33.23 | 115.00 | 0.01 | 0.20 | 0.14 | 0.56 | 0.02 | 0.00 |
| ring33 | 7 | 17.50 | 17.20 | 32.39 | 67.53 | 115.00 | 0.01 | 0.20 | 0.03 | 16.03 | 0.40 | 0.00 |
| ring38 | 8 | 1.10 | 1.36 | 1.74 | 3.21 | 3.80 | 0.01 | 0.20 | 0.41 | 0.45 | 0.02 | 0.00 |

