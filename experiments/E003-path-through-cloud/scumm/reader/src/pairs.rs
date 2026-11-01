//! The switch reach_pairs (experiments/E004-fast-exact/reach-pairs.md has the rule and its proof): next to --reach's
//! reached values, the reached pairs of two facts, the verb script variable and the room, by h² restricted to those
//! pairs (every other pair taken as reachable). A job whose seed has a set on both runs only when a reached pair meets
//! them, and runs when one arrives (reach.rs). Off by default; with it off nothing here runs.

use crate::domain::{Domain, Label, Labels};
use crate::task::TEffect;
use rustc_hash::{FxHashMap, FxHashSet};

pub type Pair = (Label, Label);

/// The facts of the pairs: VAR_VERB_SCRIPT (vars.cpp) and the room.
pub fn fact_a() -> String {
    format!("var:{}", crate::builder::VAR_VERB_SCRIPT)
}
pub const FACT_B: &str = "room";

#[derive(Clone, Debug)]
enum Writes {
    /// a value or a guess's values, as the domain labels them
    Values(Labels),
    /// an add's step from the current value (`Domain::add`)
    Add(i64),
    /// a value the domain lacks: taskcore gives it no index, it writes nothing (K_BAD)
    Bad,
}

/// One effect of an action on A or B.
#[derive(Clone, Debug)]
struct PEff {
    on_a: bool,
    /// its conditions on A and on B, each the common values of its sets there
    cond_a: Option<Labels>,
    cond_b: Option<Labels>,
    /// its other conditions, and whether they all meet the reached values (they can only come to)
    other: Vec<(String, Labels)>,
    met: bool,
    writes: Writes,
}

#[derive(Clone, Debug)]
struct PAct {
    pre_a: Option<Labels>,
    pre_b: Option<Labels>,
    effs: Vec<PEff>,
}

/// The reached pairs and the written actions with an effect on A or B (reach-pairs.md, "The rule").
#[derive(Clone, Debug)]
pub struct Pairs {
    pub a: String,
    pub b: String,
    dom_a: Domain,
    dom_b: Domain,
    set: FxHashSet<Pair>,
    list: Vec<Pair>,
    acts: Vec<PAct>,
    /// the actions written since the last commit
    new_acts: Vec<u32>,
    /// a fact of an effect's other conditions -> the actions with such an effect not met yet
    watch: FxHashMap<String, Vec<u32>>,
    /// seeds and pairs to add at the next commit
    fresh: Vec<Pair>,
}

fn common(acc: Option<Labels>, s: &Labels) -> Option<Labels> {
    Some(match acc {
        None => s.clone(),
        Some(a) => a.into_iter().filter(|x| s.contains(x)).collect(),
    })
}

impl Pairs {
    pub fn new(a: String, b: String, dom_a: Domain, dom_b: Domain) -> Pairs {
        Pairs {
            a,
            b,
            dom_a,
            dom_b,
            set: FxHashSet::default(),
            list: Vec::new(),
            acts: Vec::new(),
            new_acts: Vec::new(),
            watch: FxHashMap::default(),
            fresh: Vec::new(),
        }
    }

    /// A start's pair.
    pub fn seed(&mut self, p: Pair) {
        self.fresh.push(p);
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
    pub fn n_actions(&self) -> usize {
        self.acts.len()
    }
    /// The pairs in the order they arrived.
    pub fn pairs(&self) -> &[Pair] {
        &self.list
    }
    pub fn has(&self, p: &Pair) -> bool {
        self.set.contains(p)
    }

    /// Some reached pair has its A value in `sa` and its B value in `sb`.
    pub fn meets(&self, sa: &[Label], sb: &[Label]) -> bool {
        self.list.iter().any(|(x, y)| sa.contains(x) && sb.contains(y))
    }

    /// A written action (`record_action`'s precondition and effects); kept when an effect is on A or B. `value`
    /// labels a written value as (d) does (reach.rs `value_of`; None: the domain lacks it). `watch_key` names the
    /// fact a condition's set is read through (`at:`/`esc:` through `pos:`/`escpos:`).
    pub fn add_action(
        &mut self,
        pre: &[(String, Labels)],
        effects: &[TEffect],
        value: &dyn Fn(&str, Label) -> Option<Label>,
        watch_key: &dyn Fn(&str) -> String,
    ) {
        let mut effs = Vec::new();
        for e in effects {
            let on_a = e.key == self.a;
            if !on_a && e.key != self.b {
                continue;
            }
            let (mut cond_a, mut cond_b, mut other) = (None, None, Vec::new());
            for (k, s) in &e.when {
                if *k == self.a {
                    cond_a = common(cond_a, s);
                } else if *k == self.b {
                    cond_b = common(cond_b, s);
                } else {
                    other.push((k.clone(), s.clone()));
                }
            }
            let writes = if let Some(d) = e.add {
                Writes::Add(d)
            } else {
                let vals: Labels = match (e.value, &e.values) {
                    (Some(v), _) => vec![v],
                    (None, Some(vs)) => vs.clone(),
                    _ => continue,
                };
                let got: Vec<Option<Label>> = vals.iter().map(|&v| value(&e.key, v)).collect();
                if got.iter().any(|x| x.is_none()) {
                    Writes::Bad
                } else {
                    Writes::Values(got.into_iter().flatten().collect())
                }
            };
            effs.push(PEff { on_a, cond_a, cond_b, other, met: false, writes });
        }
        if effs.is_empty() {
            return;
        }
        let find = |k: &str| pre.iter().find(|(f, _)| f == k).map(|(_, s)| s.clone());
        let id = self.acts.len() as u32;
        let act = PAct { pre_a: find(&self.a), pre_b: find(&self.b), effs };
        let mut keys: Vec<String> = act.effs.iter().flat_map(|e| e.other.iter().map(|(k, _)| watch_key(k))).collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            self.watch.entry(k).or_default().push(id);
        }
        self.acts.push(act);
        self.new_acts.push(id);
    }

    /// What A or B can hold after `act` at `(x, y)` (reach-pairs.md, `C_A`): the values of each effect that may fire,
    /// and the value itself unless one must fire.
    fn outcomes(&self, act: &PAct, on_a: bool, x: Label, y: Label) -> Labels {
        let (own, dom) = if on_a { (x, &self.dom_a) } else { (y, &self.dom_b) };
        let mut out = Labels::new();
        let mut must = false;
        for e in act.effs.iter().filter(|e| e.on_a == on_a) {
            let holds = e.cond_a.as_ref().is_none_or(|s| s.contains(&x)) && e.cond_b.as_ref().is_none_or(|s| s.contains(&y));
            if !holds || !(e.other.is_empty() || e.met) {
                continue;
            }
            let wrote: Labels = match &e.writes {
                Writes::Values(v) => v.clone(),
                Writes::Add(d) => dom.add(own, *d).into_iter().filter(|l| dom.contains(*l)).collect(),
                Writes::Bad => continue,
            };
            if wrote.is_empty() {
                continue;
            }
            if e.other.is_empty() {
                must = true;
            }
            out.extend(wrote);
        }
        if !must {
            out.push(own);
        }
        out.sort();
        out.dedup();
        out
    }

    /// The pairs `act` leads to from `(x, y)`; none when its precondition does not hold them.
    fn apply(&self, act: &PAct, (x, y): Pair, out: &mut Vec<Pair>) {
        if act.pre_a.as_ref().is_some_and(|s| !s.contains(&x)) || act.pre_b.as_ref().is_some_and(|s| !s.contains(&y)) {
            return;
        }
        let ca = self.outcomes(act, true, x, y);
        let cb = self.outcomes(act, false, x, y);
        for &u in &ca {
            for &w in &cb {
                if !self.set.contains(&(u, w)) {
                    out.push((u, w));
                }
            }
        }
    }

    /// A round's commit (reach-pairs.md, "Computed while reading"): the effects whose other conditions newly meet the
    /// reached values (`met`), on the facts in `new_keys` and of the actions written in the round; then the closure.
    /// Returns the number of new pairs.
    pub fn commit(&mut self, new_keys: &[String], met: &dyn Fn(&str, &[Label]) -> bool) -> usize {
        let mut dirty: Vec<u32> = std::mem::take(&mut self.new_acts);
        let mut check: Vec<u32> = dirty.clone();
        for k in new_keys {
            if let Some(ids) = self.watch.get(k) {
                check.extend(ids.iter().copied());
            }
        }
        check.sort();
        check.dedup();
        for id in check {
            let act = &mut self.acts[id as usize];
            let mut flipped = false;
            for e in act.effs.iter_mut() {
                if !e.met && !e.other.is_empty() && e.other.iter().all(|(k, s)| met(k, s)) {
                    e.met = true;
                    flipped = true;
                }
            }
            if flipped {
                dirty.push(id);
            }
        }
        dirty.sort();
        dirty.dedup();
        let n0 = self.list.len();
        let mut queue: Vec<Pair> = std::mem::take(&mut self.fresh);
        for &id in &dirty {
            let act = &self.acts[id as usize];
            for &p in &self.list {
                self.apply(act, p, &mut queue);
            }
        }
        while let Some(p) = queue.pop() {
            if !self.set.insert(p) {
                continue;
            }
            self.list.push(p);
            let mut more = Vec::new();
            for act in &self.acts {
                self.apply(act, p, &mut more);
            }
            queue.extend(more);
        }
        self.list.len() - n0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(n: i64) -> Label {
        Label::Int(n)
    }
    fn set(k: &str, v: i64, when: Vec<(String, Labels)>) -> TEffect {
        TEffect { key: k.into(), value: Some(l(v)), add: None, values: None, when }
    }
    fn pairs() -> Pairs {
        Pairs::new(fact_a(), FACT_B.into(), Domain::of([0, 4, 201, 202], None, None), Domain::of([28, 33, 85], None, None))
    }
    fn value(_: &str, v: Label) -> Option<Label> {
        Some(v)
    }
    fn watch(k: &str) -> String {
        k.to_string()
    }
    fn room(r: i64) -> (String, Labels) {
        (FACT_B.to_string(), vec![l(r)])
    }

    /// A room change into 85 that sets the verb script under a condition on another fact: where the condition can
    /// fail, the verb script keeps its value in room 85 (the effect may not fire).
    #[test]
    fn an_effect_that_may_not_fire_leaves_the_value() {
        let mut p = pairs();
        p.seed((l(202), l(28)));
        let a = fact_a();
        let effs = vec![set(FACT_B, 85, vec![]), set(&a, 201, vec![("bit:5".into(), vec![l(1)])])];
        p.add_action(&[room(28)], &effs, &value, &watch);
        p.commit(&[], &|_, _| true);
        assert!(p.has(&(l(201), l(85))), "the effect fires");
        assert!(p.has(&(l(202), l(85))), "the effect does not fire: the old value in the new room");
        assert!(p.meets(&[l(202)], &[l(85)]));
    }

    /// The same change with the write unconditional: the verb script is replaced in room 85.
    #[test]
    fn an_effect_that_must_fire_replaces_the_value() {
        let mut p = pairs();
        p.seed((l(202), l(28)));
        let a = fact_a();
        p.add_action(&[room(28)], &[set(FACT_B, 85, vec![]), set(&a, 201, vec![])], &value, &watch);
        // and a write of 202 in room 85 that needs the verb script at 4, which it never holds there
        p.add_action(&[room(85), (a.clone(), vec![l(4)])], &[set(&a, 202, vec![])], &value, &watch);
        p.commit(&[], &|_, _| true);
        let mut got: Vec<Pair> = p.pairs().to_vec();
        got.sort();
        assert_eq!(got, vec![(l(201), l(85)), (l(202), l(28))]);
        assert!(!p.meets(&[l(202)], &[l(85)]), "(202, 85) is not reached");
    }

    /// Conditions on A and B are read at the pair itself: a case on the verb script's current value.
    #[test]
    fn a_condition_on_the_pair_is_read_at_the_pair() {
        let mut p = pairs();
        p.seed((l(4), l(33)));
        p.seed((l(202), l(28)));
        let a = fact_a();
        // in any room: 202 -> 0 (and 4 is left alone, so it may stay)
        p.add_action(&[], &[set(&a, 0, vec![(a.clone(), vec![l(202)])])], &value, &watch);
        p.commit(&[], &|_, _| true);
        let mut got: Vec<Pair> = p.pairs().to_vec();
        got.sort();
        assert_eq!(got, vec![(l(0), l(28)), (l(4), l(33)), (l(202), l(28))], "202 -> 0 must fire at 202; 4 is kept");
    }

    /// A condition on another fact frees its effect only once the reached values meet it, and the pairs computed in
    /// rounds are those computed at once.
    #[test]
    fn a_condition_on_another_fact_waits_for_the_reached_values() {
        let a = fact_a();
        let act = (vec![room(33)], vec![set(&a, 201, vec![("bit:7".into(), vec![l(1)])])]);
        let mut p = pairs();
        p.seed((l(4), l(33)));
        p.add_action(&act.0, &act.1, &value, &watch);
        p.commit(&[], &|_, _| false);
        assert!(!p.has(&(l(201), l(33))), "bit:7 = 1 not reached yet");
        assert_eq!(p.len(), 1);
        let n = p.commit(&["bit:7".to_string()], &|_, _| true);
        assert_eq!(n, 1);
        assert!(p.has(&(l(201), l(33))));
        // at once
        let mut q = pairs();
        q.seed((l(4), l(33)));
        q.add_action(&act.0, &act.1, &value, &watch);
        q.commit(&[], &|_, _| true);
        let (mut x, mut y) = (p.pairs().to_vec(), q.pairs().to_vec());
        x.sort();
        y.sort();
        assert_eq!(x, y);
    }

    /// An action written after a pair arrives is applied to it, and a pair that arrives after an action meets it: the
    /// order of arrival changes nothing.
    #[test]
    fn the_order_of_arrival_changes_nothing() {
        let a = fact_a();
        let acts: Vec<(Vec<(String, Labels)>, Vec<TEffect>)> = vec![
            (vec![room(28)], vec![set(FACT_B, 33, vec![])]),
            (vec![room(33)], vec![set(&a, 4, vec![]), set(FACT_B, 85, vec![(a.clone(), vec![l(202)])])]),
            (vec![room(85)], vec![set(&a, 201, vec![])]),
        ];
        let at_once = {
            let mut p = pairs();
            p.seed((l(202), l(28)));
            for (pre, e) in &acts {
                p.add_action(pre, e, &value, &watch);
            }
            p.commit(&[], &|_, _| true);
            let mut v = p.pairs().to_vec();
            v.sort();
            v
        };
        let one_by_one = {
            let mut p = pairs();
            p.seed((l(202), l(28)));
            for (pre, e) in acts.iter().rev() {
                p.add_action(pre, e, &value, &watch);
                p.commit(&[], &|_, _| true);
            }
            let mut v = p.pairs().to_vec();
            v.sort();
            v
        };
        assert_eq!(at_once, one_by_one);
        assert!(at_once.contains(&(l(201), l(85))));
        assert!(!at_once.contains(&(l(202), l(85))), "the move to 85 sets 4 unconditionally: {at_once:?}");
    }

    /// An add steps from the pair's own value; a value the domain lacks writes nothing.
    #[test]
    fn adds_and_bad_values() {
        let a = fact_a();
        let mut p = Pairs::new(a.clone(), FACT_B.into(), Domain::of([1, 2, 3], None, None), Domain::of([28], None, None));
        p.seed((l(1), l(28)));
        let add = TEffect { key: a.clone(), value: None, add: Some(1), values: None, when: vec![(a.clone(), vec![l(1), l(2)])] };
        p.add_action(&[], &[add], &value, &watch);
        p.commit(&[], &|_, _| true);
        let mut got: Vec<Pair> = p.pairs().to_vec();
        got.sort();
        assert_eq!(got, vec![(l(1), l(28)), (l(2), l(28)), (l(3), l(28))]);
        let mut q = Pairs::new(a.clone(), FACT_B.into(), Domain::of([1, 2], None, None), Domain::of([28], None, None));
        q.seed((l(1), l(28)));
        q.add_action(&[], &[set(&a, 9, vec![])], &|_, v| (v != l(9)).then_some(v), &watch);
        q.commit(&[], &|_, _| true);
        assert_eq!(q.pairs(), &[(l(1), l(28))], "9 is no value of the domain: nothing is written, the pair stays");
    }
}
