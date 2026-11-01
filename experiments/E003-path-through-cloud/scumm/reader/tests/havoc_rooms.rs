//! The switch havoc_rooms (experiments/E005-narrow-havoc/havoc-rooms.md): a room change in a havoc closure names the
//! exit code of the rooms it can leave and the entry code of the rooms it can load, not every scope room's. On scopes
//! read as Part I's is.

use scumm_ir::{known, Stmt, UnitKey};
use scumm_reader::builder::Switches;
use scumm_reader::havoc::{self, At};
use scumm_reader::oracle;
use scumm_reader::record::{Cut, CutStep, RoomStep};
use scumm_reader::Builder;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

/// A scope of `rooms` read as Part I's is (run D's switches, havoc_cut and havoc_starts).
fn builder_for(rooms: &[i32]) -> Builder<'static> {
    let g = game();
    let root = scumm_ir::default_root();
    let start = scumm_reader::scaling::start_facts(&root, 1);
    let goal = scumm_reader::scaling::part1_goal(&root);
    let sw = Switches::parse("value_sets,draw_clears,actor_rooms,split_limit,grow_limit,havoc_cut,havoc_starts").unwrap();
    let doms = scumm_reader::domains::build_with(g, &scumm_reader::domains::meta_rooms(&root), &start, &goal, sw.scope());
    let scope = scumm_reader::Scope::of_with(g, rooms, &start, sw.scope());
    let mut b = Builder::new(g, scope, doms);
    b.switches = sw;
    b.table.havoc_cut = true;
    b.table.havoc_starts = true;
    b
}

fn room_code(c: &havoc::Closure) -> (BTreeSet<i32>, BTreeSet<i32>) {
    let g = game();
    let mut exits = BTreeSet::new();
    let mut entries = BTreeSet::new();
    for &u in c.starts.keys() {
        match g.unit(u).key {
            UnitKey::Exit(r) => {
                exits.insert(r);
            }
            UnitKey::Entry(r) => {
                entries.insert(r);
            }
            _ => {}
        }
    }
    (exits, entries)
}

fn at(room: i32, actors: &[(i32, i32)]) -> At {
    At {
        res: Some(room),
        rooms: Some([room].into_iter().collect()),
        actors: actors.iter().map(|&(a, r)| (a, [r].into_iter().collect())).collect::<BTreeMap<i32, BTreeSet<i32>>>(),
    }
}

/// Every room code unit of a scope whose own code changes the room to constant rooms only, cut at its start, standing
/// in its room: on, the closure's exit code is that of R* and its entry code that of the rooms some change loads;
/// off, every scope room's.
#[test]
fn a_room_change_names_the_rooms_it_leaves_and_loads() {
    let g = game();
    let mut b = builder_for(&[33, 28, 35, 34]);
    let all: BTreeSet<i32> = b.scope.rooms.iter().copied().collect();
    let mut narrower = 0;
    for &u in &b.scope.units.clone() {
        let k = g.unit(u).key;
        let UnitKey::Local(r, _) = k else { continue };
        let cut = Cut { frames: vec![(k, Some(0))], steps: vec![] };
        b.table.havoc_rooms = false;
        let off = havoc::closure_at(g, &b.scope, &b.table, &cut, &at(r, &[(1, r)]));
        if !off.room_change || off.every_unit {
            continue;
        }
        let (ex, en) = room_code(&off);
        assert_eq!(ex, all, "{k:?} off: every scope room's exit code");
        assert_eq!(en, all, "{k:?} off: every scope room's entry code");
        b.table.havoc_rooms = true;
        let on = havoc::closure_at(g, &b.scope, &b.table, &cut, &at(r, &[(1, r)]));
        let (ex, en) = room_code(&on);
        match (&on.rooms, &on.loaded) {
            (Some(rs), Some(loaded)) => {
                assert!(rs.contains(&r), "{k:?}: the room it stands in is left");
                assert_eq!(&ex, rs, "{k:?}: the exit code of R*");
                assert_eq!(&en, loaded, "{k:?}: the entry code of the loaded rooms");
                assert!(loaded.is_subset(rs));
                if rs.len() < all.len() {
                    narrower += 1;
                }
            }
            _ => {
                assert!(!on.room_why.is_empty(), "{k:?}: every scope room only with a reason");
                assert_eq!(ex, all);
            }
        }
    }
    eprintln!("{narrower} closures name fewer rooms than the scope's");
}

/// Global 114 (MI1): loadRoom(49), startScript(73, [..., 200]), putActorInRoom(VAR_EGO, 85), actorFollowCamera(VAR_EGO).
/// In a scope of rooms 49, 85 and 33, standing in room 85 with the ego there, the change can load rooms 49 and 85 and
/// what global 73's own code loads; room 33's code is not named unless a change in the closure loads it.
#[test]
fn global_114_names_the_rooms_it_loads_and_the_egos() {
    let g = game();
    let mut b = builder_for(&[49, 85, 33]);
    b.table.havoc_rooms = true;
    // global 73's start by a local would name every unit without havoc_args
    b.table.havoc_args = Some(std::sync::Arc::new(scumm_reader::args::HavocArgs::of(g, &b.scope, &b.table)));
    let cut = Cut { frames: vec![(UnitKey::Global(114), Some(0))], steps: vec![] };
    let c = havoc::closure_at(g, &b.scope, &b.table, &cut, &at(85, &[(1, 85)]));
    assert!(c.room_change);
    let rs = c.rooms.clone().unwrap_or_else(|| panic!("rooms not known: {:?}", c.room_why));
    assert!(rs.contains(&49) && rs.contains(&85), "{rs:?}");
    // with the ego's room at the cut not known, actorFollowCamera(VAR_EGO) names every scope room
    let c2 = havoc::closure_at(g, &b.scope, &b.table, &cut, &at(85, &[]));
    assert!(c2.rooms.is_none(), "{:?}", c2.rooms);
    assert!(c2.room_why.iter().any(|w| w.contains("actorFollowCamera")), "{:?}", c2.room_why);
}

/// A pending startScene step holds its room; one whose room is not known names every scope room.
#[test]
fn a_pending_room_step_names_its_room() {
    let g = game();
    let mut b = builder_for(&[33, 28]);
    b.table.havoc_rooms = true;
    b.table.havoc_args = Some(std::sync::Arc::new(scumm_reader::args::HavocArgs::of(g, &b.scope, &b.table)));
    let cut = Cut { frames: vec![], steps: vec![CutStep::Room(RoomStep::Load(Some(28)))] };
    let c = havoc::closure_at(g, &b.scope, &b.table, &cut, &at(33, &[(1, 33)]));
    let (ex, en) = room_code(&c);
    assert!(en.contains(&28) && ex.contains(&33) && ex.contains(&28), "exits {ex:?}, entries {en:?}");
    assert_eq!(c.loaded.as_ref().map(|l| l.contains(&28)), Some(true), "{:?} {:?}", c.why, c.room_why);
    let cut = Cut { frames: vec![], steps: vec![CutStep::Room(RoomStep::Load(None))] };
    let c = havoc::closure_at(g, &b.scope, &b.table, &cut, &at(33, &[]));
    assert!(c.rooms.is_none() && !c.room_why.is_empty());
    b.table.havoc_rooms = false;
    let cut = Cut { frames: vec![], steps: vec![CutStep::Room(RoomStep::Load(Some(28)))] };
    let c = havoc::closure_at(g, &b.scope, &b.table, &cut, &at(33, &[]));
    assert!(c.rooms.is_none() && c.room_change, "off: every scope room");
}

/// Off, `writes` and `writes_at` from the same room give the same record contents.
#[test]
fn off_writes_at_is_writes() {
    let g = game();
    let b = builder_for(&[33, 28]);
    for &u in b.scope.units.iter().take(40) {
        let k = g.unit(u).key;
        let cut = Cut { frames: vec![(k, Some(0))], steps: vec![] };
        let res = g.resource(33);
        let w1 = havoc::writes(g, &b.scope, &b.table, &cut, res);
        let w2 = havoc::writes_at(g, &b.scope, &b.table, &cut, &At { res, ..Default::default() });
        assert_eq!(w1.facts, w2.facts, "{k:?}");
        assert_eq!(w1.slots, w2.slots, "{k:?}");
        assert_eq!(w1.event_units, w2.event_units, "{k:?}");
    }
    // the ops the rule reads exist in the game
    assert!(g.ins.iter().any(|x| matches!(x.st, Some(Stmt::Op { name, .. }) if name == known::ACTOR_FOLLOW_CAMERA)));
}
