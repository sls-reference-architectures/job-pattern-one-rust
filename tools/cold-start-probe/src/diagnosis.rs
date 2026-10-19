//! Explaining a forced cold start that came back warm, from the log stream of the execution
//! environment that served it. Each environment writes to its own stream.

/// One line of a function's log stream.
#[derive(Clone, Debug, PartialEq)]
pub struct LogLine {
    pub timestamp_ms: i64,
    pub message: String,
}

/// Which environment served the warm invocation. Offsets are relative to the moment the probe
/// saw its nonce update as `Successful`.
#[derive(Clone, Debug, PartialEq)]
pub enum Served {
    /// The environment had already handled invocations: Lambda routed this one to an existing
    /// environment, which ought to have been discarded by the nonce update.
    Reused {
        earlier_invocations: usize,
        initialized_at_ms: Option<i64>,
    },
    /// This was the environment's first invocation, but it was initialised before the invoke
    /// reached it: Lambda initialised it proactively, so the REPORT carries no Init Duration.
    PreInitialized {
        initialized_at_ms: i64,
        invoked_at_ms: i64,
    },
    /// The stream does not show how this invocation was served.
    Unknown,
}

fn is_start_of(line: &LogLine, request_id: &str) -> bool {
    line.message
        .strip_prefix("START RequestId: ")
        .and_then(|rest| rest.split_whitespace().next())
        == Some(request_id)
}

pub fn explain(stream: &[LogLine], request_id: &str, updated_at_ms: i64) -> Served {
    let Some(position) = stream.iter().position(|line| is_start_of(line, request_id)) else {
        return Served::Unknown;
    };
    let before = &stream[..position];
    let initialized_at_ms = before
        .iter()
        .find(|line| line.message.starts_with("INIT_START"))
        .map(|line| line.timestamp_ms - updated_at_ms);
    let earlier_invocations = before
        .iter()
        .filter(|line| line.message.starts_with("START RequestId: "))
        .count();
    match (earlier_invocations, initialized_at_ms) {
        (0, Some(initialized_at_ms)) => Served::PreInitialized {
            initialized_at_ms,
            invoked_at_ms: stream[position].timestamp_ms - updated_at_ms,
        },
        (0, None) => Served::Unknown,
        (earlier_invocations, initialized_at_ms) => Served::Reused {
            earlier_invocations,
            initialized_at_ms,
        },
    }
}

fn seconds(offset_ms: i64) -> String {
    format!("{:+.1} s", offset_ms as f64 / 1000.0)
}

/// One line for the workflow log.
pub fn describe(served: &Served) -> String {
    match served {
        Served::Reused {
            earlier_invocations,
            initialized_at_ms,
        } => {
            let since = initialized_at_ms.map_or_else(String::new, |offset| {
                format!(
                    ", initialised at {} relative to the nonce update",
                    seconds(offset)
                )
            });
            format!(
                "reused an environment that had served {earlier_invocations} earlier invocation(s){since}"
            )
        }
        Served::PreInitialized {
            initialized_at_ms,
            invoked_at_ms,
        } => format!(
            "first invocation of an environment initialised proactively at {}, invoked at {} relative to the nonce update",
            seconds(*initialized_at_ms),
            seconds(*invoked_at_ms)
        ),
        Served::Unknown => "the log stream does not show how it was served".to_owned(),
    }
}
