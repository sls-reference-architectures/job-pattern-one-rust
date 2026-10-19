//! Forces cold starts on every function of each target stack, records Lambda's REPORT figures,
//! publishes them to CloudWatch, and refreshes the comparison dashboard.
//!
//! ```text
//! cold-start-probe [--samples N] [--dry-run] [STACK=RUNTIME ...]
//! ```
//!
//! Defaults compare `job-pattern-one-dev=nodejs24.x` with `job-pattern-one-rust-dev=rust`.
//! A cold start is forced by changing an environment variable (`PROBE_NONCE`), which makes Lambda
//! discard warm execution environments; the variable is removed again afterwards.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aws_config::retry::RetryConfig;
use aws_config::{BehaviorVersion, Region, SdkConfig, meta::region::RegionProviderChain};
use aws_sdk_cloudwatch::types::{Dimension, MetricDatum, StandardUnit};
use aws_sdk_lambda::primitives::Blob;
use aws_sdk_lambda::types::{Environment, FunctionConfiguration, LastUpdateStatus, LogType};
use base64::Engine;
use probe::dashboard::{self, DASHBOARD_NAME, PATTERN};
use probe::diagnosis::{LogLine, describe, explain};
use probe::payloads::probe_for;
use probe::report::{Results, Series, markdown_summary, parse_report, request_id};
use probe::{NAMESPACE, NONCE, Target, carries_nonce, function_key, parse_target};
use tokio::sync::Semaphore;

type Error = Box<dyn std::error::Error + Send + Sync>;

const DEFAULT_SAMPLES: usize = 10;
/// Functions probed at once. Each sample makes several control-plane calls
/// (UpdateFunctionConfiguration counts against Lambda's ~15 requests/s account limit).
const CONCURRENT_FUNCTIONS: usize = 4;
/// Attempt budget per requested sample. Even after GetFunction shows the new nonce as
/// `Successful`, an invoke can come back warm (2026-10-16: 6 of 10 on the Node
/// setJobStatusStarted). No API reports when that stops, so a warm sample is retried with a fresh
/// nonce instead.
const ATTEMPTS_PER_SAMPLE: usize = 3;
/// Pause before retrying after a warm sample, per consecutive warm sample, capped. Retrying at
/// once kept coming back warm for a whole function's attempt budget (2026-10-17: 26 in a row on
/// the Rust setJobStatusFailed; 2026-10-18: 26 on the Rust onJobCreated).
const WARM_BACKOFF: Duration = Duration::from_secs(10);
const MAX_WARM_BACKOFF: Duration = Duration::from_secs(30);
/// Warm samples per function whose log stream is looked up to explain how they were served.
/// Function logs are kept 7 days and deleted with the stack, so the workflow log keeps the evidence.
const EXPLAINED_PER_FUNCTION: usize = 3;

struct Options {
    samples: usize,
    dry_run: bool,
    targets: Vec<Target>,
}

struct Function {
    key: String,
    name: String,
    runtime: String,
}

struct Measured {
    function: Function,
    series: Series,
    comparable_first_invoke: bool,
    warm: Vec<Warm>,
}

/// A sample that came back warm.
struct Warm {
    attempt: usize,
    request_id: String,
    /// When the probe saw this attempt's nonce update as `Successful` (epoch ms).
    updated_at_ms: i64,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let options = options()?;
    let region = RegionProviderChain::default_provider().or_else(Region::new("us-east-1"));
    // Lambda's control-plane APIs share a low account-wide rate limit; adaptive retries back off
    // on throttling instead of failing the sample.
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(region)
        .retry_config(RetryConfig::adaptive().with_max_attempts(10))
        .load()
        .await;
    let lambda = aws_sdk_lambda::Client::new(&config);

    let mut functions = Vec::new();
    for target in &options.targets {
        match discover(&config, target).await {
            Ok(found) => functions.extend(found),
            Err(reason) => eprintln!("skipping stack {}: {}", target.stack, chain(&*reason)),
        }
    }
    if functions.is_empty() {
        return Err("no deployed functions to probe".into());
    }

    let slots = Arc::new(Semaphore::new(CONCURRENT_FUNCTIONS));
    let measured: Vec<Measured> = futures_join(functions.into_iter().map(|function| {
        let lambda = lambda.clone();
        let samples = options.samples;
        let slots = Arc::clone(&slots);
        async move {
            let _slot = slots.acquire_owned().await.expect("semaphore is never closed");
            measure(&lambda, function, samples).await
        }
    }))
    .await;
    explain_warm_samples(&config, &measured).await;

    let runtimes: Vec<String> = options
        .targets
        .iter()
        .map(|target| target.runtime.clone())
        .collect();
    let mut results: Results = BTreeMap::new();
    for entry in &measured {
        results
            .entry(entry.function.key.clone())
            .or_default()
            .insert(entry.function.runtime.clone(), entry.series.clone());
    }
    let summary = format!(
        "## Cold-start probe ({} forced cold starts per function)\n\n{}",
        options.samples,
        markdown_summary(&results, &runtimes)
    );
    println!("{summary}");
    if let Ok(path) = std::env::var("GITHUB_STEP_SUMMARY") {
        std::fs::write(path, &summary)?;
    }

    if options.dry_run {
        return Ok(());
    }
    publish_metrics(&config, &measured).await?;
    let log_groups: Vec<String> = measured
        .iter()
        .map(|entry| format!("/aws/lambda/{}", entry.function.name))
        .collect();
    let region = config.region().map(ToString::to_string).unwrap_or_default();
    aws_sdk_cloudwatch::Client::new(&config)
        .put_dashboard()
        .dashboard_name(DASHBOARD_NAME)
        .dashboard_body(dashboard::body(&region, &runtimes, &log_groups).to_string())
        .send()
        .await?;
    let incomplete: Vec<&str> = measured
        .iter()
        .filter(|entry| entry.series.samples.len() < options.samples)
        .map(|entry| entry.function.name.as_str())
        .collect();
    if incomplete.is_empty() {
        Ok(())
    } else {
        Err(format!("fewer samples than requested for: {}", incomplete.join(", ")).into())
    }
}

/// The full cause chain: AWS SDK errors display only "service error" at the top level; the
/// service's error code and message are in the sources.
fn chain(error: &(dyn std::error::Error + 'static)) -> String {
    let mut parts = vec![error.to_string()];
    let mut source = error.source();
    while let Some(cause) = source {
        parts.push(cause.to_string());
        source = cause.source();
    }
    parts.join(": ")
}

fn options() -> Result<Options, Error> {
    let mut samples = DEFAULT_SAMPLES;
    let mut dry_run = false;
    let mut targets = Vec::new();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--samples" => samples = arguments.next().ok_or("--samples needs a value")?.parse()?,
            "--dry-run" => dry_run = true,
            other => targets.push(parse_target(other)?),
        }
    }
    if targets.is_empty() {
        targets = vec![
            parse_target("job-pattern-one-dev=nodejs24.x")?,
            parse_target("job-pattern-one-rust-dev=rust")?,
        ];
    }
    Ok(Options {
        samples,
        dry_run,
        targets,
    })
}

async fn futures_join<F: std::future::Future<Output = Measured> + Send + 'static>(
    futures: impl Iterator<Item = F>,
) -> Vec<Measured> {
    let handles: Vec<_> = futures.map(tokio::spawn).collect();
    let mut measured = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(entry) => measured.push(entry),
            Err(panic) => eprintln!("probe task failed: {panic}"),
        }
    }
    measured
}

async fn discover(config: &SdkConfig, target: &Target) -> Result<Vec<Function>, Error> {
    let cloudformation = aws_sdk_cloudformation::Client::new(config);
    let described = cloudformation
        .describe_stacks()
        .stack_name(&target.stack)
        .send()
        .await?;
    let status = described
        .stacks()
        .first()
        .and_then(|stack| stack.stack_status())
        .map(|status| status.as_str().to_owned())
        .unwrap_or_default();
    if !status.ends_with("_COMPLETE") || status.starts_with("DELETE") || status.contains("ROLLBACK") {
        return Err(format!("stack is {status}").into());
    }
    let mut functions = Vec::new();
    let mut pages = cloudformation
        .list_stack_resources()
        .stack_name(&target.stack)
        .into_paginator()
        .send();
    while let Some(page) = pages.next().await {
        for resource in page?.stack_resource_summaries() {
            if resource.resource_type() != Some("AWS::Lambda::Function") {
                continue;
            }
            let Some(name) = resource.physical_resource_id() else {
                continue;
            };
            let key = function_key(&target.stack, name);
            if probe_for(&key).is_some() {
                functions.push(Function {
                    key,
                    name: name.to_owned(),
                    runtime: target.runtime.clone(),
                });
            }
        }
    }
    Ok(functions)
}

async fn measure(lambda: &aws_sdk_lambda::Client, function: Function, samples: usize) -> Measured {
    let probe = probe_for(&function.key).expect("discovered functions all have probes");
    let payload = probe.payload.to_string();
    let mut series = Series::default();
    let mut warm = Vec::new();
    let mut consecutive_warm: u32 = 0;
    let mut attempt = 0;
    while series.samples.len() < samples && attempt < samples * ATTEMPTS_PER_SAMPLE {
        attempt += 1;
        if consecutive_warm > 0 {
            tokio::time::sleep((WARM_BACKOFF * consecutive_warm).min(MAX_WARM_BACKOFF)).await;
        }
        let nonce = format!("{}-{attempt}", std::process::id());
        let sample = async {
            set_nonce(lambda, &function.name, Some(&nonce)).await?;
            let updated_at_ms = epoch_ms();
            let log = invoke(lambda, &function.name, &payload).await?;
            Ok::<_, Error>((updated_at_ms, log))
        }
        .await;
        match sample {
            Ok((updated_at_ms, log)) => match parse_report(&log) {
                Some(sample) => {
                    consecutive_warm = 0;
                    series.samples.push(sample);
                }
                None => {
                    consecutive_warm += 1;
                    eprintln!("{}: attempt {attempt} was not a cold start", function.name);
                    if let Some(id) = request_id(&log) {
                        warm.push(Warm {
                            attempt,
                            request_id: id.to_owned(),
                            updated_at_ms,
                        });
                    }
                }
            },
            Err(reason) => eprintln!("{}: sample {attempt} failed: {}", function.name, chain(&*reason)),
        }
    }
    if let Err(reason) = set_nonce(lambda, &function.name, None).await {
        eprintln!("{}: could not remove {NONCE}: {}", function.name, chain(&*reason));
    }
    Measured {
        function,
        series,
        comparable_first_invoke: probe.comparable_first_invoke,
        warm,
    }
}

fn epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

/// Prints, for the first few warm samples of each function, how the environment that served them
/// came to be warm. Diagnostic only: lookup failures are reported, never fatal.
async fn explain_warm_samples(config: &SdkConfig, measured: &[Measured]) {
    let logs = aws_sdk_cloudwatchlogs::Client::new(config);
    for entry in measured {
        for warm in entry.warm.iter().take(EXPLAINED_PER_FUNCTION) {
            let name = &entry.function.name;
            let explanation = match served_by(&logs, name, warm).await {
                Ok(stream) => describe(&explain(&stream, &warm.request_id, warm.updated_at_ms)),
                Err(reason) => format!("could not read its log stream: {}", chain(&*reason)),
            };
            eprintln!(
                "{name}: attempt {} ({}) {explanation}",
                warm.attempt, warm.request_id
            );
        }
    }
}

/// The log stream of the environment that handled a warm sample. Log delivery lags the invoke by
/// a few seconds, so the lookup is retried briefly.
async fn served_by(
    logs: &aws_sdk_cloudwatchlogs::Client,
    function_name: &str,
    warm: &Warm,
) -> Result<Vec<LogLine>, Error> {
    let group = format!("/aws/lambda/{function_name}");
    let mut stream = None;
    for _ in 0..4 {
        // FilterLogEvents can return empty pages before the one holding the match.
        let mut pages = logs
            .filter_log_events()
            .log_group_name(&group)
            .start_time(warm.updated_at_ms - 60_000)
            .filter_pattern(format!("\"START RequestId: {}\"", warm.request_id))
            .into_paginator()
            .send();
        while let Some(page) = pages.next().await {
            stream = page?
                .events()
                .first()
                .and_then(|event| event.log_stream_name())
                .map(ToOwned::to_owned);
            if stream.is_some() {
                break;
            }
        }
        if stream.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    let Some(stream) = stream else {
        return Ok(Vec::new());
    };
    let mut lines = Vec::new();
    let mut pages = logs
        .get_log_events()
        .log_group_name(&group)
        .log_stream_name(stream)
        .start_from_head(true)
        .into_paginator()
        .send();
    while let Some(page) = pages.next().await {
        let page = page?;
        if page.events().is_empty() {
            break;
        }
        lines.extend(page.events().iter().map(|event| LogLine {
            timestamp_ms: event.timestamp().unwrap_or_default(),
            message: event.message().unwrap_or_default().to_owned(),
        }));
    }
    Ok(lines)
}

/// Sets (or, with `None`, removes) the nonce variable, preserving every other variable, then waits
/// for the update to finish. Retries while a deployment holds the function.
async fn set_nonce(lambda: &aws_sdk_lambda::Client, name: &str, nonce: Option<&str>) -> Result<(), Error> {
    for retry in 0..5u64 {
        let current = configuration(lambda, name).await?;
        let mut variables: HashMap<String, String> = current
            .environment()
            .and_then(|environment| environment.variables())
            .cloned()
            .unwrap_or_default();
        match nonce {
            Some(value) => variables.insert(NONCE.to_owned(), value.to_owned()),
            None if variables.contains_key(NONCE) => variables.remove(NONCE),
            None => return Ok(()),
        };
        let updated = lambda
            .update_function_configuration()
            .function_name(name)
            .environment(Environment::builder().set_variables(Some(variables)).build())
            .send()
            .await;
        match updated {
            Ok(_) => return wait_until_updated(lambda, name, nonce).await,
            Err(error) => {
                let error = error.into_service_error();
                if error.is_resource_conflict_exception() || error.is_too_many_requests_exception() {
                    tokio::time::sleep(Duration::from_secs(2 << retry)).await;
                } else {
                    return Err(error.into());
                }
            }
        }
    }
    Err(format!("{name} stayed busy; gave up updating {NONCE}").into())
}

/// Reads a function's configuration via GetFunction, which has a far higher rate limit than
/// GetFunctionConfiguration (a throttled control-plane call).
async fn configuration(lambda: &aws_sdk_lambda::Client, name: &str) -> Result<FunctionConfiguration, Error> {
    lambda
        .get_function()
        .function_name(name)
        .send()
        .await?
        .configuration
        .ok_or_else(|| format!("{name} has no configuration").into())
}

async fn wait_until_updated(
    lambda: &aws_sdk_lambda::Client,
    name: &str,
    nonce: Option<&str>,
) -> Result<(), Error> {
    for _ in 0..60 {
        let configuration = configuration(lambda, name).await?;
        let variables = configuration
            .environment()
            .and_then(|environment| environment.variables());
        if !carries_nonce(variables, nonce) {
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        }
        match configuration.last_update_status() {
            Some(LastUpdateStatus::Successful) => return Ok(()),
            Some(LastUpdateStatus::Failed) => {
                return Err(format!(
                    "{name} update failed: {:?}",
                    configuration.last_update_status_reason()
                )
                .into());
            }
            _ => tokio::time::sleep(Duration::from_secs(1)).await,
        }
    }
    Err(format!("{name} update did not finish within 60 s").into())
}

/// Invokes the function and returns the tail of its log output.
async fn invoke(lambda: &aws_sdk_lambda::Client, name: &str, payload: &str) -> Result<String, Error> {
    let output = lambda
        .invoke()
        .function_name(name)
        .log_type(LogType::Tail)
        .payload(Blob::new(payload.as_bytes()))
        .send()
        .await?;
    let log = base64::engine::general_purpose::STANDARD.decode(output.log_result().unwrap_or_default())?;
    Ok(String::from_utf8_lossy(&log).into_owned())
}

async fn publish_metrics(config: &SdkConfig, measured: &[Measured]) -> Result<(), Error> {
    let dimension = |name: &str, value: &str| Dimension::builder().name(name).value(value).build();
    let mut data = Vec::new();
    for entry in measured.iter().filter(|entry| !entry.series.samples.is_empty()) {
        let runtime = &entry.function.runtime;
        let scopes = [
            vec![dimension("Pattern", PATTERN), dimension("Runtime", runtime)],
            vec![
                dimension("Pattern", PATTERN),
                dimension("Runtime", runtime),
                dimension("Function", &entry.function.key),
            ],
        ];
        let mut metrics = vec![
            ("InitDuration", entry.series.init(), StandardUnit::Milliseconds),
            ("MaxMemoryUsed", entry.series.memory(), StandardUnit::Megabytes),
        ];
        if entry.comparable_first_invoke {
            metrics.push((
                "FirstInvokeDuration",
                entry.series.duration(),
                StandardUnit::Milliseconds,
            ));
        }
        for (metric, values, unit) in &metrics {
            for dimensions in &scopes {
                data.push(
                    MetricDatum::builder()
                        .metric_name(*metric)
                        .set_dimensions(Some(dimensions.clone()))
                        .set_values(Some(values.clone()))
                        .unit(unit.clone())
                        .build(),
                );
            }
        }
    }
    let cloudwatch = aws_sdk_cloudwatch::Client::new(config);
    for chunk in data.chunks(500) {
        cloudwatch
            .put_metric_data()
            .namespace(NAMESPACE)
            .set_metric_data(Some(chunk.to_vec()))
            .send()
            .await?;
    }
    Ok(())
}
