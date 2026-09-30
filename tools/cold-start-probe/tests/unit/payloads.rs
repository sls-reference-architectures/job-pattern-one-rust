use probe::payloads::{ABSENT_JOB_ID, probe_for};

const FUNCTIONS: [&str; 8] = [
    "createJob",
    "getJob",
    "onDbStreamEvent",
    "onJobCreated",
    "setJobStatusStarted",
    "translatePhrase",
    "setJobStatusComplete",
    "setJobStatusFailed",
];

#[test]
fn every_function_of_the_pattern_has_a_probe() {
    // ARRANGE
    let functions = FUNCTIONS;

    // ACT
    let missing: Vec<&str> = functions
        .into_iter()
        .filter(|key| probe_for(key).is_none())
        .collect();

    // ASSERT
    assert!(missing.is_empty(), "{missing:?}");
}

#[test]
fn unknown_functions_are_not_probed() {
    // ARRANGE
    let key = "CustomResourceHandler";

    // ACT
    let probe = probe_for(key);

    // ASSERT
    assert_eq!(probe, None);
}

#[test]
fn creating_a_job_is_probed_with_a_request_that_is_refused_before_any_write() {
    // ARRANGE
    let key = "createJob";

    // ACT
    let probe = probe_for(key).unwrap();

    // ASSERT
    assert_eq!(probe.payload["headers"]["content-type"], "text/plain");
    assert_eq!(probe.payload["requestContext"]["http"]["method"], "POST");
    assert_eq!(probe.payload["routeKey"], "POST /jobs");
}

#[test]
fn looking_up_a_job_is_probed_with_a_job_that_does_not_exist() {
    // ARRANGE
    let key = "getJob";

    // ACT
    let probe = probe_for(key).unwrap();

    // ASSERT
    assert_eq!(probe.payload["pathParameters"]["jobId"], ABSENT_JOB_ID);
    assert_eq!(probe.payload["version"], "2.0");
}

#[test]
fn status_steps_are_probed_with_a_job_that_does_not_exist() {
    // ARRANGE
    let keys = [
        "setJobStatusStarted",
        "setJobStatusComplete",
        "setJobStatusFailed",
    ];

    // ACT
    let ids: Vec<String> = keys
        .iter()
        .map(|key| probe_for(key).unwrap().payload["id"].as_str().unwrap().to_owned())
        .collect();

    // ASSERT
    assert!(ids.iter().all(|id| id == ABSENT_JOB_ID));
}

#[test]
fn the_stream_is_probed_with_a_removal_so_only_an_unconsumed_delete_event_is_published() {
    // ARRANGE
    let key = "onDbStreamEvent";

    // ACT
    let probe = probe_for(key).unwrap();

    // ASSERT
    assert_eq!(probe.payload["Records"][0]["eventName"], "REMOVE");
}

#[test]
fn starting_the_workflow_is_probed_without_a_job_and_compared_on_init_only() {
    // ARRANGE
    let key = "onJobCreated";

    // ACT
    let probe = probe_for(key).unwrap();

    // ASSERT
    assert!(probe.payload.get("detail").is_none());
    assert!(!probe.comparable_first_invoke);
}
