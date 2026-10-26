//! Fix round 1, F1 (switch input_script): where a click in a room lands.
//!
//! **The room's object list.** resetRoomObjects (object.cpp:832-901) loads a room's objects into `_objs` from slot 1
//! on, in the order of the OBCD blocks in its ROOM block (ResourceIterator over `room`, 856-872;
//! findLocalObjectSlot gives the first free slot). For v5 each object's CDHD gives its box (x, y, w, h in units of
//! 8 pixels), its parent (a slot of `_objs`, 0 for none) and its parent state (the flags' low four bits, 1 when the
//! flags are 0x80; 1143-1155).
//!
//! **findObject** (object.cpp:557-597) walks `_objs` from slot 1 and returns the first object that is not of class
//! Untouchable, whose chain of parents is each in its parent state, and whose box holds the point
//! (x_pos <= x < x_pos + width, the same for y). So the objects over a point are tried in list order.
//!
//! **Cells.** A cell is a distinct non-empty set of boxes over a point of the room. Boxes are on the 8-pixel grid, so
//! a cell is a set of grid squares; `Cell` keeps the boxes in list order, the bounding rectangle of its squares and a
//! point of it. With F5 (draw_clears) an object that a drawObject with setXY can move has a box at each of its places
//! (`places::movable`); the cells of that arrangement hold (object, place) pairs, and findObject at a cell asks which
//! place the object is at.
//!
//! The list order is read from the game's resource file (`index_file` in index.json, its data file `.001`), which
//! v5 encrypts with 0x69 (resource.cpp:196-206, getEncByte). Nothing here names a game's room or object.

use crate::places::ObjBox;
use rustc_hash::FxHashMap;
use serde_json::Value;
use std::io;
use std::path::Path;

/// v5's resource encryption byte (resource.cpp:204).
pub const ENC_BYTE: u8 = 0x69;
/// The grid boxes are on (CDHD's x, y, w and h are in units of 8 pixels, object.cpp:1146-1149).
pub const GRID: i32 = 8;

/// An object of a room's `_objs` list.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ListObj {
    pub obj: i32,
    /// the parent's slot in the list (1-based), 0 for none (CDHD v5 parent)
    pub parent: u8,
    /// the state the parent must have (object.cpp:1150-1154)
    pub parent_state: u8,
}

/// A cell of a room's box arrangement.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cell {
    /// the boxes over it: (object, place) in list order; place 0 is the CDHD place, n the object's n-th place in
    /// `places::movable`
    pub boxes: Vec<(i32, u8)>,
    /// the bounding rectangle of its grid squares, inclusive, in room pixels
    pub x: (i32, i32),
    pub y: (i32, i32),
    /// a point of the cell (the top-left pixel of its first grid square, x then y)
    pub at: (i32, i32),
    /// its grid squares
    pub squares: u32,
}

fn be32(b: &[u8], i: usize) -> usize {
    u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize
}

/// The blocks between `start` and `end`: (tag, offset, size).
fn blocks(b: &[u8], start: usize, end: usize) -> Vec<([u8; 4], usize, usize)> {
    let mut out = Vec::new();
    let mut i = start;
    while i + 8 <= end {
        let tag = [b[i], b[i + 1], b[i + 2], b[i + 3]];
        let size = be32(b, i + 4);
        if size < 8 || i + size > end {
            break;
        }
        out.push((tag, i, size));
        i += size;
    }
    out
}

/// Each room resource's `_objs` list, read from the game's resource file; the room is the one index.json gives the
/// list's first object.
pub fn read_room_lists(root: &Path, meta: &Value, object_room: &FxHashMap<i32, i32>) -> io::Result<FxHashMap<i32, Vec<ListObj>>> {
    let index = meta["game"]["index_file"].as_str().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no game.index_file"))?;
    let data_file = root.join(index.trim_end_matches(".000").to_string() + ".001");
    let raw = std::fs::read(&data_file)?;
    let b: Vec<u8> = raw.iter().map(|x| x ^ ENC_BYTE).collect();
    let mut out = FxHashMap::default();
    for (tag, i, size) in blocks(&b, 0, b.len()) {
        if &tag != b"LECF" {
            continue;
        }
        for (t2, j, s2) in blocks(&b, i + 8, i + size) {
            if &t2 != b"LFLF" {
                continue;
            }
            for (t3, k, s3) in blocks(&b, j + 8, j + s2) {
                if &t3 != b"ROOM" {
                    continue;
                }
                let mut list = Vec::new();
                for (t4, m, s4) in blocks(&b, k + 8, k + s3) {
                    if &t4 != b"OBCD" {
                        continue;
                    }
                    if s4 < 24 || &b[m + 8..m + 12] != b"CDHD" {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("an OBCD at {m} with no CDHD first")));
                    }
                    let obj = u16::from_le_bytes([b[m + 16], b[m + 17]]) as i32;
                    let flags = b[m + 22];
                    let parent = b[m + 23];
                    let parent_state = if flags == 0x80 { 1 } else { flags & 0xF };
                    list.push(ListObj { obj, parent, parent_state });
                }
                if let Some(r) = list.first().and_then(|o| object_room.get(&o.obj)) {
                    out.insert(*r, list);
                }
            }
        }
    }
    Ok(out)
}

/// The cells of one room `w` x `h` pixels: its list's boxes (each object's places, `places` gives them; an object
/// with no box is left out), on the 8-pixel grid. Sorted by their boxes.
pub fn room_cells(
    list: &[ListObj],
    boxes: &FxHashMap<i32, ObjBox>,
    places: &dyn Fn(i32) -> Vec<(i32, i32)>,
    w: i32,
    h: i32,
) -> Vec<Cell> {
    // (object, place, rectangle) in list order
    let mut rects: Vec<(i32, u8, ObjBox)> = Vec::new();
    for o in list {
        let Some(b) = boxes.get(&o.obj) else { continue };
        if b.w <= 0 || b.h <= 0 {
            continue;
        }
        for (pi, (px, py)) in places(o.obj).into_iter().enumerate() {
            rects.push((o.obj, pi as u8, ObjBox { x: px, y: py, w: b.w, h: b.h }));
        }
    }
    let mut cells: FxHashMap<Vec<(i32, u8)>, Cell> = FxHashMap::default();
    let mut gx = 0;
    while gx < w {
        let mut gy = 0;
        while gy < h {
            let set: Vec<(i32, u8)> = rects
                .iter()
                .filter(|(_, _, r)| r.x <= gx && gx < r.x + r.w && r.y <= gy && gy < r.y + r.h)
                .map(|&(o, p, _)| (o, p))
                .collect();
            if !set.is_empty() {
                let c = cells.entry(set.clone()).or_insert(Cell {
                    boxes: set,
                    x: (gx, gx + GRID - 1),
                    y: (gy, gy + GRID - 1),
                    at: (gx, gy),
                    squares: 0,
                });
                c.x = (c.x.0.min(gx), c.x.1.max(gx + GRID - 1));
                c.y = (c.y.0.min(gy), c.y.1.max(gy + GRID - 1));
                c.squares += 1;
            }
            gy += GRID;
        }
        gx += GRID;
    }
    let mut out: Vec<Cell> = cells.into_values().collect();
    out.sort_by(|a, b| a.boxes.cmp(&b.boxes));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two overlapping boxes make three cells; list order is kept inside a cell; a box with no size is left out.
    #[test]
    fn cells_of_two_boxes() {
        let list = vec![
            ListObj { obj: 7, parent: 0, parent_state: 0 },
            ListObj { obj: 5, parent: 0, parent_state: 0 },
            ListObj { obj: 9, parent: 0, parent_state: 0 },
        ];
        let mut boxes = FxHashMap::default();
        boxes.insert(7, ObjBox { x: 0, y: 0, w: 16, h: 8 });
        boxes.insert(5, ObjBox { x: 8, y: 0, w: 16, h: 8 });
        boxes.insert(9, ObjBox { x: 0, y: 0, w: 0, h: 8 });
        let home = |o: i32| vec![(boxes[&o].x, boxes[&o].y)];
        let cells = room_cells(&list, &boxes, &home, 64, 16);
        let sets: Vec<Vec<(i32, u8)>> = cells.iter().map(|c| c.boxes.clone()).collect();
        assert_eq!(sets, vec![vec![(5, 0)], vec![(7, 0)], vec![(7, 0), (5, 0)]]);
        let both = &cells[2];
        assert_eq!((both.x, both.y, both.at, both.squares), ((8, 15), (0, 7), (8, 0), 1));
        // a second place for 5 adds its box there
        let two = |o: i32| if o == 5 { vec![(8, 0), (40, 8)] } else { vec![(boxes[&o].x, boxes[&o].y)] };
        let cells = room_cells(&list, &boxes, &two, 64, 16);
        assert!(cells.iter().any(|c| c.boxes == vec![(5, 1)] && c.x == (40, 55) && c.y == (8, 15)));
    }
}
