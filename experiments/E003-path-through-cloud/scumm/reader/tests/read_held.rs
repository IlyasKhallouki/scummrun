//! E019's switch FMI_READ_HELD=1: a scope reads the code of the objects actors hold, whatever room defines them, and
//! its sentences take them.

use scumm_ir::UnitKey;
use scumm_reader::oracle;
use scumm_reader::scope::OF_OWNER_ROOM;
use scumm_reader::Scope;

#[test]
fn a_held_object_from_another_room_is_read_only_with_the_switch() {
    let g = oracle::load_game();
    let root = scumm_ir::default_root();
    let mut start = scumm_reader::scaling::start_facts(&root, 1);
    let mut objs: Vec<(i32, i32)> = g.object_room.iter().map(|(&o, &r)| (o, r)).collect();
    objs.sort_unstable();
    let &(o, home) = objs.iter().find(|&&(o, _)| g.id(UnitKey::Obj(o)).is_some()).unwrap();
    let room = objs.iter().map(|&(_, r)| r).find(|&r| r != home).unwrap();
    let (other, _) = *objs.iter().find(|&&(p, r)| p != o && r == home && g.id(UnitKey::Obj(p)).is_some()).unwrap();
    start.insert(format!("owner:{o}"), 1);
    start.insert(format!("owner:{other}"), OF_OWNER_ROOM);

    std::env::remove_var("FMI_READ_HELD");
    let off = Scope::of(&g, &[room], &start);
    assert!(!off.has_unit(UnitKey::Obj(o)), "without the switch the scope reads only its rooms' objects");
    assert!(off.held.is_empty());

    std::env::set_var("FMI_READ_HELD", "1");
    let on = Scope::of(&g, &[room], &start);
    std::env::remove_var("FMI_READ_HELD");
    assert!(on.has_unit(UnitKey::Obj(o)), "object {o} (room {home}) held by actor 1 is read in room {room}");
    assert!(!on.has_unit(UnitKey::Obj(other)), "object {other} left in room {home} is not");
    // the held object is a sentence object of the scope, not one of its rooms' objects (the hit test's)
    assert!(on.held.contains(&(o as i64)) && !on.held.contains(&(other as i64)));
    assert!(!on.objects.contains(&(o as i64)));
}
