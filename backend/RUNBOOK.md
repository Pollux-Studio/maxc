# Backend Runbook

## Startup
- Start the backend RPC server and verify `system.health` returns `ok: true`.
- Verify `system.readiness` returns `ready: true` before accepting CLI or automation traffic.

## Shutdown
- Trigger graceful shutdown and wait for the configured drain timeout.
- Confirm `shutting_down: true` in `system.health` and no active requests in `system.metrics`.

## Diagnostics
- Use `system.diagnostics` for session counts, runtime counts, subscriptions, and current breaker state.
- Use `system.metrics` for latency summaries, request counters, and active-request gauges.
- Use `system.logs` for recent structured records and span history.

## Breaker Open
- If the breaker opens, inspect recent `system.logs` entries for `breaker.open`.
- Check `system.metrics` for repeated `rpc.requests.error` growth.
- Wait for cooldown or restart after resolving the underlying storage/runtime failure.

## Event Store Recovery
- On recovery issues, inspect the event directory configured by `MAXC_EVENT_DIR`.
- Restart the backend and confirm replay completes via `system.health` and `system.diagnostics`.
- If corruption persists, restore the latest valid snapshot/segment backup before restarting.

## Perf Guardrail Failures
- Run `cargo run -p maxc-automation --bin perf-harness -- --profile ci --json`.
- Compare the output with `backend/automation/perf-baseline.json`.
- Investigate request latency growth before changing the baseline.
