# Frontend Integration

This document is the screen-level contract for building a frontend against the backend. Use it together with `rpc-api.md`. The frontend should treat RPC responses, IDs, readiness, errors, and event history as the source of truth.

## Backend Boundary

- The backend exposes a local JSON-RPC service.
- Terminal execution is real process execution.
- Browser execution prefers a real Chromium-backed runtime and falls back to a synthetic runtime when the environment cannot launch a browser.
- Agent workers are backend-managed terminal-backed workers with optional browser attachment.
- Frontend should never rely on hidden local process or browser state. It should render from RPC responses, history APIs, and subscription events.

## Required Frontend Startup Flow

1. Call `system.health` to confirm the backend process is reachable.
2. Call `session.create`.
3. Store the returned `token`, `scopes`, and expiry values.
4. Call `system.readiness` before enabling any mutating action.
5. Enable only the UI features allowed by both `scopes` and readiness.

### Action gates

Disable mutating actions when any of these is true:
- `system.readiness.ready` is `false`
- `accepting_requests` is `false`
- `breaker_open` is `true`
- `queue_saturated` is `true`
- `terminal_runtime_ready` is `false` for terminal and agent actions
- `browser_runtime_ready` is `false` for browser actions
- `artifact_root_ready` is `false` for screenshot, download, and trace actions
- `event_store_ready` is `false` for any workflow that must persist state

Also hide or disable features when the token lacks the required scope:
- diagnostics pages need `diagnostics`
- terminal and browser screens need `runtime`
- agent screens need `agent`

## Frontend State Model

Keep this state at minimum:

### Global app state
- current session `token`
- current token `scopes`
- session expiry time
- latest `system.health`
- latest `system.readiness`
- last diagnostics refresh time

### Terminal pane state
- `workspace_id`
- `surface_id`
- `terminal_session_id`
- `subscriber_id`
- `cols`
- `rows`
- last seen terminal `sequence`
- last seen terminal `timestamp_ms`
- current runtime `status`
- current runtime `runtime`
- degraded flag and last error

### Browser pane state
- `workspace_id`
- `surface_id`
- `browser_session_id`
- focused `tab_id`
- known tab list
- `subscriber_id`
- last seen browser `sequence`
- last seen browser `timestamp_ms`
- current browser `status`
- current browser `runtime`
- current `url`, `title`, and `load_state`
- degraded flag and last error

### Agent pane state
- `workspace_id`
- `surface_id`
- `agent_worker_id`
- optional `agent_task_id`
- owned `terminal_session_id`
- optional owned `browser_session_id`
- worker `status`
- task `status`
- last terminal output `sequence`
- failure reason if present

## Request Rules

### IDs
- Generate a unique request `id` for every outstanding frontend request.
- Generate a fresh `command_id` for every mutating user action.
- Reuse a `command_id` only when intentionally retrying the exact same mutation.
- Preserve all returned backend IDs exactly.

### Auth
- Put the token in `params.auth.token` for every authenticated method.
- If a token is refreshed, replace the stored token and scopes immediately.

## Terminal Screen Contract

### Create flow
1. Call `terminal.spawn` with `workspace_id`, `surface_id`, `cols`, and `rows`.
2. Store `terminal_session_id`, `pid`, `status`, `runtime`, `cols`, and `rows` from the result.
3. Call `terminal.subscribe`.
4. Store `subscriber_id`, initial queued `events`, `dropped_events`, and `last_sequence`.
5. Render the initial terminal state from the returned event list.

### Live interaction
- Send user input through `terminal.input`.
- Send viewport changes through `terminal.resize`.
- Update pane state from subscription events and mutation responses.
- `terminal.resize.applied` tells the UI whether the resize reached the runtime. Do not assume success from the request alone.

### Reconnect and redraw
1. Detect reconnect need when the subscription disconnects, the pane is restored, or the next event sequence is not exactly `last_sequence + 1`.
2. Mark the pane degraded, but keep it visible.
3. Call `terminal.history` with `from_sequence` set to the last seen sequence plus one.
4. Append returned `events` in order.
5. Update `last_sequence` from the history result.
6. Re-run `terminal.subscribe` and continue streaming.

### Close flow
- Call `terminal.kill` when the user closes a terminal surface or explicitly stops it.
- Treat `status` and `exit_code` from `terminal.history` or later events as the source of truth for termination state.

## Browser Screen Contract

### Create flow
1. Call `browser.create`.
2. Store `browser_session_id`, `runtime`, `status`, `attached`, and any returned runtime metadata.
3. Call `browser.subscribe`.
4. Store `subscriber_id`, initial `events`, `dropped_events`, and `last_sequence`.
5. Open a first tab with `browser.tab.open`.
6. Store `browser_tab_id`, `url`, `title`, `load_state`, and `status`.

### Live interaction
- Use `browser.goto`, `reload`, `back`, `forward` for navigation.
- Use `browser.click`, `type`, `key`, and `wait` for automation.
- Use `browser.evaluate`, `cookie.*`, `storage.*`, `network.intercept`, `upload`, `download`, `trace.*`, and `screenshot` as advanced actions.
- Treat returned `runtime`, `status`, `url`, `title`, `load_state`, `artifact_path`, and `artifact_bytes` as authoritative.

### Reconnect and redraw
1. Detect reconnect need on subscription loss or sequence gap.
2. Mark the browser pane degraded.
3. Call `browser.history` with `from_sequence` set to the last seen sequence plus one.
4. Append returned events in order.
5. Update stored `last_sequence`.
6. Re-run `browser.subscribe`.
7. Use current pane state plus the recovered events to rebuild tab/UI state.

### Runtime fallback handling
- If browser responses report a synthetic runtime, the frontend should still work against the same method names and event shapes.
- Gate browser features from `system.readiness.browser_runtime_ready`, not from assumptions about the local environment.

## Agent Screen Contract

### Create and run flow
1. Call `agent.worker.create`.
2. Store `agent_worker_id`, `terminal_session_id`, optional `browser_session_id`, and worker `status`.
3. If the user wants browser access, call `agent.attach.browser` with the target `browser_session_id`.
4. Start work with `agent.task.start`.
5. Store `agent_task_id`, task `status`, `terminal_session_id`, optional `browser_session_id`, and `last_output_sequence`.
6. Render task progress using `agent.worker.get`, `agent.task.get`, terminal output, browser state, and diagnostics.

### Ownership rules
- A worker owns one primary terminal session.
- Browser attachment is exclusive by default. A second worker attempting to attach the same browser session receives `CONFLICT`.
- Frontend must represent ownership conflicts as current-state problems, not generic failures.

### Cancel and close
- Use `agent.task.cancel` to stop the current task.
- Use `agent.worker.close` to close the worker and terminate its primary terminal.
- When a worker closes, clear stored worker/task state from the frontend surface after the backend confirms closure.

## Diagnostics and Operator Views

Recommended screens:
- status bar: `system.health` + `system.readiness`
- diagnostics page: `system.diagnostics`
- metrics page: `system.metrics`
- logs page: `system.logs`

Recommended polling:
- `system.health`: every 5 to 10 seconds
- `system.readiness`: every 5 seconds and after any surfaced backend error
- `system.metrics`: every 5 to 15 seconds on metrics screens
- `system.diagnostics`: every 10 to 30 seconds or on manual refresh
- `system.logs`: every 3 to 5 seconds on an active logs page

## Error Handling Contract

### `INVALID_REQUEST`
- Treat as a client bug or invalid user input.
- Show a field-level or action-level validation message.
- Do not auto-retry.

### `UNAUTHORIZED`
- Treat as missing, expired, revoked, or scope-insufficient session state.
- Clear invalid session state.
- Recreate or refresh the session before retrying.
- Hide actions the current token cannot perform.

### `NOT_FOUND`
- Treat as stale frontend state.
- Refresh the affected runtime object or parent view.
- Do not repeatedly retry the same stale mutation.

### `CONFLICT`
- Treat as a lifecycle or ownership conflict.
- Refresh current runtime state.
- Disable the impossible action until the state changes.

### `TIMEOUT`
- Treat as unknown mutation outcome.
- Show retry affordance.
- If the original action had a `command_id`, retry only by replaying the exact same logical action.

### `RATE_LIMITED`
- Treat as overload, breaker-open, shutdown reject, or policy/quota rejection.
- Check readiness and diagnostics before retrying.
- Back off and avoid automatic repeated retries.

### `INTERNAL`
- Treat as backend failure.
- Preserve `correlation_id` for debugging.
- Offer diagnostics navigation.

## Example Screen Flows

### Terminal pane
1. `session.create`
2. `system.readiness`
3. `terminal.spawn`
4. `terminal.subscribe`
5. `terminal.input`
6. `terminal.history` on reconnect
7. `terminal.kill`

### Browser pane
1. `session.create`
2. `system.readiness`
3. `browser.create`
4. `browser.subscribe`
5. `browser.tab.open`
6. `browser.goto`
7. `browser.history` on reconnect
8. `browser.close`

### Agent pane
1. `session.create`
2. `system.readiness`
3. `agent.worker.create`
4. optional `agent.attach.browser`
5. `agent.task.start`
6. `agent.task.get` or `agent.worker.get` during execution
7. `agent.task.cancel` or `agent.worker.close`

### Diagnostics page
1. `session.create` with diagnostics scope or default scopes
2. `system.readiness`
3. `system.diagnostics`
4. `system.metrics`
5. `system.logs`

## Frontend Rules That Must Hold

- Never gate features from `system.health` alone.
- Always preserve backend IDs exactly.
- Always track last seen `sequence` for terminal and browser panes.
- Always use history APIs for reconnect or sequence gaps.
- Never assume a browser attachment is shareable across workers.
- Never treat `RATE_LIMITED` as a generic transient network error.
- Build against the backend contract, not assumptions about local process embedding or browser rendering internals.
