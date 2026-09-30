//! Translation between API Gateway (HTTP API, payload v2) requests/responses and the domain.

use lambda_http::http::header::CONTENT_TYPE;
use lambda_http::{Body, Error, Request, Response};
use serde::Serialize;

use crate::domain::request::RequestError;

pub fn content_type(request: &Request) -> Option<&str> {
    request
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
}

pub fn body_text(request: &Request) -> Option<&str> {
    match request.body() {
        Body::Empty => None,
        Body::Text(text) => Some(text.as_str()),
        Body::Binary(bytes) => std::str::from_utf8(bytes).ok(),
        _ => None,
    }
}

pub fn json(status: u16, payload: &impl Serialize) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::Text(serde_json::to_string(payload)?))?)
}

pub fn error(failure: RequestError) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(failure.status)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::Text(failure.message))?)
}
