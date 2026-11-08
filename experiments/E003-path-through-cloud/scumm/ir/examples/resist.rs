//! The F flag the IR derives for script slots (`Game::resistant`), and the constant start sites behind it.
//!
//! usage: resist ROOM:LOCAL... | g:GLOBAL... | all (FMI_GAME_SCRIPTS selects the game; all: every unit's flags)

use scumm_ir::{default_root, Game};
use scumm_ir::Fact;

fn main() {
    let g = Game::load(&default_root()).expect("the game");
    if std::env::args().nth(1).as_deref() == Some("all") {
        let mut rows: Vec<String> = g.resistance.iter().map(|(k, r)| format!("{k:?} {r:?}")).collect();
        rows.sort();
        rows.iter().for_each(|r| println!("{r}"));
        return;
    }
    for a in std::env::args().skip(1) {
        let (key, res) = match a.split_once(':') {
            Some(("g", n)) => (Fact::Script(n.parse().unwrap()), None),
            Some((r, n)) => (Fact::Local(n.parse().unwrap()), Some(r.parse().unwrap())),
            None => panic!("ROOM:LOCAL or g:GLOBAL"),
        };
        println!("{a}: resistant {:?}", g.resistant(key, res));
    }
}
