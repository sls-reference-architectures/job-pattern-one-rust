//! DynamoDB persistence for jobs. Table key: `id` (S). Updates are revision-conditional.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::operation::update_item::UpdateItemError;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnValue, ReturnValuesOnConditionCheckFailure};
use lambda_runtime::Diagnostic;
use serde_json::Value;

use crate::domain::job::{Job, StatusChange};

type Item = HashMap<String, AttributeValue>;

const KEY: &str = "id";
const REVISION: &str = "revision";
/// Set at creation and never rewritten.
const IMMUTABLE: [&str; 2] = [KEY, "createdAt"];

#[derive(Debug)]
pub enum StoreError {
    NotFound { id: String },
    Conflict { stored: u64, attempted: u64 },
    Serialization(String),
    DynamoDb(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::NotFound { id } => write!(f, "No id found for {id}"),
            StoreError::Conflict { stored, attempted } => write!(
                f,
                "Conflict: Item in DB has revision [{stored}]. You are using revision [{attempted}]"
            ),
            StoreError::Serialization(message) => write!(f, "serialization failed: {message}"),
            StoreError::DynamoDb(message) => write!(f, "DynamoDB request failed: {message}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Error names surface in the workflow's `$.error.Error` when a status step fails.
impl From<StoreError> for Diagnostic {
    fn from(error: StoreError) -> Diagnostic {
        let error_type = match &error {
            StoreError::NotFound { .. } => "NotFoundError",
            StoreError::Conflict { .. } => "ConflictError",
            StoreError::Serialization(_) => "SerializationError",
            StoreError::DynamoDb(_) => "DynamoDbError",
        };
        Diagnostic {
            error_type: error_type.to_owned(),
            error_message: error.to_string(),
        }
    }
}

/// The pieces of a conditional `UpdateItem` that rewrites every mutable attribute of a job.
#[derive(Debug, PartialEq)]
pub struct UpdateRequest {
    pub update_expression: String,
    pub condition_expression: String,
    pub names: HashMap<String, String>,
    pub values: Item,
}

pub fn update_request(change: &StatusChange) -> Result<UpdateRequest, StoreError> {
    let item: Item = to_item(&change.job)?;
    let assignments: BTreeMap<String, AttributeValue> = item
        .into_iter()
        .filter(|(name, _)| !IMMUTABLE.contains(&name.as_str()))
        .collect();

    let mut names = HashMap::from([
        ("#key".to_owned(), KEY.to_owned()),
        ("#expectedRevision".to_owned(), REVISION.to_owned()),
    ]);
    let mut values = HashMap::from([(
        ":expectedRevision".to_owned(),
        AttributeValue::N(change.expected_revision.to_string()),
    )]);
    let mut clauses = Vec::with_capacity(assignments.len());
    for (index, (name, value)) in assignments.into_iter().enumerate() {
        clauses.push(format!("#a{index} = :a{index}"));
        names.insert(format!("#a{index}"), name);
        values.insert(format!(":a{index}"), value);
    }

    Ok(UpdateRequest {
        update_expression: format!("SET {}", clauses.join(", ")),
        condition_expression: "attribute_exists(#key) AND #expectedRevision = :expectedRevision".to_owned(),
        names,
        values,
    })
}

/// A failed revision condition means either the job does not exist or someone else changed it.
/// `stored` is the item as it was when the condition was evaluated, if any.
pub fn condition_failure(change: &StatusChange, stored: Option<&Item>) -> StoreError {
    let stored_revision = stored
        .and_then(|item| item.get(REVISION))
        .and_then(|value| value.as_n().ok())
        .and_then(|number| number.parse::<u64>().ok());
    match (stored, stored_revision) {
        (None, _) => StoreError::NotFound {
            id: change.job.id.clone(),
        },
        (Some(_), revision) => StoreError::Conflict {
            stored: revision.unwrap_or_default(),
            attempted: change.expected_revision,
        },
    }
}

#[derive(Clone, Debug)]
pub struct JobStore {
    client: Client,
    table: String,
}

impl JobStore {
    pub fn new(client: Client, table: impl Into<String>) -> Self {
        Self {
            client,
            table: table.into(),
        }
    }

    pub async fn put(&self, job: &Job) -> Result<(), StoreError> {
        self.client
            .put_item()
            .table_name(&self.table)
            .set_item(Some(to_item(job)?))
            .send()
            .await
            .map_err(|error| StoreError::DynamoDb(error.into_service_error().to_string()))?;
        Ok(())
    }

    /// Returns the stored job exactly as persisted (every attribute), or `None`.
    pub async fn find(&self, id: &str) -> Result<Option<Value>, StoreError> {
        let output = self
            .client
            .get_item()
            .table_name(&self.table)
            .key(KEY, AttributeValue::S(id.to_owned()))
            .send()
            .await
            .map_err(|error| StoreError::DynamoDb(error.into_service_error().to_string()))?;
        output.item.map(from_item).transpose()
    }

    /// Applies the change only if the stored job still has `expected_revision`; returns the job
    /// as stored after the change.
    pub async fn update(&self, change: &StatusChange) -> Result<Job, StoreError> {
        let request = update_request(change)?;
        let result = self
            .client
            .update_item()
            .table_name(&self.table)
            .key(KEY, AttributeValue::S(change.job.id.clone()))
            .update_expression(request.update_expression)
            .condition_expression(request.condition_expression)
            .set_expression_attribute_names(Some(request.names))
            .set_expression_attribute_values(Some(request.values))
            .return_values(ReturnValue::AllNew)
            .return_values_on_condition_check_failure(ReturnValuesOnConditionCheckFailure::AllOld)
            .send()
            .await;
        match result {
            Ok(output) => from_item(output.attributes.unwrap_or_default()),
            Err(error) => match error.into_service_error() {
                UpdateItemError::ConditionalCheckFailedException(failure) => {
                    Err(condition_failure(change, failure.item()))
                }
                other => Err(StoreError::DynamoDb(other.to_string())),
            },
        }
    }
}

fn to_item(job: &Job) -> Result<Item, StoreError> {
    serde_dynamo::to_item(job).map_err(|error| StoreError::Serialization(error.to_string()))
}

fn from_item<T: serde::de::DeserializeOwned>(item: Item) -> Result<T, StoreError> {
    serde_dynamo::from_item(item).map_err(|error| StoreError::Serialization(error.to_string()))
}
