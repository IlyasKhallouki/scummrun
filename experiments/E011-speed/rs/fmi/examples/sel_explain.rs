//! Why archive entries are or are not selected (fmi-explore `Archive::explain_selection`), under the run's env.
//!
//! usage: sel_explain ARCHIVE ENTRY...

use std::path::PathBuf;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ar = fmi_explore::archive::Archive::open(&PathBuf::from(&a[1])).expect("the archive");
    for e in &a[2..] {
        println!("{}", ar.explain_selection(e.parse().expect("an entry id")));
    }
}
