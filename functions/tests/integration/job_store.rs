use std::time::Duration;

use chrono::Utc;
use jobs::adapters::job_store::{JobStore, StoreError};
use jobs::domain::job::{Job, JobStatus};

use crate::support::{aws_config, fresh_job, stack_output};

async fn store() -> JobStore {
    let config = aws_config().await;
    let table = stack_output(&config, "TableName").await;
    JobStore::new(aws_sdk_dynamodb::Client::new(&config), table)
}

async fn settled(store: &JobStore, id: &str) -> Job {
    for _ in 0..30 {
        let stored: Job = serde_json::from_value(store.find(id).await.unwrap().unwrap()).unwrap();
        if matches!(stored.status, JobStatus::Complete | JobStatus::Failed) {
            return stored;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    panic!("job {id} never finished its workflow");
}

#[tokio::test]
async fn a_stored_job_can_be_found_with_its_attributes() {
    // ARRANGE
    let store = store().await;
    let job = fresh_job("find");

    // ACT
    store.put(&job).await.unwrap();

    // ASSERT
    let found = store.find(&job.id).await.unwrap().unwrap();
    assert_eq!(found["id"], job.id);
    assert_eq!(found["name"], job.name);
    assert_eq!(found["phrase"], job.phrase);
    assert_eq!(found["createdAt"], job.created_at);
    assert_eq!(found["ttl"], job.ttl);
}

#[tokio::test]
async fn an_unknown_job_is_not_found() {
    // ARRANGE
    let store = store().await;

    // ACT
    let found = store.find("JOB_integration_never_stored").await.unwrap();

    // ASSERT
    assert_eq!(found, None);
}

#[tokio::test]
async fn a_status_change_at_the_current_revision_is_stored() {
    // ARRANGE
    let store = store().await;
    let job = fresh_job("update");
    store.put(&job).await.unwrap();
    let current = settled(&store, &job.id).await;

    // ACT
    let updated = store
        .update(&current.clone().change_status(JobStatus::Failed, Utc::now()))
        .await
        .unwrap();

    // ASSERT
    assert_eq!(updated.status, JobStatus::Failed);
    assert_eq!(updated.revision, current.revision + 1);
    assert_eq!(updated.created_at, job.created_at);
    let stored: Job = serde_json::from_value(store.find(&job.id).await.unwrap().unwrap()).unwrap();
    assert_eq!(stored, updated);
}

#[tokio::test]
async fn a_status_change_at_a_stale_revision_conflicts() {
    // ARRANGE
    let store = store().await;
    let job = fresh_job("conflict");
    store.put(&job).await.unwrap();
    let mut stale = job.clone().change_status(JobStatus::Started, Utc::now());
    stale.expected_revision = 999;

    // ACT
    let outcome = store.update(&stale).await;

    // ASSERT
    assert!(
        matches!(outcome, Err(StoreError::Conflict { attempted: 999, stored }) if stored >= 1),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn a_status_change_for_a_missing_job_is_not_found() {
    // ARRANGE
    let store = store().await;
    let never_stored = fresh_job("missing");

    // ACT
    let outcome = store
        .update(&never_stored.clone().change_status(JobStatus::Started, Utc::now()))
        .await;

    // ASSERT
    assert!(
        matches!(&outcome, Err(StoreError::NotFound { id }) if *id == never_stored.id),
        "{outcome:?}"
    );
}
