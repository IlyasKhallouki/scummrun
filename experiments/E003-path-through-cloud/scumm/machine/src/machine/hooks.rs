//! Hook points for a machine that overrides model.py methods, as reader.py's `Reader` subclasses `model.Machine`
//! (the reader crate fills them; PORT-NOTES.md of scumm/reader lists every machine change it needed).
//!
//! Each hooked method `foo` is `foo_base` where it is defined, unchanged; `foo` here dispatches to `Hooks::foo`
//! when the machine has hooks, else to `foo_base`. Every default method of [`Hooks`] is `foo_base`, so a machine
//! without hooks (every caller in this crate) runs as before. `Machine::ext` holds the hooking side's own state;
//! `Path::ext`, `Sig::ext` and `Unknown::class` carry what it adds to paths, join signatures and unknowns. The
//! last block gives the hooking side the machine's own steps it calls (model.py's methods of the same names).

use super::*;

/// model.py methods a subclass may override. `m` is the machine; the defaults are model.py's.
#[allow(clippy::too_many_arguments)]
pub trait Hooks {
    fn advance(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle) -> Vec<Path> {
        m.advance_base(p, o)
    }
    fn read(&self, m: &mut Machine<'_>, p: &mut Path, k: Fact, o: &FactOracle) -> Val {
        m.read_base(p, k, o)
    }
    /// Fix round 1 (no model.py method): a read that must not fork, for a rule that decides only on a value it
    /// already has (F9's actorFollowCamera). A machine's read never forks, so the default is `read`; the reader
    /// reads lazily (the fact's values as a set).
    fn read_soft(&self, m: &mut Machine<'_>, p: &mut Path, k: Fact, o: &FactOracle) -> Val {
        m.read(p, k, o)
    }
    /// `oracle.get(k)`
    fn oget(&self, m: &mut Machine<'_>, o: &FactOracle, k: Fact) -> Val {
        m.oget_base(o, k)
    }
    /// `oracle.keys(prefix)`
    fn okeys(&self, _m: &Machine<'_>, o: &FactOracle, pre: Prefix) -> Vec<Fact> {
        o.keys(pre).to_vec()
    }
    fn val(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, e: ExprId, x: Option<X>) -> Val {
        m.val_base(p, o, e, x)
    }
    fn test(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, c: ExprId, x: Option<X>) -> Dec {
        m.test_base(p, o, c, x)
    }
    fn getter(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, name: Sym, args: Span, x: Option<X>) -> Val {
        m.getter_base(p, o, name, args, x)
    }
    fn owned_by(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, owner: i32) -> Result<Vec<i32>, (Fact, Val)> {
        m.owned_by_base(p, o, owner)
    }
    fn find_inventory(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, args: Span, x: Option<X>) -> Val {
        m.find_inventory_base(p, o, args, x)
    }
    fn clear(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, pre: Prefix) {
        m.clear_base(p, o, pre)
    }
    fn arith_at(&self, m: &mut Machine<'_>, p: &mut Path, op: Op, a: Val, b: Val, x: Option<X>) -> Val {
        m.arith_at_base(p, op, a, b, x)
    }
    fn jump(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, x: X, cond: Option<ExprId>, idx: Option<u32>) -> Vec<Path> {
        m.jump_base(p, o, x, cond, idx)
    }
    fn wait(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, x: X, name: Sym) -> Vec<Path> {
        m.wait_base(p, o, x, name)
    }
    /// model.py's `_op_<name>` for every op the step runs a rule for
    fn op(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, x: X, st: Stmt) -> Vec<Path> {
        m.op_base(p, o, x, st)
    }
    fn join(&self, m: &Machine<'_>, p: &Path, o: &FactOracle) -> Option<Sig> {
        m.join_base(p, o)
    }
    fn merge(&self, m: &mut Machine<'_>, o: &FactOracle, group: Vec<Path>, sig: Rc<Sig>) -> Path {
        m.merge_base(o, group, sig)
    }
    /// model.py's `_eng_<step>` for every call step
    fn eng(&self, m: &mut Machine<'_>, p: &mut Path, ci: usize, step: &Step, o: &FactOracle) -> Vec<Path> {
        m.eng_base(p, ci, step, o)
    }
    fn eng_script(
        &self,
        m: &mut Machine<'_>,
        p: &mut Path,
        ci: usize,
        o: &FactOracle,
        n: i32,
        resistant: Option<bool>,
        recursive: Option<bool>,
        args: &[(i32, Val)],
    ) {
        m.eng_script_base(p, ci, o, n, resistant, recursive, args)
    }
    fn finish(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, how: &'static str, at: S, resume: Option<i32>) {
        m.finish_base(p, o, how, at, resume)
    }
    fn count(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, name: Slot) -> Option<(i32, bool)> {
        m.count_base(p, o, name)
    }
    fn set_freeze(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, name: Slot, n: Option<(i32, bool)>, why: &str) {
        m.set_freeze_base(p, o, name, n, why)
    }
    fn freeze(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, flag: i32, x: X) {
        m.freeze_base(p, o, flag, x)
    }
    fn unfreeze(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle) {
        m.unfreeze_base(p, o)
    }
}

// --- the dispatchers: every caller in the crate calls these ---------------------------------------------------------

#[allow(clippy::too_many_arguments)]
impl<'g> Machine<'g> {
    #[inline]
    pub fn advance(&mut self, p: &mut Path, o: &FactOracle) -> Vec<Path> {
        match self.hooks {
            Some(h) => h.advance(self, p, o),
            None => self.advance_base(p, o),
        }
    }
    #[inline]
    pub fn read(&mut self, p: &mut Path, k: Fact, o: &FactOracle) -> Val {
        match self.hooks {
            Some(h) => h.read(self, p, k, o),
            None => self.read_base(p, k, o),
        }
    }
    #[inline]
    pub fn read_soft(&mut self, p: &mut Path, k: Fact, o: &FactOracle) -> Val {
        match self.hooks {
            Some(h) => h.read_soft(self, p, k, o),
            None => self.read_base(p, k, o),
        }
    }
    #[inline]
    pub fn oget(&mut self, o: &FactOracle, k: Fact) -> Val {
        match self.hooks {
            Some(h) => h.oget(self, o, k),
            None => self.oget_base(o, k),
        }
    }
    #[inline]
    pub fn okeys(&self, o: &FactOracle, pre: Prefix) -> Vec<Fact> {
        match self.hooks {
            Some(h) => h.okeys(self, o, pre),
            None => o.keys(pre).to_vec(),
        }
    }
    #[inline]
    pub fn val(&mut self, p: &mut Path, o: &FactOracle, e: ExprId, x: Option<X>) -> Val {
        match self.hooks {
            Some(h) => h.val(self, p, o, e, x),
            None => self.val_base(p, o, e, x),
        }
    }
    #[inline]
    pub fn test(&mut self, p: &mut Path, o: &FactOracle, c: ExprId, x: Option<X>) -> Dec {
        match self.hooks {
            Some(h) => h.test(self, p, o, c, x),
            None => self.test_base(p, o, c, x),
        }
    }
    #[inline]
    pub fn getter(&mut self, p: &mut Path, o: &FactOracle, name: Sym, args: Span, x: Option<X>) -> Val {
        match self.hooks {
            Some(h) => h.getter(self, p, o, name, args, x),
            None => self.getter_base(p, o, name, args, x),
        }
    }
    #[inline]
    pub fn owned_by(&mut self, p: &mut Path, o: &FactOracle, owner: i32) -> Result<Vec<i32>, (Fact, Val)> {
        match self.hooks {
            Some(h) => h.owned_by(self, p, o, owner),
            None => self.owned_by_base(p, o, owner),
        }
    }
    #[inline]
    pub fn find_inventory(&mut self, p: &mut Path, o: &FactOracle, args: Span, x: Option<X>) -> Val {
        match self.hooks {
            Some(h) => h.find_inventory(self, p, o, args, x),
            None => self.find_inventory_base(p, o, args, x),
        }
    }
    #[inline]
    pub fn clear(&mut self, p: &mut Path, o: &FactOracle, pre: Prefix) {
        match self.hooks {
            Some(h) => h.clear(self, p, o, pre),
            None => self.clear_base(p, o, pre),
        }
    }
    #[inline]
    pub fn arith_at(&mut self, p: &mut Path, op: Op, a: Val, b: Val, x: Option<X>) -> Val {
        match self.hooks {
            Some(h) => h.arith_at(self, p, op, a, b, x),
            None => self.arith_at_base(p, op, a, b, x),
        }
    }
    #[inline]
    pub fn jump(&mut self, p: &mut Path, o: &FactOracle, x: X, cond: Option<ExprId>, idx: Option<u32>) -> Vec<Path> {
        match self.hooks {
            Some(h) => h.jump(self, p, o, x, cond, idx),
            None => self.jump_base(p, o, x, cond, idx),
        }
    }
    #[inline]
    pub fn wait(&mut self, p: &mut Path, o: &FactOracle, x: X, name: Sym) -> Vec<Path> {
        match self.hooks {
            Some(h) => h.wait(self, p, o, x, name),
            None => self.wait_base(p, o, x, name),
        }
    }
    #[inline]
    pub fn op(&mut self, p: &mut Path, o: &FactOracle, x: X, st: Stmt) -> Vec<Path> {
        match self.hooks {
            Some(h) => h.op(self, p, o, x, st),
            None => self.op_base(p, o, x, st),
        }
    }
    #[inline]
    pub fn join(&self, p: &Path, o: &FactOracle) -> Option<Sig> {
        match self.hooks {
            Some(h) => h.join(self, p, o),
            None => self.join_base(p, o),
        }
    }
    #[inline]
    pub fn merge(&mut self, o: &FactOracle, group: Vec<Path>, sig: Rc<Sig>) -> Path {
        match self.hooks {
            Some(h) => h.merge(self, o, group, sig),
            None => self.merge_base(o, group, sig),
        }
    }
    #[inline]
    pub fn eng(&mut self, p: &mut Path, ci: usize, step: &Step, o: &FactOracle) -> Vec<Path> {
        match self.hooks {
            Some(h) => h.eng(self, p, ci, step, o),
            None => self.eng_base(p, ci, step, o),
        }
    }
    #[inline]
    pub fn eng_script(
        &mut self,
        p: &mut Path,
        ci: usize,
        o: &FactOracle,
        n: i32,
        resistant: Option<bool>,
        recursive: Option<bool>,
        args: &[(i32, Val)],
    ) {
        match self.hooks {
            Some(h) => h.eng_script(self, p, ci, o, n, resistant, recursive, args),
            None => self.eng_script_base(p, ci, o, n, resistant, recursive, args),
        }
    }
    #[inline]
    pub fn finish(&mut self, p: &mut Path, o: &FactOracle, how: &'static str, at: S, resume: Option<i32>) {
        match self.hooks {
            Some(h) => h.finish(self, p, o, how, at, resume),
            None => self.finish_base(p, o, how, at, resume),
        }
    }
    #[inline]
    pub fn count(&mut self, p: &mut Path, o: &FactOracle, name: Slot) -> Option<(i32, bool)> {
        match self.hooks {
            Some(h) => h.count(self, p, o, name),
            None => self.count_base(p, o, name),
        }
    }
    #[inline]
    pub fn set_freeze(&mut self, p: &mut Path, o: &FactOracle, name: Slot, n: Option<(i32, bool)>, why: &str) {
        match self.hooks {
            Some(h) => h.set_freeze(self, p, o, name, n, why),
            None => self.set_freeze_base(p, o, name, n, why),
        }
    }
    #[inline]
    pub fn freeze(&mut self, p: &mut Path, o: &FactOracle, flag: i32, x: X) {
        match self.hooks {
            Some(h) => h.freeze(self, p, o, flag, x),
            None => self.freeze_base(p, o, flag, x),
        }
    }
    #[inline]
    pub fn unfreeze(&mut self, p: &mut Path, o: &FactOracle) {
        match self.hooks {
            Some(h) => h.unfreeze(self, p, o),
            None => self.unfreeze_base(p, o),
        }
    }
}

// --- what a hook calls: the machine's own steps (model.py's methods of the same name) -------------------------------

#[allow(clippy::too_many_arguments)]
impl<'g> Machine<'g> {
    /// The current run's unknowns.
    pub fn vals(&mut self) -> &mut Vals {
        &mut self.v
    }
    /// The current run's unknowns, read only.
    pub fn vals_ref(&self) -> &Vals {
        &self.v
    }
    /// `write`
    pub fn h_write(&mut self, p: &mut Path, k: Fact, v: Val) {
        self.write(p, k, v)
    }
    /// `_halt`
    pub fn h_halt(&mut self, p: &mut Path, reason: &'static str, at: S) {
        self.halt(p, reason, at)
    }
    /// `_note`
    pub fn h_note(&mut self, p: &mut Path, cite: S, head: &str, what: &str) {
        self.note(p, cite, head, what)
    }
    /// `_done`
    pub fn h_done(&mut self, p: &mut Path, o: &FactOracle, how: &'static str, at: S, resume: Option<i32>) {
        self.done(p, o, how, at, resume)
    }
    /// `Game.resource` with its unknowns
    pub fn h_resource(&mut self, room: Val) -> Val {
        self.resource(room)
    }
    /// `_push_frame`
    pub fn h_push_frame(
        &mut self,
        p: &mut Path,
        ci: usize,
        unit: UnitId,
        pc: u32,
        locals: &[(i32, Val)],
        owned: Owned,
        recursive: Option<bool>,
        resistant: Option<bool>,
        key: Option<Fact>,
        args_known: bool,
        nested: bool,
    ) {
        self.push_frame(p, ci, unit, pc, locals, owned, recursive, resistant, key, args_known, nested)
    }
    /// `_stop_script`
    pub fn h_stop_script(&mut self, p: &mut Path, o: &FactOracle, n: i32, cite: &S, head: &str) {
        self.stop_script(p, o, n, cite, head)
    }
    /// `_live_slots`
    pub fn h_live_slots(&mut self, p: &mut Path, o: &FactOracle) -> Vec<(Slot, Option<Fact>, Val)> {
        self.live_slots(p, o)
    }
    /// A new `Call.cid` (a call the hook makes).
    pub fn h_cid(&mut self) -> u64 {
        self.cid()
    }
    /// `_drive` from `first`.
    pub fn h_drive(&mut self, o: &FactOracle, first: Path, start: String, unit: Option<UnitKey>) -> Run {
        self.drive(o, first, start, unit)
    }
    /// model.py `WAITS[head]`: the reason and what of a WaitFor op.
    pub fn h_waits(name: Sym) -> (Reason, &'static str) {
        waits(name)
    }
}
