use jobs::adapters::event_publisher::EventPublisher;
use jobs::domain::events::{ChangeKind, JobEvent};
use serde_json::json;

use crate::support::{aws_config, stack_output};

async fn publisher() -> EventPublisher {
    let config = aws_config().await;
    let bus = stack_output(&config, "EventBusName").await;
    EventPublisher::new(aws_sdk_eventbridge::Client::new(&config), bus)
}

#[tokio::test]
async fn events_are_accepted_by_the_bus() {
    // ARRANGE
    let publisher = publisher().await;
    let events = [JobEvent {
        kind: ChangeKind::Delete,
        detail: json!({ "id": "JOB_integration_publish" }),
    }];

    // ACT
    let outcome = publisher.publish(&events).await;

    // ASSERT
    assert!(outcome.is_ok(), "{outcome:?}");
}

#[tokio::test]
async fn more_events_than_one_request_holds_are_all_accepted() {
    // ARRANGE
    let publisher = publisher().await;
    let events: Vec<JobEvent> = (0..23)
        .map(|index| JobEvent {
            kind: ChangeKind::Delete,
            detail: json!({ "id": format!("JOB_integration_batch_{index}") }),
        })
        .collect();

    // ACT
    let outcome = publisher.publish(&events).await;

    // ASSERT
    assert!(outcome.is_ok(), "{outcome:?}");
}
