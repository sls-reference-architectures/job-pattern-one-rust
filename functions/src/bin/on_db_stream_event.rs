//! DynamoDB stream -> EventBridge. A failure (including any rejected entry) fails the batch so
//! Lambda retries it and no change is lost.

use aws_lambda_events::dynamodb::Event;
use jobs::adapters::event_publisher::EventPublisher;
use jobs::domain::events::job_events;
use jobs::runtime;
use lambda_runtime::{Error, LambdaEvent, run, service_fn};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let mut startup = runtime::Startup::begin();
    lambda_runtime::tracing::init_default_subscriber();
    startup.mark("tracing");
    let config = runtime::aws_config().await;
    startup.mark("aws_config");
    let publisher = EventPublisher::new(
        aws_sdk_eventbridge::Client::new(&config),
        runtime::env("EVENT_BUS_NAME")?,
    );
    startup.mark("clients");
    startup.report();
    run(service_fn(|event: LambdaEvent<Event>| publish(&publisher, event))).await
}

async fn publish(publisher: &EventPublisher, event: LambdaEvent<Event>) -> Result<(), Error> {
    let events = job_events(&event.payload.records)?;
    publisher.publish(&events).await?;
    Ok(())
}
