# RPC API

All backend requests use a JSON-RPC-style envelope.

## Envelope

Request:

```json
{
  "id": "req-1",
  "method": "system.health",
  "params": {}
}
```

Success response:

```json
{
  "id": "req-1",
  "result": {
    "ok": true
  }
}
```

Error response:

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

## Common Rules

- `id` must be unique per outstanding client request.
- Every mutating method should include a unique `command_id`.
- Reusing the same `command_id` for the same logical mutation returns the stored prior result.
- Preserve all backend IDs exactly as returned.
- `workspace_id` and `surface_id` are required on runtime methods.

## Auth and Scopes

- `session.create`: no auth required
- `session.refresh`, `session.revoke`: require `auth.token`
- `system.health`: no auth required
- `system.readiness`, `system.diagnostics`, `system.metrics`, `system.logs`: require `diagnostics` scope
- `terminal.*`, `browser.*`: require `runtime` scope
- `agent.*`: require `agent` scope

`session.create` may request a narrower scope set through `params.scopes`. `session.refresh` may narrow to a subset of the current token scopes.

## Session Methods

### `session.create`

Required params:
- `command_id`

Optional params:
- `scopes`: subset of configured default scopes

Returns:
- `token`
- `scopes`
- `issued_at_ms`
- `expires_at_ms`

### `session.refresh`

Required params:
- `command_id`
- `auth.token`

Optional params:
- `scopes`: subset of the existing token scopes

Returns:
- `token`
- `scopes`
- `expires_at_ms`

### `session.revoke`

Required params:
- `command_id`
- `auth.token`

Returns:
- `revoked`

## System Methods

### `system.health`

No auth required.

Returns:
- `ok`
- `version`
- `shutting_down`
- `breaker_open`
- `active_requests`
- `uptime_ms`

### `system.readiness`

Requires `diagnostics` scope.

Returns:
- `ready`
- `accepting_requests`
- `breaker_open`
- `queue_saturated`
- `store_available`
- `browser_runtime_ready`
- `terminal_runtime_ready`
- `artifact_root_ready`
- `event_store_ready`

Use this as the action gate for frontend and automation work.

### `system.diagnostics`

Requires `diagnostics` scope.

Returns aggregated backend state, including:
- session counts
- browser session and tab counts
- terminal, browser, and agent live runtime counts
- runtime backend names
- browser runtime snapshots
- agent worker and task snapshots
- subscription counts
- terminal and browser history buffer usage
- artifact counts and bytes
- dependency readiness flags
- `active_requests`, `shutting_down`, `breaker_open`
- embedded metrics snapshot

### `system.metrics`

Requires `diagnostics` scope.

Returns a telemetry snapshot with:
- `counters`
- `gauges`
- `latencies`

Important gauges include:
- `rpc.active_requests`
- `runtime.terminal.sessions`
- `runtime.browser.sessions`
- `runtime.agent.workers`
- `runtime.agent.tasks`
- `runtime.browser.ready`
- `runtime.terminal.ready`
- `runtime.artifacts.ready`
- `storage.event_dir.ready`
- artifact file and byte counts

### `system.logs`

Requires `diagnostics` scope.

Returns the in-memory telemetry snapshot with structured `logs` and `spans`.

## Terminal Methods

### `terminal.spawn`

Required params:
- `command_id`
- `workspace_id`
- `surface_id`
- `auth.token`

Optional params:
- `shell`
- `program`
- `args`
- `cwd`
- `env`
- `cols`
- `rows`

Returns additive runtime metadata such as:
- `terminal_session_id`
- `pid`
- `program`
- `cwd`
- `status`
- `runtime`
- `cols`
- `rows`

### `terminal.input`

Required params:
- `command_id`
- `workspace_id`
- `surface_id`
- `terminal_session_id`
- `auth.token`
- `input`

Returns:
- `accepted`
- `bytes`
- `terminal_session_id`
- `status`

### `terminal.resize`

Required params:
- `command_id`
- `workspace_id`
- `surface_id`
- `terminal_session_id`
- `auth.token`
- `cols`
- `rows`

Returns:
- `terminal_session_id`
- `cols`
- `rows`
- `applied`
- `status`

### `terminal.history`

Required params:
- `workspace_id`
- `surface_id`
- `terminal_session_id`
- `auth.token`

Optional params:
- `command_id`
- `from_sequence`
- `max_events`

Returns:
- `terminal_session_id`
- `runtime`
- `status`
- `pid`
- `cols`
- `rows`
- `last_sequence`
- `events`
- `has_more`
- `exit_code`

### `terminal.subscribe`

Required params:
- `command_id`
- `workspace_id`
- `surface_id`
- `terminal_session_id`
- `auth.token`

Returns:
- `subscribed`
- `terminal_session_id`
- `subscriber_id`
- `events`
- `dropped_events`
- `last_sequence`

Terminal events are ordered and include additive fields such as:
- `type`
- `sequence`
- `timestamp_ms`
- `status`
- `runtime`

### `terminal.kill`

Required params:
- `command_id`
- `workspace_id`
- `surface_id`
- `terminal_session_id`
- `auth.token`

Returns:
- `killed`
- `terminal_session_id`
- `status`

## Browser Methods

### Lifecycle

- `browser.create`
- `browser.attach`
- `browser.detach`
- `browser.close`

`browser.create` requires `command_id`, `workspace_id`, `surface_id`, and `auth.token`.

Typical create result fields:
- `browser_session_id`
- `runtime`
- `status`
- `attached`
- `closed`
- `executable`

### Tabs

- `browser.tab.open`
- `browser.tab.list`
- `browser.tab.focus`
- `browser.tab.close`

`browser.tab.open` requires `browser_session_id`, `workspace_id`, `surface_id`, `auth.token`, and optional `url`.

Typical tab result fields:
- `browser_tab_id`
- `browser_session_id`
- `url`
- `title`
- `load_state`
- `status`
- `runtime`

### Navigation and automation

- `browser.goto`
- `browser.reload`
- `browser.back`
- `browser.forward`
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
- `browser.raw.command`

Most browser mutation methods require:
- `command_id`
- `workspace_id`
- `surface_id`
- `browser_session_id`
- `auth.token`

Tab-targeted methods also require `tab_id`.

Additive result fields vary by method and can include:
- `runtime`
- `status`
- `url`
- `title`
- `load_state`
- `artifact_path`
- `artifact_bytes`
- `attached`
- `closed`

`browser.raw.command` additionally requires `allow_raw: true` and is still governed by backend policy.

### `browser.history`

Required params:
- `workspace_id`
- `surface_id`
- `browser_session_id`
- `auth.token`

Optional params:
- `command_id`
- `from_sequence`
- `max_events`

Returns:
- `browser_session_id`
- `runtime`
- `status`
- `last_sequence`
- `events`
- `has_more`
- `attached`
- `closed`

### `browser.subscribe`

Required params:
- `command_id`
- `workspace_id`
- `surface_id`
- `browser_session_id`
- `auth.token`

Returns:
- `subscribed`
- `browser_session_id`
- `subscriber_id`
- `events`
- `dropped_events`
- `last_sequence`

Browser events are ordered and include additive fields such as:
- `type`
- `sequence`
- `timestamp_ms`
- `status`
- `runtime`
- `url`
- `title`
- `load_state`

## Agent Methods

### `agent.worker.create`

Required params:
- `command_id`
- `workspace_id`
- `surface_id`
- `auth.token`

Optional params are passed through terminal spawn policy, such as `shell`, `program`, `args`, `cwd`, `env`, and optional `browser_session_id`.

Returns:
- `agent_worker_id`
- `workspace_id`
- `surface_id`
- `status`
- `terminal_session_id`
- `browser_session_id`

### `agent.worker.list` and `agent.worker.get`

Require:
- `workspace_id`
- `surface_id`
- `auth.token`

`agent.worker.get` also requires `agent_worker_id`.

Worker payloads include:
- `agent_worker_id`
- `status`
- `terminal_session_id`
- `browser_session_id`
- `current_task_id`
- `closed`

### `agent.task.start`

Required params:
- `command_id`
- `workspace_id`
- `surface_id`
- `agent_worker_id`
- `auth.token`
- `prompt` or `input`

Returns:
- `agent_task_id`
- `agent_worker_id`
- `status`
- `terminal_session_id`
- `browser_session_id`
- `last_output_sequence`
- redacted `prompt_preview`

### `agent.task.list` and `agent.task.get`

Require:
- `workspace_id`
- `surface_id`
- `auth.token`

`agent.task.get` requires `agent_task_id` and may include `agent_worker_id`.

Task payloads include:
- `agent_task_id`
- `agent_worker_id`
- `status`
- `terminal_session_id`
- `browser_session_id`
- `last_output_sequence`
- `failure_reason`
- redacted `prompt` and `prompt_preview`

### `agent.task.cancel`

Required params:
- `command_id`
- `workspace_id`
- `surface_id`
- `agent_task_id`
- `auth.token`

Optional params:
- `reason`

### Attach and detach

- `agent.attach.terminal`
- `agent.detach.terminal`
- `agent.attach.browser`
- `agent.detach.browser`

These methods require the matching worker ID plus the delegated resource ID when attaching. Browser attachment is exclusive and returns `CONFLICT` when already owned by another worker.

## Error Codes

- `INVALID_REQUEST`: malformed JSON, missing required fields, unsupported values, or invalid identifiers
- `UNAUTHORIZED`: missing token, invalid token, expired token, revoked token, or missing required scope
- `NOT_FOUND`: unknown method or missing runtime object
- `CONFLICT`: invalid lifecycle transition, closed resource, or exclusive ownership conflict
- `TIMEOUT`: request timeout or dropped-response fault path
- `RATE_LIMITED`: rate limit, overload, breaker-open state, shutdown reject, or configured quota/policy rejection
- `INTERNAL`: persistence or runtime failure

## Client Usage Rules

- Call `system.readiness` before mutating automated work.
- Treat `system.health` as liveness only.
- Do not retry `CONFLICT` and `NOT_FOUND` blindly; refresh state first.
- Do not retry `RATE_LIMITED` aggressively; inspect readiness and diagnostics.
- Use history APIs after reconnect or sequence gaps instead of assuming subscriptions are lossless.
