//! Domain vocabulary for the job pattern: a caller requests jobs and follows their progress.

use std::time::Duration;

use crate::adapters::http::{JobsHttpAdapter, Lookup, Submission};
use crate::adapters::{Error, JobView};

pub use crate::adapters::JobView as Job;

/// The longest a job may take from request to completion before we call it stuck.
const COMPLETION_DEADLINE: Duration = Duration::from_secs(60);
const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Accepted(Job),
    Refused,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    Job(Job),
    Nothing,
    Refused,
}

pub struct Caller {
    adapter: JobsHttpAdapter,
}

impl Caller {
    /// A caller whose identity the service can verify (signed with the test runner's AWS identity).
    pub async fn identified() -> Result<Self, Error> {
        Ok(Self {
            adapter: JobsHttpAdapter::connect(true).await?,
        })
    }

    /// A caller who presents no identity.
    pub async fn anonymous() -> Result<Self, Error> {
        Ok(Self {
            adapter: JobsHttpAdapter::connect(false).await?,
        })
    }

    pub async fn requests_job(&self, name: &str, phrase: &str) -> Result<Request, Error> {
        Ok(match self.adapter.submit(name, phrase).await? {
            Submission::Accepted(job) => Request::Accepted(job),
            Submission::Refused => Request::Refused,
            Submission::Rejected(_) => Request::Invalid,
        })
    }

    /// Convenience for multi-step scenarios: request a job that must be accepted.
    pub async fn has_requested_job(&self, name: &str, phrase: &str) -> Result<Job, Error> {
        match self.requests_job(name, phrase).await? {
            Request::Accepted(job) => Ok(job),
            other => Err(format!("job request was not accepted: {other:?}").into()),
        }
    }

    pub async fn looks_up(&self, id: &str) -> Result<Found, Error> {
        Ok(match self.adapter.fetch(id).await? {
            Lookup::Found(job) => Found::Job(job),
            Lookup::Missing => Found::Nothing,
            Lookup::Refused => Found::Refused,
        })
    }

    /// Follows a job until it finishes (Complete or Failed) or the deadline passes; returns the
    /// last state seen either way so the specification decides what counts as success.
    pub async fn follows_until_finished(&self, id: &str) -> Result<Job, Error> {
        let deadline = tokio::time::Instant::now() + COMPLETION_DEADLINE;
        loop {
            let latest = match self.looks_up(id).await? {
                Found::Job(job) => job,
                other => return Err(format!("job {id} disappeared while following it: {other:?}").into()),
            };
            let finished = matches!(latest.status.as_deref(), Some("Complete" | "Failed"));
            if finished || tokio::time::Instant::now() >= deadline {
                return Ok(latest);
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }
}

pub fn unique_name(label: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("acceptance-{label}-{nanos}")
}

pub fn status_of(job: &JobView) -> &str {
    job.status.as_deref().unwrap_or_default()
}
