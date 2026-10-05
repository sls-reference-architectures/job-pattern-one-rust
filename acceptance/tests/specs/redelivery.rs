use acceptance::dsl::{Caller, Found, JobProcessing, Redelivery, status_of, unique_name};

#[tokio::test]
async fn hearing_twice_that_a_job_was_created_does_not_disturb_it() {
    // ARRANGE
    let caller = Caller::identified().await.unwrap();
    let job = caller
        .has_requested_job(&unique_name("redelivered"), "one two three")
        .await
        .unwrap();
    let completed = caller.follows_until_finished(&job.id).await.unwrap();
    let processing = JobProcessing::connect().await.unwrap();

    // ACT
    let redelivery = processing.hears_again_that_job_was_created(&job).await.unwrap();

    // ASSERT
    assert_eq!(redelivery, Redelivery::Ignored);
    assert_eq!(status_of(&completed), "Complete");
    let Found::Job(after) = caller.looks_up(&job.id).await.unwrap() else {
        panic!("job {} disappeared", job.id);
    };
    assert_eq!(after.record, completed.record);
}
