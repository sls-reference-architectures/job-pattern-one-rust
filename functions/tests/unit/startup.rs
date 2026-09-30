use jobs::runtime::startup_line;

#[test]
fn startup_phases_are_logged_on_one_parseable_line() {
    // ARRANGE
    let phases = [("tracing", 0.123), ("aws_config", 4.5678), ("clients", 0.9)];

    // ACT
    let line = startup_line(&phases, 5.59);

    // ASSERT
    assert_eq!(
        line,
        "startup tracing=0.12ms aws_config=4.57ms clients=0.90ms in_main=5.59ms"
    );
}

#[test]
fn a_function_without_aws_clients_logs_only_its_phases() {
    // ARRANGE
    let phases = [("tracing", 0.2)];

    // ACT
    let line = startup_line(&phases, 0.2);

    // ASSERT
    assert_eq!(line, "startup tracing=0.20ms in_main=0.20ms");
}
