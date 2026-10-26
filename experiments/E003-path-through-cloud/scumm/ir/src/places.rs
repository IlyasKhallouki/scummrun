//! Fix round 1, F5 (switch draw_clears): where each object is drawn.
//!
//! o5_drawObject (script_v5.cpp:1031-1129, v5): with a setXY sub-op whose x is not 255 the object's place moves to
//! (x * 8, y * 8), stored as int16 (1108-1114); then every object of the room whose place, width and height equal the
//! drawn object's goes to state 0 (1118-1126, the loop over _objs from _numLocalObjects - 1 down to 1), and the drawn
//! object gets its state (1128). The places and sizes come from each object's CDHD (object.cpp:1145-1150, v5: x, y,
//! w and h are bytes, times 8), loaded again with the room (room.cpp:195 resetRoomObjects), so a move lasts for one
//! room visit. The test compares the int16 place with the drawn one's as uint16 (1116-1117, 1124): a negative x or y
//! matches nothing.
//!
//! Nothing here names a game's object, room or script.

use crate::ast::{Stmt, UnitKey};
use crate::read::{by_int_key, Script};
use crate::sym::known;
use rustc_hash::FxHashMap;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

/// An object's box from its CDHD, in pixels.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ObjBox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// An int stored as the engine's int16.
#[inline]
pub fn i16w(v: i32) -> i32 {
    ((v.wrapping_add(0x8000)) & 0xFFFF) - 0x8000
}

/// The place setXY(x, y) gives (script_v5.cpp:1108-1114): None for x 255, which the engine reads as no setXY.
pub fn set_xy_place(x: i32, y: i32) -> Option<(i32, i32)> {
    (x != 0xFF).then(|| (i16w(x.wrapping_mul(8)), i16w(y.wrapping_mul(8))))
}

/// Whether two places are the same to the engine's test (script_v5.cpp:1116-1124): the int16 place of the other
/// object against the drawn one's as uint16, so a negative coordinate matches nothing.
#[inline]
pub fn same_place(a: (i32, i32), b: (i32, i32)) -> bool {
    a == b && a.0 >= 0 && a.1 >= 0
}

/// Every object's CDHD box, read from its OBCD block (index.json's "block"; the CDHD is its first sub-block).
pub fn read_boxes(root: &Path, meta: &Value) -> io::Result<FxHashMap<i32, ObjBox>> {
    let mut out = FxHashMap::default();
    for (o, ob) in by_int_key(&meta["objects"]) {
        let Some(block) = ob.get("block").and_then(|b| b.as_str()) else { continue };
        let path = root.join(block);
        if !path.is_file() {
            continue;
        }
        let b = std::fs::read(&path)?;
        // "OBCD" + size, then "CDHD" + size + obj_id (u16), x, y, w, h (bytes)
        if b.len() < 22 || &b[0..4] != b"OBCD" || &b[8..12] != b"CDHD" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{}: no CDHD after OBCD", path.display())));
        }
        let id = u16::from_le_bytes([b[16], b[17]]) as i32;
        if id != o {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{}: CDHD names object {id}", path.display())));
        }
        out.insert(o, ObjBox { x: b[18] as i32 * 8, y: b[19] as i32 * 8, w: b[20] as i32 * 8, h: b[21] as i32 * 8 });
    }
    Ok(out)
}

/// The objects of each room resource that have a box, sorted.
pub fn room_objects(object_room: &FxHashMap<i32, i32>, boxes: &FxHashMap<i32, ObjBox>) -> FxHashMap<i32, Vec<i32>> {
    let mut out: FxHashMap<i32, Vec<i32>> = FxHashMap::default();
    for (&o, &r) in object_room {
        if boxes.contains_key(&o) {
            out.entry(r).or_default().push(o);
        }
    }
    for v in out.values_mut() {
        v.sort_unstable();
    }
    out
}

/// The objects a drawObject with setXY can move, with their places: the CDHD place first, then every constant
/// target in sorted order. A site whose object is not a constant can move any object of its unit's room (a global
/// script's: any object). A site whose target is not a constant adds no place: the machine writes such a move as an
/// unknown. An object no setXY can move is left out (its place is its CDHD place).
pub fn movable(
    sc: &Script,
    object_room: &FxHashMap<i32, i32>,
    boxes: &FxHashMap<i32, ObjBox>,
    rooms: &FxHashMap<i32, Vec<i32>>,
) -> FxHashMap<i32, Vec<(i32, i32)>> {
    let ast = &sc.ast;
    let mut targets: BTreeMap<i32, BTreeSet<(i32, i32)>> = BTreeMap::new();
    for u in sc.ids() {
        let key = sc.unit(u).key;
        for x in sc.ins(u) {
            let Some(Stmt::Op { name: known::DRAW_OBJECT, args, subops, .. }) = x.st else { continue };
            let Some(s) = ast.subs(subops).iter().find(|s| s.name == known::SET_XY) else { continue };
            let objs: Vec<i32> = match ast.constant(ast.kids(args)[0]) {
                Some(o) => vec![o],
                None => {
                    let room = match key {
                        UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => Some(r),
                        UnitKey::Obj(o) => object_room.get(&o).copied(),
                        UnitKey::Global(_) => None,
                    };
                    match room {
                        Some(r) => rooms.get(&r).cloned().unwrap_or_default(),
                        None => boxes.keys().copied().collect(),
                    }
                }
            };
            let xy: Vec<Option<i32>> = ast.kids(s.args).iter().map(|&a| ast.constant(a)).collect();
            let place = match xy.as_slice() {
                [Some(px), Some(py)] => set_xy_place(*px, *py),
                _ => None,
            };
            if matches!(xy.as_slice(), [Some(0xFF), _]) {
                continue; // x 255: no move (script_v5.cpp:1108)
            }
            for o in objs {
                if !boxes.contains_key(&o) {
                    continue;
                }
                let t = targets.entry(o).or_default();
                if let Some(pl) = place {
                    t.insert(pl);
                }
            }
        }
    }
    let mut out = FxHashMap::default();
    for (o, t) in targets {
        let b = boxes[&o];
        let home = (b.x, b.y);
        let mut places = vec![home];
        places.extend(t.into_iter().filter(|&p| p != home));
        out.insert(o, places);
    }
    out
}
