use aws_sdk_eventbridge::types::PutEventsResultEntry;
use jobs::adapters::event_publisher::{MAX_ENTRIES_PER_REQUEST, PublishError, rejections};

#[test]
fn a_fully_accepted_batch_is_a_success() {
    // ARRANGE
    let results = [
        PutEventsResultEntry::builder().event_id("1").build(),
        PutEventsResultEntry::builder().event_id("2").build(),
    ];

    // ACT
    let outcome = rejections(&results);

    // ASSERT
    assert!(outcome.is_ok());
}

#[test]
fn any_rejected_entry_fails_the_batch_with_its_reason() {
    // ARRANGE
    let results = [
        PutEventsResultEntry::builder().event_id("1").build(),
        PutEventsResultEntry::builder()
            .error_code("ThrottlingException")
            .error_message("Rate exceeded")
            .build(),
    ];

    // ACT
    let outcome = rejections(&results);

    // ASSERT
    let error = outcome.unwrap_err();
    assert!(matches!(&error, PublishError::Rejected(reasons) if reasons.len() == 1));
    assert_eq!(
        error.to_string(),
        "EventBridge rejected 1 event(s): ThrottlingException: Rate exceeded"
    );
}

#[test]
fn events_are_sent_in_requests_of_at_most_ten() {
    // ARRANGE
    let limit = MAX_ENTRIES_PER_REQUEST;

    // ACT
    let chunks: Vec<usize> = (0..23)
        .collect::<Vec<_>>()
        .chunks(limit)
        .map(<[usize]>::len)
        .collect();

    // ASSERT
    assert_eq!(limit, 10);
    assert_eq!(chunks, [10, 10, 3]);
}
