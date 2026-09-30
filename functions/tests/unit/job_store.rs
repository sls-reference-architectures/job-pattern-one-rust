use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{TimeZone, Utc};
use jobs::adapters::job_store::{StoreError, condition_failure, update_request};
use jobs::domain::job::{Job, JobStatus, NewJob, StatusChange};

fn started_change() -> StatusChange {
    let job = Job::create(
        NewJob {
            name: "n".to_owned(),
            phrase: "p".to_owned(),
        },
        "U",
        Utc.timestamp_opt(1_790_000_000, 0).unwrap(),
    );
    job.change_status(JobStatus::Started, Utc.timestamp_opt(1_790_000_060, 0).unwrap())
}

fn assigned(request: &jobs::adapters::job_store::UpdateRequest) -> HashMap<String, AttributeValue> {
    request
        .names
        .iter()
        .filter(|(placeholder, _)| placeholder.starts_with("#a"))
        .map(|(placeholder, attribute)| {
            let value_placeholder = placeholder.replacen('#', ":", 1);
            (attribute.clone(), request.values[&value_placeholder].clone())
        })
        .collect()
}

#[test]
fn an_update_rewrites_every_mutable_attribute() {
    // ARRANGE
    let change = started_change();

    // ACT
    let request = update_request(&change).unwrap();

    // ASSERT
    let attributes = assigned(&request);
    let mut names: Vec<_> = attributes.keys().cloned().collect();
    names.sort();
    assert_eq!(
        names,
        ["name", "phrase", "revision", "status", "ttl", "updatedAt"]
    );
    assert_eq!(attributes["status"], AttributeValue::S("Started".to_owned()));
    assert_eq!(attributes["revision"], AttributeValue::N("2".to_owned()));
    assert_eq!(
        attributes["updatedAt"],
        AttributeValue::S("2026-09-21T14:14:20.000Z".to_owned())
    );
}

#[test]
fn an_update_never_rewrites_the_key_or_creation_time() {
    // ARRANGE
    let change = started_change();

    // ACT
    let request = update_request(&change).unwrap();

    // ASSERT
    let attributes = assigned(&request);
    assert!(!attributes.contains_key("id"));
    assert!(!attributes.contains_key("createdAt"));
}

#[test]
fn an_update_includes_workflow_results_when_present() {
    // ARRANGE
    let mut change = started_change();
    change.job.translated_phrase = Some("cba".to_owned());
    change.job.error = Some(serde_json::json!({ "Error": "Boom" }));

    // ACT
    let request = update_request(&change).unwrap();

    // ASSERT
    let attributes = assigned(&request);
    assert_eq!(
        attributes["translatedPhrase"],
        AttributeValue::S("cba".to_owned())
    );
    assert!(matches!(attributes["error"], AttributeValue::M(_)));
}

#[test]
fn an_update_applies_only_to_an_existing_job_at_the_expected_revision() {
    // ARRANGE
    let change = started_change();

    // ACT
    let request = update_request(&change).unwrap();

    // ASSERT
    assert_eq!(
        request.condition_expression,
        "attribute_exists(#key) AND #expectedRevision = :expectedRevision"
    );
    assert_eq!(request.names["#key"], "id");
    assert_eq!(request.names["#expectedRevision"], "revision");
    assert_eq!(
        request.values[":expectedRevision"],
        AttributeValue::N("1".to_owned())
    );
    assert!(request.update_expression.starts_with("SET #a0 = :a0"));
}

#[test]
fn a_failed_condition_without_a_stored_job_means_not_found() {
    // ARRANGE
    let change = started_change();

    // ACT
    let error = condition_failure(&change, None);

    // ASSERT
    assert!(matches!(&error, StoreError::NotFound { id } if id == "JOB_U"));
    assert_eq!(error.to_string(), "No id found for JOB_U");
}

#[test]
fn a_failed_condition_with_a_stored_job_means_a_conflicting_revision() {
    // ARRANGE
    let change = started_change();
    let stored = HashMap::from([
        ("id".to_owned(), AttributeValue::S("JOB_U".to_owned())),
        ("revision".to_owned(), AttributeValue::N("5".to_owned())),
    ]);

    // ACT
    let error = condition_failure(&change, Some(&stored));

    // ASSERT
    assert!(matches!(
        error,
        StoreError::Conflict {
            stored: 5,
            attempted: 1
        }
    ));
    assert_eq!(
        error.to_string(),
        "Conflict: Item in DB has revision [5]. You are using revision [1]"
    );
}

#[test]
fn store_errors_surface_to_the_workflow_by_name() {
    // ARRANGE
    let errors = [
        StoreError::NotFound {
            id: "JOB_U".to_owned(),
        },
        StoreError::Conflict {
            stored: 2,
            attempted: 1,
        },
    ];

    // ACT
    let names: Vec<String> = errors
        .into_iter()
        .map(|error| lambda_runtime::Diagnostic::from(error).error_type)
        .collect();

    // ASSERT
    assert_eq!(names, ["NotFoundError", "ConflictError"]);
}
