# Architecture

## High-Level Flow

Client or `maxc-cli` -> JSON-RPC request -> `backend/automation` dispatch -> auth, scope, limit, and ownership checks -> runtime execution + event-store persistence -> telemetry/metrics snapshots -> JSON-RPC response.

## Crate Responsibilities

### `backend/automation`

- Owns `RpcServer`, request validation, method dispatch, and runtime state.
- Enforces auth scopes, rate limits, overload rejection, breaker behavior, shutdown drain, and fault injection.
- Runs terminal, browser, and agent workflows.
- Maintains subscription buffers, history buffers, telemetry snapshots, and perf harness logic.

### `backend/core`

- Defines typed IDs and `BackendConfig`.
- Parses environment variables into validated runtime configuration.

### `backend/storage`

- Implements the append-only event store, snapshots, replay, and command-result idempotency.
- Restores durable projections for sessions, browser state, terminal state, and agent state.

### `backend/telemetry`

- Defines structured logs, spans, counters, gauges, and latency snapshots.
- Stores telemetry in-memory for diagnostics RPCs.

### `backend/cli`

- Parses local commands and turns them into JSON-RPC requests.
- Uses Windows named pipes as the transport.
- Provides smoke coverage against in-process `RpcServer`.

## Request Lifecycle

1. `handle_json_line` parses the raw request and allocates a correlation ID.
2. The server records request-start telemetry.
3. Payload size, shutdown state, breaker state, connection concurrency, and rate limits are checked.
4. The method is dispatched to `session.*`, `system.*`, `terminal.*`, `browser.*`, or `agent.*`.
5. Authenticated methods validate `auth.token` and required scope.
6. Ownership and policy checks run before runtime work.
7. Mutating methods use `command_id` for idempotent replay.
8. Durable changes are appended to the event store and applied to the live projection.
9. Runtime-specific buffers, logs, spans, and metrics are updated.
10. A JSON-RPC result or error is returned.

## Durable vs Live State

- Durable projection: sessions, browser session metadata, browser tab metadata, terminal metadata, agent workers, agent tasks, and stored command results.
- Live runtime state: terminal process handles, browser process/page handles, live subscriber queues, breaker counters, in-memory metrics, and telemetry buffers.
- Recovery reconstructs durable state only. Live runtime counts are rebuilt from current processes, not replayed blindly from the store.

## Runtime Event Model

- Terminal and browser subscriptions use bounded per-session queues.
- Terminal and browser history APIs return bounded buffered events with `last_sequence` and `has_more`.
- Agent status is not a separate subscription stream today; frontend should use worker/task reads plus terminal/browser streams.

## Reliability Path

- Breaker opens after repeated internal or timeout faults.
- Shutdown flips the backend into reject-new-work mode and drains in-flight requests before clearing runtime state.
- Artifact cleanup runs at startup and after browser artifact writes.
- Readiness depends on actual runtime and storage dependencies, not only process liveness.
