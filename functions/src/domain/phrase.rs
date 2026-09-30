use serde_json::Value;

/// The "translation" this pattern performs: reverse the phrase, one Unicode scalar value at a time.
pub fn reverse(phrase: &str) -> String {
    phrase.chars().rev().collect()
}

/// Workflow step: returns the incoming state with `translatedPhrase` added. A missing or
/// non-string `phrase` translates to the empty string.
pub fn translate(mut state: Value) -> Value {
    let translated = reverse(state.get("phrase").and_then(Value::as_str).unwrap_or_default());
    if let Value::Object(fields) = &mut state {
        fields.insert("translatedPhrase".to_owned(), Value::String(translated));
        state
    } else {
        serde_json::json!({ "translatedPhrase": translated })
    }
}
