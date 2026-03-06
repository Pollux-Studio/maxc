# Architecture

## High-Level Flow

Client -> CLI or direct JSON-RPC -> `backend/automation` -> validation/auth/routing -> runtime state + event store -> telemetry/metrics -> JSON-RPC response.

## Crate Responsibilities

### `backend/automation`

- Owns `RpcRequest`, `RpcSuccess`, `RpcErrorCode`, and `RpcServer`.
- Handles method dispatch, auth checks, idempotency, timeouts, rate limits, overload control, breaker logic, shutdown draining, and diagnostics.
- Tracks in-memory runtime state for terminal and browser sessions.
- Exposes perf harness and compatibility tests.

### `backend/core`

- Defines typed IDs and `BackendConfig`.
- Converts environment variables into validated runtime configuration.

### `backend/storage`

- Stores append-only events with checksums and index files.
- Reconstructs projections for sessions, browser state, and command results.
- Supports snapshots, replay, and compaction.

### `backend/telemetry`

- Defines structured log records, span records, latency snapshots, metrics snapshots, and an in-memory collector.

### `backend/cli`

- Parses local commands.
- Builds JSON-RPC requests.
- Talks to the local named-pipe server on Windows.
- Has in-process smoke tests against `RpcServer`.

## Request Lifecycle

1. `handle_json_line` parses the raw JSON and allocates a correlation ID.
2. The server records a start log entry.
3. Size limits, shutdown state, breaker state, and overload limits are checked.
4. The request is validated and dispatched.
5. Authenticated methods validate `auth.token`.
6. Mutating requests enforce idempotency through `command_id`.
7. State changes are persisted to the event store and applied to the live projection.
8. Metrics, spans, and completion/error logs are recorded.

## Reliability and Recovery

- Event store replay reconstructs durable state after restart.
- Shutdown flips the backend into reject-new-work mode, waits for in-flight requests, then clears runtime state.
- Breaker state opens after repeated internal or timeout failures and blocks further work until cooldown expires.
- Fault hooks are test-only and target dispatch, persistence, snapshot, and response paths.

## Observability Flow

- Logs are structured and stored in an in-memory ring buffer.
- Spans track request-level timing and attributes.
- Metrics track counters, gauges, and latency distributions.
- `system.metrics` and `system.logs` return snapshots of that in-memory state.
