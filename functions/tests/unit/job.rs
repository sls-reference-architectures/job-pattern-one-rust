use chrono::{DateTime, TimeZone, Utc};
use jobs::domain::job::{Job, JobStatus, NewJob, TIME_TO_LIVE_SECONDS, iso_timestamp};

fn instant(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0).unwrap()
}

fn created_job() -> Job {
    Job::create(
        NewJob {
            name: "greeting".to_owned(),
            phrase: "one two three".to_owned(),
        },
        "01J00000000000000000000000",
        instant(1_790_000_000),
    )
}

#[test]
fn a_new_job_is_pending_at_revision_one_with_a_prefixed_id() {
    // ARRANGE
    let request = NewJob {
        name: "greeting".to_owned(),
        phrase: "one two three".to_owned(),
    };

    // ACT
    let job = Job::create(request, "01J00000000000000000000000", instant(1_790_000_000));

    // ASSERT
    assert_eq!(job.id, "JOB_01J00000000000000000000000");
    assert_eq!(job.name, "greeting");
    assert_eq!(job.phrase, "one two three");
    assert_eq!(job.status, JobStatus::Pending);
    assert_eq!(job.revision, 1);
    assert_eq!(job.translated_phrase, None);
    assert_eq!(job.error, None);
}

#[test]
fn a_new_job_expires_five_days_after_creation() {
    // ARRANGE
    let now = instant(1_790_000_000);

    // ACT
    let job = Job::create(
        NewJob {
            name: "n".to_owned(),
            phrase: "p".to_owned(),
        },
        "U",
        now,
    );

    // ASSERT
    assert_eq!(TIME_TO_LIVE_SECONDS, 432_000);
    assert_eq!(job.ttl, 1_790_000_000 + 432_000);
}

#[test]
fn a_new_job_has_identical_created_and_updated_timestamps() {
    // ARRANGE
    let now = instant(1_790_000_000);

    // ACT
    let job = Job::create(
        NewJob {
            name: "n".to_owned(),
            phrase: "p".to_owned(),
        },
        "U",
        now,
    );

    // ASSERT
    assert_eq!(job.created_at, "2026-09-21T14:13:20.000Z");
    assert_eq!(job.updated_at, job.created_at);
}

#[test]
fn timestamps_are_iso_8601_utc_with_milliseconds() {
    // ARRANGE
    let now = Utc.timestamp_millis_opt(1_790_000_000_123).unwrap();

    // ACT
    let text = iso_timestamp(now);

    // ASSERT
    assert_eq!(text, "2026-09-21T14:13:20.123Z");
}

#[test]
fn changing_status_bumps_the_revision_and_expects_the_prior_one() {
    // ARRANGE
    let job = created_job();

    // ACT
    let change = job.change_status(JobStatus::Started, instant(1_790_000_060));

    // ASSERT
    assert_eq!(change.expected_revision, 1);
    assert_eq!(change.job.revision, 2);
    assert_eq!(change.job.status, JobStatus::Started);
}

#[test]
fn changing_status_refreshes_updated_at_but_keeps_created_at() {
    // ARRANGE
    let job = created_job();

    // ACT
    let change = job.change_status(JobStatus::Complete, instant(1_790_000_060));

    // ASSERT
    assert_eq!(change.job.created_at, "2026-09-21T14:13:20.000Z");
    assert_eq!(change.job.updated_at, "2026-09-21T14:14:20.000Z");
}

#[test]
fn changing_status_keeps_workflow_results() {
    // ARRANGE
    let mut job = created_job();
    job.translated_phrase = Some("eerht owt eno".to_owned());
    job.error = Some(serde_json::json!({ "Error": "Boom", "Cause": "because" }));

    // ACT
    let change = job.change_status(JobStatus::Failed, instant(1_790_000_060));

    // ASSERT
    assert_eq!(change.job.translated_phrase.as_deref(), Some("eerht owt eno"));
    assert_eq!(
        change.job.error,
        Some(serde_json::json!({ "Error": "Boom", "Cause": "because" }))
    );
}

#[test]
fn a_job_serializes_with_camel_case_fields_and_omits_absent_results() {
    // ARRANGE
    let job = created_job();

    // ACT
    let json = serde_json::to_value(&job).unwrap();

    // ASSERT
    assert_eq!(
        json,
        serde_json::json!({
            "id": "JOB_01J00000000000000000000000",
            "name": "greeting",
            "phrase": "one two three",
            "status": "Pending",
            "ttl": 1_790_432_000,
            "createdAt": "2026-09-21T14:13:20.000Z",
            "updatedAt": "2026-09-21T14:13:20.000Z",
            "revision": 1
        })
    );
}

#[test]
fn a_workflow_state_with_results_deserializes_into_a_job() {
    // ARRANGE
    let state = serde_json::json!({
        "id": "JOB_X", "name": "n", "phrase": "abc", "status": "Started", "ttl": 1,
        "createdAt": "c", "updatedAt": "u", "revision": 2, "translatedPhrase": "cba",
        "error": { "Error": "States.TaskFailed", "Cause": "x" }
    });

    // ACT
    let job: Job = serde_json::from_value(state).unwrap();

    // ASSERT
    assert_eq!(job.status, JobStatus::Started);
    assert_eq!(job.revision, 2);
    assert_eq!(job.translated_phrase.as_deref(), Some("cba"));
    assert_eq!(job.error.unwrap()["Error"], "States.TaskFailed");
}
