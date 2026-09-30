//! Locates the deployed service from its CloudFormation stack outputs.

use aws_config::{BehaviorVersion, Region, SdkConfig, meta::region::RegionProviderChain};

use super::Error;

pub const DEFAULT_STACK_NAME: &str = "job-pattern-one-rust-dev";
const DEFAULT_REGION: &str = "us-east-1";

pub fn stack_name() -> String {
    std::env::var("STACK_NAME").unwrap_or_else(|_| DEFAULT_STACK_NAME.to_owned())
}

pub async fn aws_config() -> SdkConfig {
    let region = RegionProviderChain::default_provider().or_else(Region::new(DEFAULT_REGION));
    aws_config::defaults(BehaviorVersion::latest())
        .region(region)
        .load()
        .await
}

pub async fn output(config: &SdkConfig, stack: &str, key: &str) -> Result<String, Error> {
    let described = aws_sdk_cloudformation::Client::new(config)
        .describe_stacks()
        .stack_name(stack)
        .send()
        .await?;
    described
        .stacks()
        .first()
        .and_then(|stack| {
            stack
                .outputs()
                .iter()
                .find(|output| output.output_key() == Some(key))
        })
        .and_then(|output| output.output_value())
        .map(|value| value.trim_end_matches('/').to_owned())
        .ok_or_else(|| format!("stack {stack} has no output {key}").into())
}
