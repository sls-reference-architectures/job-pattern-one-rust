//! Change-data-capture: every change to a stored job becomes a domain event on the bus.

use aws_lambda_events::dynamodb::EventRecord;
use serde_json::{Value, json};

pub const SOURCE: &str = "job";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Create,
    Update,
    Delete,
}

impl ChangeKind {
    pub fn detail_type(self) -> &'static str {
        match self {
            ChangeKind::Create => "create",
            ChangeKind::Update => "update",
            ChangeKind::Delete => "delete",
        }
    }

    fn from_event_name(name: &str) -> Option<Self> {
        match name {
            "INSERT" => Some(ChangeKind::Create),
            "MODIFY" => Some(ChangeKind::Update),
            "REMOVE" => Some(ChangeKind::Delete),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct JobEvent {
    pub kind: ChangeKind,
    pub detail: Value,
}

impl JobEvent {
    pub fn detail_type(&self) -> &'static str {
        self.kind.detail_type()
    }
}

#[derive(Debug)]
pub struct UnreadableImage(pub String);

impl std::fmt::Display for UnreadableImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unreadable stream image: {}", self.0)
    }
}

impl std::error::Error for UnreadableImage {}

/// Maps a batch of stream records to events, ordered creates, then updates, then deletes.
///
/// * create: the new image
/// * update: `{ "old": <old image>, "new": <new image> }`
/// * delete: the old image
///
/// Records with an unrecognized event name are ignored.
pub fn job_events(records: &[EventRecord]) -> Result<Vec<JobEvent>, UnreadableImage> {
    let mut events = Vec::with_capacity(records.len());
    for kind in [ChangeKind::Create, ChangeKind::Update, ChangeKind::Delete] {
        for record in records
            .iter()
            .filter(|record| ChangeKind::from_event_name(&record.event_name) == Some(kind))
        {
            let change = &record.change;
            let detail = match kind {
                ChangeKind::Create => to_json(&change.new_image)?,
                ChangeKind::Update => json!({
                    "old": to_json(&change.old_image)?,
                    "new": to_json(&change.new_image)?,
                }),
                ChangeKind::Delete => to_json(&change.old_image)?,
            };
            events.push(JobEvent { kind, detail });
        }
    }
    Ok(events)
}

fn to_json(image: &serde_dynamo::Item) -> Result<Value, UnreadableImage> {
    serde_dynamo::from_item(image.clone()).map_err(|error| UnreadableImage(error.to_string()))
}
