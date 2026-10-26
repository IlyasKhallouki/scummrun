//! Fix round 1 (fix-round1.md), F5, in the Rust only: model.py has no such rules, and the Python stays the frozen
//! reference (decisions.md D7). Each rule runs only with its switch on; with every switch off nothing here runs.
//!
//! Switch `draw_clears`: o5_drawObject (script_v5.cpp:1031-1129). A setXY sub-op whose x is not 255 moves the
//! object to (x * 8, y * 8) as int16 (1108-1114): `objpos:O`, an index into `Game::movable[O]` (0: its CDHD place).
//! Every other object of the room with the drawn object's width, height and place goes to state 0 (1118-1126); a
//! negative coordinate matches nothing (the int16 place against the uint16 one). Then the drawn object gets its
//! state (1128). Loading a room puts its objects back at their CDHD places (room.cpp:195): `objpos:O` = 0 for the
//! room's movable objects. A place the path does not know (an `objpos:` fact it cannot read) makes the clearing
//! uncertain: the other object's state becomes the set of its value and 0. Nothing in the model reads an object's
//! walk point (walkActorToObject has no effect, getDist is not a fact), so the walk point setXY also moves
//! (1109, 1111) has no fact.

use super::*;
use scumm_ir::places::{same_place, set_xy_place};

impl<'g> Machine<'g> {
    /// F5: the places object `ob` can be at on this path (one when known), or None when it has no box.
    fn places_of(&mut self, p: &mut Path, o: &FactOracle, ob: i32) -> Option<Vec<(i32, i32)>> {
        let g = self.game;
        let b = g.object_box.get(&ob)?;
        let Some(places) = g.movable.get(&ob) else { return Some(vec![(b.x, b.y)]) };
        let v = self.read(p, Fact::ObjPos(ob), o);
        let all = || places.clone();
        let idx = match self.v.values_of(v) {
            Some(vs) => vs,
            None => return Some(all()),
        };
        if idx.iter().any(|&i| i < 0 || i as usize >= places.len()) {
            return Some(all());
        }
        Some(idx.into_iter().map(|i| places[i as usize]).collect())
    }

    /// F5: o5_drawObject with switch draw_clears (the module's docstring).
    pub(super) fn draw_object_f5(&mut self, p: &mut Path, o: &FactOracle, x: X, args: &[ExprId], subops: Span) -> Vec<Path> {
        let g = self.game;
        let cite = self.cite(x);
        let ob = self.val(p, o, args[0], Some(x));
        let mut state = Val::Int(1);
        let mut xy: Option<(Val, Val)> = None;
        for s in g.ast.subs(subops) {
            if s.name == known::SET_IMAGE {
                state = self.val(p, o, g.ast.kids(s.args)[0], Some(x));
            } else if s.name == known::SET_XY {
                let k = g.ast.kids(s.args);
                let a = self.val(p, o, k[0], Some(x));
                let b = self.val(p, o, k[1], Some(x));
                xy = Some((a, b));
            }
        }
        let Val::Int(ob) = ob else {
            let w = format!("object {}", self.show(ob));
            p.unknown_writes.push((rc("drawObject"), cite, w.into()));
            return Vec::new();
        };
        let room = self.read(p, Fact::Room, o);
        let res = match self.resource(room) {
            res @ Val::Unk(_) => {
                let u = self.v.mk(Reason::Work, format!("room {}", self.show(res)));
                self.write(p, Fact::State(ob), u);
                let w = format!("the objects of room {} that share object {ob}'s place and size", self.show(res));
                p.unknown_writes.push((rc("drawObject"), cite, w.into()));
                return Vec::new();
            }
            Val::Int(r) => r,
        };
        if g.object_room.get(&ob) != Some(&res) {
            return Vec::new(); // getObjectIndex finds only the room's objects
        }
        if let Some((vx, vy)) = xy {
            let moved = match (vx, vy) {
                (Val::Int(px), Val::Int(py)) => match set_xy_place(px, py) {
                    None => None, // x 255: no setXY to the engine
                    Some(pl) => Some(match g.movable.get(&ob).and_then(|ps| ps.iter().position(|&q| q == pl)) {
                        Some(i) => Val::Int(i as i32),
                        None => self.v.mk(Reason::Position, format!("objpos:{ob}: place {pl:?}, none of its places")),
                    }),
                },
                _ => {
                    let w = format!("objpos:{ob} after setXY({}, {})", self.show(vx), self.show(vy));
                    Some(self.v.mk(Reason::Position, w))
                }
            };
            if let Some(v) = moved {
                if g.movable.contains_key(&ob) {
                    self.write(p, Fact::ObjPos(ob), v);
                } else {
                    let w = format!("objpos:{ob}: object {ob} has no place table (no setXY reaches it)");
                    p.unknown_writes.push((rc("drawObject"), cite.clone(), w.into()));
                }
            }
        }
        if let (Some(b), Some(here)) = (g.object_box.get(&ob).copied(), self.places_of(p, o, ob)) {
            let others: Vec<i32> = g.room_objects.get(&res).map(|v| v.iter().rev().copied().collect()).unwrap_or_default();
            for other in others {
                let ob2 = g.object_box[&other];
                if other == ob || ob2.w != b.w || ob2.h != b.h {
                    continue;
                }
                let there = self.places_of(p, o, other).unwrap();
                if !here.iter().any(|&a| there.iter().any(|&c| same_place(a, c))) {
                    continue;
                }
                if here.len() == 1 && there.len() == 1 {
                    self.write(p, Fact::State(other), Val::Int(0));
                    continue;
                }
                let old = self.read(p, Fact::State(other), o);
                let what = format!("state:{other}: 0 if it shares object {ob}'s place");
                let v = match self.v.values_of(old) {
                    Some(mut vs) => {
                        vs.push(0);
                        self.v.one_of(vs, Reason::Position, what)
                    }
                    None => self.v.mk(Reason::Position, what),
                };
                self.write(p, Fact::State(other), v);
            }
        }
        self.write(p, Fact::State(ob), state);
        Vec::new()
    }

    /// F5: loading room resource `res` puts its movable objects back at their CDHD places (room.cpp:195).
    pub(super) fn objpos_reset(&mut self, p: &mut Path, res: Val) {
        if !self.switches.draw_clears {
            return;
        }
        let g = self.game;
        let Val::Int(res) = res else { return };
        let Some(objs) = g.room_objects.get(&res) else { return };
        for &ob in objs {
            if g.movable.contains_key(&ob) {
                self.write(p, Fact::ObjPos(ob), Val::Int(0));
            }
        }
    }
}
