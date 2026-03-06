# Development

## Workspace Layout

- `backend/core`: shared config and ID types
- `backend/automation`: server and RPC behavior
- `backend/browser`: browser contract models
- `backend/terminal`: terminal contract models
- `backend/storage`: persistence and recovery
- `backend/security`: token validation
- `backend/telemetry`: logs, spans, metrics
- `backend/cli`: local command-line client

## How To Add a New RPC Method

1. Add or confirm the request shape in `backend/automation`.
2. Add routing in `dispatch`, `terminal_dispatch`, `browser_dispatch`, or `system.*`.
3. Validate auth, IDs, limits, and `command_id` handling.
4. Persist state changes if the method is mutating.
5. Emit structured logs and update metrics if the method changes runtime behavior.
6. Add tests for success, auth failures, invalid input, and error mapping.
7. Update `docs/rpc-api.md` and any related usage docs in the same change.

## How To Extend CLI Support

1. Add parsing in `backend/cli/src/main.rs`.
2. Map the command to a concrete RPC request.
3. Add request-builder tests.
4. Add or update in-process smoke tests.
5. Update `docs/cli.md`.

## How To Extend Telemetry

1. Add the data model in `backend/telemetry`.
2. Emit the new record or metric from `backend/automation`.
3. Surface it through diagnostics only if it is operator-useful.
4. Add direct tests for collector behavior and end-to-end tests for server snapshots.

## Documentation Rules

- `docs/` is the detailed source of truth.
- `README.md` stays overview-level.
- Remove stale docs rather than keeping roadmap-like duplicates.
- Every code change that affects behavior, config, diagnostics, CLI, or operations must update the matching page under `docs/`.
