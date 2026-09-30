//! The jobs HTTP API (API Gateway HTTP API, IAM-authorized) as plain domain calls.

use std::time::{Duration, SystemTime};

use aws_credential_types::Credentials;
use aws_credential_types::provider::ProvideCredentials;
use aws_sigv4::http_request::{SignableBody, SignableRequest, SigningSettings, sign};
use aws_sigv4::sign::v4;
use reqwest::{Method, StatusCode};
use serde_json::json;

use super::{Error, JobView, stack};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Submission {
    Accepted(JobView),
    Refused,
    Rejected(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup {
    Found(JobView),
    Missing,
    Refused,
}

/// A freshly created HTTP API answers 404 for every route for a few seconds while routes and the
/// stage propagate (the Monday run after the weekly teardown is a cold create). An unsigned
/// request is refused (403) once routing works, so poll until the 404s stop.
async fn wait_until_routable(client: &reqwest::Client, base_url: &str) -> Result<(), Error> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    loop {
        let status = client
            .get(format!("{base_url}/jobs/readiness-check"))
            .send()
            .await
            .map(|response| response.status());
        match status {
            Ok(status) if status != StatusCode::NOT_FOUND => return Ok(()),
            _ if tokio::time::Instant::now() >= deadline => {
                return Err(format!("{base_url} still not routable after 90 s: {status:?}").into());
            }
            _ => tokio::time::sleep(Duration::from_secs(2)).await,
        }
    }
}

pub struct JobsHttpAdapter {
    base_url: String,
    region: String,
    credentials: Option<Credentials>,
    client: reqwest::Client,
}

impl JobsHttpAdapter {
    /// `signed`: whether requests carry the caller's AWS identity (SigV4) or are anonymous.
    pub async fn connect(signed: bool) -> Result<Self, Error> {
        let config = stack::aws_config().await;
        let base_url = stack::output(&config, &stack::stack_name(), "HttpApiUrl").await?;
        let region = config.region().map(ToString::to_string).unwrap_or_default();
        let credentials = if signed {
            let provider = config
                .credentials_provider()
                .ok_or("no AWS credentials provider configured")?;
            Some(provider.provide_credentials().await?)
        } else {
            None
        };
        let client = reqwest::Client::new();
        wait_until_routable(&client, &base_url).await?;
        Ok(Self {
            base_url,
            region,
            credentials,
            client,
        })
    }

    pub async fn submit(&self, name: &str, phrase: &str) -> Result<Submission, Error> {
        let body = json!({ "name": name, "phrase": phrase }).to_string();
        let response = self.send(Method::POST, "/jobs", Some(body)).await?;
        match response.status() {
            StatusCode::CREATED => Ok(Submission::Accepted(response.json().await?)),
            StatusCode::FORBIDDEN => Ok(Submission::Refused),
            other => Ok(Submission::Rejected(other.as_u16())),
        }
    }

    pub async fn fetch(&self, id: &str) -> Result<Lookup, Error> {
        let response = self.send(Method::GET, &format!("/jobs/{id}"), None).await?;
        match response.status() {
            StatusCode::OK => Ok(Lookup::Found(response.json().await?)),
            StatusCode::NOT_FOUND => Ok(Lookup::Missing),
            StatusCode::FORBIDDEN => Ok(Lookup::Refused),
            other => Err(format!("GET /jobs/{id} returned unexpected {other}").into()),
        }
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<reqwest::Response, Error> {
        let url = format!("{}{path}", self.base_url);
        let mut headers: Vec<(String, String)> = Vec::new();
        if body.is_some() {
            headers.push(("content-type".to_owned(), "application/json".to_owned()));
        }
        if let Some(credentials) = &self.credentials {
            headers.extend(self.signature(credentials, &method, &url, &headers, body.as_deref())?);
        }
        let mut request = self.client.request(method, &url);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        if let Some(body) = body {
            request = request.body(body);
        }
        Ok(request.send().await?)
    }

    fn signature(
        &self,
        credentials: &Credentials,
        method: &Method,
        url: &str,
        headers: &[(String, String)],
        body: Option<&str>,
    ) -> Result<Vec<(String, String)>, Error> {
        let identity = credentials.clone().into();
        let params = v4::SigningParams::builder()
            .identity(&identity)
            .region(&self.region)
            .name("execute-api")
            .time(SystemTime::now())
            .settings(SigningSettings::default())
            .build()?
            .into();
        let signable = SignableRequest::new(
            method.as_str(),
            url,
            headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
            SignableBody::Bytes(body.unwrap_or_default().as_bytes()),
        )?;
        let (instructions, _signature) = sign(signable, &params)?.into_parts();
        Ok(instructions
            .headers()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect())
    }
}
