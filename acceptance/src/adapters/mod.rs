pub mod http;
pub mod stack;
pub mod workflow;

use serde::Deserialize;
use serde_json::Value;

/// A job as the protocol layer reports it, stripped to what the domain cares about.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: String,
    pub name: String,
    pub phrase: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub translated_phrase: Option<String>,
    #[serde(default)]
    pub revision: Option<u64>,
    /// The job exactly as the service returned it (every attribute), for comparisons and for
    /// replaying it as an event payload.
    #[serde(skip)]
    pub record: Value,
}

impl JobView {
    pub fn from_record(record: Value) -> Result<Self, Error> {
        let mut job: JobView = serde_json::from_value(record.clone())?;
        job.record = record;
        Ok(job)
    }
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
