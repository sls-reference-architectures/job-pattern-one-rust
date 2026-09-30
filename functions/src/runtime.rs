//! Cold-start setup shared by every function. Everything here runs once per execution
//! environment, during Lambda's init phase, so SDK clients are reused across invocations.

use std::time::Instant;

use aws_config::{BehaviorVersion, SdkConfig};
use chrono::Utc;
use lambda_runtime::{Diagnostic, Error, LambdaEvent, service_fn};

use crate::adapters::job_store::JobStore;
use crate::domain::job::{Job, JobStatus};

pub async fn aws_config() -> SdkConfig {
    aws_config::load_defaults(BehaviorVersion::latest()).await
}

pub fn env(name: &str) -> Result<String, Error> {
    std::env::var(name).map_err(|_| format!("environment variable {name} is not set").into())
}

pub fn job_store(config: &SdkConfig) -> Result<JobStore, Error> {
    Ok(JobStore::new(
        aws_sdk_dynamodb::Client::new(config),
        env("TABLE_NAME")?,
    ))
}

/// Times the init phases inside `main` and logs them as one line per cold start, so init cost
/// can be attributed. Lambda's REPORT `Init Duration` minus `in_main` is process start-up
/// (exec, dynamic loading) that happens before `main`.
pub struct Startup {
    began: Instant,
    last: Instant,
    phases: Vec<(&'static str, f64)>,
}

impl Startup {
    pub fn begin() -> Self {
        let now = Instant::now();
        Self {
            began: now,
            last: now,
            phases: Vec::new(),
        }
    }

    /// Records the time since the previous mark (or `begin`) under `phase`.
    pub fn mark(&mut self, phase: &'static str) {
        let now = Instant::now();
        self.phases
            .push((phase, now.duration_since(self.last).as_secs_f64() * 1000.0));
        self.last = now;
    }

    pub fn report(self) {
        let in_main = self.last.duration_since(self.began).as_secs_f64() * 1000.0;
        lambda_runtime::tracing::info!("{}", startup_line(&self.phases, in_main));
    }
}

/// `startup tracing=0.12ms aws_config=4.56ms clients=0.78ms in_main=5.46ms`
pub fn startup_line(phases: &[(&str, f64)], in_main_ms: f64) -> String {
    let mut line = String::from("startup");
    for (phase, ms) in phases {
        line.push_str(&format!(" {phase}={ms:.2}ms"));
    }
    line.push_str(&format!(" in_main={in_main_ms:.2}ms"));
    line
}

/// Entry point for the workflow steps that move a job to `status`. The step's input is the job;
/// its output is the job as stored afterwards (which becomes the next step's input).
pub async fn run_status_step(status: JobStatus) -> Result<(), Error> {
    let mut startup = Startup::begin();
    lambda_runtime::tracing::init_default_subscriber();
    startup.mark("tracing");
    let config = aws_config().await;
    startup.mark("aws_config");
    let store = job_store(&config)?;
    startup.mark("clients");
    startup.report();
    lambda_runtime::run(service_fn(|event: LambdaEvent<Job>| {
        let store = &store;
        async move {
            let change = event.payload.change_status(status, Utc::now());
            store.update(&change).await.map_err(Diagnostic::from)
        }
    }))
    .await
}
