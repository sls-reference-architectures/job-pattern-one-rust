//! EventBridge `job`/`create` -> starts the translation workflow with the job as its input.

use aws_lambda_events::eventbridge::EventBridgeEvent;
use chrono::Utc;
use jobs::adapters::workflow_starter::WorkflowStarter;
use jobs::domain::workflow::execution_name;
use jobs::runtime;
use lambda_runtime::{Error, LambdaEvent, run, service_fn};
use serde_json::Value;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let mut startup = runtime::Startup::begin();
    lambda_runtime::tracing::init_default_subscriber();
    startup.mark("tracing");
    let config = runtime::aws_config().await;
    startup.mark("aws_config");
    let starter = WorkflowStarter::new(
        aws_sdk_sfn::Client::new(&config),
        runtime::env("TRANSLATE_STATE_MACHINE_ARN")?,
    );
    startup.mark("clients");
    startup.report();
    run(service_fn(|event: LambdaEvent<EventBridgeEvent<Value>>| {
        start(&starter, event)
    }))
    .await
}

async fn start(starter: &WorkflowStarter, event: LambdaEvent<EventBridgeEvent<Value>>) -> Result<(), Error> {
    let job = event.payload.detail;
    let job_id = job
        .get("id")
        .and_then(Value::as_str)
        .ok_or("job created event has no id")?;
    let name = execution_name(job_id, Utc::now().timestamp_millis());
    starter.start(&name, &job.to_string()).await?;
    Ok(())
}
