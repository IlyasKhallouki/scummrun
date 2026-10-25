//! A list that forks in O(1): model.py's per-path lists (assumptions, notes, unknown writes, errors, turns,
//! ...), which only grow, are copied at every fork and joined at every merge.
//!
//! model.py copies a list at each fork (`list(self.assumptions)`: a pointer per item). With calls on, its merges
//! add each branch's own part again (`_merge`), so in a loop these lists grow at every merge: one unit reaches
//! 18 million items copied over its forks. Here the items live in frozen chunks of `CHUNK` items that forks share
//! (an `Rc` chain, newest first), plus a short owned tail. A fork copies the tail only. Chunk `i` always holds
//! items `i * CHUNK ..`, so two lists forked from one another share their chunks up to the fork, and the common
//! prefix `_merge` looks for is found chunk by chunk, by pointer, then item by item by value, as model.py
//! compares.

use std::rc::Rc;

const CHUNK: usize = 64;

struct Chunk<T> {
    prev: Option<Rc<Chunk<T>>>,
    items: Box<[T]>,
}

/// An append-only list whose clones share its frozen part.
pub struct PList<T> {
    head: Option<Rc<Chunk<T>>>,
    /// the number of items in chunks (a multiple of CHUNK)
    frozen: usize,
    tail: Vec<T>,
}

impl<T: Clone> Clone for PList<T> {
    fn clone(&self) -> Self {
        PList { head: self.head.clone(), frozen: self.frozen, tail: self.tail.clone() }
    }
}

impl<T> Default for PList<T> {
    fn default() -> Self {
        PList { head: None, frozen: 0, tail: Vec::new() }
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for PList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T> Drop for PList<T> {
    /// Drops a long chain without recursion: each chunk this list held last is taken apart in a loop.
    fn drop(&mut self) {
        let mut cur = self.head.take();
        while let Some(c) = cur {
            match Rc::try_unwrap(c) {
                Ok(mut chunk) => cur = chunk.prev.take(),
                Err(_) => break,
            }
        }
    }
}

impl<T> PList<T> {
    pub fn new() -> PList<T> {
        PList::default()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.frozen + self.tail.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&mut self, x: T) {
        self.tail.push(x);
        if self.tail.len() == CHUNK {
            let items = std::mem::replace(&mut self.tail, Vec::with_capacity(CHUNK)).into_boxed_slice();
            self.head = Some(Rc::new(Chunk { prev: self.head.take(), items }));
            self.frozen += CHUNK;
        }
    }

    pub fn last(&self) -> Option<&T> {
        self.tail.last().or_else(|| self.head.as_ref().map(|c| &c.items[CHUNK - 1]))
    }

    /// The items from index `k` on. Only the chunks from the one that holds item `k` are walked (none when it is in
    /// the tail).
    pub fn iter_from(&self, k: usize) -> impl Iterator<Item = &T> {
        let n = self.frozen / CHUNK;
        let first = (k / CHUNK).min(n);
        let mut chunks = Vec::new();
        let mut cur = self.head.as_deref();
        for _ in first..n {
            let c = cur.expect("a chunk");
            chunks.push(c);
            cur = c.prev.as_deref();
        }
        chunks.reverse();
        let skip = if first < n { k % CHUNK } else { k.saturating_sub(self.frozen) };
        let frozen = chunks.into_iter().flat_map(|c| c.items.iter());
        frozen.chain(self.tail.iter()).skip(skip)
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.iter_from(0)
    }
}

impl<T: Clone> PList<T> {
    /// Keeps the first `n` items (model.py `del list[n:]`).
    pub fn truncate(&mut self, n: usize) {
        if n >= self.len() {
            return;
        }
        if n >= self.frozen {
            self.tail.truncate(n - self.frozen);
            return;
        }
        // the chunk that holds item n becomes the tail, cut at n
        let k = n / CHUNK;
        let mut i = self.frozen / CHUNK - 1;
        let mut cur = self.head.clone().expect("a chunk");
        while i > k {
            cur = cur.prev.clone().expect("a chunk");
            i -= 1;
        }
        self.tail = cur.items[..n % CHUNK].to_vec();
        self.head = cur.prev.clone();
        self.frozen = k * CHUNK;
    }

    pub fn extend<'a>(&mut self, items: impl Iterator<Item = &'a T>)
    where
        T: 'a,
    {
        for x in items {
            self.push(x.clone());
        }
    }

    pub fn to_vec(&self) -> Vec<T> {
        self.iter().cloned().collect()
    }
}

/// Equality with a fast path: shared strings are compared by pointer first (the same test as `==`).
pub trait FastEq {
    fn fast_eq(&self, other: &Self) -> bool;
}

/// `a == b` for shared strings, by pointer first.
#[inline]
pub fn s_eq(a: &std::rc::Rc<str>, b: &std::rc::Rc<str>) -> bool {
    Rc::ptr_eq(a, b) || **a == **b
}

impl FastEq for (Rc<str>, Rc<str>, Rc<str>) {
    #[inline]
    fn fast_eq(&self, o: &Self) -> bool {
        s_eq(&self.0, &o.0) && s_eq(&self.1, &o.1) && s_eq(&self.2, &o.2)
    }
}

impl<A: PartialEq, B: PartialEq> FastEq for (A, B) {
    #[inline]
    fn fast_eq(&self, o: &Self) -> bool {
        self == o
    }
}

impl<A: PartialEq, B: PartialEq, C: PartialEq, D: PartialEq, E: PartialEq> FastEq for (A, B, C, D, E) {
    #[inline]
    fn fast_eq(&self, o: &Self) -> bool {
        self == o
    }
}

impl FastEq for usize {
    fn fast_eq(&self, o: &Self) -> bool {
        self == o
    }
}

impl<T: Clone + PartialEq + FastEq> PList<T> {
    /// The length of the common prefix of two lists, by value (model.py `_merge`'s `while mine[k] == theirs[k]`).
    /// Chunks the two share by pointer are equal; past them, items are compared.
    pub fn common_prefix(a: &PList<T>, b: &PList<T>) -> usize {
        // a chunk's predecessors are shared when it is: the shared chunks are a prefix. Walk both chains from their
        // newest chunk, the longer one first down to the other's length, then both until they meet
        let (na, nb) = (a.frozen / CHUNK, b.frozen / CHUNK);
        let (mut ca, mut cb) = (a.head.as_deref(), b.head.as_deref());
        for _ in nb..na {
            ca = ca.and_then(|c| c.prev.as_deref());
        }
        for _ in na..nb {
            cb = cb.and_then(|c| c.prev.as_deref());
        }
        let mut i = na.min(nb);
        while let (Some(x), Some(y)) = (ca, cb) {
            if std::ptr::eq(x, y) {
                break;
            }
            ca = x.prev.as_deref();
            cb = y.prev.as_deref();
            i -= 1;
        }
        let mut k = i * CHUNK;
        for (x, y) in a.iter_from(k).zip(b.iter_from(k)) {
            if !x.fast_eq(y) {
                break;
            }
            k += 1;
        }
        k
    }

    /// model.py `_merge`'s list rule: add to `self` what `theirs` holds past its common prefix with `mine`.
    pub fn add_branch(&mut self, mine: &PList<T>, theirs: &PList<T>) {
        let k = PList::common_prefix(mine, theirs);
        self.extend(theirs.iter_from(k));
    }
}

impl<T: Clone + Eq + std::hash::Hash + FastEq> PList<T> {
    /// model.py `_merge`'s list rule with settle: what `theirs` holds past its common prefix with `mine` and
    /// `seen` (the items `self` holds, made from it when first needed) does not, all of it checked before any is
    /// added (two equal new items both go in, as model.py's list comprehension takes them).
    pub fn add_branch_new(&mut self, mine: &PList<T>, theirs: &PList<T>, seen: &mut Option<rustc_hash::FxHashSet<T>>) {
        let k = PList::common_prefix(mine, theirs);
        if k >= theirs.len() {
            return;
        }
        let new = theirs.len() - k;
        if seen.is_none() && new * self.len() <= 512 {
            // a few items against a short list: compared one by one (the same test as the set's)
            let add: Vec<T> = theirs.iter_from(k).filter(|x| !self.iter().any(|y| y.fast_eq(x))).cloned().collect();
            for x in add {
                self.push(x);
            }
            return;
        }
        let set = seen.get_or_insert_with(|| self.iter().cloned().collect());
        let add: Vec<T> = theirs.iter_from(k).filter(|x| !set.contains(*x)).cloned().collect();
        for x in add {
            set.insert(x.clone());
            self.push(x);
        }
    }
}

impl<T: PartialEq> PartialEq for PList<T> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().zip(other.iter()).all(|(a, b)| a == b)
    }
}

impl<T: Eq> Eq for PList<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(n: usize) -> PList<usize> {
        let mut l = PList::new();
        for i in 0..n {
            l.push(i);
        }
        l
    }

    #[test]
    fn push_iter_truncate() {
        for n in [0, 1, 63, 64, 65, 200] {
            let l = list(n);
            assert_eq!(l.len(), n);
            assert_eq!(l.to_vec(), (0..n).collect::<Vec<_>>());
            assert_eq!(l.last().copied(), n.checked_sub(1));
            for k in [0, 1, 63, 64, 65, 130, 199, 200, 300] {
                assert_eq!(l.iter_from(k).copied().collect::<Vec<_>>(), (k.min(n)..n).collect::<Vec<_>>(), "{n} {k}");
                let mut t = l.clone();
                t.truncate(k);
                assert_eq!(t.to_vec(), (0..k.min(n)).collect::<Vec<_>>(), "{n} {k}");
                t.push(7);
                assert_eq!(t.len(), k.min(n) + 1);
                assert_eq!(l.to_vec(), (0..n).collect::<Vec<_>>());
            }
        }
    }

    #[test]
    fn prefixes() {
        let base = list(150);
        let mut a = base.clone();
        let mut b = base.clone();
        for i in 0..100 {
            a.push(1000 + i);
            b.push(if i < 30 { 1000 + i } else { 2000 + i }); // equal by value past the fork, then not
        }
        assert_eq!(PList::common_prefix(&a, &b), 180);
        assert_eq!(PList::common_prefix(&a, &base), 150);
        assert_eq!(PList::common_prefix(&list(10), &list(300)), 10);
        let mut q = a.clone();
        q.add_branch(&a, &b);
        assert_eq!(q.len(), 250 + 70);
        assert_eq!(q.iter_from(250).copied().collect::<Vec<_>>(), (30..100).map(|i| 2000 + i).collect::<Vec<_>>());
        let long = list(100_000);
        drop(long); // no recursion on a long chain
    }
}
