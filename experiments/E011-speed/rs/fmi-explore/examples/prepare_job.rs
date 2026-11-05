//! E021: `prepare_job ARCHIVE ENTRY`: a job folder for ENTRY as the driver makes one (job.json under the run's switches),
//! for a try run by hand (`fmi try DIR`). Prints the folder.

use fmi_explore::archive::Archive;
use std::path::Path;

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() != 3 {
        eprintln!("usage: prepare_job ARCHIVE ENTRY");
        std::process::exit(2);
    }
    let a = Archive::open(Path::new(&argv[1])).unwrap_or_else(|e| panic!("{}: {e}", argv[1]));
    let e: usize = argv[2].parse().expect("ENTRY: an entry id");
    let d = a.prepare_job(e).unwrap_or_else(|err| panic!("prepare_job {e}: {err}"));
    println!("{}", d.display());
}
