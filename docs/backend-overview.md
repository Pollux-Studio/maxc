# Backend Overview

The backend is implemented as a Rust workspace centered on a local JSON-RPC server. It manages terminal sessions, browser sessions, durable state recovery, diagnostics, observability, and a small local CLI.

## Current Crates

- `backend/automation`: JSON-RPC contracts, server, runtime state, reliability controls, perf harness.
- `backend/core`: shared IDs and `BackendConfig`.
- `backend/browser`: browser IDs and contract models.
- `backend/terminal`: terminal capability models.
- `backend/storage`: append-only event store, replay, snapshots, compaction.
- `backend/security`: session token validation.
- `backend/telemetry`: structured log, span, and metric data models.
- `backend/cli`: local command-line client for core flows.

## Implemented Capabilities

- Session APIs: create, refresh, revoke.
- Terminal APIs: spawn, input, resize, history, kill, subscribe.
- Browser APIs: create, attach, detach, close, tab open/list/focus/close, goto/reload/back/forward, click/type/key/wait/screenshot/evaluate, cookie and storage controls, network intercept toggle, upload/download, tracing, raw commands, history, subscribe.
- Agent APIs: worker create/list/get/close, task start/list/get/cancel, terminal attach/detach, browser attach/detach.
- System APIs: health, readiness, diagnostics, metrics, logs.
- Reliability: rate limiting, overload rejection, request timeout, breaker, graceful shutdown, fault injection, recovery, idempotent `command_id`.
- Release gate: synthetic CI perf guardrails, Windows real-runtime perf guardrails, dependency-aware readiness, artifact cleanup accounting, and restart-safe runtime recovery.
- Security and policy controls: token scopes, ownership checks, terminal env/cwd/program allowlists, browser raw-command toggle, bounded browser artifact retention, and prompt/log redaction.
- Observability: structured logs, spans, metrics snapshots, diagnostics RPCs.

## How To Use It

Typical flow:

1. Create a session token with `session.create`.
2. Use that token in `auth.token` for all authenticated methods.
3. Send a unique `command_id` for every mutating request.
4. Use `system.readiness` before automation runs.
5. Use `system.metrics`, `system.logs`, and `system.diagnostics` for troubleshooting.

For frontend implementation, use `frontend-integration.md` together with `rpc-api.md`. Those two documents define the practical screen-level integration contract.

## Important Runtime Notes

- Terminal execution is now backed by real local processes through the `terminal.*` RPC surface. On Windows the backend prefers ConPTY for process execution and resize behavior; non-Windows and test flows fall back to `process-stdio`.
- Browser execution now prefers a real Chromium-backed CDP runtime and falls back to the synthetic runtime only when the current environment cannot launch a browser process.
- Browser and terminal subscriptions now carry ordered `sequence`, `timestamp_ms`, `status`, and `runtime` fields, and both runtimes expose history APIs for reconnect-safe redraw.
- Multi-agent orchestration runs on top of the existing real terminal runtime. Each worker owns one primary terminal session and may hold one browser session attachment at a time.
- `system.readiness` now reflects real dependency health for terminal runtime, browser runtime, artifact storage, and event-store writability instead of assuming those dependencies are available.
- `system.health` is unauthenticated.
- `system.readiness`, `system.diagnostics`, `system.metrics`, and `system.logs` require a valid session token.
- Session tokens now carry additive `scopes` so diagnostics, runtime, and agent actions can be separated without changing the transport.
- The installed CLI binary name is `maxc-cli`. In examples below, use `cargo run -p maxc-cli -- ...` unless you rename the produced binary later.
