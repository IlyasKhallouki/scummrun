//! This thread's instruction and cycle counts (Linux perf_event_open, user space only), for measuring on a shared
//! machine where wall and CPU seconds move with the load: instructions do not move, and cycles move less than seconds.
//! When the counters cannot be opened (another OS, or perf_event_paranoid above 2), every reading is 0.

use std::sync::OnceLock;

#[repr(C)]
#[derive(Default)]
struct Attr {
    kind: u32,
    size: u32,
    config: u64,
    sample: u64,
    sample_type: u64,
    read_format: u64,
    flags: u64,
    wakeup: u32,
    bp_type: u32,
    config1: u64,
    config2: u64,
    branch_sample_type: u64,
    sample_regs_user: u64,
    sample_stack_user: u32,
    clockid: i32,
    sample_regs_intr: u64,
    aux_watermark: u32,
    sample_max_stack: u16,
    reserved2: u16,
    aux_sample_size: u32,
    reserved3: u32,
    sig_data: u64,
    config3: u64,
}

extern "C" {
    fn syscall(n: i64, ...) -> i64;
    fn read(fd: i32, buf: *mut u8, n: usize) -> isize;
}

const SYS_PERF_EVENT_OPEN: i64 = 298;

fn open(config: u64) -> i32 {
    let mut a = Attr { kind: 0, size: std::mem::size_of::<Attr>() as u32, config, ..Default::default() };
    // exclude_kernel (bit 5), exclude_hv (bit 6)
    a.flags = (1 << 5) | (1 << 6);
    // SAFETY: perf_event_open(attr, pid 0 = this thread, cpu -1, group -1, flags 0); a negative result is an error
    let fd = unsafe { syscall(SYS_PERF_EVENT_OPEN, &a as *const Attr, 0i32, -1i32, -1i32, 0u64) };
    if fd < 0 {
        -1
    } else {
        fd as i32
    }
}

thread_local! {
    static FDS: (i32, i32) = (open(1), open(0));
}

static OFF: OnceLock<bool> = OnceLock::new();

fn value(fd: i32) -> u64 {
    if fd < 0 {
        return 0;
    }
    let mut b = [0u8; 8];
    // SAFETY: read 8 bytes from a perf event fd into b
    let n = unsafe { read(fd, b.as_mut_ptr(), 8) };
    if n == 8 {
        u64::from_ne_bytes(b)
    } else {
        0
    }
}

/// (instructions, cycles) of this thread so far, user space only; (0, 0) when unavailable or TASKCORE_NO_PERF is set.
pub fn now() -> (u64, u64) {
    if *OFF.get_or_init(|| std::env::var_os("TASKCORE_NO_PERF").is_some()) {
        return (0, 0);
    }
    FDS.with(|&(i, c)| (value(i), value(c)))
}

/// Millions of (instructions, cycles) since `from`.
pub fn since(from: (u64, u64)) -> (f64, f64) {
    let n = now();
    ((n.0.wrapping_sub(from.0)) as f64 / 1e6, (n.1.wrapping_sub(from.1)) as f64 / 1e6)
}
