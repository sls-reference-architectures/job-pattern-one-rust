# CLAUDE.md

Guidance for agents working in this repo. The README holds the behavior contract; this file covers how to change things safely.

## What this repo is for

This is a reference architecture for **agents that build serverless systems**, and it is judged by how it performs at scale. It is the Rust sibling of `../job-pattern-one` (Node.js). The two must keep the **same external behavior and infrastructure topology**, so their cold-start comparison stays fair. Never modify `job-pattern-one` as part of work here.

## Layout

| Path | What |
|---|---|
| `functions/src/domain/` | **Pure** behavior: job lifecycle, validation, event mapping, phrase reversal. No I/O. |
| `functions/src/adapters/` | AWS wrappers (DynamoDB job store, EventBridge publisher, Step Functions starter, HTTP translation). Pure helpers such as `update_request` and `condition_failure` live here too. |
| `functions/src/bin/*.rs` | One binary per Lambda (camelCase names = Serverless function keys). Thin shells only. |
| `functions/src/runtime.rs` | Init-phase setup: AWS config, clients, the shared status-step entry point. |
| `functions/tests/unit` | Unit tests (no AWS). |
| `functions/tests/integration` | Remocal tests against the deployed stack (feature `integration`). |
| `acceptance/` | Farley three-layer acceptance tests: `tests/specs` (domain language) → `src/dsl` → `src/adapters` (SigV4 HTTP). Feature `acceptance`. |
| `tools/cold-start-probe/` | The benchmark CLI. It is never deployed. |
| `serverless.yml`, `package.json` | Infrastructure (osls v4 + plugins). Node is used only for this tooling. |

## Rules

- **Tests.** Follow the layered testing standards:
  - Unit tests are pure and use no mocks. If something seems to need a mock, extract a pure function.
  - Integration tests run against real AWS.
  - Acceptance specs contain only domain language.
  - The only comments allowed in test files are `// ARRANGE`, `// ACT` and `// ASSERT`.
  - Integration data is scoped by generated ids, and tests never assume a fresh stack. Every insert kicks off the workflow, which updates the job concurrently. So a test that updates a job must first wait for the workflow to finish, or use a revision the workflow can't reach.
- **Behavior changes** must be reflected in the README contract. If they differ from the Node sibling, add them to the "Deliberate differences" table.
- **Topology parity.** Keep memory (1024), timeout (6), arm64, the per-function IAM, and the state-machine definition identical to the sibling. Resource names derive from `${self:service}`. The sibling uses fixed names (`job-pattern-one`, `TranslateStateMachine`), so never hardcode those.
- **Init-phase work.** SDK clients are built once in `main`, before `run()`. This mirrors the Node functions' module-scope clients, which keeps the init comparison fair. Don't move client construction into handlers.
- **Middleware.** There is no Middy equivalent, and none is needed: request parsing and error mapping are plain domain functions. For real cross-cutting concerns (logging, auth), use **tower** layers. Tower is built into `lambda_runtime`/`lambda_http` (see Luciano Mammino, "Writing middlewares for Rust Lambda functions", 2026). Don't adopt a Middy-clone crate.
- **Git and CI.** Trunk-based: push straight to `master`, then watch `CI & Test` until it finishes.

## CI

**`ci.yml`**
- Runs daily at `25 6 * * *` and on every push.
- Steps: fmt → clippy `-D warnings` → unit tests → `cargo lambda build` → `sls deploy` → integration tests → acceptance tests.

**`teardown.yml`**
- Runs Sundays at `15 12 * * 0`.
- Shares the concurrency group `one-at-a-time-please` with `ci.yml`.
- Never add `--conceal` to `sls remove`.

**`dependabot-pr-ci.yml`**
- Runs lint, unit tests, and integration tests against the live stack.
- Needs the IAM role `dependabot-job-pattern-one-rust` (policies in `.github/dependabot-role/`) and a *Dependabot* secret `AWS_ROLE_TO_ASSUME` holding that role's ARN.
- Integration tests fail between the Sunday teardown and Monday's deploy; this is a known gap fleet-wide.

**`cold-start-benchmark.yml`**
- Runs daily at `45 6 * * *`.
- Probes both stacks. It briefly sets `PROBE_NONCE` on every function in both stacks, including the Node sibling's, and then removes it.
- Skips any stack that isn't in a `*_COMPLETE` state.

## Known constraints

- The cross-compile prints `linker stderr: ignoring deprecated linker optimization setting '1'` (zig 0.16 with cargo-lambda). It's harmless.
- A freshly created HTTP API returns 404 for several seconds. The acceptance adapter polls until routes answer (`wait_until_routable`).
- The cold-start dashboard is created by the probe (`PutDashboard`), not by the stack, so it survives teardowns.
- Probe payloads (`tools/cold-start-probe/src/payloads.rs`) must stay side-effect free:
  - no job writes;
  - no workflow starts;
  - the only permitted effect is an unconsumed `delete` event.
