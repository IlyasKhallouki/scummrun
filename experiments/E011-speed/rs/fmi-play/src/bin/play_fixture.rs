//! fmi-play's differential tests against the fixtures experiments/E011-speed/play/capture.py records from E009's
//! step test (a) (plan.md, "play", tests 1-4), through the test doubles (svm-w3b over the JSON protocol, the reading's
//! four files).
//!
//! usage: play-fixture recs FIXTURES            the plan's records as the search tracks them, against the capture
//!        play-fixture children FIXTURES [W]    test 2: every child from its fork frame, the capture's shared reads
//!        play-fixture strict FIXTURES [RUNS..] test 3: the three plans' windows, strict, fork order; the records
//!                                              against the capture's and other recorded runs' windows.jsonl
//!        play-fixture ready FIXTURES           test 4: the same with play when ready
//!        play-fixture frames FIXTURES N        the observed child frame's cost, N frames from test (a)'s third window
//! FIXTURES is the capture's output folder (plans.jsonl, windows.jsonl, children.jsonl, step-a/).

use fmi_play::json_engine::JsonHost;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    // FMI_LEAN=1: the bridge's lean switches (a build with patch 0002, FAST_MI_SCUMMVM_BIN)
    let lean = std::env::var("FMI_LEAN").is_ok_and(|v| v == "1");
    let ok = fmi_play::fixture::run(&a, Box::new(JsonHost::default()), lean);
    std::process::exit(if ok { 0 } else { 1 });
}
