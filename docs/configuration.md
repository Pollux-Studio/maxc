# Configuration

The backend reads `BackendConfig` from environment variables. Defaults are defined in `backend/core`.

## Core RPC and Runtime

| Env Var | Default | Purpose |
| --- | --- | --- |
| `MAXC_SOCKET_PATH` | `\\.\pipe\maxc-rpc` | Windows named-pipe path |
| `MAXC_REQUEST_TIMEOUT_MS` | `5000` | Per-request timeout |
| `MAXC_QUEUE_LIMIT` | `1024` | General queue and subscriber buffer limit |
| `MAXC_MAX_PAYLOAD_BYTES` | `65536` | Max raw request size |
| `MAXC_MAX_INFLIGHT_PER_CONNECTION` | `64` | Connection-level concurrency cap |
| `MAXC_SESSION_TTL_MS` | `3600000` | Session lifetime |
| `MAXC_RATE_LIMIT_PER_SEC` | `100` | Global and per-connection request rate |
| `MAXC_BURST_LIMIT` | `200` | Burst token bucket size |

## Storage

| Env Var | Default | Purpose |
| --- | --- | --- |
| `MAXC_EVENT_DIR` | `.maxc/events` | Event store location |
| `MAXC_SEGMENT_MAX_BYTES` | `1048576` | Segment rotation threshold |
| `MAXC_SNAPSHOT_INTERVAL_EVENTS` | `100` | Snapshot interval |
| `MAXC_SNAPSHOT_RETAIN_COUNT` | `3` | Snapshot retention count |

## Browser

| Env Var | Default | Purpose |
| --- | --- | --- |
| `MAXC_BROWSER_RUNTIME` | `chromium` | Declared runtime |
| `MAXC_BROWSER_DRIVER` | `playwright` | Declared driver |
| `MAXC_BROWSER_EXECUTABLE_OR_CHANNEL` | `chromium` | Channel/executable selector |
| `MAXC_BROWSER_LAUNCH_ARGS` | empty | Semicolon-delimited launch arguments |
| `MAXC_BROWSER_MAX_CONTEXTS` | `8` | Context/session cap |
| `MAXC_BROWSER_NAV_TIMEOUT_MS` | `30000` | Navigation timeout |
| `MAXC_BROWSER_ACTION_TIMEOUT_MS` | `10000` | Action timeout |
| `MAXC_BROWSER_SCREENSHOT_MAX_BYTES` | `5242880` | Screenshot size cap |
| `MAXC_BROWSER_DOWNLOAD_MAX_BYTES` | `52428800` | Download size cap |
| `MAXC_BROWSER_SUBSCRIPTION_LIMIT` | `32` | Browser and terminal subscriber count cap |
| `MAXC_BROWSER_RAW_RATE_LIMIT_PER_SEC` | `10` | Raw-command throttle |
| `MAXC_BROWSER_ALLOW_RAW_COMMANDS` | `true` | Permissive local-dev default for `browser.raw.command` |
| `MAXC_BROWSER_ALLOWED_DOWNLOAD_ROOTS` | empty | Optional semicolon-delimited allowlist for backend-managed download roots |
| `MAXC_BROWSER_ALLOWED_UPLOAD_ROOTS` | empty | Optional semicolon-delimited allowlist for upload source paths |
| `MAXC_BROWSER_ALLOWED_TRACE_ROOTS` | empty | Optional semicolon-delimited allowlist for trace artifact roots |
| `MAXC_BROWSER_MAX_TABS_PER_SESSION` | `16` | Open-tab cap per browser session |

## Reliability and Operability

| Env Var | Default | Purpose |
| --- | --- | --- |
| `MAXC_TERMINAL_RUNTIME` | `conpty` on Windows, `process-stdio` elsewhere | Declared terminal runtime preference |
| `MAXC_TERMINAL_MAX_SESSIONS` | `32` | Global terminal session cap |
| `MAXC_TERMINAL_MAX_SESSIONS_PER_WORKSPACE` | `8` | Per-workspace terminal session cap |
| `MAXC_TERMINAL_MAX_HISTORY_EVENTS` | `512` | Buffered terminal history event cap |
| `MAXC_TERMINAL_MAX_HISTORY_BYTES` | `262144` | Buffered terminal history byte cap |
| `MAXC_TERMINAL_MAX_INPUT_BYTES` | `8192` | Max bytes accepted by one `terminal.input` request |
| `MAXC_TERMINAL_MAX_ENV_BYTES` | `8192` | Max combined environment bytes for spawned terminal sessions |
| `MAXC_TERMINAL_ALLOWED_CWD_ROOTS` | empty | Optional semicolon-delimited working-directory allowlist |
| `MAXC_TERMINAL_ALLOWED_PROGRAMS` | empty | Optional semicolon-delimited program allowlist |
| `MAXC_ENV_ALLOWLIST` | empty | Optional semicolon-delimited env-key allowlist for terminal and agent launches |
| `MAXC_AGENT_ALLOWED_WORKSPACE_ROOTS` | empty | Optional semicolon-delimited cwd/workspace allowlist for agent workers |
| `MAXC_AGENT_ALLOWED_PROGRAMS` | empty | Optional semicolon-delimited program allowlist for agent-backed terminal launches |
| `MAXC_AGENT_MAX_WORKERS` | `8` | Concurrent agent worker cap |
| `MAXC_AGENT_MAX_TASKS_PER_WORKER` | `8` | Running-task cap per worker |
| `MAXC_ARTIFACT_MAX_FILES` | `256` | Global retained browser artifact file cap |
| `MAXC_ARTIFACT_MAX_TOTAL_BYTES` | `268435456` | Global retained browser artifact byte cap |
| `MAXC_ARTIFACT_TTL_MS` | `86400000` | Artifact retention TTL |
| `MAXC_ARTIFACT_MAX_FILES_PER_SESSION` | `64` | Per-browser-session artifact file cap |
| `MAXC_DEFAULT_SESSION_SCOPES` | `diagnostics;runtime;agent` | Default token scopes returned by `session.create` |
| `MAXC_SHUTDOWN_DRAIN_TIMEOUT_MS` | `3000` | Graceful shutdown wait window |
| `MAXC_OVERLOAD_REJECT_THRESHOLD` | `1024` | Reject-new-work threshold |
| `MAXC_BREAKER_FAILURE_THRESHOLD` | `5` | Consecutive failure count before opening breaker |
| `MAXC_BREAKER_COOLDOWN_MS` | `10000` | Breaker cooldown |
| `MAXC_LOG_LEVEL` | `info` | Declared log verbosity |

## Recommended Usage

- Use defaults for local development first.
- Raise `MAXC_EVENT_DIR` to a persistent path in CI or long-lived runs.
- Configure `MAXC_TERMINAL_ALLOWED_CWD_ROOTS` and `MAXC_TERMINAL_ALLOWED_PROGRAMS` before exposing terminal spawn to untrusted local workflows.
- Keep `MAXC_BROWSER_ALLOW_RAW_COMMANDS=true` only for trusted local workflows. Set it to `false` and require explicit config when tightening local security.
- Set the browser and agent allowlists before letting untrusted agents control uploads, downloads, or custom programs.
- Use `MAXC_DEFAULT_SESSION_SCOPES` to issue diagnostics-only tokens for operator panels and narrower runtime tokens for UI clients.
- Tune artifact retention with the `MAXC_ARTIFACT_*` settings instead of relying on manual cleanup.
- Lower `MAXC_OVERLOAD_REJECT_THRESHOLD` only when testing overload behavior.
- Keep `MAXC_BREAKER_FAILURE_THRESHOLD` and `MAXC_BREAKER_COOLDOWN_MS` conservative in development so failures are visible quickly.
- Release readiness depends on four runtime checks: terminal runtime availability, browser runtime availability, artifact root writability, and event-store writability. Use `system.readiness` and `system.diagnostics` to confirm all four before enabling full frontend workflows.
