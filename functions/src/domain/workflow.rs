/// Step Functions execution names must be unique per state machine; the job id plus the start
/// instant (epoch milliseconds) is.
pub fn execution_name(job_id: &str, epoch_millis: i64) -> String {
    format!("{job_id}-{epoch_millis}")
}
