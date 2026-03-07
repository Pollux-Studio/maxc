# Development

## Workspace Layout

- `backend/core`: typed IDs and `BackendConfig`
- `backend/automation`: RPC server, runtime execution, readiness, diagnostics, and perf harness
- `backend/browser`: browser contract models
- `backend/terminal`: terminal contract models
- `backend/storage`: event store, replay, snapshots, and command-result persistence
- `backend/security`: token validation helpers
- `backend/telemetry`: log, span, and metric models
- `backend/cli`: local command-line RPC client

## How To Add or Change an RPC Method

1. Add or update the request handling in `backend/automation`.
2. Place routing under the correct dispatcher: `system.*`, `terminal.*`, `browser.*`, `agent.*`, or session handlers.
3. Enforce required auth scope, IDs, ownership, policy, quotas, and `command_id` behavior.
4. Persist durable mutations through the event store.
5. Update runtime buffers, diagnostics, metrics, and logs if visible behavior changes.
6. Add tests for success, auth failure, invalid input, ownership conflict, and error mapping.
7. Update `docs/rpc-api.md` and any impacted usage docs in the same change.

## How To Extend CLI Support

1. Add parsing in `backend/cli/src/main.rs`.
2. Build the matching JSON-RPC request exactly once in the request builder path.
3. Add parser and request-shape tests.
4. Add or update an in-process smoke test against `RpcServer`.
5. Update `docs/cli.md`.

## How To Extend Runtime Diagnostics

1. Add or update the runtime state in `backend/automation`.
2. Surface only operator-useful fields through diagnostics and metrics.
3. Keep readiness limited to actual gating signals, not informational fields.
4. Add tests for diagnostics, metrics, and readiness field stability.
5. Update `docs/operations.md`, `docs/rpc-api.md`, and `docs/frontend-integration.md` if frontend-visible behavior changes.

## Documentation Rules

- `docs/` is the detailed backend source of truth.
- `docs/frontend-integration.md` is the frontend contract document.
- `docs/rpc-api.md` is the wire-level and method-level API reference.
- `README.md`, `CONTRIBUTION.md`, and `AGENTS.md` should stay summary-level and point to `docs/` rather than duplicate detailed backend behavior.
- Remove stale or speculative backend wording instead of keeping roadmap-like leftovers.
- Any code change that affects behavior, config, CLI, diagnostics, readiness, or operations must update the matching docs in the same change.
