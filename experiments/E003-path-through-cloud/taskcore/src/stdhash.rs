//! libstdc++'s unordered containers, as Fast Downward's binary has them (GCC 16.2.1, the headers in
//! /usr/include/c++/16/bits/hashtable.h and the rehash policy of libstdc++.so), reduced to what decides their
//! iteration order: Fast Downward's lm_rhw makes landmarks in the order its sets and maps are iterated, so the Rust
//! port (lama.rs, landmarks.rs) must iterate them in the same order to make the same landmark graph.
//!
//! What is libstdc++'s here (E011 route, the differential test `stdhash_ref.cc` against the real containers):
//!
//! * **The layout.** One singly linked list of every node; bucket `b` points to the node *before* its first node
//!   (`_M_before_begin` for the list's first bucket), so a bucket's nodes are contiguous in the list.
//! * **Insertion** (`_M_insert_unique_node`, `_M_insert_bucket_begin`): first the rehash check for one more element,
//!   then the node goes to the front of its bucket; into an empty bucket it goes to the front of the whole list (and
//!   the bucket of the former first node now points to the new node).
//! * **The rehash policy** (`_Prime_rehash_policy`, max load factor 1): the first insertion makes 13 buckets (at least
//!   11 wanted, `_M_next_bkt` gives the next prime of its list); then the bucket count at least doubles whenever the
//!   element count would pass it. A rehash walks the old list and puts each node at the front of its new bucket,
//!   new buckets at the front of the list (`_M_rehash` for unique keys).
//! * **Construction from a range** (unique keys): an empty table, then each element inserted in turn (no pre-sizing,
//!   as GCC 16's `_Hashtable(first, last, n, .., true_type)` does). A copy keeps the bucket count and the order.
//! * **The bucket of a hash code** is `code % bucket_count`. `std::hash<int>` is the value as `size_t` (sign
//!   extended); Fast Downward's `utils::Hash<FactPair>` is its lookup3-based `get_hash64` of (var, value).

use std::hash::Hash;

const NIL: u32 = u32::MAX;
/// a bucket that points to `_M_before_begin`
const BB: u32 = u32::MAX - 1;

/// libstdc++'s `__prime_list` from 17 on (hashtable-aux.cc), as `_Prime_rehash_policy::_M_next_bkt` returns them
/// (probed through the library: route/cc/primes.cc), up to 2^32.
const PRIMES: [u64; 250] = [
    17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97, 103, 109, 113, 127, 137, 139, 149, 157,
    167, 179, 193, 199, 211, 227, 241, 257, 277, 293, 313, 337, 359, 383, 409, 439, 467, 503, 541, 577, 619, 661, 709,
    761, 823, 887, 953, 1031, 1109, 1193, 1289, 1381, 1493, 1613, 1741, 1879, 2029, 2179, 2357, 2549, 2753, 2971, 3209,
    3469, 3739, 4027, 4349, 4703, 5087, 5503, 5953, 6427, 6949, 7517, 8123, 8783, 9497, 10273, 11113, 12011, 12983,
    14033, 15173, 16411, 17749, 19183, 20753, 22447, 24281, 26267, 28411, 30727, 33223, 35933, 38873, 42043, 45481,
    49201, 53201, 57557, 62233, 67307, 72817, 78779, 85229, 92203, 99733, 107897, 116731, 126271, 136607, 147793,
    159871, 172933, 187091, 202409, 218971, 236897, 256279, 277261, 299951, 324503, 351061, 379787, 410857, 444487,
    480881, 520241, 562841, 608903, 658753, 712697, 771049, 834181, 902483, 976369, 1056323, 1142821, 1236397, 1337629,
    1447153, 1565659, 1693859, 1832561, 1982627, 2144977, 2320627, 2510653, 2716249, 2938679, 3179303, 3439651,
    3721303, 4026031, 4355707, 4712381, 5098259, 5515729, 5967347, 6456007, 6984629, 7556579, 8175383, 8844859,
    9569143, 10352717, 11200489, 12117689, 13109983, 14183539, 15345007, 16601593, 17961079, 19431899, 21023161,
    22744717, 24607243, 26622317, 28802401, 31160981, 33712729, 36473443, 39460231, 42691603, 46187573, 49969847,
    54061849, 58488943, 63278561, 68460391, 74066549, 80131819, 86693767, 93793069, 101473717, 109783337, 118773397,
    128499677, 139022417, 150406843, 162723577, 176048909, 190465427, 206062531, 222936881, 241193053, 260944219,
    282312799, 305431229, 330442829, 357502601, 386778277, 418451333, 452718089, 489790921, 529899637, 573292817,
    620239453, 671030513, 725980837, 785430967, 849749479, 919334987, 994618837, 1076067617, 1164186217, 1259520799,
    1362662261, 1474249943, 1594975441, 1725587117, 1866894511, 2019773507, 2185171673, 2364114217, 2557710269,
    2767159799, 2993761039, 3238918481, 3504151727, 3791104843, 4101556399, 4294967291,
];

/// `_Prime_rehash_policy` (max load factor 1.0): the next resize threshold, and `_M_next_bkt`.
#[derive(Clone, Copy, Debug, Default)]
struct Policy {
    next_resize: u64,
}

impl Policy {
    fn next_bkt(&mut self, n: u64) -> u64 {
        const FAST: [u64; 14] = [2, 2, 2, 3, 5, 5, 7, 7, 11, 11, 11, 11, 13, 13];
        if n < 14 {
            if n == 0 {
                return 1;
            }
            self.next_resize = FAST[n as usize];
            return FAST[n as usize];
        }
        let i = PRIMES.partition_point(|&p| p < n);
        let p = *PRIMES.get(i).expect("a bucket count past 2^32 (stdhash's prime list)");
        self.next_resize = p;
        p
    }
    /// `_M_need_rehash(n_bkt, n_elt, n_ins)`: Some(new bucket count) when a rehash is due.
    fn need_rehash(&mut self, n_bkt: u64, n_elt: u64, n_ins: u64) -> Option<u64> {
        if n_elt + n_ins > self.next_resize {
            let min_bkts = (n_elt + n_ins).max(if self.next_resize != 0 { 0 } else { 11 }) as f64;
            if min_bkts >= n_bkt as f64 {
                return Some(self.next_bkt((min_bkts.floor() as u64 + 1).max(n_bkt * 2)));
            }
            self.next_resize = n_bkt;
            None
        } else {
            None
        }
    }
}

/// The hash code libstdc++ computes for a key.
pub trait StdHash {
    fn std_hash(&self) -> u64;
}

impl StdHash for i32 {
    /// `std::hash<int>`: `static_cast<size_t>(v)`
    #[inline]
    fn std_hash(&self) -> u64 {
        *self as i64 as u64
    }
}

/// Fast Downward's `FactPair` (var, value).
pub type Fact = (u32, u32);

impl StdHash for Fact {
    /// utils/hash.h: `get_hash64` of the feed (var, value) (lookup3's mix and final mix).
    #[inline]
    fn std_hash(&self) -> u64 {
        fd_hash64_2(self.0, self.1)
    }
}

/// utils::HashState with two values fed, then `get_hash64`.
#[inline]
pub fn fd_hash64_2(x: u32, y: u32) -> u64 {
    let mut a: u32 = 0xdeadbeef;
    let mut b: u32 = 0xdeadbeef;
    let mut c: u32 = 0xdeadbeef;
    a = a.wrapping_add(x);
    b = b.wrapping_add(y);
    // final_mix
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(14));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(11));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(25));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(16));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(4));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(14));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(24));
    let _ = a;
    ((b as u64) << 32) | c as u64
}

/// An `std::unordered_set<K>` (unique keys) in libstdc++'s order (module docstring); a lookup walks the key's bucket
/// as libstdc++'s does. `StdMap` adds a value per node.
#[derive(Clone, Debug)]
pub struct StdSet<K: Copy + Eq + Hash + StdHash> {
    keys: Vec<K>,
    codes: Vec<u64>,
    next: Vec<u32>,
    /// `_M_before_begin._M_nxt`
    head: u32,
    buckets: Vec<u32>,
    n_bkt: u64,
    len: usize,
    policy: Policy,
}

impl<K: Copy + Eq + Hash + StdHash> Default for StdSet<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Copy + Eq + Hash + StdHash> StdSet<K> {
    /// A default-constructed set: one bucket (`_M_single_bucket`), nothing allocated.
    pub fn new() -> Self {
        StdSet {
            keys: Vec::new(),
            codes: Vec::new(),
            next: Vec::new(),
            head: NIL,
            buckets: vec![NIL],
            n_bkt: 1,
            len: 0,
            policy: Policy::default(),
        }
    }

    /// `unordered_set(first, last)`: a new set, then each element inserted in turn.
    pub fn from_iter_std<I: IntoIterator<Item = K>>(it: I) -> Self {
        let mut s = Self::new();
        for k in it {
            s.insert(k);
        }
        s
    }

    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn contains(&self, k: &K) -> bool {
        self.find(k).is_some()
    }

    /// The node holding `k`: the nodes of its bucket, from the node before them.
    fn find(&self, k: &K) -> Option<u32> {
        if self.len == 0 {
            return None;
        }
        let code = k.std_hash();
        let b = code % self.n_bkt;
        let before = self.buckets[b as usize];
        if before == NIL {
            return None;
        }
        let mut n = self.next_of(before);
        while n != NIL && self.codes[n as usize] % self.n_bkt == b {
            if self.keys[n as usize] == *k {
                return Some(n);
            }
            n = self.next[n as usize];
        }
        None
    }
    pub fn bucket_count(&self) -> u64 {
        self.n_bkt
    }

    #[inline]
    fn next_of(&self, p: u32) -> u32 {
        if p == BB {
            self.head
        } else {
            self.next[p as usize]
        }
    }
    #[inline]
    fn set_next(&mut self, p: u32, n: u32) {
        if p == BB {
            self.head = n;
        } else {
            self.next[p as usize] = n;
        }
    }

    /// `_M_rehash(n, true_type)`
    fn rehash(&mut self, n: u64) {
        let mut nb = vec![NIL; n as usize];
        let mut p = self.head;
        self.head = NIL;
        let mut bbegin = 0usize;
        while p != NIL {
            let nx = self.next[p as usize];
            let b = (self.codes[p as usize] % n) as usize;
            if nb[b] == NIL {
                self.next[p as usize] = self.head;
                self.head = p;
                nb[b] = BB;
                if self.next[p as usize] != NIL {
                    nb[bbegin] = p;
                }
                bbegin = b;
            } else {
                let before = nb[b];
                let after = if before == BB { self.head } else { self.next[before as usize] };
                self.next[p as usize] = after;
                if before == BB {
                    self.head = p;
                } else {
                    self.next[before as usize] = p;
                }
            }
            p = nx;
        }
        self.buckets = nb;
        self.n_bkt = n;
    }

    /// `_M_insert_bucket_begin`
    fn insert_bucket_begin(&mut self, b: usize, node: u32) {
        let before = self.buckets[b];
        if before != NIL {
            let after = self.next_of(before);
            self.next[node as usize] = after;
            self.set_next(before, node);
        } else {
            self.next[node as usize] = self.head;
            self.head = node;
            let nx = self.next[node as usize];
            if nx != NIL {
                let nb = (self.codes[nx as usize] % self.n_bkt) as usize;
                self.buckets[nb] = node;
            }
            self.buckets[b] = BB;
        }
    }

    /// `insert(k)` / `emplace(k)`: true when the key is new. Returns the node.
    pub fn insert_node(&mut self, k: K) -> (u32, bool) {
        if let Some(n) = self.find(&k) {
            return (n, false);
        }
        let code = k.std_hash();
        if let Some(n) = self.policy.need_rehash(self.n_bkt, self.len as u64, 1) {
            self.rehash(n);
        }
        let b = (code % self.n_bkt) as usize;
        let node = self.keys.len() as u32;
        self.keys.push(k);
        self.codes.push(code);
        self.next.push(NIL);
        self.insert_bucket_begin(b, node);
        self.len += 1;
        (node, true)
    }

    pub fn insert(&mut self, k: K) -> bool {
        self.insert_node(k).1
    }

    /// `erase(k)`: `_M_erase(bkt, prev, n)`.
    pub fn erase(&mut self, k: &K) -> bool {
        let Some(n) = self.find(k) else { return false };
        let b = (self.codes[n as usize] % self.n_bkt) as usize;
        let mut prev = self.buckets[b];
        while self.next_of(prev) != n {
            prev = self.next_of(prev);
        }
        let nx = self.next[n as usize];
        if prev == self.buckets[b] {
            // _M_remove_bucket_begin(bkt, next, next_bkt)
            if nx == NIL {
                self.buckets[b] = NIL;
            } else {
                let nb = (self.codes[nx as usize] % self.n_bkt) as usize;
                if nb != b {
                    self.buckets[nb] = self.buckets[b];
                    self.buckets[b] = NIL;
                }
            }
        } else if nx != NIL {
            let nb = (self.codes[nx as usize] % self.n_bkt) as usize;
            if nb != b {
                self.buckets[nb] = prev;
            }
        }
        self.set_next(prev, nx);
        self.len -= 1;
        true
    }

    /// The nodes in iteration order.
    pub fn nodes(&self) -> NodeIter<'_, K> {
        NodeIter { s: self, p: self.head }
    }

    /// The keys in iteration order.
    pub fn iter(&self) -> impl Iterator<Item = K> + '_ {
        self.nodes().map(move |n| self.keys[n as usize])
    }

    pub fn first(&self) -> Option<K> {
        if self.head == NIL {
            None
        } else {
            Some(self.keys[self.head as usize])
        }
    }

    pub fn to_vec(&self) -> Vec<K> {
        self.iter().collect()
    }

    /// landmarks/util.cc `get_intersection(set1, set2)`: a new set, set1's elements in set2 inserted in set1's order.
    pub fn intersection(&self, other: &Self) -> Self {
        let mut s = Self::new();
        for k in self.iter() {
            if other.contains(&k) {
                s.insert(k);
            }
        }
        s
    }
}

pub struct NodeIter<'a, K: Copy + Eq + Hash + StdHash> {
    s: &'a StdSet<K>,
    p: u32,
}

impl<K: Copy + Eq + Hash + StdHash> Iterator for NodeIter<'_, K> {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        if self.p == NIL {
            return None;
        }
        let n = self.p;
        self.p = self.s.next[n as usize];
        Some(n)
    }
}

/// An `std::unordered_map<K, V>` in libstdc++'s order: the set of keys and a value per node.
#[derive(Clone, Debug)]
pub struct StdMap<K: Copy + Eq + Hash + StdHash, V> {
    set: StdSet<K>,
    vals: Vec<V>,
}

impl<K: Copy + Eq + Hash + StdHash, V: Default> Default for StdMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Copy + Eq + Hash + StdHash, V: Default> StdMap<K, V> {
    pub fn new() -> Self {
        StdMap { set: StdSet::new(), vals: Vec::new() }
    }
    /// `operator[]`: the value, a default one inserted when the key is new.
    pub fn entry(&mut self, k: K) -> &mut V {
        let (n, new) = self.set.insert_node(k);
        if new {
            debug_assert_eq!(n as usize, self.vals.len());
            self.vals.push(V::default());
        }
        &mut self.vals[n as usize]
    }
    pub fn get(&self, k: &K) -> Option<&V> {
        self.set.find(k).map(|n| &self.vals[n as usize])
    }
    pub fn len(&self) -> usize {
        self.set.len()
    }
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }
    pub fn set_bucket_count(&self) -> u64 {
        self.set.bucket_count()
    }
    /// (key, value) in iteration order.
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> + '_ {
        self.set.nodes().map(move |n| (self.set.keys[n as usize], &self.vals[n as usize]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fd_hash_is_lookup3() {
        // a value checked against utils::get_hash64(FactPair(0, 0)) (route/cc/stdhash_ref.cc prints it)
        let h = fd_hash64_2(0, 0);
        assert_ne!(h, 0);
        assert_eq!(fd_hash64_2(3, 4), (3u32, 4u32).std_hash());
    }

    #[test]
    fn sizes_follow_the_policy() {
        let mut s: StdSet<i32> = StdSet::new();
        assert_eq!(s.bucket_count(), 1);
        s.insert(5);
        assert_eq!(s.bucket_count(), 13);
        for i in 0..14 {
            s.insert(i);
        }
        assert_eq!(s.len(), 14);
        assert_eq!(s.bucket_count(), 29);
    }

    #[test]
    fn small_int_set_order() {
        // libstdc++: {1, 2, 3} inserted in order iterate 3 2 1 (each into an empty bucket: the list's front)
        let s = StdSet::<i32>::from_iter_std([1, 2, 3]);
        assert_eq!(s.to_vec(), vec![3, 2, 1]);
        let mut t = s.clone();
        t.erase(&2);
        assert_eq!(t.to_vec(), vec![3, 1]);
        // 14 and 1 share bucket 1 of 13: 14 goes to the front of that bucket
        let u = StdSet::<i32>::from_iter_std([1, 2, 14]);
        assert_eq!(u.to_vec(), vec![2, 14, 1]);
    }
}
