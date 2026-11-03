//! play-lean: fmi-play's fixture tests (play-fixture recs|children|strict|ready FIXTURES [...], plan.md "play", tests
//! 2-4) on the lean build through fmi-engine's binary channel instead of the JSON double (G10's play part).
//! Built alone (`cargo build --release -p fmi-checks`), so the json-double feature never reaches the fmi binary.

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let host = fmi_engine::Host::from_env().expect("fmi-engine's host");
    let ok = fmi_play::fixture::run(&a, Box::new(host), true);
    std::process::exit(if ok { 0 } else { 1 });
}
