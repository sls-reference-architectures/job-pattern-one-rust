//! The translation workflow (an Express Step Functions state machine), driven directly.

use serde_json::Value;

use super::{Error, stack};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunOutcome {
    Succeeded,
    Failed { error: String, cause: String },
}

pub struct WorkflowAdapter {
    client: aws_sdk_sfn::Client,
    state_machine_arn: String,
}

impl WorkflowAdapter {
    pub async fn connect() -> Result<Self, Error> {
        let config = stack::aws_config().await;
        let state_machine_arn =
            stack::output(&config, &stack::stack_name(), "TranslateStateMachineArn").await?;
        Ok(Self {
            client: aws_sdk_sfn::Client::new(&config),
            state_machine_arn,
        })
    }

    /// Runs the workflow to completion with `input` (Express workflows support synchronous runs).
    pub async fn run(&self, input: &Value) -> Result<RunOutcome, Error> {
        let output = self
            .client
            .start_sync_execution()
            .state_machine_arn(&self.state_machine_arn)
            .input(input.to_string())
            .send()
            .await?;
        Ok(match output.status() {
            aws_sdk_sfn::types::SyncExecutionStatus::Succeeded => RunOutcome::Succeeded,
            _ => RunOutcome::Failed {
                error: output.error().unwrap_or_default().to_owned(),
                cause: output.cause().unwrap_or_default().to_owned(),
            },
        })
    }
}
