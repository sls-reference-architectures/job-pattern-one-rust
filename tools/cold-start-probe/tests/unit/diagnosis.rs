use probe::diagnosis::{LogLine, Served, describe, explain};

const UPDATED_AT: i64 = 1_000_000;

fn line(timestamp_ms: i64, message: &str) -> LogLine {
    LogLine {
        timestamp_ms,
        message: message.to_owned(),
    }
}

#[test]
fn an_environment_that_served_earlier_invocations_was_reused() {
    // ARRANGE
    let stream = vec![
        line(
            UPDATED_AT - 4_000,
            "INIT_START Runtime Version: provided:al2023.v120",
        ),
        line(UPDATED_AT - 3_900, "START RequestId: first Version: $LATEST"),
        line(
            UPDATED_AT - 3_890,
            "REPORT RequestId: first\tDuration: 1.00 ms\tInit Duration: 60.00 ms\t",
        ),
        line(UPDATED_AT - 1_900, "START RequestId: second Version: $LATEST"),
        line(UPDATED_AT + 200, "START RequestId: warm Version: $LATEST"),
    ];

    // ACT
    let served = explain(&stream, "warm", UPDATED_AT);

    // ASSERT
    assert_eq!(
        served,
        Served::Reused {
            earlier_invocations: 2,
            initialized_at_ms: Some(-4_000),
        }
    );
}

#[test]
fn a_first_invocation_after_an_earlier_init_was_proactively_initialized() {
    // ARRANGE
    let stream = vec![
        line(
            UPDATED_AT + 300,
            "INIT_START Runtime Version: provided:al2023.v120",
        ),
        line(UPDATED_AT + 1_250, "START RequestId: warm Version: $LATEST"),
        line(UPDATED_AT + 1_260, "REPORT RequestId: warm\tDuration: 1.00 ms\t"),
    ];

    // ACT
    let served = explain(&stream, "warm", UPDATED_AT);

    // ASSERT
    assert_eq!(
        served,
        Served::PreInitialized {
            initialized_at_ms: 300,
            invoked_at_ms: 1_250,
        }
    );
}

#[test]
fn application_lines_that_mention_start_are_not_invocations() {
    // ARRANGE
    let stream = vec![
        line(UPDATED_AT + 100, "INIT_START Runtime Version: nodejs:24.v12"),
        line(
            UPDATED_AT + 400,
            "2026-10-19T13:59:03.180Z\tundefined\tINFO\tSTART RequestId: elsewhere",
        ),
        line(UPDATED_AT + 900, "START RequestId: warm Version: $LATEST"),
    ];

    // ACT
    let served = explain(&stream, "warm", UPDATED_AT);

    // ASSERT
    assert_eq!(
        served,
        Served::PreInitialized {
            initialized_at_ms: 100,
            invoked_at_ms: 900,
        }
    );
}

#[test]
fn a_stream_without_the_request_is_unknown() {
    // ARRANGE
    let stream = vec![line(UPDATED_AT, "START RequestId: other Version: $LATEST")];

    // ACT
    let served = explain(&stream, "warm", UPDATED_AT);

    // ASSERT
    assert_eq!(served, Served::Unknown);
}

#[test]
fn a_reused_environment_is_described_with_its_history() {
    // ARRANGE
    let served = Served::Reused {
        earlier_invocations: 3,
        initialized_at_ms: Some(-6_250),
    };

    // ACT
    let described = describe(&served);

    // ASSERT
    assert_eq!(
        described,
        "reused an environment that had served 3 earlier invocation(s), initialised at -6.2 s relative to the nonce update"
    );
}

#[test]
fn a_proactive_initialization_is_described_with_its_timing() {
    // ARRANGE
    let served = Served::PreInitialized {
        initialized_at_ms: 300,
        invoked_at_ms: 1_250,
    };

    // ACT
    let described = describe(&served);

    // ASSERT
    assert_eq!(
        described,
        "first invocation of an environment initialised proactively at +0.3 s, invoked at +1.2 s relative to the nonce update"
    );
}
