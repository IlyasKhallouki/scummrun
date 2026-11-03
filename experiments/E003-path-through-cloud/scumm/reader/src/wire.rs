//! A binary encoding of the reader's run outputs and jobs, for the job memo's store (memo.rs, `CrossMemo`): every
//! value written with [`Wire::put`] reads back equal with [`Wire::take`]. Little-endian; a length is a u32; text is
//! UTF-8. The encoding of a value is a function of the value only (maps in key order, lists in their order), so two
//! equal jobs encode to the same bytes, and the store compares jobs by their bytes.

use crate::builder::{Deltas, Job, RdCounts};
use crate::domain::{Domain, Label};
use crate::memo::Query;
use crate::py::{Counter, Py, PyCounter};
use crate::reach::{Guide, Held};
use crate::record::{Assumption, Eff, Event, Record, UClass, Unknown, Val, What};
use scumm_ir::UnitKey;
use std::collections::{BTreeMap, BTreeSet};

/// A cursor over stored bytes. A read past the end is a store written by another code: it panics, naming it.
pub struct Rd<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Rd<'a> {
    pub fn new(b: &'a [u8]) -> Rd<'a> {
        Rd { b, i: 0 }
    }
    #[inline]
    pub fn bytes(&mut self, n: usize) -> &'a [u8] {
        let end = self.i.checked_add(n).filter(|&e| e <= self.b.len()).unwrap_or_else(|| panic!("the memo store: a value past the end of its bytes"));
        let s = &self.b[self.i..end];
        self.i = end;
        s
    }
    pub fn at_end(&self) -> bool {
        self.i == self.b.len()
    }
    pub fn pos(&self) -> usize {
        self.i
    }
}

pub trait Wire: Sized {
    fn put(&self, b: &mut Vec<u8>);
    fn take(r: &mut Rd) -> Self;
}

#[inline]
pub fn put_len(b: &mut Vec<u8>, n: usize) {
    b.extend_from_slice(&u32::try_from(n).expect("a length past u32").to_le_bytes());
}

#[inline]
pub fn take_len(r: &mut Rd) -> usize {
    u32::take(r) as usize
}

macro_rules! ints {
    ($($t:ty),*) => {$(
        impl Wire for $t {
            #[inline]
            fn put(&self, b: &mut Vec<u8>) {
                b.extend_from_slice(&self.to_le_bytes());
            }
            #[inline]
            fn take(r: &mut Rd) -> Self {
                <$t>::from_le_bytes(r.bytes(std::mem::size_of::<$t>()).try_into().unwrap())
            }
        }
    )*};
}
ints!(u8, u32, u64, i32, i64);

impl Wire for bool {
    fn put(&self, b: &mut Vec<u8>) {
        b.push(*self as u8);
    }
    fn take(r: &mut Rd) -> Self {
        match u8::take(r) {
            0 => false,
            1 => true,
            x => panic!("the memo store: a bool {x}"),
        }
    }
}

impl Wire for String {
    fn put(&self, b: &mut Vec<u8>) {
        self.as_str().put(b)
    }
    fn take(r: &mut Rd) -> Self {
        let n = take_len(r);
        std::str::from_utf8(r.bytes(n)).expect("the memo store: text not UTF-8").to_string()
    }
}

impl Wire for &str {
    fn put(&self, b: &mut Vec<u8>) {
        put_len(b, self.len());
        b.extend_from_slice(self.as_bytes());
    }
    fn take(_: &mut Rd) -> Self {
        unreachable!("a &str is read as a String")
    }
}

impl<T: Wire> Wire for Option<T> {
    fn put(&self, b: &mut Vec<u8>) {
        match self {
            None => b.push(0),
            Some(x) => {
                b.push(1);
                x.put(b);
            }
        }
    }
    fn take(r: &mut Rd) -> Self {
        bool::take(r).then(|| T::take(r))
    }
}

impl<T: Wire> Wire for Vec<T> {
    fn put(&self, b: &mut Vec<u8>) {
        put_len(b, self.len());
        for x in self {
            x.put(b);
        }
    }
    fn take(r: &mut Rd) -> Self {
        let n = take_len(r);
        (0..n).map(|_| T::take(r)).collect()
    }
}

impl<A: Wire, B: Wire> Wire for (A, B) {
    fn put(&self, b: &mut Vec<u8>) {
        self.0.put(b);
        self.1.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        let a = A::take(r);
        (a, B::take(r))
    }
}

impl<A: Wire, B: Wire, C: Wire> Wire for (A, B, C) {
    fn put(&self, b: &mut Vec<u8>) {
        self.0.put(b);
        self.1.put(b);
        self.2.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        let a = A::take(r);
        let bb = B::take(r);
        (a, bb, C::take(r))
    }
}

impl<A: Wire, B: Wire, C: Wire, D: Wire> Wire for (A, B, C, D) {
    fn put(&self, b: &mut Vec<u8>) {
        self.0.put(b);
        self.1.put(b);
        self.2.put(b);
        self.3.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        let a = A::take(r);
        let bb = B::take(r);
        let c = C::take(r);
        (a, bb, c, D::take(r))
    }
}

/// `T::take`, where a name of T's own would hide it (`Option::take`).
#[inline]
pub fn get<T: Wire>(r: &mut Rd) -> T {
    T::take(r)
}

impl<K: Wire + Ord, V: Wire> Wire for BTreeMap<K, V> {
    fn put(&self, b: &mut Vec<u8>) {
        put_len(b, self.len());
        for (k, v) in self {
            k.put(b);
            v.put(b);
        }
    }
    fn take(r: &mut Rd) -> Self {
        let n = take_len(r);
        (0..n).map(|_| <(K, V)>::take(r)).collect()
    }
}

impl<T: Wire + Ord> Wire for BTreeSet<T> {
    fn put(&self, b: &mut Vec<u8>) {
        put_len(b, self.len());
        for x in self {
            x.put(b);
        }
    }
    fn take(r: &mut Rd) -> Self {
        let n = take_len(r);
        (0..n).map(|_| T::take(r)).collect()
    }
}

impl Wire for Label {
    fn put(&self, b: &mut Vec<u8>) {
        match self {
            Label::Below => b.push(0),
            Label::Int(n) => {
                b.push(1);
                n.put(b);
            }
            Label::Above => b.push(2),
        }
    }
    fn take(r: &mut Rd) -> Self {
        match u8::take(r) {
            0 => Label::Below,
            1 => Label::Int(i64::take(r)),
            2 => Label::Above,
            x => panic!("the memo store: a label tag {x}"),
        }
    }
}

impl Wire for Domain {
    fn put(&self, b: &mut Vec<u8>) {
        self.values.put(b);
        self.low.put(b);
        self.high.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        let values = Vec::take(r);
        let low = get(r);
        Domain { values, low, high: get(r) }
    }
}

impl Wire for Py {
    fn put(&self, b: &mut Vec<u8>) {
        match self {
            Py::None => b.push(0),
            Py::Bool(x) => {
                b.push(1);
                x.put(b);
            }
            Py::Int(n) => {
                b.push(2);
                n.put(b);
            }
            Py::Str(s) => {
                b.push(3);
                s.put(b);
            }
            Py::List(v) => {
                b.push(4);
                v.put(b);
            }
            Py::Tuple(v) => {
                b.push(5);
                v.put(b);
            }
            Py::Set(v) => {
                b.push(6);
                v.put(b);
            }
            Py::Dict(v) => {
                b.push(7);
                v.put(b);
            }
            Py::Map(v) => {
                b.push(8);
                v.put(b);
            }
            Py::Obj(name, v) => {
                b.push(9);
                name.put(b);
                v.put(b);
            }
        }
    }
    fn take(r: &mut Rd) -> Self {
        match u8::take(r) {
            0 => Py::None,
            1 => Py::Bool(bool::take(r)),
            2 => Py::Int(i64::take(r)),
            3 => Py::Str(String::take(r)),
            4 => Py::List(Vec::take(r)),
            5 => Py::Tuple(Vec::take(r)),
            6 => Py::Set(Vec::take(r)),
            7 => Py::Dict(Vec::take(r)),
            8 => Py::Map(Vec::take(r)),
            9 => {
                let name = String::take(r);
                Py::Obj(name, Vec::take(r))
            }
            x => panic!("the memo store: a Python value tag {x}"),
        }
    }
}

impl Wire for Val {
    fn put(&self, b: &mut Vec<u8>) {
        match self {
            Val::Int(n) => {
                b.push(0);
                n.put(b);
            }
            Val::Unk(u) => {
                b.push(1 + match u.class {
                    UClass::Unknown => 0,
                    UClass::Cond => 1,
                    UClass::Copy => 2,
                });
                u.reason.put(b);
                u.what.put(b);
                u.values.put(b);
            }
        }
    }
    fn take(r: &mut Rd) -> Self {
        let class = match u8::take(r) {
            0 => return Val::Int(i64::take(r)),
            1 => UClass::Unknown,
            2 => UClass::Cond,
            3 => UClass::Copy,
            x => panic!("the memo store: a value tag {x}"),
        };
        let reason = String::take(r);
        let what = String::take(r);
        Val::Unk(Unknown { class, reason, what, values: get(r) })
    }
}

impl Wire for Eff {
    fn put(&self, b: &mut Vec<u8>) {
        match self {
            Eff::Set(l) => {
                b.push(0);
                l.put(b);
            }
            Eff::Guess(ls) => {
                b.push(1);
                ls.put(b);
            }
            Eff::Add(d) => {
                b.push(2);
                d.put(b);
            }
            Eff::When(k, v) => {
                b.push(3);
                k.put(b);
                v.put(b);
            }
            Eff::Copy(k) => {
                b.push(4);
                k.put(b);
            }
            Eff::Cases(cs) => {
                b.push(5);
                cs.put(b);
            }
        }
    }
    fn take(r: &mut Rd) -> Self {
        match u8::take(r) {
            0 => Eff::Set(Label::take(r)),
            1 => Eff::Guess(Vec::take(r)),
            2 => Eff::Add(i64::take(r)),
            3 => {
                let k = String::take(r);
                Eff::When(k, i64::take(r))
            }
            4 => Eff::Copy(String::take(r)),
            5 => Eff::Cases(Vec::take(r)),
            x => panic!("the memo store: an effect tag {x}"),
        }
    }
}

impl Wire for Assumption {
    fn put(&self, b: &mut Vec<u8>) {
        self.cite.put(b);
        self.cond.put(b);
        self.side.put(b);
        self.why.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        let cite = String::take(r);
        let cond = String::take(r);
        let side = Py::take(r);
        Assumption { cite, cond, side, why: String::take(r) }
    }
}

impl Wire for UnitKey {
    fn put(&self, b: &mut Vec<u8>) {
        match *self {
            UnitKey::Entry(r) => {
                b.push(0);
                r.put(b);
            }
            UnitKey::Exit(r) => {
                b.push(1);
                r.put(b);
            }
            UnitKey::Global(n) => {
                b.push(2);
                n.put(b);
            }
            UnitKey::Local(r, n) => {
                b.push(3);
                r.put(b);
                n.put(b);
            }
            UnitKey::Obj(o) => {
                b.push(4);
                o.put(b);
            }
        }
    }
    fn take(r: &mut Rd) -> Self {
        match u8::take(r) {
            0 => UnitKey::Entry(i32::take(r)),
            1 => UnitKey::Exit(i32::take(r)),
            2 => UnitKey::Global(i32::take(r)),
            3 => {
                let room = i32::take(r);
                UnitKey::Local(room, i32::take(r))
            }
            4 => UnitKey::Obj(i32::take(r)),
            x => panic!("the memo store: a unit tag {x}"),
        }
    }
}

impl Wire for Event {
    fn put(&self, b: &mut Vec<u8>) {
        self.name.put(b);
        self.unit.put(b);
        self.w.put(b);
        self.sig.put(b);
        self.locals.put(b);
        self.args_known.put(b);
        self.recursive.put(b);
        self.resistant.put(b);
        self.owned.put(b);
        self.abort.put(b);
        self.override_.put(b);
        self.cut_data.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        Event {
            name: String::take(r),
            unit: UnitKey::take(r),
            w: i64::take(r),
            sig: i64::take(r),
            locals: Vec::take(r),
            args_known: bool::take(r),
            recursive: Py::take(r),
            resistant: Py::take(r),
            owned: Py::take(r),
            abort: bool::take(r),
            override_: i64::take(r),
            cut_data: Vec::take(r),
        }
    }
}

impl Wire for What {
    fn put(&self, b: &mut Vec<u8>) {
        match self {
            What::Sentence { verb, a, b: bb } => {
                b.push(0);
                verb.put(b);
                a.put(b);
                bb.put(b);
            }
            What::Verb(v) => {
                b.push(1);
                v.put(b);
            }
            What::Event(e) => {
                b.push(2);
                e.put(b);
            }
        }
    }
    fn take(r: &mut Rd) -> Self {
        match u8::take(r) {
            0 => {
                let verb = i64::take(r);
                let a = i64::take(r);
                What::Sentence { verb, a, b: get(r) }
            }
            1 => What::Verb(i64::take(r)),
            2 => What::Event(Event::take(r)),
            x => panic!("the memo store: a run kind tag {x}"),
        }
    }
}

impl Wire for Record {
    fn put(&self, b: &mut Vec<u8>) {
        self.kind.put(b);
        self.label.put(b);
        self.pre.put(b);
        self.effects.put(b);
        self.checks.put(b);
        self.end.put(b);
        self.notes.put(b);
        self.next_events.put(b);
        self.source.put(b);
        self.either.put(b);
        self.engine_pre.put(b);
        self.unknown.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        Record {
            kind: String::take(r),
            label: String::take(r),
            pre: BTreeMap::take(r),
            effects: Vec::take(r),
            checks: Vec::take(r),
            end: get(r),
            notes: Vec::take(r),
            next_events: Vec::take(r),
            source: What::take(r),
            either: Vec::take(r),
            engine_pre: Vec::take(r),
            unknown: Vec::take(r),
        }
    }
}

impl Wire for Job {
    fn put(&self, b: &mut Vec<u8>) {
        self.kind.put(b);
        self.label.put(b);
        self.pins.put(b);
        self.pre.put(b);
        self.what.put(b);
        self.checks.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        Job {
            kind: String::take(r),
            label: String::take(r),
            pins: BTreeMap::take(r),
            pre: BTreeMap::take(r),
            what: What::take(r),
            checks: Vec::take(r),
        }
    }
}

impl Wire for Counter {
    fn put(&self, b: &mut Vec<u8>) {
        self.0.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        Counter(BTreeMap::take(r))
    }
}

impl Wire for PyCounter {
    fn put(&self, b: &mut Vec<u8>) {
        self.0.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        // made again in its order: the keys are distinct, so `add` appends each once
        let items: Vec<(Py, i64)> = Vec::take(r);
        let mut c = PyCounter::default();
        for (k, n) in items {
            c.add(k, n);
        }
        c
    }
}

impl Wire for RdCounts {
    fn put(&self, b: &mut Vec<u8>) {
        self.counts.put(b);
        self.forks.put(b);
        self.fork_keys.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        let counts = Counter::take(r);
        let forks = Counter::take(r);
        RdCounts { counts, forks, fork_keys: Counter::take(r) }
    }
}

/// A guide's fact as its key text (`machine::pkey_of` reads it back).
fn put_guide(g: &Guide, b: &mut Vec<u8>) {
    put_len(b, g.len());
    for (k, s) in g {
        crate::machine::pkey_text(*k).as_str().put(b);
        s.put(b);
    }
}

fn take_guide(r: &mut Rd) -> Guide {
    let n = take_len(r);
    (0..n)
        .map(|_| {
            let k = crate::machine::pkey_of(&String::take(r));
            (k, Vec::take(r))
        })
        .collect()
}

/// A rerun's guides (`Reached::guides_of`), none or a list.
pub fn put_guides(g: Option<&Vec<Guide>>, b: &mut Vec<u8>) {
    match g {
        None => b.push(0),
        Some(gs) => {
            b.push(1);
            put_len(b, gs.len());
            for x in gs {
                put_guide(x, b);
            }
        }
    }
}

impl Wire for Held {
    fn put(&self, b: &mut Vec<u8>) {
        put_guide(&self.guide, b);
        self.key.put(b);
        self.set.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        let guide = take_guide(r);
        let key = String::take(r);
        Held { guide, key, set: Vec::take(r) }
    }
}

impl Wire for Query {
    fn put(&self, b: &mut Vec<u8>) {
        self.key.put(b);
        self.set.put(b);
        self.before.put(b);
        self.answer.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        let key = String::take(r);
        let set = Vec::take(r);
        let before = bool::take(r);
        Query { key, set, before, answer: bool::take(r) }
    }
}

impl Wire for Deltas {
    fn put(&self, b: &mut Vec<u8>) {
        self.counts.put(b);
        self.ends.put(b);
        self.drops.put(b);
        self.nonfact_checks.put(b);
        self.guesses.put(b);
        self.unknown_writes.put(b);
        self.unknown_sites.put(b);
        self.out_of_domain.put(b);
        self.outside_points.put(b);
        self.rd.put(b);
        self.sig_ids.put(b);
        self.args.put(b);
        self.held.put(b);
    }
    fn take(r: &mut Rd) -> Self {
        Deltas {
            counts: Counter::take(r),
            ends: PyCounter::take(r),
            drops: PyCounter::take(r),
            nonfact_checks: Counter::take(r),
            guesses: Counter::take(r),
            unknown_writes: Counter::take(r),
            unknown_sites: PyCounter::take(r),
            out_of_domain: BTreeMap::take(r),
            outside_points: BTreeMap::take(r),
            rd: RdCounts::take(r),
            sig_ids: BTreeMap::take(r),
            args: Vec::take(r),
            held: Vec::take(r),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round<T: Wire + PartialEq + std::fmt::Debug>(x: &T) {
        let mut b = Vec::new();
        x.put(&mut b);
        let mut r = Rd::new(&b);
        let y = T::take(&mut r);
        assert!(r.at_end());
        assert_eq!(*x, y);
    }

    #[test]
    fn values_read_back_equal() {
        round(&Py::Obj("C".into(), vec![("a".into(), Py::Set(vec![Py::Int(-3), Py::None])), ("b".into(), Py::Map(vec![(Py::Tuple(vec![]), Py::Bool(true))]))]));
        round(&Val::Unk(Unknown { class: UClass::Cond, reason: "r".into(), what: "var:3".into(), values: Some(vec![0, 1]) }));
        round(&Val::Int(-7));
        round(&Domain::of([3, -9, 4, 100], Some(-5), Some(50)));
        round(&Eff::Cases(vec![(vec![("var:1".into(), vec![Label::Below, Label::Int(2)])], vec![Label::Above])]));
        let ev = Event {
            name: "local:200".into(),
            unit: UnitKey::Local(33, 200),
            w: 17,
            sig: 1234,
            locals: vec![(0, Val::Int(5))],
            args_known: true,
            recursive: Py::None,
            resistant: Py::Bool(false),
            owned: Py::Bool(true),
            abort: false,
            override_: 1,
            cut_data: vec![9],
        };
        round(&What::Event(ev.clone()));
        round(&What::Sentence { verb: 7, a: 320, b: None });
        let mut pc = PyCounter::default();
        pc.add(Py::Tuple(vec![Py::str("event"), Py::str("end")]), 3);
        pc.add(Py::str("x"), 0);
        let mut b = Vec::new();
        pc.put(&mut b);
        let back = PyCounter::take(&mut Rd::new(&b));
        assert_eq!(back, pc);
    }
}
