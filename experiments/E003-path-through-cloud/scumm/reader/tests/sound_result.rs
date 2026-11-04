//! E014 amendment (8)'s switch FMI_READ_SOUND: VAR_SOUNDRESULT, which the engine's sound queue writes outside the
//! scripts, is an engine variable (no fact) with the switch, and as before without it.

use scumm_reader::domains::{engine_why, engine_why_with, VAR_SOUNDRESULT};

#[test]
fn var_soundresult_is_the_engines_with_the_switch() {
    assert_eq!(scumm_ir::vars::var_of("VAR_SOUNDRESULT"), Some(VAR_SOUNDRESULT));
    assert!(engine_why_with(VAR_SOUNDRESULT, true).is_some_and(|w| w.starts_with("VAR_SOUNDRESULT")));
    assert_eq!(engine_why_with(VAR_SOUNDRESULT, false), None);
    // the other engine variables are the same either way
    for n in [2, 3, 46, 71] {
        assert!(engine_why_with(n, false).is_some());
        assert_eq!(engine_why_with(n, true), engine_why_with(n, false));
    }
    // a script's variable is no engine variable either way
    assert_eq!(engine_why_with(300, true), None);
    // this test's process has the switch unset
    if std::env::var("FMI_READ_SOUND").is_err() {
        assert_eq!(engine_why(VAR_SOUNDRESULT), None);
    }
}
