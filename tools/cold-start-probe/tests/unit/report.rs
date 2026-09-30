use std::collections::BTreeMap;

use probe::report::{Results, Sample, Series, markdown_summary, parse_report, percentile};

const COLD: &str = "START RequestId: abc Version: $LATEST\nhello\nEND RequestId: abc\nREPORT RequestId: abc\tDuration: 12.34 ms\tBilled Duration: 80 ms\tMemory Size: 1024 MB\tMax Memory Used: 33 MB\tInit Duration: 66.25 ms\t\n";
const WARM: &str = "REPORT RequestId: abc\tDuration: 2.10 ms\tBilled Duration: 3 ms\tMemory Size: 1024 MB\tMax Memory Used: 35 MB\t\n";

fn sample(init_ms: f64) -> Sample {
    Sample {
        init_ms,
        duration_ms: init_ms / 10.0,
        max_memory_mb: 40.0,
    }
}

#[test]
fn a_cold_report_line_yields_init_duration_and_memory() {
    // ARRANGE
    let log = COLD;

    // ACT
    let parsed = parse_report(log);

    // ASSERT
    assert_eq!(
        parsed,
        Some(Sample {
            init_ms: 66.25,
            duration_ms: 12.34,
            max_memory_mb: 33.0
        })
    );
}

#[test]
fn duration_is_not_confused_with_billed_or_init_duration() {
    // ARRANGE
    let log = "REPORT RequestId: x\tInit Duration: 500.00 ms\tBilled Duration: 900 ms\tDuration: 7.50 ms\tMax Memory Used: 90 MB\t";

    // ACT
    let parsed = parse_report(log).unwrap();

    // ASSERT
    assert_eq!(parsed.duration_ms, 7.5);
    assert_eq!(parsed.init_ms, 500.0);
}

#[test]
fn a_warm_report_line_is_not_a_sample() {
    // ARRANGE
    let log = WARM;

    // ACT
    let parsed = parse_report(log);

    // ASSERT
    assert_eq!(parsed, None);
}

#[test]
fn a_log_without_a_report_line_is_not_a_sample() {
    // ARRANGE
    let log = "START RequestId: abc\nsomething went wrong\n";

    // ACT
    let parsed = parse_report(log);

    // ASSERT
    assert_eq!(parsed, None);
}

#[test]
fn percentiles_use_the_nearest_rank() {
    // ARRANGE
    let values = [50.0, 10.0, 40.0, 20.0, 30.0, 60.0, 70.0, 80.0, 90.0, 100.0];

    // ACT
    let quantiles = (
        percentile(&values, 50.0),
        percentile(&values, 95.0),
        percentile(&values, 0.0),
    );

    // ASSERT
    assert_eq!(quantiles, (Some(50.0), Some(100.0), Some(10.0)));
}

#[test]
fn the_percentile_of_nothing_is_nothing() {
    // ARRANGE
    let values: [f64; 0] = [];

    // ACT
    let median = percentile(&values, 50.0);

    // ASSERT
    assert_eq!(median, None);
}

#[test]
fn the_summary_compares_runtimes_per_function_and_overall() {
    // ARRANGE
    let mut results: Results = BTreeMap::new();
    results.entry("getJob".to_owned()).or_default().insert(
        "nodejs24.x".to_owned(),
        Series {
            samples: vec![sample(400.0), sample(420.0)],
        },
    );
    results.entry("getJob".to_owned()).or_default().insert(
        "rust".to_owned(),
        Series {
            samples: vec![sample(40.0), sample(60.0)],
        },
    );
    let runtimes = vec!["nodejs24.x".to_owned(), "rust".to_owned()];

    // ACT
    let summary = markdown_summary(&results, &runtimes);

    // ASSERT
    let lines: Vec<&str> = summary.lines().collect();
    assert_eq!(
        lines[0],
        "| Function | nodejs24.x init p50 | nodejs24.x init p95 | nodejs24.x first invoke p50 | nodejs24.x n | rust init p50 | rust init p95 | rust first invoke p50 | rust n |"
    );
    assert_eq!(lines[2], "| getJob | 400 | 420 | 40 | 2 | 40 | 60 | 4 | 2 |");
    assert_eq!(
        lines[3],
        "| **all functions** | **400** | **420** | 40 | 2 | **40** | **60** | 4 | 2 |"
    );
}

#[test]
fn a_runtime_without_samples_shows_dashes() {
    // ARRANGE
    let mut results: Results = BTreeMap::new();
    results.entry("getJob".to_owned()).or_default().insert(
        "rust".to_owned(),
        Series {
            samples: vec![sample(50.0)],
        },
    );
    let runtimes = vec!["nodejs24.x".to_owned(), "rust".to_owned()];

    // ACT
    let summary = markdown_summary(&results, &runtimes);

    // ASSERT
    assert!(
        summary.contains("| getJob | – | – | – | 0 | 50 | 50 | 5 | 1 |"),
        "{summary}"
    );
}
