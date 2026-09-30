//! Probe payloads: each drives the function through its normal code path, making at most one
//! real AWS call and leaving no lasting side effects, so both runtimes do identical work.

use serde_json::{Value, json};

/// An id no job will ever have.
pub const ABSENT_JOB_ID: &str = "JOB_COLD_START_PROBE_ABSENT";

#[derive(Clone, Debug, PartialEq)]
pub struct Probe {
    pub payload: Value,
    /// Whether the first-invocation duration is comparable across runtimes (both take the same
    /// path). When false, only init duration is meaningful.
    pub comparable_first_invoke: bool,
}

pub fn probe_for(function_key: &str) -> Option<Probe> {
    let comparable = |payload| {
        Some(Probe {
            payload,
            comparable_first_invoke: true,
        })
    };
    match function_key {
        // Real GetItem for a job that does not exist -> 404.
        "getJob" => comparable(http_event(
            "GET",
            &format!("/jobs/{ABSENT_JOB_ID}"),
            "GET /jobs/{jobId}",
            json!({ "jobId": ABSENT_JOB_ID }),
            json!({}),
            None,
        )),
        // Rejected before any write (415), so no job or workflow is created.
        "createJob" => comparable(http_event(
            "POST",
            "/jobs",
            "POST /jobs",
            Value::Null,
            json!({ "content-type": "text/plain" }),
            Some("cold start probe"),
        )),
        // Real conditional UpdateItem that fails (job absent) -> no write.
        "setJobStatusStarted" | "setJobStatusComplete" | "setJobStatusFailed" => comparable(absent_job()),
        "translatePhrase" => comparable(json!({ "phrase": "cold start probe" })),
        // Real PutEvents of a `delete` event, which nothing subscribes to.
        "onDbStreamEvent" => comparable(stream_removal()),
        // Starting a workflow would have side effects; send an event without a job so both
        // runtimes fail fast before calling Step Functions. Init duration only.
        "onJobCreated" => Some(Probe {
            payload: json!({
                "version": "0",
                "id": "cold-start-probe",
                "detail-type": "create",
                "source": "job",
                "account": "000000000000",
                "time": "2026-01-01T00:00:00Z",
                "region": "us-east-1",
                "resources": []
            }),
            comparable_first_invoke: false,
        }),
        _ => None,
    }
}

fn absent_job() -> Value {
    json!({
        "id": ABSENT_JOB_ID,
        "name": "cold start probe",
        "phrase": "cold start probe",
        "status": "Pending",
        "ttl": 1_790_000_000,
        "createdAt": "2026-01-01T00:00:00.000Z",
        "updatedAt": "2026-01-01T00:00:00.000Z",
        "revision": 1
    })
}

fn stream_removal() -> Value {
    json!({
        "Records": [{
            "eventID": "cold-start-probe",
            "eventName": "REMOVE",
            "eventVersion": "1.1",
            "eventSource": "aws:dynamodb",
            "awsRegion": "us-east-1",
            "dynamodb": {
                "ApproximateCreationDateTime": 1_767_225_600.0,
                "Keys": { "id": { "S": ABSENT_JOB_ID } },
                "OldImage": { "id": { "S": ABSENT_JOB_ID }, "revision": { "N": "1" } },
                "SequenceNumber": "1",
                "SizeBytes": 1,
                "StreamViewType": "NEW_AND_OLD_IMAGES"
            },
            "eventSourceARN": "arn:aws:dynamodb:us-east-1:000000000000:table/probe/stream/probe"
        }]
    })
}

/// An API Gateway HTTP API (payload format 2.0) event.
fn http_event(
    method: &str,
    path: &str,
    route_key: &str,
    path_parameters: Value,
    headers: Value,
    body: Option<&str>,
) -> Value {
    let mut event = json!({
        "version": "2.0",
        "routeKey": route_key,
        "rawPath": path,
        "rawQueryString": "",
        "headers": headers,
        "requestContext": {
            "accountId": "000000000000",
            "apiId": "probe",
            "domainName": "probe.execute-api.us-east-1.amazonaws.com",
            "domainPrefix": "probe",
            "http": {
                "method": method,
                "path": path,
                "protocol": "HTTP/1.1",
                "sourceIp": "127.0.0.1",
                "userAgent": "cold-start-probe"
            },
            "requestId": "cold-start-probe",
            "routeKey": route_key,
            "stage": "$default",
            "time": "01/Jan/2026:00:00:00 +0000",
            "timeEpoch": 1_767_225_600_000_i64
        },
        "isBase64Encoded": false
    });
    if !path_parameters.is_null() {
        event["pathParameters"] = path_parameters;
    }
    if let Some(body) = body {
        event["body"] = Value::String(body.to_owned());
    }
    event
}
