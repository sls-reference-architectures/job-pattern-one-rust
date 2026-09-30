use probe::dashboard::body;

#[test]
fn the_dashboard_plots_each_runtime_and_scans_the_given_log_groups() {
    // ARRANGE
    let runtimes = vec!["nodejs24.x".to_owned(), "rust".to_owned()];
    let log_groups = vec![
        "/aws/lambda/job-pattern-one-dev-getJob".to_owned(),
        "/aws/lambda/job-pattern-one-rust-dev-getJob".to_owned(),
    ];

    // ACT
    let dashboard = body("us-east-1", &runtimes, &log_groups);

    // ASSERT
    let widgets = dashboard["widgets"].as_array().unwrap();
    let init_p50 = &widgets[1]["properties"];
    assert_eq!(init_p50["stat"], "p50");
    assert_eq!(init_p50["metrics"][0][5], "nodejs24.x");
    assert_eq!(init_p50["metrics"][1][5], "rust");
    let query = widgets.last().unwrap()["properties"]["query"].as_str().unwrap();
    assert!(query.starts_with(
        "SOURCE '/aws/lambda/job-pattern-one-dev-getJob' | SOURCE '/aws/lambda/job-pattern-one-rust-dev-getJob' | filter"
    ));
}

#[test]
fn the_per_function_chart_searches_the_probe_namespace() {
    // ARRANGE
    let runtimes = vec!["rust".to_owned()];

    // ACT
    let dashboard = body("us-east-1", &runtimes, &[]);

    // ASSERT
    let expression = dashboard["widgets"][3]["properties"]["metrics"][0][0]["expression"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        expression,
        "SEARCH('{SlsRa/ColdStart,Pattern,Runtime,Function} MetricName=\"InitDuration\" Pattern=\"job-pattern-one\"', 'p50', 86400)"
    );
}
