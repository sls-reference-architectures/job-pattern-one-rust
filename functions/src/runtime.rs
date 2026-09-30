//! Cold-start setup shared by every function. Everything here runs once per execution
//! environment, during Lambda's init phase, so SDK clients are reused across invocations.

use aws_config::{BehaviorVersion, SdkConfig};
use chrono::Utc;
use lambda_runtime::{Diagnostic, Error, LambdaEvent, service_fn};

use crate::adapters::job_store::JobStore;
use crate::domain::job::{Job, JobStatus};

pub async fn aws_config() -> SdkConfig {
    aws_config::load_defaults(BehaviorVersion::latest()).await
}

pub fn env(name: &str) -> Result<String, Error> {
    std::env::var(name).map_err(|_| format!("environment variable {name} is not set").into())
}

pub async fn job_store() -> Result<JobStore, Error> {
    let config = aws_config().await;
    Ok(JobStore::new(
        aws_sdk_dynamodb::Client::new(&config),
        env("TABLE_NAME")?,
    ))
}

/// Entry point for the workflow steps that move a job to `status`. The step's input is the job;
/// its output is the job as stored afterwards (which becomes the next step's input).
pub async fn run_status_step(status: JobStatus) -> Result<(), Error> {
    lambda_runtime::tracing::init_default_subscriber();
    let store = job_store().await?;
    lambda_runtime::run(service_fn(|event: LambdaEvent<Job>| {
        let store = &store;
        async move {
            let change = event.payload.change_status(status, Utc::now());
            store.update(&change).await.map_err(Diagnostic::from)
        }
    }))
    .await
}
