# Frontend Integration

This document is the frontend contract for the current backend. Use it with `rpc-api.md`. The goal is to let a frontend team implement screens, request flow, auth flow, and operator tooling without guessing backend behavior.

## Current Backend Boundary

- The backend exposes a local JSON-RPC service.
- The backend provides real terminal execution and a real Chromium-backed browser runtime when the environment can launch a browser process.
- Frontend work should treat the RPC interface, IDs, errors, readiness, and diagnostics behavior as the stable integration surface.
- Frontend work can rely on real Windows ConPTY-backed terminal execution when `runtime` reports `conpty`, and on real browser execution when browser responses report `runtime: "chromium-cdp"`. The frontend should still treat rendering as driven by RPC events and returned state rather than hidden local state.

## Core Frontend Flows

### 1. App startup

1. Check backend reachability with `system.health`.
2. Create a session with `session.create`.
3. Store the returned token in frontend runtime state.
4. Call `system.readiness` before enabling terminal, browser, or diagnostics actions.

Disable mutating controls when:

- `system.health` fails
- `system.readiness.ready` is `false`
- `system.readiness.accepting_requests` is `false`
- `system.readiness.breaker_open` is `true`
- `system.readiness.queue_saturated` is `true`
- `system.readiness.browser_runtime_ready` is `false` for browser actions
- `system.readiness.terminal_runtime_ready` is `false` for terminal or agent actions

### 2. Session lifecycle

- `session.create` creates a token for authenticated methods.
- `session.refresh` replaces or extends session validity.
- `session.revoke` invalidates the token.
- On `UNAUTHORIZED`, the frontend should clear the stored token, create a new session, and retry only if the action is safe to retry.

### 3. Terminal screen flow

1. Call `terminal.spawn`.
2. Render the returned `terminal_session_id` in the UI state for that workspace/surface.
3. Send keystrokes or pasted text with `terminal.input`.
4. Send viewport changes with `terminal.resize`.
5. Use `terminal.subscribe` to keep the terminal panel updated.
6. Use `terminal.history` after reconnects or redraws to recover buffered output from the last known sequence.
7. Call `terminal.kill` when the user closes the surface or stops the terminal.

Frontend state should track:

- `workspace_id`
- `surface_id`
- `terminal_session_id`
- current size in `cols` and `rows`
- subscription status
- last terminal event timestamp
- last terminal event sequence

### 4. Browser screen flow

1. Call `browser.create`.
2. Store `browser_session_id`.
3. Open a tab with `browser.tab.open`.
4. Store `tab_id` and focused tab state.
5. Navigate with `browser.goto`, `browser.reload`, `browser.back`, and `browser.forward`.
6. Use action methods such as `browser.click`, `browser.type`, `browser.key`, and `browser.wait`.
7. Use `browser.subscribe` to update the browser panel state.
8. Use `browser.history` after reconnects or detected sequence gaps to recover buffered browser events.
9. Close tabs with `browser.tab.close` and sessions with `browser.close`.

Frontend state should track:

- `workspace_id`
- `surface_id`
- `browser_session_id`
- active `tab_id`
- tab list
- last known URL
- loading state
- subscription status
- last browser event timestamp
- last browser event sequence

### 5. Agent screen flow

1. Call `agent.worker.create` to provision a worker with its own terminal session.
2. Store `agent_worker_id` and the returned `terminal_session_id`.
3. Optionally attach a browser session with `agent.attach.browser`.
4. Start work with `agent.task.start`.
5. Use `agent.worker.get`, `agent.task.get`, `system.diagnostics`, and the terminal/browser subscriptions to render current worker state.
6. Cancel work with `agent.task.cancel` or close the worker with `agent.worker.close`.

Frontend state should track:

- `agent_worker_id`
- `agent_task_id`
- assigned `terminal_session_id`
- optional `browser_session_id`
- worker `status`
- task `status`
- last terminal output sequence
- failure reason if present

### 5. Diagnostics and operator views

Use:

- `system.health` for liveness
- `system.readiness` for action gating
- `system.diagnostics` for broad backend state
- `system.metrics` for charts and counters
- `system.logs` for recent structured events

Recommended screen split:

- status bar: `system.health` + `system.readiness`
- diagnostics page: `system.diagnostics`
- metrics page: `system.metrics`
- recent activity page: `system.logs`

## Request Contract

### Envelope

Every request should follow:

```json
{
  "id": "req-ui-1",
  "method": "system.readiness",
  "params": {
    "auth": {
      "token": "<token>"
    }
  }
}
```

Rules:

- `id` must be unique per outstanding request from the frontend.
- Mutating methods should include a unique `command_id`.
- Authenticated methods require `params.auth.token`.
- Stable typed IDs should be preserved exactly as returned by the backend.

### Command IDs

Use a fresh `command_id` for each user action or automation action. Good patterns:

- `ui-terminal-spawn-<uuid>`
- `ui-browser-goto-<uuid>`
- `ui-session-refresh-<uuid>`

Do not reuse `command_id` across unrelated actions. Reuse only when intentionally retrying the exact same mutation and expecting idempotent replay.

### Error Handling Contract

Map backend errors to frontend behavior:

- `INVALID_REQUEST`: validation problem; show actionable input error.
- `UNAUTHORIZED`: session missing or expired; clear token and re-authenticate.
- `NOT_FOUND`: stale UI state; refresh the affected runtime object.
- `CONFLICT`: invalid lifecycle action; refresh object state and disable impossible action.
- `TIMEOUT`: show retry affordance; do not assume mutation succeeded unless command replay confirms it.
- `RATE_LIMITED`: backend overloaded, breaker open, or shutting down; disable repeated retries and surface backend state.
- `INTERNAL`: backend fault; show diagnostics link and preserve correlation data if present.

### Polling and Subscriptions

Use this model:

- `system.health`: poll every 5-10 seconds
- `system.readiness`: poll every 5 seconds and after every surfaced backend error
- `system.metrics`: poll every 5-15 seconds on diagnostics screens
- `system.diagnostics`: poll every 10-30 seconds or refresh on demand
- `system.logs`: poll every 3-5 seconds on an active logs screen
- `terminal.subscribe` and `browser.subscribe`: keep active while the matching panel is visible
- `terminal.history`: call on reconnect, restore, or detected sequence gap
- `browser.history`: call on reconnect, restore, or detected sequence gap

If a subscription fails:

1. keep the surface visible
2. mark it degraded
3. offer reconnect
4. call `terminal.history` from the last known sequence for redraw
5. call `browser.history` from the last known sequence for browser redraw if needed
6. continue limited polling from diagnostics if available

## UI State Mapping

### Terminal panel

Display at minimum:

- session identifier
- connection state
- current dimensions
- last activity time
- latest event/error badge

Actions:

- spawn
- send input
- resize
- reconnect subscription
- kill

### Browser panel

Display at minimum:

- browser session identifier
- active tab identifier
- tab list
- URL
- loading/idle state
- latest event/error badge

Actions:

- create browser
- open/focus/close tab
- goto/back/forward/reload
- click/type/key/wait
- screenshot and evaluate if exposed in the UI
- reconnect subscription
- close browser

### Diagnostics UI

Display at minimum:

- health status
- readiness status
- breaker state
- shutdown state
- queue saturation
- active request count
- session/runtime/subscription counts
- recent logs
- key counters and latency summaries

## Frontend Safety Rules

- Never treat `system.health` as permission to enable all actions.
- Always gate user actions with `system.readiness` for authenticated workload.
- Preserve and reuse backend IDs exactly.
- Keep correlation IDs from errors visible in developer tools or debug panels.
- Avoid automatic infinite retries on `RATE_LIMITED`, `TIMEOUT`, or `INTERNAL`.
- Treat `UNAUTHORIZED` as a session-state transition, not as a generic transient network error.

## Minimum Frontend Implementation Checklist

- create and store a session token
- call readiness before enabling mutating actions
- support terminal lifecycle methods
- support browser lifecycle, tab, and navigation methods
- support browser history recovery
- support agent worker/task lifecycle and ownership state
- expose diagnostics, metrics, and logs views
- handle every documented error code
- generate unique `id` and `command_id` values
- show degraded state for breaker-open, queue saturation, and shutdown

## Known Backend Limits

- Terminal sessions now run real local processes and stream real output through `terminal.subscribe`.
- Browser sessions prefer the real Chromium-backed runtime and fall back to a synthetic session only when the backend cannot launch a browser in the current environment.
- Non-Windows named-pipe transport is not implemented in the CLI.
- The frontend should build against the RPC contract, not hidden assumptions about process or browser embedding internals.
