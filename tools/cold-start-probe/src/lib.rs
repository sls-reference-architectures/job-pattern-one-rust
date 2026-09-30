//! Pure building blocks for the cold-start probe; `main.rs` wires them to AWS.

pub mod dashboard;
pub mod payloads;
pub mod report;

/// CloudWatch namespace every probe metric is published under.
pub const NAMESPACE: &str = "SlsRa/ColdStart";

/// The deployed stack under test and the runtime label its samples are recorded with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub stack: String,
    pub runtime: String,
}

/// Parses `stack=runtime` pairs, e.g. `job-pattern-one-dev=nodejs24.x`.
pub fn parse_target(argument: &str) -> Result<Target, String> {
    match argument.split_once('=') {
        Some((stack, runtime)) if !stack.is_empty() && !runtime.is_empty() => Ok(Target {
            stack: stack.to_owned(),
            runtime: runtime.to_owned(),
        }),
        _ => Err(format!("expected STACK=RUNTIME, got {argument:?}")),
    }
}

/// Serverless Framework names functions `<service>-<stage>-<functionKey>`, i.e. `<stack>-<key>`.
pub fn function_key(stack: &str, function_name: &str) -> String {
    function_name
        .strip_prefix(stack)
        .and_then(|rest| rest.strip_prefix('-'))
        .unwrap_or(function_name)
        .to_owned()
}
