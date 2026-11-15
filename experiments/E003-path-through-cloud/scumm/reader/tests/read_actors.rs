//! E021's switch FMI_READ_ACTORS=1: the actors other than ego that the start reports with a class are sentence
//! objects of the scope (`Scope.actors`); with the switch off, none.

use scumm_reader::oracle;
use scumm_reader::{Facts, Scope};

#[test]
fn actors_with_a_class_are_sentence_objects_with_the_switch() {
    let g = oracle::load_game();
    let room = *g.object_room.values().min().unwrap();
    let mut start = Facts::new();
    start.insert("var:1".into(), 1);
    start.insert("class:1:13".into(), 1);
    start.insert("class:5:13".into(), 1);
    start.insert("class:7:2".into(), 0);
    std::env::remove_var("FMI_READ_ACTORS");
    assert!(Scope::of(&g, &[room], &start).actors.is_empty());
    std::env::set_var("FMI_READ_ACTORS", "1");
    let s = Scope::of(&g, &[room], &start);
    std::env::remove_var("FMI_READ_ACTORS");
    // ego (var:1) aside; a class key at 0 does not count
    assert_eq!(s.actors, vec![5]);
}
