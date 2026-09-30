//! Experiment harness: times AWS SDK setup strategies plus one real DynamoDB call, in a fresh
//! process per strategy (the SDK caches TLS roots per process). Run inside the Lambda base image:
//!
//! ```text
//! init_variants <default|trusted|direct|direct-trusted>
//! ```
//!
//! Prints `startup variant=<v> aws_config=<ms> client=<ms> first_call=<ms> total=<ms>`.

use std::time::Instant;

use aws_config::environment::EnvironmentVariableCredentialsProvider;
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_credential_types::provider::SharedCredentialsProvider;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_smithy_runtime_api::client::http::SharedHttpClient;
use jobs::runtime::amazon_https_client as trusted_http_client;

fn direct_config(http_client: Option<SharedHttpClient>) -> SdkConfig {
    let mut builder = SdkConfig::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new(std::env::var("AWS_REGION").expect("AWS_REGION")))
        .credentials_provider(SharedCredentialsProvider::new(
            EnvironmentVariableCredentialsProvider::new(),
        ));
    if let Some(client) = http_client {
        builder = builder.http_client(client);
    }
    builder.build()
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let variant = std::env::args().nth(1).unwrap_or_else(|| "default".to_owned());
    let began = Instant::now();

    let config = match variant.as_str() {
        "default" => aws_config::load_defaults(BehaviorVersion::latest()).await,
        "trusted" => {
            aws_config::defaults(BehaviorVersion::latest())
                .http_client(trusted_http_client())
                .load()
                .await
        }
        "direct" => direct_config(None),
        "direct-trusted" => direct_config(Some(trusted_http_client())),
        other => panic!("unknown variant {other}"),
    };
    let configured = Instant::now();

    let client = aws_sdk_dynamodb::Client::new(&config);
    let built = Instant::now();

    let outcome = client
        .get_item()
        .table_name("init-variants-probe")
        .key("id", AttributeValue::S("x".to_owned()))
        .send()
        .await;
    let called = Instant::now();

    let ms = |from: Instant, to: Instant| to.duration_since(from).as_secs_f64() * 1000.0;
    let reached_aws = match &outcome {
        Ok(_) => "ok".to_owned(),
        Err(error) => error
            .as_service_error()
            .and_then(|e| aws_sdk_dynamodb::error::ProvideErrorMetadata::code(e).map(str::to_owned))
            .unwrap_or_else(|| format!("{error:?}").chars().take(80).collect()),
    };
    println!(
        "startup variant={variant} aws_config={:.2}ms client={:.2}ms first_call={:.2}ms total={:.2}ms reached={reached_aws}",
        ms(began, configured),
        ms(configured, built),
        ms(built, called),
        ms(began, called)
    );
}
