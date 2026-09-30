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
- **Performance tuning stays inside the AWS SDK's public configuration.** The official SDK and runtime are why this repo uses Rust.
  - **How to tune:** use feature flags, `aws_config::defaults(..)` builder options, and the HTTP-client builder.
  - **Where to stop:** stop tuning when the next gain would require replacing or bypassing SDK behavior. Examples:
    - hand-building `SdkConfig` instead of `load_defaults`, which drops profile/env settings, retry mode, endpoint overrides, FIPS and dual-stack;
    - a custom HTTP stack;
    - patching or forking SDK crates.
  - **Record trade-offs:** when a supported option still carries a trade-off, write it down in "Known constraints". The Amazon-only trust store is one: it is supported, but it means we own the CA list and no longer inherit the SDK's default HTTPS client improvements.
  - **Prove it first:** measure with the per-phase `startup` log line (or `examples/init_variants.rs`) before and after a change. Keep only changes that show up in Lambda's Init Duration.
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
- This repo was created after GitHub switched new repos to **immutable OIDC subjects**. Its token `sub` is
  `repo:sls-reference-architectures@96598675/job-pattern-one-rust@1398379420:<ref|pull_request>`, not
  `repo:sls-reference-architectures/job-pattern-one-rust:…`. IAM trust policies must match the `@<id>` form.
- **TLS trusts only the Amazon Trust Services roots** (`functions/certs/amazon-trust-services.pem`,
  wired in `runtime::aws_config`). Loading the OS CA bundle (143 certs) was ~60% of SDK init.
  Every AWS endpoint chains to one of the five roots. If a call ever fails with an unknown-issuer
  TLS error, AWS has changed CAs: re-extract from the AL2023 bundle and verify SPKI hashes against
  https://www.amazontrust.com/repository/. Never add non-AWS roots: these functions call only AWS.
  Costs we accepted:
  - Because we build the HTTPS client ourselves, SDK upgrades that change the default client
    (TLS provider, settings) are not inherited. Review `aws-smithy-http-client` release notes when
    Dependabot bumps it.
  - Any HTTPS endpoint not signed by an Amazon root (a local AWS emulator over TLS, a
    TLS-intercepting proxy) fails. When copying this pattern to such a setting, don't copy the
    trust store.
- `functions/examples/init_variants.rs` reproduces the SDK-init experiment in the Lambda base image
  (see its header). Per-phase init timings are logged on every cold start as `startup ...`.
- **Measured and rejected: replacing the default logging setup.** The `tracing=` startup phase reads
  ~5-6 ms in Lambda, but the setup itself costs ~0.18 ms (AL2023 container, 0.58 vCPU). A level-only
  subscriber without `EnvFilter` saved ~0.02 ms, because filtering work moved to the first log line.
  The Lambda figure is mostly first-execution overhead (paging in code), which lands on whatever
  runs first in `main`. Dropping `EnvFilter` would also lose per-module `RUST_LOG` directives. Keep
  `lambda_runtime::tracing::init_default_subscriber()`.
- **zig is installed from the official tarball** in `ci.yml`, pinned by version and SHA-256 (the
  `mlugg/setup-zig` action targets the deprecated Node 20 Actions runtime). Dependabot can't bump
  it. To upgrade, take `tarball`/`shasum` for `x86_64-linux` from https://ziglang.org/download/index.json.
