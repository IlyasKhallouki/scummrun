//! domains.py `Domain`: the finite set of values a fact can hold, ints with `BELOW` first and `ABOVE` last when
//! values past the compared constants collapse into them.

use crate::py::Py;
use std::collections::BTreeSet;

/// A domain value: an int, or a collapsed `BELOW` / `ABOVE`. The derived order is the domain's (BELOW, ints
/// ascending, ABOVE), as domains.py's tuple holds them.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Label {
    Below,
    Int(i64),
    Above,
}

pub const ABOVE: &str = "above";
pub const BELOW: &str = "below";

impl Label {
    /// Python's `str(v)`: the int, "above" or "below" (the packed format's and the SAS names' text).
    pub fn text(self) -> String {
        match self {
            Label::Int(n) => n.to_string(),
            Label::Above => ABOVE.to_string(),
            Label::Below => BELOW.to_string(),
        }
    }
    /// Python's `repr(v)`.
    pub fn repr(self) -> String {
        match self {
            Label::Int(n) => n.to_string(),
            Label::Above => "'above'".to_string(),
            Label::Below => "'below'".to_string(),
        }
    }
    pub fn py(self) -> Py {
        match self {
            Label::Int(n) => Py::Int(n),
            Label::Above => Py::str(ABOVE),
            Label::Below => Py::str(BELOW),
        }
    }
    pub fn from_py(p: &Py) -> Label {
        match p {
            Py::Int(n) => Label::Int(*n),
            Py::Str(s) if s == ABOVE => Label::Above,
            Py::Str(s) if s == BELOW => Label::Below,
            _ => panic!("not a domain value: {}", p.text()),
        }
    }
    pub fn int(self) -> Option<i64> {
        match self {
            Label::Int(n) => Some(n),
            _ => None,
        }
    }
}

/// A set of domain values, sorted (a frozenset of labels).
pub type Labels = Vec<Label>;

pub fn labels_py(s: &[Label]) -> Py {
    Py::Set(s.iter().map(|l| l.py()).collect())
}

pub fn labels_from_py(p: &Py) -> Labels {
    let mut v: Labels = p.items().iter().map(Label::from_py).collect();
    v.sort();
    v.dedup();
    v
}

/// domains.py `Domain`.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct Domain {
    pub values: Vec<Label>,
    pub low: Option<i64>,
    pub high: Option<i64>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Cmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Cmp {
    pub fn parse(s: &str) -> Cmp {
        match s {
            "==" => Cmp::Eq,
            "!=" => Cmp::Ne,
            "<" => Cmp::Lt,
            "<=" => Cmp::Le,
            ">" => Cmp::Gt,
            ">=" => Cmp::Ge,
            _ => panic!("not a comparison: {s}"),
        }
    }
    pub fn of(op: scumm_ir::CmpOp) -> Cmp {
        Cmp::parse(op.as_str())
    }
    pub fn eval(self, a: i64, b: i64) -> bool {
        match self {
            Cmp::Eq => a == b,
            Cmp::Ne => a != b,
            Cmp::Lt => a < b,
            Cmp::Le => a <= b,
            Cmp::Gt => a > b,
            Cmp::Ge => a >= b,
        }
    }
}

impl Domain {
    /// `Domain.of(ints, low, high)`: the domain of `ints` with values outside [low, high] collapsed.
    pub fn of(ints: impl IntoIterator<Item = i64>, low: Option<i64>, high: Option<i64>) -> Domain {
        let mut set = BTreeSet::new();
        for n in ints {
            let l = if low.is_some_and(|lo| n < lo) {
                Label::Below
            } else if high.is_some_and(|hi| n > hi) {
                Label::Above
            } else {
                Label::Int(n)
            };
            set.insert(l);
        }
        let values: Vec<Label> = set.into_iter().collect();
        let below = values.first() == Some(&Label::Below);
        let above = values.last() == Some(&Label::Above);
        Domain { values, low: if below { low } else { None }, high: if above { high } else { None } }
    }

    /// A domain of exactly these values (domains.py's `Domain(values)`, as task.py builds a pick variable's).
    pub fn plain(values: Vec<Label>) -> Domain {
        Domain { values, low: None, high: None }
    }

    pub fn binary() -> Domain {
        Domain::plain(vec![Label::Int(0), Label::Int(1)])
    }

    pub fn exact(&self) -> bool {
        !self.values.iter().any(|v| matches!(v, Label::Above | Label::Below))
    }

    pub fn contains(&self, l: Label) -> bool {
        self.values.binary_search(&l).is_ok()
    }

    /// `Domain.label(n)`: the value an engine value has here, or None when the domain does not hold it.
    pub fn label(&self, n: i64) -> Option<Label> {
        if self.high.is_some_and(|h| n > h) {
            return Some(Label::Above);
        }
        if self.low.is_some_and(|l| n < l) {
            return Some(Label::Below);
        }
        self.contains(Label::Int(n)).then_some(Label::Int(n))
    }

    /// `Domain.label(v) if isinstance(v, int) else v`: a label kept as it is, an int labelled.
    pub fn label_of(&self, l: Label) -> Option<Label> {
        match l {
            Label::Int(n) => self.label(n),
            other => Some(other),
        }
    }

    /// `Domain.test(op, c)`: the values `v` with `v op c`; None when a collapsed value cannot decide it.
    pub fn test(&self, op: Cmp, c: i64) -> Option<Labels> {
        let mut out = Vec::new();
        for &v in &self.values {
            let ok = match v {
                Label::Above => {
                    let h = self.high.unwrap();
                    if c > h {
                        return None;
                    }
                    op.eval(h + 1, c)
                }
                Label::Below => {
                    let l = self.low.unwrap();
                    if c < l {
                        return None;
                    }
                    op.eval(l - 1, c)
                }
                Label::Int(n) => op.eval(n, c),
            };
            if ok {
                out.push(v);
            }
        }
        Some(out)
    }

    /// `Domain.add(v, d)`: the values `v + d` can have (sorted).
    pub fn add(&self, v: Label, d: i64) -> Labels {
        match v {
            Label::Above => {
                if d >= 0 {
                    return vec![Label::Above];
                }
                let h = self.high.unwrap();
                self.values.iter().copied().filter(|x| *x == Label::Above || matches!(x, Label::Int(n) if *n > h + d)).collect()
            }
            Label::Below => {
                if d <= 0 {
                    return vec![Label::Below];
                }
                let l = self.low.unwrap();
                self.values.iter().copied().filter(|x| *x == Label::Below || matches!(x, Label::Int(n) if *n < l + d)).collect()
            }
            Label::Int(n) => self.label(n + d).into_iter().collect(),
        }
    }

    /// golden.py's encoding of the dataclass.
    pub fn py(&self) -> Py {
        Py::obj(
            "Domain",
            vec![
                ("values", Py::Tuple(self.values.iter().map(|l| l.py()).collect())),
                ("low", Py::opt_int(self.low)),
                ("high", Py::opt_int(self.high)),
            ],
        )
    }

    pub fn from_py(p: &Py) -> Domain {
        let p = p.class("Domain");
        let low = p.get("low");
        let high = p.get("high");
        Domain {
            values: p.get("values").items().iter().map(Label::from_py).collect(),
            low: (!low.is_none()).then(|| low.int()),
            high: (!high.is_none()).then(|| high.int()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn of_label_add() {
        let d = Domain::of([0, 1, 2, 7, -3], Some(0), Some(2));
        assert_eq!(d.values, vec![Label::Below, Label::Int(0), Label::Int(1), Label::Int(2), Label::Above]);
        assert_eq!(d.label(5), Some(Label::Above));
        assert_eq!(d.label(-1), Some(Label::Below));
        assert_eq!(d.add(Label::Above, -2), vec![Label::Int(1), Label::Int(2), Label::Above]);
        assert_eq!(d.add(Label::Below, 1), vec![Label::Below, Label::Int(0)]);
        assert_eq!(d.add(Label::Below, -1), vec![Label::Below]);
        assert_eq!(d.add(Label::Int(1), 5), vec![Label::Above]);
        let e = Domain::of([0, 3], None, None);
        assert_eq!(e.low, None);
        assert!(e.add(Label::Int(0), 1).is_empty());
        assert_eq!(e.test(Cmp::Gt, 0), Some(vec![Label::Int(3)]));
    }
}
