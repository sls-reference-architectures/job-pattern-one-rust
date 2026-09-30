use aws_lambda_events::dynamodb::Event;
use jobs::domain::events::{ChangeKind, job_events};
use serde_json::{Value, json};

fn stream_record(event_name: &str, old_revision: Option<u64>, new_revision: Option<u64>) -> Value {
    let image = |revision: u64| {
        json!({
            "id": { "S": "JOB_X" },
            "phrase": { "S": "hi" },
            "revision": { "N": revision.to_string() },
            "ttl": { "N": "1790432000" },
            "error": { "M": { "Error": { "S": "Boom" } } }
        })
    };
    let mut change = json!({
        "ApproximateCreationDateTime": 1790000000.0,
        "Keys": { "id": { "S": "JOB_X" } },
        "SequenceNumber": "1",
        "SizeBytes": 10,
        "StreamViewType": "NEW_AND_OLD_IMAGES"
    });
    if let Some(revision) = old_revision {
        change["OldImage"] = image(revision);
    }
    if let Some(revision) = new_revision {
        change["NewImage"] = image(revision);
    }
    json!({
        "eventID": "e",
        "eventName": event_name,
        "eventVersion": "1.1",
        "eventSource": "aws:dynamodb",
        "awsRegion": "us-east-1",
        "dynamodb": change,
        "eventSourceARN": "arn:aws:dynamodb:us-east-1:123456789012:table/t/stream/s"
    })
}

fn batch(records: Vec<Value>) -> Event {
    serde_json::from_value(json!({ "Records": records })).unwrap()
}

#[test]
fn an_insert_becomes_a_create_event_carrying_the_new_job() {
    // ARRANGE
    let event = batch(vec![stream_record("INSERT", None, Some(1))]);

    // ACT
    let events = job_events(&event.records).unwrap();

    // ASSERT
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, ChangeKind::Create);
    assert_eq!(events[0].detail_type(), "create");
    assert_eq!(events[0].detail["id"], "JOB_X");
}

#[test]
fn stored_numbers_are_published_as_json_numbers() {
    // ARRANGE
    let event = batch(vec![stream_record("INSERT", None, Some(3))]);

    // ACT
    let events = job_events(&event.records).unwrap();

    // ASSERT
    assert_eq!(events[0].detail["revision"], json!(3));
    assert_eq!(events[0].detail["ttl"], json!(1_790_432_000));
    assert_eq!(events[0].detail["error"], json!({ "Error": "Boom" }));
}

#[test]
fn a_modify_becomes_an_update_event_carrying_old_and_new() {
    // ARRANGE
    let event = batch(vec![stream_record("MODIFY", Some(1), Some(2))]);

    // ACT
    let events = job_events(&event.records).unwrap();

    // ASSERT
    assert_eq!(events[0].detail_type(), "update");
    assert_eq!(events[0].detail["old"]["revision"], json!(1));
    assert_eq!(events[0].detail["new"]["revision"], json!(2));
}

#[test]
fn a_remove_becomes_a_delete_event_carrying_the_old_job() {
    // ARRANGE
    let event = batch(vec![stream_record("REMOVE", Some(4), None)]);

    // ACT
    let events = job_events(&event.records).unwrap();

    // ASSERT
    assert_eq!(events[0].detail_type(), "delete");
    assert_eq!(events[0].detail["revision"], json!(4));
}

#[test]
fn a_batch_publishes_creates_then_updates_then_deletes() {
    // ARRANGE
    let event = batch(vec![
        stream_record("REMOVE", Some(9), None),
        stream_record("MODIFY", Some(1), Some(2)),
        stream_record("INSERT", None, Some(1)),
        stream_record("MODIFY", Some(2), Some(3)),
    ]);

    // ACT
    let events = job_events(&event.records).unwrap();

    // ASSERT
    let kinds: Vec<_> = events.iter().map(|event| event.detail_type()).collect();
    assert_eq!(kinds, ["create", "update", "update", "delete"]);
    assert_eq!(events[1].detail["new"]["revision"], json!(2));
    assert_eq!(events[2].detail["new"]["revision"], json!(3));
}

#[test]
fn an_empty_batch_publishes_nothing() {
    // ARRANGE
    let event = batch(vec![]);

    // ACT
    let events = job_events(&event.records).unwrap();

    // ASSERT
    assert!(events.is_empty());
}
