#!/bin/bash
# FD's and the Rust search's progress lines on the three ring(38) goals neither ends in 20 s, 120 s each
DW=/home/ilyask/projects/fast-mi/third_party/downward/builds/release/bin/downward
FF='let(hff,eval_modify_costs(ff(),cost_type=plusone),lazy_greedy([hff],preferred=[hff],reopen_closed=false))'
TCB=/var/tmp/fast-mi/e011-plan-scratch/taskcore-trace
cp /var/tmp/fast-mi/wt-e011-plan/experiments/E003-path-through-cloud/taskcore/target/release/taskcore $TCB
TC=/home/ilyask/.cache/fast-mi-e011/profile-planner/readings/r38/reading.tc
J=/home/ilyask/.cache/fast-mi-e008/archives/kitchen/jobs
for g in bit:118=1 var:53=-1 var:52=-1; do
  d=/var/tmp/fast-mi/e011-plan-scratch/trace38/$(echo $g | tr ':=' '__'); mkdir -p $d; cd $d
  $TCB sas --task $TC --start-file $J/j00091/run/a00/plans/plan-000/start.txt --cost-caps $J/j00091/run/a00/plans/plan-000/caps.txt --goal $g --start-check needed --out task.sas
  timeout 120 $DW --search "$FF" --internal-plan-file sas_plan < task.sas > fd.log 2>&1
  grep -E "New best|^\[.*\] g=|Initial heuristic" fd.log | sed -E 's/^\[t=[^]]*\] //' > fd_trace.txt
  TASKCORE_TRACE=1 timeout 120 $TCB search --sas task.sas --plan rust_plan > rust.json 2> rust_trace.txt
  if diff -q fd_trace.txt rust_trace.txt > /dev/null; then echo "same trace $g: $(wc -l < fd_trace.txt) lines, last: $(tail -1 fd_trace.txt)"; else echo "DIFF trace $g"; diff fd_trace.txt rust_trace.txt | head -5; fi
done
