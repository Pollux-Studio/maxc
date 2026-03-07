# Operations

## Start and Verify

1. Start the backend server.
2. Call `system.health`.
3. Call `system.readiness` with a valid session token before starting real work.

Health indicates the process is alive. Readiness indicates it is safe to accept authenticated workload.

## Proper Health Usage

- Use `system.health` for process liveness and version checks.
- Use `system.readiness` for automation gates.
- Do not start new workload if:
  - `ready` is `false`
  - `accepting_requests` is `false`
  - `breaker_open` is `true`
  - `queue_saturated` is `true`

## Diagnostics

Use these in order:

1. `system.health`
2. `system.readiness`
3. `system.diagnostics`
4. `system.metrics`
5. `system.logs`

`system.diagnostics` gives the broad operational picture. `system.metrics` is better for counters and latency. `system.logs` is best for recent event history and request-level tracing.

## Security and Scope Checks

- Tokens now carry scopes. Use diagnostics-only tokens for operator screens when possible.
- A method can fail with `UNAUTHORIZED` even with a valid token if the token lacks the required scope.
- Cross-workspace or cross-surface runtime access is rejected instead of silently reusing the resource.
- Raw browser commands remain available by default for trusted local development, but operators should disable them with `MAXC_BROWSER_ALLOW_RAW_COMMANDS=false` for stricter local setups.

## Artifact Retention

- Screenshots, downloads, and traces are retained under the backend artifact root with bounded cleanup.
- Cleanup runs on backend startup and after artifact writes.
- Retention is controlled by `MAXC_ARTIFACT_MAX_FILES`, `MAXC_ARTIFACT_MAX_TOTAL_BYTES`, `MAXC_ARTIFACT_TTL_MS`, and `MAXC_ARTIFACT_MAX_FILES_PER_SESSION`.
- Use `system.diagnostics` or `system.metrics` to inspect current retained artifact counts and bytes.

## Graceful Shutdown

- Shutdown flips the backend into reject-new-work mode.
- Existing in-flight requests are allowed to drain until `MAXC_SHUTDOWN_DRAIN_TIMEOUT_MS`.
- After the drain window, runtime state and subscriptions are cleared.

## Breaker Behavior

- Internal and timeout failures increase the breaker failure count.
- Once the threshold is reached, the breaker opens.
- While open, new work is rejected with `RATE_LIMITED`.
- After cooldown, the server allows a probe request.
- A successful probe closes the breaker.

## Event Store Recovery

- The event store lives under `MAXC_EVENT_DIR`.
- On restart, the backend loads the latest snapshot and replays remaining events.
- If recovery fails, inspect the event directory and logs before deleting state.

## Performance Guardrails

Run:

```bash
cargo run -p maxc-automation --bin perf-harness --offline -- --profile ci --json
```

Compare results against `backend/automation/perf-baseline.json`.

## Example Diagnostic Flow

```bash
cargo run -p maxc-cli -- session create
cargo run -p maxc-cli -- readiness --token <token> --pretty
cargo run -p maxc-cli -- diagnostics --token <token> --pretty
```
