//! Pure building blocks for the cold-start probe; `main.rs` wires them to AWS.

pub mod dashboard;
pub mod payloads;
pub mod report;

use std::collections::HashMap;

/// CloudWatch namespace every probe metric is published under.
pub const NAMESPACE: &str = "SlsRa/ColdStart";

/// Environment variable whose change forces Lambda to discard warm execution environments.
pub const NONCE: &str = "PROBE_NONCE";

/// Whether a function's environment reflects the nonce update just made (`None`: removed).
/// GetFunction can briefly return the configuration from before that update, still marked
/// `Successful` from the previous one; invoking then reuses a warm environment.
pub fn carries_nonce(variables: Option<&HashMap<String, String>>, nonce: Option<&str>) -> bool {
    variables
        .and_then(|variables| variables.get(NONCE))
        .map(String::as_str)
        == nonce
}

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
