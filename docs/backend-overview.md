# Backend Overview

The backend is a local Rust control plane built around a JSON-RPC server. It manages authenticated sessions, real terminal execution, browser automation, multi-agent orchestration, event-store recovery, diagnostics, and a thin CLI.

## Implemented Backend Surface

- Session APIs: `session.create`, `session.refresh`, `session.revoke`
- System APIs: `system.health`, `system.readiness`, `system.diagnostics`, `system.metrics`, `system.logs`
- Terminal APIs: `terminal.spawn`, `terminal.input`, `terminal.resize`, `terminal.history`, `terminal.kill`, `terminal.subscribe`
- Browser APIs: lifecycle, tab control, navigation, automation actions, history, subscriptions, raw commands, tracing, uploads, downloads, cookies, and storage
- Agent APIs: worker create/list/get/close, task start/list/get/cancel, terminal attach/detach, browser attach/detach

## Runtime Behavior

- Terminal sessions run real local processes. Windows prefers ConPTY when configured and available; fallback and non-Windows execution use `process-stdio`.
- Browser sessions prefer a real Chromium-backed CDP runtime. When Chromium cannot launch, the backend falls back to a synthetic browser runtime without changing RPC method names.
- Agent workers run on top of the terminal runtime. Each worker owns one primary terminal session and may hold one browser attachment at a time.
- Runtime subscriptions and history APIs expose ordered `sequence` and `timestamp_ms` values for reconnect-safe UI redraw.

## Persistence and Recovery

- Durable state is stored in the event store under `MAXC_EVENT_DIR`.
- Sessions, browser metadata, terminal metadata, command results, and agent state recover from replay.
- Live runtime handles do not survive restart. After restart, diagnostics reflect recovered durable state and current live runtime counts separately.

## Safety and Control

- Session tokens carry scopes: `diagnostics`, `runtime`, and `agent`.
- Terminal, browser, and agent mutations enforce workspace and surface ownership.
- Terminal and agent launches can be constrained by program, working-directory, environment, and quota policies.
- Browser raw commands, uploads, downloads, traces, and retained artifacts are policy-controlled.

## Readiness and Release Gate

- `system.health` is process liveness only.
- `system.readiness` is the backend action gate. It reflects shutdown state, breaker state, overload, terminal runtime availability, browser runtime availability, artifact-root writability, and event-store writability.
- Release validation uses both deterministic synthetic perf checks and Windows real-runtime perf checks.

## Supporting Docs

- Use `frontend-integration.md` for frontend behavior and state rules.
- Use `rpc-api.md` for method-level contract details.
- Use `operations.md` for runtime troubleshooting and release checks.
