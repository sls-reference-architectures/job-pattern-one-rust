//! Parsing Lambda REPORT lines and summarising samples.

use std::collections::BTreeMap;

/// One forced cold start, as reported by Lambda.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub init_ms: f64,
    pub duration_ms: f64,
    pub max_memory_mb: f64,
}

/// Extracts a sample from invocation log output (the tail returned by `Invoke` with
/// `LogType=Tail`). Returns `None` when no REPORT line carries an `Init Duration` (i.e. the
/// invocation was not a cold start).
pub fn parse_report(log: &str) -> Option<Sample> {
    let line = log.lines().rev().find(|line| line.starts_with("REPORT "))?;
    let field = |label: &str| -> Option<f64> {
        line.split('\t').find_map(|part| {
            let value = part.trim().strip_prefix(label)?.strip_prefix(':')?.trim();
            value.split_whitespace().next()?.parse().ok()
        })
    };
    Some(Sample {
        init_ms: field("Init Duration")?,
        duration_ms: field("Duration")?,
        max_memory_mb: field("Max Memory Used")?,
    })
}

/// Nearest-rank percentile (`p` in 0..=100). `None` for an empty slice.
pub fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = ((p / 100.0) * sorted.len() as f64).ceil().max(1.0) as usize;
    Some(sorted[rank.min(sorted.len()) - 1])
}

/// Samples for one function on one runtime.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Series {
    pub samples: Vec<Sample>,
}

impl Series {
    pub fn init(&self) -> Vec<f64> {
        self.samples.iter().map(|sample| sample.init_ms).collect()
    }

    pub fn duration(&self) -> Vec<f64> {
        self.samples.iter().map(|sample| sample.duration_ms).collect()
    }

    pub fn memory(&self) -> Vec<f64> {
        self.samples.iter().map(|sample| sample.max_memory_mb).collect()
    }
}

/// function key -> runtime label -> samples
pub type Results = BTreeMap<String, BTreeMap<String, Series>>;

/// A markdown comparison: one row per function, init p50/p95 and first-invoke p50 per runtime.
pub fn markdown_summary(results: &Results, runtimes: &[String]) -> String {
    let format = |value: Option<f64>| value.map_or_else(|| "–".to_owned(), |ms| format!("{ms:.0}"));
    let mut header = vec!["Function".to_owned()];
    for runtime in runtimes {
        header.push(format!("{runtime} init p50"));
        header.push(format!("{runtime} init p95"));
        header.push(format!("{runtime} first invoke p50"));
        header.push(format!("{runtime} n"));
    }
    let mut lines = vec![
        format!("| {} |", header.join(" | ")),
        format!("|{}", "---|".repeat(header.len())),
    ];
    let mut all: BTreeMap<&String, Series> = BTreeMap::new();
    for (function, by_runtime) in results {
        let mut row = vec![function.clone()];
        for runtime in runtimes {
            let series = by_runtime.get(runtime).cloned().unwrap_or_default();
            row.push(format(percentile(&series.init(), 50.0)));
            row.push(format(percentile(&series.init(), 95.0)));
            row.push(format(percentile(&series.duration(), 50.0)));
            row.push(series.samples.len().to_string());
            all.entry(runtime).or_default().samples.extend(series.samples);
        }
        lines.push(format!("| {} |", row.join(" | ")));
    }
    let mut total = vec!["**all functions**".to_owned()];
    for runtime in runtimes {
        let series = all.get(runtime).cloned().unwrap_or_default();
        total.push(format!("**{}**", format(percentile(&series.init(), 50.0))));
        total.push(format!("**{}**", format(percentile(&series.init(), 95.0))));
        total.push(format(percentile(&series.duration(), 50.0)));
        total.push(series.samples.len().to_string());
    }
    lines.push(format!("| {} |", total.join(" | ")));
    lines.join("\n") + "\n"
}
