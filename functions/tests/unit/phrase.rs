use jobs::domain::phrase::{reverse, translate};
use serde_json::json;

#[test]
fn a_phrase_is_reversed() {
    // ARRANGE
    let phrase = "one two three";

    // ACT
    let reversed = reverse(phrase);

    // ASSERT
    assert_eq!(reversed, "eerht owt eno");
}

#[test]
fn reversal_keeps_multi_byte_characters_intact() {
    // ARRANGE
    let phrase = "héllo 👋";

    // ACT
    let reversed = reverse(phrase);

    // ASSERT
    assert_eq!(reversed, "👋 olléh");
}

#[test]
fn translation_adds_the_reversed_phrase_and_keeps_everything_else() {
    // ARRANGE
    let state = json!({ "id": "JOB_X", "phrase": "hello", "revision": 2 });

    // ACT
    let translated = translate(state);

    // ASSERT
    assert_eq!(
        translated,
        json!({ "id": "JOB_X", "phrase": "hello", "revision": 2, "translatedPhrase": "olleh" })
    );
}

#[test]
fn translating_an_empty_or_missing_phrase_yields_an_empty_translation() {
    // ARRANGE
    let states = [
        json!({ "phrase": "" }),
        json!({ "id": "JOB_X" }),
        json!({ "phrase": 7 }),
    ];

    // ACT
    let translations: Vec<_> = states.into_iter().map(translate).collect();

    // ASSERT
    assert!(translations.iter().all(|state| state["translatedPhrase"] == ""));
}

#[test]
fn translating_a_non_object_state_yields_only_the_translation() {
    // ARRANGE
    let state = json!("not an object");

    // ACT
    let translated = translate(state);

    // ASSERT
    assert_eq!(translated, json!({ "translatedPhrase": "" }));
}
