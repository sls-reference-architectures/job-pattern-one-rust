//! Validation of incoming HTTP requests and the errors callers see.

use serde_json::Value;

use super::job::NewJob;

/// A caller-facing failure: an HTTP status plus a plain-text message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestError {
    pub status: u16,
    pub message: String,
}

impl RequestError {
    pub fn unsupported_media_type() -> Self {
        Self::new(415, "Unsupported Media Type")
    }

    pub fn malformed_json() -> Self {
        Self::new(422, "Invalid or malformed JSON was provided")
    }

    pub fn missing_fields() -> Self {
        Self::new(400, "name and phrase are required and must be strings")
    }

    pub fn job_not_found(id: &str) -> Self {
        Self::new(404, &format!("No id found for {id}"))
    }

    fn new(status: u16, message: &str) -> Self {
        Self {
            status,
            message: message.to_owned(),
        }
    }
}

/// Accepts `application/json` and structured-syntax suffixes such as `application/vnd.x+json`,
/// with or without parameters (`; charset=utf-8`).
pub fn is_json_content_type(content_type: &str) -> bool {
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match media_type.strip_prefix("application/") {
        Some("json") => true,
        Some(subtype) => subtype.len() > "+json".len() && subtype.ends_with("+json"),
        None => false,
    }
}

pub fn parse_new_job(content_type: Option<&str>, body: Option<&str>) -> Result<NewJob, RequestError> {
    if !content_type.is_some_and(is_json_content_type) {
        return Err(RequestError::unsupported_media_type());
    }
    let body = body
        .filter(|text| !text.is_empty())
        .ok_or_else(RequestError::malformed_json)?;
    let document: Value = serde_json::from_str(body).map_err(|_| RequestError::malformed_json())?;
    let text_field = |field: &str| document.get(field).and_then(Value::as_str).map(str::to_owned);
    match (text_field("name"), text_field("phrase")) {
        (Some(name), Some(phrase)) => Ok(NewJob { name, phrase }),
        _ => Err(RequestError::missing_fields()),
    }
}
