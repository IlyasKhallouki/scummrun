//! Values (model.py `Unknown`, `one_of`, `values_of`, `_join_values`, `_decide`, `_arith`, `i32`, `i16`).
//!
//! A value is an int or an unknown. Unknowns are interned per run in [`Vals`]: a [`Val`] is a `Copy`
//! 8-byte value, and two unknowns are equal exactly when their ids are, as model.py's frozen dataclass
//! compares (reason, what, values). An int never equals an unknown, as in Python.
//!
//! Every value the machine stores is an int32 (model.py wraps with `i32` before it writes); intermediate
//! sums are taken in i64.

use rustc_hash::FxHashMap;
use scumm_ir::Reason;
use std::fmt::Write;

/// An interned unknown's index in [`Vals`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct UnkId(pub u32);

/// A value: model.py's int or `Unknown`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Val {
    Int(i32),
    Unk(UnkId),
}

/// Python's `hash(-1)` is -2 (CPython keeps -1 for errors), so two states that differ only by a value -1 against
/// -2 hash alike, and model.py's state hashes (`_spins`, `_state_key`, `_position_key`, `round_slots`) take them as
/// one state. A value's hash here does the same: -1 hashes as -2 (equality still tells them apart).
#[inline]
pub fn py_int(n: i32) -> i32 {
    if n == -1 {
        -2
    } else {
        n
    }
}

impl std::hash::Hash for Val {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        match *self {
            Val::Int(n) => {
                h.write_u8(0);
                h.write_i32(py_int(n));
            }
            Val::Unk(id) => {
                h.write_u8(1);
                h.write_u32(id.0);
            }
        }
    }
}

impl Val {
    #[inline]
    pub fn is_unknown(self) -> bool {
        matches!(self, Val::Unk(_))
    }
    #[inline]
    pub fn int(self) -> Option<i32> {
        match self {
            Val::Int(v) => Some(v),
            Val::Unk(_) => None,
        }
    }
    /// Python truthiness: an unknown (a dataclass) is true, an int when it is not 0.
    #[inline]
    pub fn truthy(self) -> bool {
        match self {
            Val::Int(v) => v != 0,
            Val::Unk(_) => true,
        }
    }
    /// `v == n` for an int n (an unknown never equals an int).
    #[inline]
    pub fn eq_int(self, n: i32) -> bool {
        self == Val::Int(n)
    }
}

/// model.py `Unknown(reason, what, values)`; `values` sorted and distinct.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Unknown {
    pub reason: Reason,
    pub what: Box<str>,
    pub values: Option<Box<[i32]>>,
}

/// MAX_VALUES: a value set larger than this becomes a plain unknown, counted in the caps.
pub const MAX_VALUES: usize = 64;

/// The unknowns of a run, interned, and model.py's `VALUE_CAPS` counter (sites in first-count order).
#[derive(Default, Debug)]
pub struct Vals {
    list: Vec<Unknown>,
    map: FxHashMap<Unknown, UnkId>,
    pub caps: Vec<(&'static str, u64)>,
    /// `str(unknown)`, made when first asked for
    shown: Vec<Option<std::rc::Rc<str>>>,
}

/// What `_decide` gives: a truth value, or the unknown that decides nothing.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Dec {
    Bool(bool),
    Unk(Val),
}

impl Vals {
    /// Forgets the run's unknowns and caps (a new run).
    pub fn clear(&mut self) {
        self.list.clear();
        self.map.clear();
        self.caps.clear();
        self.shown.clear();
    }

    /// `str(v)` as a shared string, made once per unknown.
    pub fn show_rc(&mut self, v: Val) -> std::rc::Rc<str> {
        let Val::Unk(id) = v else { return self.show(v).into() };
        let i = id.0 as usize;
        if self.shown.len() <= i {
            self.shown.resize(i + 1, None);
        }
        if let Some(s) = &self.shown[i] {
            return s.clone();
        }
        let s: std::rc::Rc<str> = self.show(v).into();
        self.shown[i] = Some(s.clone());
        s
    }

    pub fn intern(&mut self, u: Unknown) -> Val {
        if let Some(&id) = self.map.get(&u) {
            return Val::Unk(id);
        }
        let id = UnkId(self.list.len() as u32);
        self.list.push(u.clone());
        self.map.insert(u, id);
        Val::Unk(id)
    }

    /// `Unknown(reason, what)`.
    pub fn mk(&mut self, reason: Reason, what: impl Into<Box<str>>) -> Val {
        self.intern(Unknown { reason, what: what.into(), values: None })
    }

    /// `Unknown(reason, what, values)` with a value set as given (sorted here).
    pub fn mk_set(&mut self, reason: Reason, what: impl Into<Box<str>>, mut values: Vec<i32>) -> Val {
        values.sort_unstable();
        values.dedup();
        self.intern(Unknown { reason, what: what.into(), values: Some(values.into_boxed_slice()) })
    }

    #[inline]
    pub fn get(&self, id: UnkId) -> &Unknown {
        &self.list[id.0 as usize]
    }

    /// The unknown behind a value, if it is one.
    #[inline]
    pub fn unk(&self, v: Val) -> Option<&Unknown> {
        match v {
            Val::Unk(id) => Some(self.get(id)),
            Val::Int(_) => None,
        }
    }

    /// A plain unknown: one with no value set.
    #[inline]
    pub fn is_plain(&self, v: Val) -> bool {
        matches!(self.unk(v), Some(u) if u.values.is_none())
    }

    /// `VALUE_CAPS[site] += 1`.
    pub fn cap(&mut self, site: &'static str) {
        match self.caps.iter_mut().find(|c| c.0 == site) {
            Some(c) => c.1 += 1,
            None => self.caps.push((site, 1)),
        }
    }

    /// Python `str(v)`: the int, or `Unknown.__str__`.
    pub fn show(&self, v: Val) -> String {
        match v {
            Val::Int(n) => n.to_string(),
            Val::Unk(id) => self.show_unknown(self.get(id)),
        }
    }

    pub fn show_unknown(&self, u: &Unknown) -> String {
        let mut s = String::with_capacity(32 + u.what.len());
        s.push_str("?(");
        s.push_str(&u.reason.as_str());
        if !u.what.is_empty() {
            s.push_str(": ");
            s.push_str(&u.what);
        }
        if let Some(vs) = &u.values {
            s.push_str(" in [");
            for (k, v) in vs.iter().enumerate() {
                if k > 0 {
                    s.push_str(", ");
                }
                write!(s, "{v}").unwrap();
            }
            s.push(']');
        }
        s.push(')');
        s
    }

    /// Python `repr(v)`: the int, or the dataclass repr `Unknown(reason='...', what='...', values=None)`. A value
    /// set prints as `frozenset({1, 2})` in ascending order: CPython prints a frozenset in its hash-table order,
    /// which for small non-negative ints is ascending and otherwise rests on how the set was built (not
    /// reproduced; PORT-NOTES.md, kept apart).
    pub fn py_repr(&self, v: Val) -> String {
        let Val::Unk(id) = v else { return self.show(v) };
        let u = self.get(id);
        let mut s = format!(
            "Unknown(reason={}, what={}, values=",
            scumm_ir::py::repr(&u.reason.as_str()),
            scumm_ir::py::repr(&u.what)
        );
        match &u.values {
            None => s.push_str("None"),
            Some(vs) => {
                s.push_str("frozenset({");
                for (k, x) in vs.iter().enumerate() {
                    if k > 0 {
                        s.push_str(", ");
                    }
                    write!(s, "{x}").unwrap();
                }
                s.push_str("})");
            }
        }
        s.push(')');
        s
    }

    /// `values_of(v)` for a value that is an int or an unknown with a set (None for a plain unknown).
    pub fn values_of(&self, v: Val) -> Option<Vec<i32>> {
        match v {
            Val::Int(n) => Some(vec![n]),
            Val::Unk(id) => self.get(id).values.as_ref().map(|vs| vs.to_vec()),
        }
    }

    /// `one_of(values, reason, what)`: the value itself when there is one, else an unknown that carries
    /// them (none past MAX_VALUES, counted).
    pub fn one_of(&mut self, mut values: Vec<i32>, reason: Reason, what: impl Into<Box<str>>) -> Val {
        values.sort_unstable();
        values.dedup();
        if values.len() == 1 {
            return Val::Int(values[0]);
        }
        if values.len() > MAX_VALUES {
            self.cap("value set");
            return self.mk(reason, what);
        }
        self.intern(Unknown { reason, what: what.into(), values: Some(values.into_boxed_slice()) })
    }

    /// `_join_values(vals)`: the value of a fact after merging paths that hold `vals`.
    pub fn join_values(&mut self, vals: &[Val]) -> Val {
        let first = vals[0];
        if vals[1..].iter().all(|&v| v == first) {
            return first;
        }
        if let Some(&plain) = vals.iter().find(|&&v| self.is_plain(v)) {
            return plain;
        }
        let reason = vals.iter().find_map(|&v| self.unk(v).map(|u| u.reason)).unwrap_or(Reason::Branch);
        let mut all = Vec::new();
        for &v in vals {
            all.extend(self.values_of(v).expect("no plain unknown here"));
        }
        self.one_of(all, reason, "merged paths")
    }

    /// `_decide(test, *vals)`: True or False, also on value sets when every combination agrees; else
    /// the first unknown (a plain one first).
    pub fn decide(&mut self, vals: &[Val], test: impl Fn(&[i32]) -> bool) -> Dec {
        if let Some(&plain) = vals.iter().find(|&&v| self.is_plain(v)) {
            return Dec::Unk(plain);
        }
        let Some(&unknown) = vals.iter().find(|v| v.is_unknown()) else {
            if vals.len() <= 4 {
                // the tests take one or two values: no allocation
                let mut ints = [0i32; 4];
                for (i, v) in vals.iter().enumerate() {
                    ints[i] = v.int().unwrap();
                }
                return Dec::Bool(test(&ints[..vals.len()]));
            }
            let ints: Vec<i32> = vals.iter().map(|v| v.int().unwrap()).collect();
            return Dec::Bool(test(&ints));
        };
        let mut combos: Vec<Vec<i32>> = vec![Vec::new()];
        for &v in vals {
            let vs = self.values_of(v).unwrap();
            combos = combos.iter().flat_map(|c| vs.iter().map(move |&y| [c.as_slice(), &[y]].concat())).collect();
            if combos.len() > MAX_VALUES {
                self.cap("test");
                let u = self.unk(unknown).unwrap().clone();
                return Dec::Unk(self.mk(u.reason, u.what));
            }
        }
        let first = test(&combos[0]);
        if combos.iter().all(|c| test(c) == first) {
            Dec::Bool(first)
        } else {
            Dec::Unk(unknown)
        }
    }

    /// `_arith(op, a, b)`: o5's arithmetic on int32 values, value by value on sets.
    pub fn arith(&mut self, op: Op, a: Val, b: Val) -> Val {
        if self.is_plain(a) {
            return a;
        }
        if self.is_plain(b) {
            return b;
        }
        if a.is_unknown() || b.is_unknown() {
            let u = self.unk(if a.is_unknown() { a } else { b }).unwrap().clone();
            let (va, vb) = (self.values_of(a).unwrap(), self.values_of(b).unwrap());
            if va.len() * vb.len() > MAX_VALUES {
                self.cap("arithmetic");
                return self.mk(u.reason, u.what);
            }
            let mut out = Vec::new();
            for &x in &va {
                for &y in &vb {
                    match arith_int(op, x, y) {
                        Some(r) => out.push(r),
                        None => return self.mk(Reason::Work, "divide by zero among the values"),
                    }
                }
            }
            return self.one_of(out, u.reason, u.what);
        }
        let (x, y) = (a.int().unwrap(), b.int().unwrap());
        match arith_int(op, x, y) {
            Some(r) => Val::Int(r),
            None => self.mk(Reason::Work, "divide by zero"),
        }
    }
}

/// An arithmetic operator of `_arith` ("+", "-", "*", and "/" for anything else).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

impl From<scumm_ir::ArithOp> for Op {
    fn from(o: scumm_ir::ArithOp) -> Op {
        match o {
            scumm_ir::ArithOp::Add => Op::Add,
            scumm_ir::ArithOp::Sub => Op::Sub,
            scumm_ir::ArithOp::Mul => Op::Mul,
            scumm_ir::ArithOp::Div => Op::Div,
        }
    }
}

/// `_arith` on two ints; None for a division by zero.
pub fn arith_int(op: Op, a: i32, b: i32) -> Option<i32> {
    let (a, b) = (a as i64, b as i64);
    Some(match op {
        Op::Add => wrap32(a + b),
        Op::Sub => wrap32(a - b),
        Op::Mul => wrap32(a * b),
        Op::Div => {
            if b == 0 {
                return None;
            }
            let q = a.abs() / b.abs();
            wrap32(if (a >= 0) == (b >= 0) { q } else { -q })
        }
    })
}

/// model.py `i32`: wraps to int32.
#[inline]
pub fn wrap32(v: i64) -> i32 {
    v as i32
}

/// model.py `i16`: wraps to int16.
#[inline]
pub fn i16w(v: i32) -> i32 {
    v as i16 as i32
}

/// An i64 sum (an index, a key number) as the i32 a fact key holds; model.py's ints are unbounded,
/// so one past i32 would be a key the port cannot hold: stop loudly.
#[inline]
pub fn key_num(v: i64) -> i32 {
    i32::try_from(v).unwrap_or_else(|_| panic!("a key number past int32: {v} (Python would go on)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Python's `repr` of values, as `_merge`'s `input_drift` sort reads them (the strings are CPython's).
    #[test]
    fn py_repr() {
        let mut v = Vals::default();
        assert_eq!(v.py_repr(Val::Int(-12)), "-12");
        let a = v.mk(Reason::Held, "var:53");
        assert_eq!(v.py_repr(a), "Unknown(reason='not held', what='var:53', values=None)");
        let b = v.mk_set(Reason::Branch, "it's", vec![2, 1]);
        assert_eq!(v.py_repr(b), "Unknown(reason='branch', what=\"it's\", values=frozenset({1, 2}))");
    }

    #[test]
    fn show_and_sets() {
        let mut v = Vals::default();
        let a = v.mk(Reason::Local, "Local[1]");
        assert_eq!(v.show(a), "?(local: Local[1])");
        assert_eq!(a, v.mk(Reason::Local, "Local[1]"));
        let s = v.one_of(vec![3, 1, 2], Reason::Branch, "merged paths");
        assert_eq!(v.show(s), "?(branch: merged paths in [1, 2, 3])");
        assert_eq!(v.one_of(vec![5, 5], Reason::Branch, "x"), Val::Int(5));
        let e = v.mk(Reason::Work, "");
        assert_eq!(v.show(e), "?(cannot work out)");
        // a test every value agrees on decides
        assert_eq!(v.decide(&[s, Val::Int(0)], |x| x[0] > x[1]), Dec::Bool(true));
        assert_eq!(v.decide(&[s, Val::Int(2)], |x| x[0] > x[1]), Dec::Unk(s));
        assert_eq!(v.arith(Op::Add, s, Val::Int(1)), v.one_of(vec![2, 3, 4], Reason::Branch, "merged paths"));
        assert_eq!(v.join_values(&[Val::Int(1), Val::Int(2)]), v.one_of(vec![1, 2], Reason::Branch, "merged paths"));
        assert_eq!(v.join_values(&[Val::Int(1), a]), a);
    }

    #[test]
    fn int_rules() {
        assert_eq!(arith_int(Op::Div, -7, 2), Some(-3));
        assert_eq!(arith_int(Op::Div, 7, -2), Some(-3));
        assert_eq!(arith_int(Op::Div, 7, 0), None);
        assert_eq!(arith_int(Op::Add, i32::MAX, 1), Some(i32::MIN));
        assert_eq!(i16w(0x8000), -0x8000);
        assert_eq!(i16w(-1), -1);
    }
}
