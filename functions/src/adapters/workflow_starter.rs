//! Starts the translation workflow (a Step Functions state machine) for a job.

use aws_sdk_sfn::Client;

#[derive(Clone, Debug)]
pub struct WorkflowStarter {
    client: Client,
    state_machine_arn: String,
}

impl WorkflowStarter {
    pub fn new(client: Client, state_machine_arn: impl Into<String>) -> Self {
        Self {
            client,
            state_machine_arn: state_machine_arn.into(),
        }
    }

    pub async fn start(&self, execution_name: &str, input: &str) -> Result<String, lambda_runtime::Error> {
        let output = self
            .client
            .start_execution()
            .state_machine_arn(&self.state_machine_arn)
            .name(execution_name)
            .input(input)
            .send()
            .await
            .map_err(|error| error.into_service_error())?;
        Ok(output.execution_arn)
    }
}
