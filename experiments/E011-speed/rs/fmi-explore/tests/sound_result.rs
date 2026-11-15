//! E020 (FMI_SKIP_SOUNDRESULT=1): a checkpoint is compared without VAR_SOUNDRESULT (var 56), and every other fact is
//! still compared.

use fmi_core::{Obs, Status};
use fmi_explore::step::fact_map_of;
use fmi_explore::tryjob::compare_checkpoint;
use serde_json::json;

#[test]
fn the_sound_result_is_left_out_with_the_switch() {
    let mut want_obs = Obs::default();
    want_obs.vars = vec![0; 100];
    want_obs.vars[56] = -1;
    want_obs.vars[5] = 3;
    let want = json!([0, fact_map_of(&want_obs), {}]);
    let mut got = want_obs.clone();
    got.vars[56] = 0;
    std::env::remove_var("FMI_SKIP_SOUNDRESULT");
    assert_eq!(compare_checkpoint(&want, &got, &Status::default())["first"], json!([["var:56", [-1, 0]]]));
    std::env::set_var("FMI_SKIP_SOUNDRESULT", "1");
    assert!(compare_checkpoint(&want, &got, &Status::default()).is_empty());
    got.vars[5] = 4;
    assert_eq!(compare_checkpoint(&want, &got, &Status::default())["first"], json!([["var:5", [3, 4]]]));
    std::env::remove_var("FMI_SKIP_SOUNDRESULT");
}
