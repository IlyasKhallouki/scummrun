//! Fix round 1 (fix-round1.md), F9, in the Rust only: model.py has no such rules, and the Python stays the frozen
//! reference (decisions.md D7). Each rule runs only with its switch on; with every switch off nothing here runs.
//!
//! Switch `actor_rooms`: an actor's room (Actor::_room, a byte) is `actorroom:A`. o5_putActorInRoom writes it
//! (script_v5.cpp:2160-2196), o5_loadRoomWithEgo writes the ego's before its startScene (1857-1866); nothing else
//! in v5 does (actorOps' Init is initActor(0), which keeps the room: actor.cpp:161-192). getActorRoom reads it, 0
//! for an actor number that is no actor (script_v5.cpp:1289-1322, isValidActor: actor.cpp:2277-2279);
//! actorFollowCamera's setCameraFollows tests it against the current room and changes to it (camera.cpp:62-70).

use super::*;

impl<'g> Machine<'g> {
    /// F9: the room of actor value `a` (getActorRoom: 0 for a number that is no actor).
    pub(super) fn actor_room(&mut self, p: &mut Path, o: &FactOracle, a: i32) -> Val {
        if !(0..NUM_ACTORS).contains(&a) {
            return Val::Int(0);
        }
        self.read(p, Fact::ActorRoom(a), o)
    }

    /// F9: o5_putActorInRoom with switch actor_rooms: the actor's room is the value as a byte. The stopTalk it runs
    /// for a visible talking actor leaving the current room (2165-2167) stays out of the model, as before.
    pub(super) fn put_actor_in_room(&mut self, p: &mut Path, o: &FactOracle, x: X, args: &[ExprId]) -> Vec<Path> {
        let cite = self.cite(x);
        let a = self.val(p, o, args[0], Some(x));
        let r = self.val(p, o, args[1], Some(x));
        let a = match a {
            Val::Int(a) => a,
            Val::Unk(_) => {
                let w = format!("actorroom of actor {}", self.show(a));
                p.unknown_writes.push((rc("putActorInRoom"), cite, w.into()));
                return Vec::new();
            }
        };
        if !(0..NUM_ACTORS).contains(&a) {
            let d = format!("putActorInRoom: actor {a} is no actor (derefActor's error(), actor.cpp:2281-2290)");
            self.error(p, "no such actor", cite, &d, true);
            return Vec::new();
        }
        let v = self.byte(r, format!("actorroom:{a}"));
        self.write(p, Fact::ActorRoom(a), v);
        Vec::new()
    }

    /// F9: o5_loadRoomWithEgo's `a->putActor(room)` (script_v5.cpp:1866): the ego's room before its startScene.
    pub(super) fn ego_room(&mut self, p: &mut Path, o: &FactOracle, x: X, room: Val) {
        if !self.switches.actor_rooms {
            return;
        }
        let ego = self.read(p, Fact::Var(VAR_EGO), o);
        match ego {
            Val::Int(e) if (0..NUM_ACTORS).contains(&e) => {
                let v = self.byte(room, format!("actorroom:{e}"));
                self.write(p, Fact::ActorRoom(e), v);
            }
            _ => {
                let cite = self.cite(x);
                let w = format!("actorroom of the ego {}", self.show(ego));
                p.unknown_writes.push((rc("loadRoomWithEgo"), cite, w.into()));
            }
        }
    }

    /// A value stored in a byte (Actor::_room): itself when every value it can have fits.
    fn byte(&mut self, v: Val, what: String) -> Val {
        match v {
            Val::Int(n) => Val::Int(n & 0xFF),
            Val::Unk(_) => match self.v.values_of(v) {
                Some(vs) if vs.iter().any(|n| !(0..=0xFF).contains(n)) => {
                    let vs = vs.into_iter().map(|n| n & 0xFF).collect();
                    self.v.one_of(vs, Reason::Actor, what)
                }
                _ => v,
            },
        }
    }
}
