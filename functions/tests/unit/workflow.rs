use jobs::domain::workflow::execution_name;

#[test]
fn an_execution_is_named_after_the_job_and_start_instant() {
    // ARRANGE
    let job_id = "JOB_01J00000000000000000000000";

    // ACT
    let name = execution_name(job_id, 1_790_000_000_123);

    // ASSERT
    assert_eq!(name, "JOB_01J00000000000000000000000-1790000000123");
    assert!(name.len() <= 80);
}
