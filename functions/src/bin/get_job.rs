//! GET /jobs/{jobId}: returns the job as stored (200) or 404.

use jobs::adapters::{http, job_store::JobStore};
use jobs::domain::request::RequestError;
use jobs::runtime;
use lambda_http::{Body, Error, Request, RequestExt, Response, run, service_fn};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    lambda_http::tracing::init_default_subscriber();
    let store = runtime::job_store().await?;
    run(service_fn(|request: Request| get_job(&store, request))).await
}

async fn get_job(store: &JobStore, request: Request) -> Result<Response<Body>, Error> {
    let id = request
        .path_parameters_ref()
        .and_then(|parameters| parameters.first("jobId"))
        .unwrap_or_default()
        .to_owned();
    match store.find(&id).await? {
        Some(job) => http::json(200, &job),
        None => http::error(RequestError::job_not_found(&id)),
    }
}
