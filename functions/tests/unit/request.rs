use jobs::domain::job::NewJob;
use jobs::domain::request::{RequestError, is_json_content_type, parse_new_job};

const JSON: Option<&str> = Some("application/json");

#[test]
fn a_json_body_with_name_and_phrase_is_accepted() {
    // ARRANGE
    let body = r#"{"name":"greeting","phrase":"one two three"}"#;

    // ACT
    let parsed = parse_new_job(JSON, Some(body));

    // ASSERT
    assert_eq!(
        parsed,
        Ok(NewJob {
            name: "greeting".to_owned(),
            phrase: "one two three".to_owned()
        })
    );
}

#[test]
fn extra_fields_in_the_body_are_ignored() {
    // ARRANGE
    let body = r#"{"name":"n","phrase":"p","status":"Complete","id":"JOB_forged"}"#;

    // ACT
    let parsed = parse_new_job(JSON, Some(body));

    // ASSERT
    assert_eq!(
        parsed,
        Ok(NewJob {
            name: "n".to_owned(),
            phrase: "p".to_owned()
        })
    );
}

#[test]
fn a_non_json_content_type_is_unsupported() {
    // ARRANGE
    let body = r#"{"name":"n","phrase":"p"}"#;

    // ACT
    let parsed = parse_new_job(Some("text/plain"), Some(body));

    // ASSERT
    assert_eq!(parsed, Err(RequestError::unsupported_media_type()));
    assert_eq!(RequestError::unsupported_media_type().status, 415);
}

#[test]
fn a_missing_content_type_is_unsupported() {
    // ARRANGE
    let body = r#"{"name":"n","phrase":"p"}"#;

    // ACT
    let parsed = parse_new_job(None, Some(body));

    // ASSERT
    assert_eq!(parsed.unwrap_err().status, 415);
}

#[test]
fn json_content_types_with_parameters_and_suffixes_are_accepted() {
    // ARRANGE
    let accepted = [
        "application/json",
        "application/json; charset=utf-8",
        "Application/JSON",
        "application/vnd.api+json",
        "application/problem+json;charset=utf-8",
    ];

    // ACT
    let results: Vec<bool> = accepted.iter().map(|value| is_json_content_type(value)).collect();

    // ASSERT
    assert!(results.iter().all(|accepted| *accepted), "{results:?}");
}

#[test]
fn look_alike_content_types_are_rejected() {
    // ARRANGE
    let rejected = [
        "text/json",
        "application/jsonp",
        "application/+json",
        "application/xml",
        "",
    ];

    // ACT
    let results: Vec<bool> = rejected.iter().map(|value| is_json_content_type(value)).collect();

    // ASSERT
    assert!(results.iter().all(|accepted| !*accepted), "{results:?}");
}

#[test]
fn malformed_json_is_unprocessable() {
    // ARRANGE
    let body = r#"{"name":"n","#;

    // ACT
    let parsed = parse_new_job(JSON, Some(body));

    // ASSERT
    assert_eq!(parsed, Err(RequestError::malformed_json()));
    assert_eq!(RequestError::malformed_json().status, 422);
    assert_eq!(
        RequestError::malformed_json().message,
        "Invalid or malformed JSON was provided"
    );
}

#[test]
fn a_missing_or_empty_body_is_unprocessable() {
    // ARRANGE
    let bodies = [None, Some("")];

    // ACT
    let results: Vec<_> = bodies.iter().map(|body| parse_new_job(JSON, *body)).collect();

    // ASSERT
    assert!(
        results
            .iter()
            .all(|result| result == &Err(RequestError::malformed_json()))
    );
}

#[test]
fn a_missing_name_or_phrase_is_a_bad_request() {
    // ARRANGE
    let bodies = [r#"{"phrase":"p"}"#, r#"{"name":"n"}"#, r#"{}"#];

    // ACT
    let results: Vec<_> = bodies
        .iter()
        .map(|body| parse_new_job(JSON, Some(body)))
        .collect();

    // ASSERT
    assert!(
        results
            .iter()
            .all(|result| result == &Err(RequestError::missing_fields()))
    );
    assert_eq!(RequestError::missing_fields().status, 400);
}

#[test]
fn a_non_string_name_or_phrase_is_a_bad_request() {
    // ARRANGE
    let bodies = [
        r#"{"name":1,"phrase":"p"}"#,
        r#"{"name":"n","phrase":null}"#,
        "null",
        "[]",
        "42",
    ];

    // ACT
    let results: Vec<_> = bodies
        .iter()
        .map(|body| parse_new_job(JSON, Some(body)))
        .collect();

    // ASSERT
    assert!(
        results
            .iter()
            .all(|result| result == &Err(RequestError::missing_fields()))
    );
}

#[test]
fn an_unknown_job_is_not_found_with_its_id_in_the_message() {
    // ARRANGE
    let id = "JOB_missing";

    // ACT
    let failure = RequestError::job_not_found(id);

    // ASSERT
    assert_eq!(failure.status, 404);
    assert_eq!(failure.message, "No id found for JOB_missing");
}
