# E003 model check: growth table

One row per feature set (check.py). Percentages over Part I's counted moves; precision over certain predicted changes at the done frame and at the next move's start, with the certain hits and certain predictions behind it in brackets; recall over the tracked facts that changed, less the excluded ones (engine variables written outside opcode handlers, and the model's non-facts). Rows (model.ROWS, cumulative): 0 own_entry, 1 + dispatch and calls, 2 + room_change, 3 + settle, 4 + dialog, 5 + esc. Rows 0-2 run only sentences and walks, up to the end of the click's frame; row 3 lets time pass after them in rounds and runs waits the same way; row 4 runs a dialog line as a click on its verb (the input script, then settle); row 5 runs a skip as the Esc key (abortCutscene rewinds the armed override, the Esc frame runs, then settle). Dialog lines before row 4, and skips in rows 0-4, run nothing, so they count as accepted when usable and predict nothing (check-part1-rowN.log gives acceptance over the moves the model runs, the counts behind every number, the settle counts and precision split from row 3, the dialog counts from row 4, the Esc counts from row 5, and the ground truth file's hash). Every row runs with all switches off (model.SWITCHES).

| row | features | moves | accepted % | precision done % (hits/certain) | precision post % (hits/certain) | recall % | recall with possible % | moves with no prediction |
|---|---|---|---|---|---|---|---|---|
| 0 | own_entry | 239 | 94.6% | 42.3% (110/260) | 39.2% (102/260) | 1.9% | 2.0% | 111 |
| 1 | own_entry, dispatch, calls | 239 | 96.2% | 68.4% (141/206) | 68.0% (140/206) | 2.4% | 12.3% | 107 |
| 2 | own_entry, dispatch, calls, room_change | 239 | 96.2% | 68.4% (141/206) | 68.0% (140/206) | 2.4% | 34.3% | 107 |
| 3 | own_entry, dispatch, calls, room_change, settle | 239 | 96.2% | 83.9% (405/483) | 84.7% (409/483) | 6.9% | 42.3% | 89 |
| 4 | own_entry, dispatch, calls, room_change, settle, dialog | 239 | 96.2% | 73.2% (702/959) | 78.7% (755/959) | 11.9% | 53.3% | 30 |
| 5 | own_entry, dispatch, calls, room_change, settle, dialog, esc | 239 | 96.2% | 76.7% (924/1205) | 81.7% (985/1205) | 15.6% | 58.3% | 9 |
