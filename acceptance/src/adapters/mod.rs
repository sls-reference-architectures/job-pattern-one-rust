pub mod http;
pub mod stack;

use serde::Deserialize;

/// A job as the protocol layer reports it, stripped to what the domain cares about.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: String,
    pub name: String,
    pub phrase: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub translated_phrase: Option<String>,
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
