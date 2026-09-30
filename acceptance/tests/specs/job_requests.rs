use acceptance::dsl::{Caller, Found, Request, status_of, unique_name};

#[tokio::test]
async fn a_requested_job_starts_out_pending() {
    // ARRANGE
    let caller = Caller::identified().await.unwrap();
    let name = unique_name("pending");

    // ACT
    let request = caller.requests_job(&name, "one two three").await.unwrap();

    // ASSERT
    let Request::Accepted(job) = request else {
        panic!("expected the job to be accepted, got {request:?}");
    };
    assert_eq!(job.name, name);
    assert_eq!(job.phrase, "one two three");
    assert_eq!(status_of(&job), "Pending");
    assert!(job.id.starts_with("JOB_"), "unexpected id {}", job.id);
}

#[tokio::test]
async fn a_requested_job_is_eventually_completed_with_its_phrase_reversed() {
    // ARRANGE
    let caller = Caller::identified().await.unwrap();
    let job = caller
        .has_requested_job(&unique_name("completes"), "one two three")
        .await
        .unwrap();

    // ACT
    let finished = caller.follows_until_finished(&job.id).await.unwrap();

    // ASSERT
    assert_eq!(status_of(&finished), "Complete");
    assert_eq!(finished.translated_phrase.as_deref(), Some("eerht owt eno"));
}

#[tokio::test]
async fn a_requested_job_can_be_looked_up() {
    // ARRANGE
    let caller = Caller::identified().await.unwrap();
    let name = unique_name("lookup");
    let job = caller.has_requested_job(&name, "look me up").await.unwrap();

    // ACT
    let found = caller.looks_up(&job.id).await.unwrap();

    // ASSERT
    let Found::Job(found) = found else {
        panic!("expected to find job {}, got {found:?}", job.id);
    };
    assert_eq!(found.id, job.id);
    assert_eq!(found.name, name);
    assert_eq!(found.phrase, "look me up");
}

#[tokio::test]
async fn looking_up_an_unknown_job_finds_nothing() {
    // ARRANGE
    let caller = Caller::identified().await.unwrap();

    // ACT
    let found = caller.looks_up(&unique_name("never-requested")).await.unwrap();

    // ASSERT
    assert_eq!(found, Found::Nothing);
}

#[tokio::test]
async fn anonymous_callers_cannot_request_jobs() {
    // ARRANGE
    let stranger = Caller::anonymous().await.unwrap();

    // ACT
    let request = stranger.requests_job("anonymous", "let me in").await.unwrap();

    // ASSERT
    assert_eq!(request, Request::Refused);
}

#[tokio::test]
async fn anonymous_callers_cannot_look_up_jobs() {
    // ARRANGE
    let stranger = Caller::anonymous().await.unwrap();

    // ACT
    let found = stranger.looks_up("JOB_anything").await.unwrap();

    // ASSERT
    assert_eq!(found, Found::Refused);
}
