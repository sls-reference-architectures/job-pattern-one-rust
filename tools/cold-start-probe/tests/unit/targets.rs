use probe::{Target, function_key, parse_target};

#[test]
fn a_target_names_a_stack_and_its_runtime_label() {
    // ARRANGE
    let argument = "job-pattern-one-rust-dev=rust";

    // ACT
    let target = parse_target(argument);

    // ASSERT
    assert_eq!(
        target,
        Ok(Target {
            stack: "job-pattern-one-rust-dev".to_owned(),
            runtime: "rust".to_owned()
        })
    );
}

#[test]
fn a_target_without_both_parts_is_rejected() {
    // ARRANGE
    let arguments = ["job-pattern-one-dev", "=rust", "stack="];

    // ACT
    let results: Vec<_> = arguments.iter().map(|argument| parse_target(argument)).collect();

    // ASSERT
    assert!(results.iter().all(Result::is_err));
}

#[test]
fn function_keys_match_across_sibling_stacks() {
    // ARRANGE
    let node = ("job-pattern-one-dev", "job-pattern-one-dev-setJobStatusStarted");
    let rust = (
        "job-pattern-one-rust-dev",
        "job-pattern-one-rust-dev-setJobStatusStarted",
    );

    // ACT
    let keys = (function_key(node.0, node.1), function_key(rust.0, rust.1));

    // ASSERT
    assert_eq!(
        keys,
        ("setJobStatusStarted".to_owned(), "setJobStatusStarted".to_owned())
    );
}

#[test]
fn a_function_named_outside_the_convention_keeps_its_name() {
    // ARRANGE
    let stack = "job-pattern-one-dev";

    // ACT
    let key = function_key(stack, "SomeOtherFunction");

    // ASSERT
    assert_eq!(key, "SomeOtherFunction");
}
