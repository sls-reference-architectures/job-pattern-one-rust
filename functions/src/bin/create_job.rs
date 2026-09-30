//! POST /jobs: records a new Pending job and returns it (201). Processing continues
//! asynchronously via the table's stream.

use chrono::Utc;
use jobs::adapters::{http, job_store::JobStore};
use jobs::domain::{job::Job, request::parse_new_job};
use jobs::runtime;
use lambda_http::{Body, Error, Request, Response, run, service_fn};
use ulid::Ulid;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    lambda_http::tracing::init_default_subscriber();
    let store = runtime::job_store().await?;
    run(service_fn(|request: Request| create_job(&store, request))).await
}

async fn create_job(store: &JobStore, request: Request) -> Result<Response<Body>, Error> {
    let new_job = match parse_new_job(http::content_type(&request), http::body_text(&request)) {
        Ok(new_job) => new_job,
        Err(failure) => return http::error(failure),
    };
    let job = Job::create(new_job, &Ulid::generate().to_string(), Utc::now());
    store.put(&job).await?;
    http::json(201, &job)
}
