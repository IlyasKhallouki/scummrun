//! Feature room_change (row 2): startScene as a routine of the opcode that changes the room (model.py
//! `_eng_scene` and the steps it pushes: `_eng_scene_begin`, `_eng_room_code`, `_eng_scene_kill` with
//! `_kill_room`, `_eng_scene_load`, `_eng_scene_ego`).
//!
//! Each method is the model.py method of the same name, in the same order of reads, writes, notes, errors
//! and ends. model.py's docstrings hold the engine rules and the ScummVM cites.

use super::calls::RoomSlot;
use super::*;

impl<'g> Machine<'g> {
    /// startScene(room, ego, object) (model.py `_eng_scene`): its steps, as a call under the opcode's caller
    /// with the opcode's call's activity.
    pub(super) fn eng_scene(&mut self, p: &mut Path, ci: usize, room: Val, ego: Option<Val>) {
        let mut steps = vec![
            Step::SceneBegin(room),
            Step::Hook { var: VAR_EXIT_SCRIPT, args: Vec::new() },
            Step::RoomCode { entry: false },
            Step::Hook { var: VAR_EXIT_SCRIPT2, args: Vec::new() },
            Step::SceneKill,
            Step::SceneLoad(room),
        ];
        if let Some(e) = ego {
            steps.push(Step::SceneEgo(e));
        }
        steps.extend([
            Step::Hook { var: VAR_ENTRY_SCRIPT, args: Vec::new() },
            Step::RoomCode { entry: true },
            Step::Hook { var: VAR_ENTRY_SCRIPT2, args: Vec::new() },
        ]);
        let (caller, active) = {
            let c = p.call_at(ci);
            (c.caller, c.active)
        };
        let cid = self.cid();
        let call = Call {
            head: rc("startScene"),
            cite: rc("room.cpp:42-260"),
            steps: steps.into(),
            caller,
            active,
            i: 0,
            cid,
        };
        p.stack.push(Entry::Call(call));
    }

    /// room.cpp:53-76 (model.py `_eng_scene_begin`): stopTalk, the running room slot no longer resumes (an
    /// open cutscene there ends the path), VAR_NEW_ROOM.
    pub(super) fn eng_scene_begin(&mut self, p: &mut Path, ci: usize, o: &FactOracle, room: Val) {
        let talk = self.read(p, Fact::Var(VAR_TALK_ACTOR), o);
        match talk {
            Val::Unk(_) => {
                let u = self.v.mk(Reason::Work, format!("stopTalk with VAR_TALK_ACTOR {}", self.show(talk)));
                self.write(p, Fact::Var(VAR_TALK_ACTOR), u);
            }
            Val::Int(t) if 0 < t && t < 0x80 => self.write(p, Fact::Var(VAR_TALK_ACTOR), Val::Int(0xFF)),
            Val::Int(_) => {}
        }
        let (head, cite) = Self::head_cite(p, ci);
        let (caller, active) = {
            let c = p.call_at(ci);
            (c.caller, c.active)
        };
        if let (true, Some(fi)) = (active, p.frame_of(caller)) {
            let f = p.frame_at(fi);
            let (unit, override_) = (f.unit, f.override_);
            match self.room_slot(p.frame_at(fi)) {
                RoomSlot::Unk(u) => {
                    let w = format!("slot kind unknown ({}); taken as surviving the room change", self.show(u));
                    self.note(p, cite, &head, &w);
                }
                RoomSlot::Yes => {
                    if override_ != 0 {
                        let name = Game::unit_name(self.unit_key(unit));
                        let d = format!("{name} (room.cpp:61-62, 67-68, an error())");
                        self.error(p, "room change in an open cutscene", cite, &d, true);
                    }
                    p.call_at(ci).active = false;
                }
                RoomSlot::No => {}
            }
        }
        self.write(p, Fact::Var(VAR_NEW_ROOM), room);
    }

    /// The room's EXCD or ENCD run nested in a room slot with no arguments, when the room has it (model.py
    /// `_eng_room_code`).
    pub(super) fn eng_room_code(&mut self, p: &mut Path, ci: usize, o: &FactOracle, entry: bool) {
        let room = self.read(p, Fact::Room, o);
        let res = self.resource(room);
        let which = if entry { "entry" } else { "exit" };
        let r = match res {
            Val::Unk(_) => {
                let (head, cite) = Self::head_cite(p, ci);
                let w = format!("the {which} code of room {}", self.show(res));
                p.unknown_writes.push((head, cite, rc(&w)));
                return;
            }
            Val::Int(r) => r,
        };
        let unit = if entry { UnitKey::Entry(r) } else { UnitKey::Exit(r) };
        if let Some(uid) = self.game.id(unit) {
            self.push_frame(p, ci, uid, 0, &[], Owned::Bool(false), Some(false), Some(false), None, true, true);
        }
    }

    /// killScriptsAndResources (model.py `_kill_room`): every room, flobject and local slot dies, frames on the
    /// stack too; inventory and global slots live on. An object slot off the stack is a room slot when its
    /// object is the room's now: an assumption.
    pub(super) fn kill_room(&mut self, p: &mut Path, ci: usize, o: &FactOracle) {
        let (head, cite) = Self::head_cite(p, ci);
        for i in 0..p.stack.len() {
            let Entry::Frame(f) = &p.stack[i] else { continue };
            let (unit, killed, override_) = (f.unit, f.killed, f.override_);
            match self.room_slot(f) {
                RoomSlot::Unk(u) => {
                    let name = Game::unit_name(self.unit_key(unit));
                    let w = format!("{name}: slot kind {}; taken as surviving", self.show(u));
                    self.note(p, cite.clone(), &head, &w);
                }
                RoomSlot::Yes if killed.is_none() => {
                    if override_ != 0 {
                        let name = Game::unit_name(self.unit_key(unit));
                        let d = format!("{name} (a warning, script.cpp:1089-1093)");
                        self.error(p, "killed with an open cutscene", cite.clone(), &d, false);
                    }
                    p.frame_at(i).killed = Some("room");
                    self.gone(p, o, unit);
                }
                _ => {}
            }
        }
        for (name, key, _live) in self.live_slots(p, o) {
            match name {
                Slot::Obj(ob) => {
                    let owner = self.read(p, Fact::Owner(ob), o);
                    let Val::Int(ow) = owner else {
                        let u = self.v.mk(Reason::Work, format!("{name}: slot kind not known ({})", self.show(owner)));
                        self.write(p, key.expect("an object slot's fact"), u);
                        continue;
                    };
                    let kind = if ow == OF_OWNER_ROOM { "room" } else { "inventory" };
                    p.assumptions.push(Assumption {
                        cite: cite.clone(),
                        cond: rc(&format!("{name} runs in a {kind} slot (its owner is {ow})")),
                        side: Side::Bool(true),
                        why: rc("the slot kind is not a fact"),
                    });
                    if ow != OF_OWNER_ROOM {
                        continue;
                    }
                }
                Slot::Local(_) | Slot::Entry | Slot::Exit => {}
                Slot::Script(_) => continue,
            }
            if let Some(k) = key {
                self.write(p, k, Val::Int(0));
            }
            self.clear(p, o, Prefix::At(name));
            self.clear(p, o, Prefix::Esc(name));
            self.set_freeze(p, o, name, Some((0, true)), "");
            p.pop_slot(name);
        }
    }

    /// room.cpp:149-218 (model.py `_eng_scene_load`): the room, VAR_ROOM, VAR_ROOM_RESOURCE, VAR_ROOM_FLAG, the
    /// camera bounds; room 0 and a room whose resource is 0 stop here.
    pub(super) fn eng_scene_load(&mut self, p: &mut Path, ci: usize, o: &FactOracle, room: Val) {
        let g = self.game;
        let res = self.resource(room);
        self.write(p, Fact::Room, room);
        self.write(p, Fact::Var(VAR_ROOM), room);
        self.write(p, Fact::Var(VAR_ROOM_RESOURCE), res);
        if room == Val::Int(0) {
            let c = p.call_at(ci);
            c.i = c.steps.len() as u32;
            return;
        }
        if res != Val::Int(0) {
            let flag = self.read(p, Fact::Var(VAR_ROOM_FLAG), o);
            // model.py builds this unknown directly: no value cap, and {1} would stay a set (flag 1 writes 1)
            let what = "1 if the room resource was not in memory";
            let v = match flag {
                Val::Int(1) => Val::Int(1),
                Val::Int(f) => self.v.mk_set(Reason::Cache, what, vec![f, 1]),
                Val::Unk(_) => self.v.mk(Reason::Cache, what),
            };
            self.write(p, Fact::Var(VAR_ROOM_FLAG), v);
        }
        self.write(p, Fact::Var(VAR_CAMERA_MIN_X), Val::Int(SCREEN_WIDTH / 2));
        let width = res.int().and_then(|r| g.room_width.get(&r).copied());
        let max = match width {
            Some(w) => Val::Int(w - SCREEN_WIDTH / 2),
            None => self.v.mk(Reason::Work, format!("width of room {}", self.show(res))),
        };
        self.write(p, Fact::Var(VAR_CAMERA_MAX_X), max);
        if res == Val::Int(0) {
            let c = p.call_at(ci);
            c.i = c.steps.len() as u32;
        }
    }

    /// room.cpp:223-227 (model.py `_eng_scene_ego`): the ego's object must be in the new room (the engine's
    /// error() otherwise: the path ends).
    pub(super) fn eng_scene_ego(&mut self, p: &mut Path, ci: usize, o: &FactOracle, obj: Val) {
        let w = match obj {
            Val::Unk(_) => Where::Unk(obj),
            Val::Int(ob) => self.where_(p, o, ob),
        };
        let (head, cite) = Self::head_cite(p, ci);
        match w {
            Where::Unk(u) => {
                let s = format!("whether object {} is in the new room: {}", self.show(obj), self.show(u));
                self.note(p, cite, &head, &s);
            }
            Where::Room => {}
            Where::Inventory | Where::Nowhere => {
                let d = format!("startScene: Object {} is not in room (room.cpp:225-227, an error())", self.show(obj));
                self.error(p, "ego object not in room", cite, &d, true);
            }
        }
    }
}
