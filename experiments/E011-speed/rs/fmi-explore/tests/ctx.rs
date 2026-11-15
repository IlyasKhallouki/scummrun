//! E022 (FMI_PLACE_CTX=1): a place's context, the set of its room's input keys a state holds nonzero, less the keys
//! seen cleared from a parent's state to its child's; the place's text carries it (`place_text`).

use fmi_explore::archive::FactMap;
use fmi_explore::{ctx, progress};

fn facts(kv: &[(&str, i64)]) -> FactMap {
    kv.iter().map(|&(k, v)| (k.to_string(), v)).collect()
}

#[test]
fn a_room_input_key_opens_a_context_until_it_is_seen_cleared() {
    let g = fmi_explore::guide::load_game(&fmi_explore::guide::default_root()).unwrap();
    let keys = ctx::static_keys_of(&g);
    // some room's code tests a var or bit that only other code writes
    let (&room, ks) = keys.iter().find(|(_, ks)| !ks.is_empty()).expect("a room with input keys");
    let k = ks[0].clone();
    let base = facts(&[("room", room)]);
    let moved = facts(&[("room", room), (k.as_str(), 5)]);
    std::env::remove_var("FMI_PLACE_CTX");
    assert_eq!(progress::place_text(room, &moved), progress::place_text(room, &base));
    std::env::set_var("FMI_PLACE_CTX", "1");
    // the process's own table: the test's game is the process's (scumm-ir's default root)
    if ctx::report(room).0.contains(&k) {
        assert!(ctx::context_of(room, &base).is_empty());
        assert_eq!(ctx::context_of(room, &moved), vec![k.clone()]);
        assert_ne!(progress::place_text(room, &moved), progress::place_text(room, &base));
        // a parent holding it and a child not: the key comes and goes, so it opens no context any more
        ctx::note_edge(&moved, &base);
        assert!(ctx::context_of(room, &moved).is_empty());
        assert_eq!(progress::place_text(room, &moved), progress::place_text(room, &base));
    }
    std::env::remove_var("FMI_PLACE_CTX");
}
