//! reader-scope: one reading of a scope by its command line (the library's `run` module has the reading and its
//! usage: src/run/mod.rs), in a process of its own.

/// mimalloc: the workers allocate and free many small objects at once, and the sink thread frees what they made
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    scumm_reader::run::main_of(&args);
}
