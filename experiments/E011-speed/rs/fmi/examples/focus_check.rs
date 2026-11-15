//! E022 amendment (7): the plan focus (fmi-explore focus.rs) of a reading, a start file and one goal value.
//!
//! usage: focus_check READING_DIR START_TXT KEY VALUE

use fmi_core::{Facts, Key};
use std::path::Path;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rd = fmi_reading::Reading::open(Path::new(&a[1])).expect("the reading");
    let text = std::fs::read_to_string(&a[2]).expect("the start file");
    let pairs = text
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .filter_map(|(k, v)| v.parse::<i64>().ok().map(|v| (Key::parse(k), v)))
        .collect();
    let goal = vec![(Key::parse(&a[3]), vec![a[4].parse().expect("the goal value")])];
    let t = Instant::now();
    match fmi_explore::focus::focus(&rd, &Facts::from_pairs(pairs), &goal) {
        Some(f) => println!(
            "{{\"families\": {:?}, \"actions\": {}, \"rounds\": {}, \"bans\": {}, \"seconds\": {:.3}}}",
            f.families,
            f.actions,
            f.rounds,
            f.bans.len(),
            t.elapsed().as_secs_f64()
        ),
        None => println!("{{\"focus\": null, \"seconds\": {:.3}}}", t.elapsed().as_secs_f64()),
    }
}
