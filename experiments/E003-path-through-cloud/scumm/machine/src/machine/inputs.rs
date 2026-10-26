//! Fix round 1 (fix-round1.md), F1, in the Rust only: model.py has no such rules, and the Python stays the frozen
//! reference (decisions.md D7). Each rule runs only with switch `input_script` on; with it off nothing here runs.
//!
//! **An input is one frame** (scumm.cpp:3107-3255): processInput reads the mouse (3177) and updateScummVars copies
//! it into VAR_MOUSE_X/Y and VAR_VIRT_MOUSE_X/Y (3198, 3385-3388); every live slot takes its turn (3231);
//! checkExecVerbs takes the click (3232; verbs.cpp:586-692) and runs the input script VAR_VERB_SCRIPT holds then
//! (runInputScript, script.cpp:1478-1511); checkAndRunSentenceScript runs a sentence the frame queued (3255;
//! script.cpp:1176-1250). The caller (the reader's input run) builds that frame; this module gives the machine
//! what it lacks for it:
//!
//! * **findObject at a cell.** Where the path's mouse is a cell of the room's box arrangement (`Path.cell`, set by the
//!   frame's mouse step; scumm-ir `cells.rs`), findObject(VAR_VIRT_MOUSE_X, VAR_VIRT_MOUSE_Y) is decided by facts
//!   (object.cpp:557-597): the cell's objects in the room's list order, the first one not of class Untouchable,
//!   whose parents are each in their parent state, and, with F5 (draw_clears), drawn at the place whose box makes
//!   the cell. None of them: 0.
//! * **doSentence queues** (script.cpp:1145-1172): the sentence goes on the path's queue, and doSentence(STOP)
//!   empties it (script_v5.cpp:1004-1008), as settle has it; the frame's last step runs the queue's top
//!   (`eng_sentences`). A sentence held because the sentence script is live and not frozen (script.cpp:1186-1191)
//!   stays queued in the engine; the path lists it as an unknown write (the queue is no fact).
//!
//! Nothing here names a game's room, object, verb or script.

use super::*;

impl<'g> Machine<'g> {
    /// F1: findObject at the path's cell (the module's docstring), or None when the arguments are not the path's
    /// mouse at a cell. A fact the path cannot read as a number makes the result the set of the objects still
    /// possible and 0.
    pub(super) fn find_object_at_cell(&mut self, p: &mut Path, o: &FactOracle, args: &[Val]) -> Option<Val> {
        let g = self.game;
        let (res, ci) = p.cell?;
        let vx = *p.writes.get(&Fact::Var(VAR_VIRT_MOUSE_X))?;
        let vy = *p.writes.get(&Fact::Var(VAR_VIRT_MOUSE_Y))?;
        if args != [vx, vy] {
            return None;
        }
        let cell = g.cells[self.switches.draw_clears as usize].get(&res)?.get(ci as usize)?.clone();
        let list = g.room_list.get(&res)?;
        let mut maybe: Vec<i32> = Vec::new();
        let mut found = false;
        'next: for &(ob, place) in &cell.boxes {
            let mut sure = true;
            // getClass(obj, kObjectClassUntouchable) (object.cpp:562)
            match self.read(p, Fact::Class(ob, CLASS_UNTOUCHABLE), o) {
                Val::Int(0) => {}
                Val::Int(_) => continue,
                Val::Unk(_) => sure = false,
            }
            // the chain of parents, each in its parent state (object.cpp:571-593; v5's mask is 0xF)
            let Some(mut at) = list.iter().position(|l| l.obj == ob) else { continue };
            for _ in 0..list.len() {
                let lo = list[at];
                if lo.parent == 0 {
                    break;
                }
                let Some(par) = list.get(lo.parent as usize - 1) else { continue 'next };
                match self.read(p, Fact::State(par.obj), o) {
                    Val::Int(s) if (s & 0xF) == lo.parent_state as i32 => {}
                    Val::Int(_) => continue 'next,
                    Val::Unk(_) => sure = false,
                }
                at = lo.parent as usize - 1;
            }
            // F5: the object's box is at the place that makes this cell
            if self.switches.draw_clears && g.movable.contains_key(&ob) {
                match self.read(p, Fact::ObjPos(ob), o) {
                    Val::Int(n) if n == place as i32 => {}
                    Val::Int(_) => continue,
                    Val::Unk(_) => sure = false,
                }
            } else if place != 0 {
                continue;
            }
            if sure && maybe.is_empty() {
                return Some(Val::Int(ob));
            }
            maybe.push(ob);
            if sure {
                found = true;
                break;
            }
        }
        if maybe.is_empty() {
            return Some(Val::Int(0));
        }
        if !found {
            maybe.push(0);
        }
        Some(self.v.one_of(maybe, Reason::Position, "findObject at the mouse's cell"))
    }

    /// F1: actorFromPos at the path's cell (actor.cpp:2115-2147): a cell is a target with no actor under the click,
    /// which the replay checks (an actor is a target of its own); None when the arguments are not the path's mouse.
    pub(super) fn actor_at_cell(&mut self, p: &mut Path, args: &[Val], x: Option<X>) -> Option<Val> {
        let (res, ci) = p.cell?;
        let vx = *p.writes.get(&Fact::Var(VAR_VIRT_MOUSE_X))?;
        let vy = *p.writes.get(&Fact::Var(VAR_VIRT_MOUSE_Y))?;
        if args != [vx, vy] {
            return None;
        }
        let cite = match x {
            Some(x) => self.cite(x),
            None => rc("actor.cpp:2115-2147"),
        };
        let cond = rc(&format!("no actor under the click at cell {res}.{ci}"));
        let why = rc(&format!("?({}: actorFromPos at the mouse's cell)", Reason::Position.as_str()));
        let a = Assumption { cite, cond, side: Side::Bool(true), why };
        if !p.assumptions.iter().any(|b| *b == a) {
            p.assumptions.push(a);
        }
        Some(Val::Int(0))
    }

    /// F1: the frame's last step on a path whose sentence stays queued (the sentence script holds it): listed.
    pub(super) fn sentence_held(&mut self, p: &mut Path, ci: usize, busy: &[String]) {
        let (_, c) = Self::head_cite(p, ci);
        let w = format!("a queued sentence waits for {} (script.cpp:1186-1191): the queue is no fact", busy.join(", "));
        p.unknown_writes.push((rc("checkAndRunSentenceScript"), c, w.into()));
    }
}
