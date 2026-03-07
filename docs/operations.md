# Operations

## Startup Checks

1. Start the backend server.
2. Call `system.health` to verify the process is alive.
3. Create a session token.
4. Call `system.readiness` with that token before enabling mutating workload.

Health is liveness only. Readiness is the backend work gate.

## Readiness Interpretation

Do not start new workload when any of these are true:
- `ready` is `false`
- `accepting_requests` is `false`
- `breaker_open` is `true`
- `queue_saturated` is `true`
- `terminal_runtime_ready` is `false` for terminal or agent work
- `browser_runtime_ready` is `false` for browser work
- `artifact_root_ready` is `false` for screenshot, download, or trace workflows
- `event_store_ready` or `store_available` is `false`

## Recommended Diagnostic Order

1. `system.health`
2. `system.readiness`
3. `system.diagnostics`
4. `system.metrics`
5. `system.logs`

Use:
- `system.diagnostics` for the broad operational picture
- `system.metrics` for counters, gauges, and latency summaries
- `system.logs` for recent request-level and lifecycle events

## Scopes and Auth

- `system.health` requires no auth.
- `system.readiness`, `system.diagnostics`, `system.metrics`, and `system.logs` require a token with `diagnostics` scope.
- Tokens may be narrower than full backend scope, so a valid token can still receive `UNAUTHORIZED` for unsupported method families.

## Artifact Retention and Cleanup

- Browser screenshots, downloads, and traces are retained under the artifact root below `MAXC_EVENT_DIR`.
- Cleanup runs on backend startup and after browser artifact writes.
- Retention is controlled by:
  - `MAXC_ARTIFACT_MAX_FILES`
  - `MAXC_ARTIFACT_MAX_TOTAL_BYTES`
  - `MAXC_ARTIFACT_TTL_MS`
  - `MAXC_ARTIFACT_MAX_FILES_PER_SESSION`
- Inspect current artifact counts and bytes through diagnostics and metrics.
- Metrics also expose artifact cleanup run counts and evicted file or byte counts.

## Graceful Shutdown

- Shutdown flips the backend into reject-new-work mode.
- In-flight requests are allowed to drain until `MAXC_SHUTDOWN_DRAIN_TIMEOUT_MS`.
- After the drain window, runtime state, worker state, and subscriptions are cleared.
- Frontend should react to shutdown from `system.readiness.accepting_requests` and `system.health.shutting_down`.

## Breaker Behavior

- Internal and timeout failures increase the breaker failure count.
- The breaker opens after `MAXC_BREAKER_FAILURE_THRESHOLD` consecutive failures.
- While open, new work is rejected with `RATE_LIMITED`.
- After `MAXC_BREAKER_COOLDOWN_MS`, the backend allows a probe request.
- A successful probe closes the breaker.

## Event Store Recovery

- The event store lives under `MAXC_EVENT_DIR`.
- On restart, the backend replays snapshots and later events to reconstruct durable state.
- Live runtime handles are not restored from the event store.
- If recovery is unhealthy, inspect the event directory and diagnostics before deleting state.

## Performance Guardrails

Deterministic CI guardrail:

```bash
cargo run -p maxc-automation --bin perf-harness --offline -- --mode synthetic --profile ci --json
```

Windows real-runtime guardrail:

```bash
cargo run -p maxc-automation --bin perf-harness --offline -- --mode real-runtime --profile ci --json
```

Synthetic mode is the stable CI baseline. Real-runtime mode validates the actual terminal, browser, and agent startup latencies on Windows.

## Release Checklist

1. Confirm `system.health.ok` is `true`.
2. Confirm `system.readiness.ready` is `true`.
3. Confirm readiness dependency fields are healthy for the workflows you plan to enable.
4. Run the quality gate:
   - `cargo fmt --check`
   - `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`
   - `cargo test --workspace --all-features --offline`
   - `cargo llvm-cov --workspace --all-features --fail-under-lines 85`
5. Run synthetic perf validation.
6. Run Windows real-runtime perf validation.
7. Inspect diagnostics, metrics, and logs for dependency failures, cleanup churn, and abnormal runtime counts.

## Example Operator Flow

```bash
cargo run -p maxc-cli -- session create
cargo run -p maxc-cli -- readiness --token <token> --pretty
cargo run -p maxc-cli -- diagnostics --token <token> --pretty
cargo run -p maxc-cli -- health --pretty
```
