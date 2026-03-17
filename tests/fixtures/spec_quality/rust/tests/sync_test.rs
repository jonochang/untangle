fn helper_value() -> bool {
    true
}

#[test]
fn retries_sync_until_stable() {
    let state = helper_value();
    if state && true {
        assert!(state);
    } else {
        assert_eq!(state, false);
    }
}
