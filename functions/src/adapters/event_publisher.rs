//! Publishes job events to the service's EventBridge bus.

use std::fmt;

use aws_sdk_eventbridge::Client;
use aws_sdk_eventbridge::types::{PutEventsRequestEntry, PutEventsResultEntry};

use crate::domain::events::{JobEvent, SOURCE};

/// PutEvents accepts at most ten entries per request.
pub const MAX_ENTRIES_PER_REQUEST: usize = 10;

#[derive(Debug)]
pub enum PublishError {
    /// EventBridge accepted the request but rejected some entries; the whole batch must be
    /// retried so no event is lost.
    Rejected(Vec<String>),
    EventBridge(String),
}

impl fmt::Display for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PublishError::Rejected(reasons) => {
                write!(
                    f,
                    "EventBridge rejected {} event(s): {}",
                    reasons.len(),
                    reasons.join("; ")
                )
            }
            PublishError::EventBridge(message) => write!(f, "PutEvents failed: {message}"),
        }
    }
}

impl std::error::Error for PublishError {}

/// Returns an error describing each rejected entry, if any were rejected.
pub fn rejections(results: &[PutEventsResultEntry]) -> Result<(), PublishError> {
    let reasons: Vec<String> = results
        .iter()
        .filter(|entry| entry.error_code().is_some())
        .map(|entry| {
            format!(
                "{}: {}",
                entry.error_code().unwrap_or_default(),
                entry.error_message().unwrap_or_default()
            )
        })
        .collect();
    if reasons.is_empty() {
        Ok(())
    } else {
        Err(PublishError::Rejected(reasons))
    }
}

#[derive(Clone, Debug)]
pub struct EventPublisher {
    client: Client,
    bus: String,
}

impl EventPublisher {
    pub fn new(client: Client, bus: impl Into<String>) -> Self {
        Self {
            client,
            bus: bus.into(),
        }
    }

    pub async fn publish(&self, events: &[JobEvent]) -> Result<(), PublishError> {
        for chunk in events.chunks(MAX_ENTRIES_PER_REQUEST) {
            let entries = chunk
                .iter()
                .map(|event| {
                    PutEventsRequestEntry::builder()
                        .source(SOURCE)
                        .detail_type(event.detail_type())
                        .detail(event.detail.to_string())
                        .event_bus_name(&self.bus)
                        .build()
                })
                .collect();
            let output = self
                .client
                .put_events()
                .set_entries(Some(entries))
                .send()
                .await
                .map_err(|error| PublishError::EventBridge(error.into_service_error().to_string()))?;
            rejections(output.entries())?;
        }
        Ok(())
    }
}
