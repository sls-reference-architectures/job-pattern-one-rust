//! Workflow step: adds `translatedPhrase` (the phrase reversed) to the job.

use jobs::domain::phrase::translate;
use lambda_runtime::{Error, LambdaEvent, run, service_fn};
use serde_json::Value;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let mut startup = jobs::runtime::Startup::begin();
    lambda_runtime::tracing::init_default_subscriber();
    startup.mark("tracing");
    startup.report();
    run(service_fn(|event: LambdaEvent<Value>| async move {
        Ok::<_, Error>(translate(event.payload))
    }))
    .await
}
