//! Workflow step: marks the job Started.

use jobs::domain::job::JobStatus;
use lambda_runtime::Error;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    jobs::runtime::run_status_step(JobStatus::Started).await
}
