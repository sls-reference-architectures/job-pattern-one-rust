use std::collections::HashMap;

use probe::{NONCE, carries_nonce};

fn environment(nonce: Option<&str>) -> HashMap<String, String> {
    let mut variables = HashMap::from([("TABLE_NAME".to_owned(), "jobs".to_owned())]);
    if let Some(nonce) = nonce {
        variables.insert(NONCE.to_owned(), nonce.to_owned());
    }
    variables
}

#[test]
fn a_configuration_still_showing_the_previous_nonce_is_not_yet_updated() {
    // ARRANGE
    let stale = environment(Some("4242-2"));

    // ACT
    let updated = carries_nonce(Some(&stale), Some("4242-3"));

    // ASSERT
    assert!(!updated);
}

#[test]
fn a_configuration_showing_the_new_nonce_is_updated() {
    // ARRANGE
    let current = environment(Some("4242-3"));

    // ACT
    let updated = carries_nonce(Some(&current), Some("4242-3"));

    // ASSERT
    assert!(updated);
}

#[test]
fn removing_the_nonce_is_applied_once_it_is_absent() {
    // ARRANGE
    let stale = environment(Some("4242-9"));
    let current = environment(None);

    // ACT
    let while_stale = carries_nonce(Some(&stale), None);
    let once_applied = carries_nonce(Some(&current), None);
    let without_environment = carries_nonce(None, None);

    // ASSERT
    assert!(!while_stale);
    assert!(once_applied);
    assert!(without_environment);
}
