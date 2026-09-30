use aws_config::{BehaviorVersion, Region, SdkConfig, meta::region::RegionProviderChain};
use chrono::Utc;
use jobs::domain::job::{Job, NewJob};

pub async fn aws_config() -> SdkConfig {
    let region = RegionProviderChain::default_provider().or_else(Region::new("us-east-1"));
    aws_config::defaults(BehaviorVersion::latest())
        .region(region)
        .load()
        .await
}

pub async fn stack_output(config: &SdkConfig, key: &str) -> String {
    let stack = std::env::var("STACK_NAME").unwrap_or_else(|_| "job-pattern-one-rust-dev".to_owned());
    let described = aws_sdk_cloudformation::Client::new(config)
        .describe_stacks()
        .stack_name(&stack)
        .send()
        .await
        .unwrap();
    described.stacks()[0]
        .outputs()
        .iter()
        .find(|output| output.output_key() == Some(key))
        .and_then(|output| output.output_value())
        .unwrap_or_else(|| panic!("stack {stack} has no output {key}"))
        .to_owned()
}

pub fn fresh_job(label: &str) -> Job {
    Job::create(
        NewJob {
            name: format!("integration-{label}"),
            phrase: "integration phrase".to_owned(),
        },
        &ulid::Ulid::generate().to_string(),
        Utc::now(),
    )
}
