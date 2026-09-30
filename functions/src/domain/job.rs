use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ID_PREFIX: &str = "JOB_";

/// Jobs expire (via DynamoDB TTL) five days after creation.
pub const TIME_TO_LIVE_SECONDS: i64 = 5 * 24 * 60 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Started,
    Complete,
    Failed,
}

/// What a caller supplies when requesting a job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewJob {
    pub name: String,
    pub phrase: String,
}

/// A job as stored, returned by the API, published on the bus, and passed through the workflow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub name: String,
    pub phrase: String,
    pub status: JobStatus,
    pub ttl: i64,
    pub created_at: String,
    pub updated_at: String,
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translated_phrase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

/// A job moved to a new status, plus the revision the stored copy must still have for the
/// change to apply (optimistic concurrency).
#[derive(Clone, Debug, PartialEq)]
pub struct StatusChange {
    pub expected_revision: u64,
    pub job: Job,
}

impl Job {
    /// `unique` is a fresh ULID; `now` is the creation instant.
    pub fn create(request: NewJob, unique: &str, now: DateTime<Utc>) -> Job {
        let timestamp = iso_timestamp(now);
        Job {
            id: format!("{ID_PREFIX}{unique}"),
            name: request.name,
            phrase: request.phrase,
            status: JobStatus::Pending,
            ttl: now.timestamp() + TIME_TO_LIVE_SECONDS,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            revision: 1,
            translated_phrase: None,
            error: None,
        }
    }

    pub fn change_status(self, status: JobStatus, now: DateTime<Utc>) -> StatusChange {
        let expected_revision = self.revision;
        StatusChange {
            expected_revision,
            job: Job {
                status,
                updated_at: iso_timestamp(now),
                revision: expected_revision + 1,
                ..self
            },
        }
    }
}

/// ISO-8601 with millisecond precision and a `Z` suffix, e.g. `2026-09-30T12:00:00.000Z`.
pub fn iso_timestamp(instant: DateTime<Utc>) -> String {
    instant.to_rfc3339_opts(SecondsFormat::Millis, true)
}
