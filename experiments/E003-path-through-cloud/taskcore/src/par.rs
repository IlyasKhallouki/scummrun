//! Work split over threads with results in a fixed order (E011 round 2): the same output whatever the number of threads
//! and whatever thread finishes first, since every item's result is computed alone and the results are put back in
//! item order.

use std::sync::atomic::{AtomicUsize, Ordering};

/// The number of threads the fast forms use: TASKCORE_THREADS, else 4 (at least 1).
pub fn threads() -> usize {
    std::env::var("TASKCORE_THREADS").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(4).max(1)
}

/// `f` on each item of 0..n, items taken one at a time by `t` threads (each with its own state from `init`), the
/// results in item order.
pub fn map_dyn<S, T, I, F>(n: usize, t: usize, init: I, f: F) -> Vec<T>
where
    T: Send,
    I: Fn() -> S + Sync,
    F: Fn(&mut S, usize) -> T + Sync,
{
    if t <= 1 || n < 2 {
        let mut s = init();
        return (0..n).map(|i| f(&mut s, i)).collect();
    }
    let next = AtomicUsize::new(0);
    let mut parts: Vec<Vec<(usize, T)>> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..t.min(n))
            .map(|_| {
                sc.spawn(|| {
                    let mut s = init();
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= n {
                            break;
                        }
                        out.push((i, f(&mut s, i)));
                    }
                    out
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("a worker thread panicked")).collect()
    });
    let mut all: Vec<Option<T>> = (0..n).map(|_| None).collect();
    for p in parts.iter_mut() {
        for (i, x) in p.drain(..) {
            all[i] = Some(x);
        }
    }
    all.into_iter().map(|x| x.expect("every item done")).collect()
}

/// `f` on `t` contiguous chunks of 0..n (each `f(state, range)`), the results in chunk order.
pub fn map_chunks<S, T, I, F>(n: usize, t: usize, init: I, f: F) -> Vec<T>
where
    T: Send,
    I: Fn() -> S + Sync,
    F: Fn(&mut S, std::ops::Range<usize>) -> T + Sync,
{
    let t = t.max(1).min(n.max(1));
    let size = n.div_ceil(t);
    let chunks: Vec<std::ops::Range<usize>> = (0..t).map(|k| (k * size).min(n)..((k + 1) * size).min(n)).collect();
    map_dyn(chunks.len(), t, init, |s, k| f(s, chunks[k].clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_in_order() {
        for t in [1, 2, 3, 8] {
            let v = map_dyn(100, t, || 0u64, |s, i| {
                *s += 1;
                i * i
            });
            assert_eq!(v, (0..100).map(|i| i * i).collect::<Vec<_>>());
            let c = map_chunks(10, t, || (), |_, r| r.collect::<Vec<_>>());
            assert_eq!(c.concat(), (0..10).collect::<Vec<_>>());
        }
    }
}
