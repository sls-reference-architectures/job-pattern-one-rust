//! The CloudWatch dashboard comparing runtimes. It lives outside both stacks so it survives the
//! weekly teardowns; the metrics it plots are retained by CloudWatch for 15 months.

use serde_json::{Value, json};

use crate::NAMESPACE;

pub const DASHBOARD_NAME: &str = "job-pattern-one-cold-starts";
pub const PATTERN: &str = "job-pattern-one";

/// `runtimes`: labels to plot; `log_groups`: Lambda log groups to scan for organic cold starts.
pub fn body(region: &str, runtimes: &[String], log_groups: &[String]) -> Value {
    let by_runtime = |metric: &str| -> Value {
        runtimes
            .iter()
            .map(|runtime| json!([NAMESPACE, metric, "Pattern", PATTERN, "Runtime", runtime, { "label": runtime }]))
            .collect()
    };
    let per_function = |metric: &str, stat: &str| -> Value {
        json!([[{
            "expression": format!(
                "SEARCH('{{{NAMESPACE},Pattern,Runtime,Function}} MetricName=\"{metric}\" Pattern=\"{PATTERN}\"', '{stat}', 86400)"
            ),
            "id": "e1",
            "label": ""
        }]])
    };
    let chart = |x: u32, y: u32, width: u32, title: &str, stat: &str, metrics: Value| {
        json!({
            "type": "metric",
            "x": x, "y": y, "width": width, "height": 7,
            "properties": {
                "title": title,
                "region": region,
                "view": "timeSeries",
                "stat": stat,
                "period": 86400,
                "metrics": metrics,
                "yAxis": { "left": { "min": 0 } }
            }
        })
    };
    let sources: Vec<String> = log_groups
        .iter()
        .map(|group| format!("SOURCE '{group}'"))
        .collect();
    let organic_query = format!(
        "{} | filter @type = \"REPORT\" and ispresent(@initDuration) | stats count() as coldStarts, pct(@initDuration, 50) as initP50, pct(@initDuration, 95) as initP95 by @log | sort initP50 desc",
        sources.join(" | ")
    );
    json!({
        "widgets": [
            {
                "type": "text",
                "x": 0, "y": 0, "width": 24, "height": 2,
                "properties": {
                    "markdown": "## job-pattern-one: Node.js vs Rust cold starts\nDaily forced cold starts from `cold-start-benchmark.yml` in job-pattern-one-rust (10 per function, 1024 MB arm64). Daily points; metrics retained 15 months."
                }
            },
            chart(0, 2, 12, "Init duration p50 (ms), all functions", "p50", by_runtime("InitDuration")),
            chart(12, 2, 12, "Init duration p95 (ms), all functions", "p95", by_runtime("InitDuration")),
            chart(0, 9, 24, "Init duration p50 (ms) by function", "p50", per_function("InitDuration", "p50")),
            chart(0, 16, 12, "First invocation duration p50 (ms)", "p50", by_runtime("FirstInvokeDuration")),
            chart(12, 16, 12, "Max memory used (MB), average", "Average", by_runtime("MaxMemoryUsed")),
            {
                "type": "log",
                "x": 0, "y": 23, "width": 24, "height": 8,
                "properties": {
                    "title": "Organic cold starts, last 7 days (log retention)",
                    "region": region,
                    "view": "table",
                    "query": organic_query
                }
            }
        ]
    })
}
