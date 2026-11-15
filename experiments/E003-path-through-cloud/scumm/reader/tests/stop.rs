//! E026 C2 (experiments/E026-part1/plan.md): a stopped reading stops. A machine whose stop flag is set ends its run
//! within `STOP_LOOK` steps by an unwind with `STOPPED`; a pool whose flag is set ends the reading and its workers
//! with it; a flag that is there and never set changes nothing, record for record.

use scumm_machine::machine::STOPPED;
use scumm_reader::oracle::{self, Static};
use scumm_reader::par;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock};

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

fn statics() -> &'static Static {
    static S: OnceLock<Static> = OnceLock::new();
    S.get_or_init(Static::load)
}

fn text(e: &Box<dyn std::any::Any + Send>) -> String {
    e.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| e.downcast_ref::<String>().cloned()).unwrap_or_default()
}

#[test]
fn a_machine_whose_flag_is_set_ends_its_run() {
    let mut b = oracle::builder(game(), statics(), "room33");
    let mut m = scumm_reader::machine::Reader::new(game());
    m.m.stop = Some(Arc::new(AtomicBool::new(true)));
    let r = panic::catch_unwind(AssertUnwindSafe(|| b.read_all_into(&mut m, true, None, 1, None)));
    assert_eq!(text(&r.expect_err("the reading went on")), STOPPED);
}

#[test]
fn a_pool_whose_flag_is_set_stops_with_its_workers() {
    for threads in [0, 3] {
        let mut b = oracle::builder(game(), statics(), "room33");
        let scope = b.scope.clone();
        let flag = Arc::new(AtomicBool::new(true));
        let r = panic::catch_unwind(AssertUnwindSafe(|| {
            par::with_pool(game(), &scope, threads, |pool| {
                pool.set_abort(flag.clone());
                b.read_all_into(pool, true, None, 2, None);
            })
        }));
        assert_eq!(text(&r.expect_err("the reading went on")), STOPPED, "{threads} threads");
    }
}

#[test]
fn a_flag_never_set_changes_no_record() {
    let read = |flag: Option<Arc<AtomicBool>>| {
        let mut b = oracle::builder(game(), statics(), "room33");
        b.keep_records = true;
        let scope = b.scope.clone();
        par::with_pool(game(), &scope, 3, |pool| {
            if let Some(f) = flag {
                pool.set_abort(f);
            }
            b.read_all_into(pool, true, None, 2, None);
        });
        (b.records.iter().map(|r| r.py().text()).collect::<Vec<String>>(), b.counters_py().text())
    };
    let (a, b) = (read(None), read(Some(Arc::new(AtomicBool::new(false)))));
    assert_eq!(a.0.len(), 878);
    assert_eq!(a, b, "the records and the counters");
}
