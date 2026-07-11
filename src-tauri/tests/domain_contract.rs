use mistake_trainer_next_lib::domain::profile::ProfileName;
use mistake_trainer_next_lib::domain::review::{
    FsrsRating, ReviewEvent, SimpleRating, ordered_events,
};
use uuid::Uuid;

#[test]
fn profile_name_is_trimmed_and_safe_for_display() {
    let name = ProfileName::parse("  小树  ").expect("valid profile name");
    assert_eq!(name.as_str(), "小树");
}

#[test]
fn profile_name_rejects_path_like_and_empty_values() {
    for invalid in ["", "   ", "..", "../other", r"..\other", "math/science"] {
        assert!(ProfileName::parse(invalid).is_err(), "accepted {invalid:?}");
    }
}

#[test]
fn simple_buttons_map_to_fsrs_again_and_good() {
    assert_eq!(SimpleRating::Forgot.into_fsrs(), FsrsRating::Again);
    assert_eq!(SimpleRating::Remembered.into_fsrs(), FsrsRating::Good);
}

#[test]
fn review_events_have_stable_order_across_devices() {
    let problem_id = Uuid::now_v7();
    let device_id = Uuid::now_v7();
    let later = ReviewEvent::new(
        Uuid::from_u128(2),
        problem_id,
        device_id,
        FsrsRating::Good,
        20,
        1_800,
    );
    let first_tie = ReviewEvent::new(
        Uuid::from_u128(1),
        problem_id,
        device_id,
        FsrsRating::Again,
        10,
        3_100,
    );
    let second_tie = ReviewEvent::new(
        Uuid::from_u128(3),
        problem_id,
        device_id,
        FsrsRating::Good,
        10,
        2_000,
    );

    let ordered = ordered_events(vec![later, second_tie, first_tie]);
    let ids: Vec<_> = ordered.into_iter().map(|event| event.id).collect();

    assert_eq!(
        ids,
        vec![Uuid::from_u128(1), Uuid::from_u128(3), Uuid::from_u128(2)]
    );
}
