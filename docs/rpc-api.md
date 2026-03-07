# RPC API

All requests use JSON-RPC-style envelopes:

```json
{
  "id": "req-1",
  "method": "system.health",
  "params": {}
}
```

Success responses:

```json
{
  "id": "req-1",
  "result": {
    "ok": true
  }
}
```

Error responses:

```json
{
  "id": "req-1",
  "error": {
    "code": "UNAUTHORIZED",
    "message": "unauthorized",
    "data": {
      "correlation_id": "corr-1"
    }
  }
}
```

## Auth and Idempotency

- `session.create` does not require auth.
- Most non-session methods require `params.auth.token`.
- Session responses now include `scopes`; `session.create` may optionally request a narrower `params.scopes` subset.
- Every mutating command should include a unique `command_id`.
- Reusing a previous `command_id` returns the stored prior result.
- Frontends should also keep request `id` values unique per outstanding request.

## Request Rules

- Preserve backend IDs exactly as returned.
- Include `workspace_id` and `surface_id` on terminal and browser runtime calls.
- Treat `command_id` as required for all mutating frontend actions, even if a local tool could omit it.
- Use `system.readiness` to gate UI actions instead of relying on `system.health`.

## Session Methods

- `session.create`
- `session.refresh`
- `session.revoke`

Example:

```json
{
  "id": 1,
  "method": "session.create",
  "params": {
    "command_id": "cmd-session-1"
  }
}
```

Scope rules:

- `system.health`: no scope required
- `system.readiness`, `system.diagnostics`, `system.metrics`, `system.logs`: `diagnostics`
- `terminal.*`, `browser.*`: `runtime`
- `agent.*`: `agent`

Successful `terminal.spawn` responses now include additive real-runtime metadata such as `pid`, `program`, `cwd`, `status`, and `runtime`.

## System Methods

- `system.health`
  - No auth required.
  - Returns `ok`, `version`, `shutting_down`, `breaker_open`, `active_requests`, `uptime_ms`.
- `system.readiness`
  - Requires auth.
  - Returns `ready`, `accepting_requests`, `breaker_open`, `queue_saturated`, `store_available`, `browser_runtime_ready`, `terminal_runtime_ready`.
- `system.diagnostics`
  - Requires auth.
  - Returns session counts, runtime counts, subscription counts, artifact counts, breaker and shutdown state, and embedded metric snapshots.
- `system.metrics`
  - Requires auth.
  - Returns counters, gauges, and latency summaries.
- `system.logs`
  - Requires auth.
  - Returns recent structured logs and spans.

## Terminal Methods

- `terminal.spawn`
- `terminal.input`
- `terminal.resize`
- `terminal.history`
- `terminal.kill`
- `terminal.subscribe`

Terminal runtime notes:

- `terminal.spawn` responses include additive metadata such as `pid`, `program`, `cwd`, `status`, and `runtime`.
- `terminal.subscribe` events include additive metadata such as `sequence`, `timestamp_ms`, `status`, and `runtime`.
- `terminal.history` returns buffered terminal events for reconnect and redraw flows.

Example:

```json
{
  "id": 2,
  "method": "terminal.spawn",
  "params": {
    "command_id": "cmd-term-1",
    "workspace_id": "ws-1",
    "surface_id": "sf-1",
    "cols": 120,
    "rows": 30,
    "auth": { "token": "<session-token>" }
  }
}
```

## Browser Methods

Lifecycle:

- `browser.create`
- `browser.attach`
- `browser.detach`
- `browser.close`

Tabs:

- `browser.tab.open`
- `browser.tab.list`
- `browser.tab.focus`
- `browser.tab.close`

Navigation:

- `browser.goto`
- `browser.reload`
- `browser.back`
- `browser.forward`

Automation:

- `browser.click`
- `browser.type`
- `browser.key`
- `browser.wait`
- `browser.screenshot`
- `browser.evaluate`
- `browser.cookie.get`
- `browser.cookie.set`
- `browser.storage.get`
- `browser.storage.set`
- `browser.network.intercept`
- `browser.upload`
- `browser.download`
- `browser.trace.start`
- `browser.trace.stop`
- `browser.history`
- `browser.subscribe`
- `browser.raw.command`

Browser runtime notes:

- `browser.create`, `browser.tab.open`, navigation methods, `browser.evaluate`, and `browser.screenshot` now prefer a real Chromium-backed runtime and return additive fields such as `runtime`, `title`, `load_state`, `artifact_path`, and `artifact_bytes`.
- When the backend cannot launch a browser in the current environment, browser sessions fall back to a synthetic runtime and continue to preserve the same RPC envelopes.
- `browser.subscribe` events and `browser.history` entries include ordered `sequence`, `timestamp_ms`, `status`, and `runtime` fields for reconnect-safe redraw.
- `browser.raw.command` still requires `params.allow_raw=true` and is additionally governed by `MAXC_BROWSER_ALLOW_RAW_COMMANDS`.
- Upload, download, and trace paths are subject to configured allowlists and bounded artifact retention.

## Agent Methods

- `agent.worker.create`
- `agent.worker.list`
- `agent.worker.get`
- `agent.worker.close`
- `agent.task.start`
- `agent.task.list`
- `agent.task.get`
- `agent.task.cancel`
- `agent.attach.terminal`
- `agent.detach.terminal`
- `agent.attach.browser`
- `agent.detach.browser`

Agent runtime notes:

- `agent.worker.create` provisions a dedicated terminal-backed worker and returns `agent_worker_id`, `status`, and `terminal_session_id`.
- `agent.task.start` routes work to the worker terminal and returns `agent_task_id`, `status`, `terminal_session_id`, and the last known terminal output sequence.
- Browser attachment is exclusive per browser session. A second `agent.attach.browser` against the same `browser_session_id` returns `CONFLICT`.
- Agent task responses and diagnostics return redacted prompt previews rather than full prompt bodies.
- `system.diagnostics` now exposes agent workers and tasks in addition to terminal and browser runtime state.

Example:

```json
{
  "id": 3,
  "method": "browser.goto",
  "params": {
    "command_id": "cmd-browser-goto-1",
    "workspace_id": "ws-1",
    "surface_id": "sf-1",
    "browser_session_id": "bs-123",
    "tab_id": "tab-123",
    "url": "https://example.com",
    "auth": { "token": "<session-token>" }
  }
}
```

## Error Codes

- `INVALID_REQUEST`: malformed JSON, missing fields, bad IDs, or out-of-range request parameters.
- `UNAUTHORIZED`: missing token, invalid token, expired token, revoked token, or insufficient session scope.
- `NOT_FOUND`: unknown method or missing runtime object.
- `CONFLICT`: invalid lifecycle transition or closed resource.
- `TIMEOUT`: request timed out or response fault path was hit.
- `RATE_LIMITED`: rate limit, overload limit, breaker-open state, shutdown reject, or policy/quota rejection.
- `INTERNAL`: persistence or runtime failure.

## Proper Usage Rules

- Always generate a fresh `command_id` per request.
- Use `system.readiness` before automated batches.
- Do not treat `system.health` as permission to start mutating work if `system.readiness.ready` is false.
- Expect authenticated diagnostics methods to fail with `UNAUTHORIZED` if the session is expired or revoked.
- On `RATE_LIMITED`, check readiness and breaker state before retrying.
- On `NOT_FOUND` or `CONFLICT`, refresh the affected frontend runtime state instead of blindly retrying.
