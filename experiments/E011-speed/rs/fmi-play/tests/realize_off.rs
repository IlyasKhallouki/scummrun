//! E015 G1: with FMI_REALIZE unset the routine refuses to run (this test file runs in its own process, so no other
//! test sets the switch).

use fmi_core::*;
use fmi_play::realize::{self, Cfg, Step};

#[test]
fn switch_off_refuses() {
    std::env::remove_var(realize::SWITCH);
    assert!(!realize::on());
    // the check comes before the engine is touched: an engine that fails every command would surface otherwise
    struct Dead(Status);
    impl Engine for Dead {
        fn pid(&self) -> u32 { 0 }
        fn status(&self) -> &Status { &self.0 }
        fn run(&mut self, _s: &RunSpec) -> EResult<Status> { panic!("run") }
        fn inputs(&mut self, _e: &[Event]) -> EResult<()> { panic!("inputs") }
        fn obs(&mut self, _r: &ObsReq) -> EResult<Obs> { panic!("obs") }
        fn facts(&mut self) -> EResult<Facts> { panic!("facts") }
        fn holds(&mut self, _c: &[Cond]) -> EResult<bool> { panic!("holds") }
        fn at(&mut self, _x: i32, _y: i32) -> EResult<At> { panic!("at") }
        fn set_rng(&mut self, _s: u32) -> EResult<()> { panic!("set_rng") }
        fn segment(&mut self, _o: bool) -> EResult<()> { panic!("segment") }
        fn runs(&mut self) -> EResult<RunsReply> { panic!("runs") }
        fn explain(&mut self, _a: Option<&ExplainArm>) -> EResult<()> { panic!("explain") }
        fn explain_log(&mut self) -> EResult<ExplainReply> { panic!("explain_log") }
        fn fork(&mut self) -> EResult<Box<dyn Engine>> { panic!("fork") }
        fn fork_each(&mut self, _n: u32) -> EResult<(Status, Vec<Box<dyn Engine>>)> { panic!("fork_each") }
        fn state_json(&mut self, _r: &ObsReq) -> EResult<String> { panic!("state_json") }
        fn cpu_s(&self) -> f64 { 0.0 }
        fn quit(self: Box<Self>) {}
    }
    let mut e: Box<dyn Engine> = Box::new(Dead(Status::default()));
    for s in [Step::Dialog { v: 120 }, Step::Sentence { v: 10, a: 596, b: None }] {
        match realize::realize(&mut e, &s, &Cfg::default()) {
            Err(EngineError::Bridge(m)) => assert!(m.contains(realize::SWITCH)),
            _ => panic!("ran with the switch off"),
        }
    }
}
