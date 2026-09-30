# job-pattern-one-rust

The **job pattern** on AWS Lambda, implemented in Rust. A caller requests a potentially long-running action, gets a job id back immediately, and polls for its status and result. The work runs asynchronously: a DynamoDB stream feeds EventBridge, which starts a Step Functions workflow.

This is the Rust sibling of [job-pattern-one](https://github.com/sls-reference-architectures/job-pattern-one) (Node.js). Both deploy side by side with the **same external behavior and the same infrastructure topology**, and a daily probe compares their cold starts. See [Cold starts](#cold-starts).

## Architecture

```
caller ──SigV4──▶ HTTP API ──▶ createJob ──PutItem──▶ JobsTable (stream: NEW_AND_OLD_IMAGES, TTL: ttl)
                          └──▶ getJob ───GetItem──▶      │
                                                           ▼
                                                  onDbStreamEvent ──PutEvents──▶ JobsEventBus
                                                                                     │ source=job, detail-type=create
                                                                                     ▼
                                                                               onJobCreated ──StartExecution──▶
  Express state machine:  Set Job Status Started ─▶ Translate Phrase ─▶ Set Job Status Completed
                                     └──────── any error (after 3 retries) ─▶ Set Job Status Failed ─▶ Fail
```

## Behavior contract

**HTTP API.** The API is IAM-authorized: every request must be SigV4-signed (`execute-api`), and unsigned requests get `403`.

| Request | Response |
|---|---|
| `POST /jobs` with a JSON body `{"name": string, "phrase": string}` | `201` and the job (below), `status: "Pending"` |
| Content-Type is not `application/json` (or `application/*+json`) | `415 Unsupported Media Type` |
| Body missing or not valid JSON | `422 Invalid or malformed JSON was provided` |
| `name` or `phrase` missing or not a string | `400 name and phrase are required and must be strings` |
| `GET /jobs/{jobId}` | `200` and the job as stored, or `404 No id found for <jobId>` |

Error bodies are `text/plain`; success bodies are `application/json`.

**Job**
```json
{
  "id": "JOB_<ulid>", "name": "…", "phrase": "…",
  "status": "Pending | Started | Complete | Failed",
  "ttl": 1790432000, "createdAt": "2026-09-21T14:13:20.000Z", "updatedAt": "…", "revision": 1,
  "translatedPhrase": "…", "error": { "Error": "…", "Cause": "…" }
}
```
- `translatedPhrase` appears once translated; `error` appears only on failure.
- `ttl` is 5 days after creation. DynamoDB deletes the job after that, which also publishes a `delete` event.
- A job normally reaches `Complete`, with `translatedPhrase` set to the phrase reversed, within a few seconds.

**Events.** Every change to a job is published to the `job-pattern-one-rust` bus with `source: job`:
- `create`: the new job.
- `update`: `{ "old": …, "new": … }`.
- `delete`: the old job.
- Within a batch, events are ordered creates, then updates, then deletes. If any entry is rejected, the whole batch fails and is retried.

**Status changes** use optimistic concurrency. Each one increments `revision` and succeeds only if the stored revision is unchanged. The failures are `NotFoundError` (no such job) and `ConflictError` (the stored revision differs).

### Deliberate differences from the Node.js sibling
| Behavior | Node.js | Rust |
|---|---|---|
| `name`/`phrase` missing | 500, empty body | 400 with a message |
| PutEvents partially rejected | ignored (events lost) | batch fails and is retried |
| State-machine log retention | never expires | 7 days |
| Reversing non-ASCII phrases | by UTF-16 unit (splits emoji) | by Unicode scalar value |

## Cold starts

`.github/workflows/cold-start-benchmark.yml` runs daily and records how each stack starts cold.

**What it measures.** For every function in both stacks, it forces 10 cold starts and records:
- Init Duration
- the duration of the first invocation
- max memory used

**Where the results go**
- CloudWatch metrics, namespace `SlsRa/ColdStart`, kept for 15 months.
- The `job-pattern-one-cold-starts` dashboard.
- The workflow run's summary.

**Baseline** (2026-09-30, 1024 MB arm64, Node.js 24): median init is about 440 ms, and about 230 ms for the first invocation that calls DynamoDB.

## Commands

```bash
cargo test --workspace                                              # unit tests (pure, no AWS)
npm ci && npm run build                                             # cargo lambda build → target/lambda/*/bootstrap.zip
npx sls deploy                                                      # deploy stack job-pattern-one-rust-dev
cargo test -p jobs --features integration --test integration        # adapters against the deployed stack
cargo test -p acceptance --features acceptance --test acceptance    # specifications against the deployed API
STACK_NAME=job-pattern-one-dev cargo test -p acceptance --features acceptance --test acceptance  # same specs vs Node.js
cargo run -p cold-start-probe -- --dry-run --samples 3              # measure without publishing
```

Prerequisites: Rust (pinned by `rust-toolchain.toml`), [cargo-lambda](https://www.cargo-lambda.info/), [zig](https://ziglang.org/) (to cross-compile for arm64), Node.js 24 (for the Serverless Framework tooling), and AWS credentials for account 833818295128 in `us-east-1`.

## License

MIT
