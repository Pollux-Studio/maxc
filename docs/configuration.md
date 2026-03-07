# Configuration

The backend reads `BackendConfig` from environment variables. Defaults are defined in `backend/core`.

## Core RPC and Scheduling

| Env Var | Default | Meaning |
| --- | --- | --- |
| `MAXC_SOCKET_PATH` | `\\.\pipe\maxc-rpc` | Windows named-pipe path |
| `MAXC_REQUEST_TIMEOUT_MS` | `5000` | Per-request timeout |
| `MAXC_QUEUE_LIMIT` | `1024` | Shared queue and buffer bound |
| `MAXC_MAX_PAYLOAD_BYTES` | `65536` | Max raw request size |
| `MAXC_MAX_INFLIGHT_PER_CONNECTION` | `64` | Per-connection concurrency limit |
| `MAXC_SESSION_TTL_MS` | `3600000` | Session lifetime |
| `MAXC_RATE_LIMIT_PER_SEC` | `100` | Base request rate limit |
| `MAXC_BURST_LIMIT` | `200` | Token bucket burst size |

## Event Store

| Env Var | Default | Meaning |
| --- | --- | --- |
| `MAXC_EVENT_DIR` | `.maxc/events` | Event-store root and runtime artifact parent |
| `MAXC_SEGMENT_MAX_BYTES` | `1048576` | Event segment rotation threshold |
| `MAXC_SNAPSHOT_INTERVAL_EVENTS` | `100` | Snapshot interval |
| `MAXC_SNAPSHOT_RETAIN_COUNT` | `3` | Snapshot retention count |

## Browser Runtime

| Env Var | Default | Meaning |
| --- | --- | --- |
| `MAXC_BROWSER_RUNTIME` | `chromium` | Declared browser runtime |
| `MAXC_BROWSER_DRIVER` | `playwright` | Declared driver label |
| `MAXC_BROWSER_EXECUTABLE_OR_CHANNEL` | `chromium` | Chromium executable or channel selector |
| `MAXC_BROWSER_LAUNCH_ARGS` | empty | Semicolon-delimited extra launch arguments |
| `MAXC_BROWSER_MAX_CONTEXTS` | `8` | Browser session limit |
| `MAXC_BROWSER_NAV_TIMEOUT_MS` | `30000` | Navigation timeout |
| `MAXC_BROWSER_ACTION_TIMEOUT_MS` | `10000` | Action timeout |
| `MAXC_BROWSER_SCREENSHOT_MAX_BYTES` | `5242880` | Screenshot size cap |
| `MAXC_BROWSER_DOWNLOAD_MAX_BYTES` | `52428800` | Download size cap |
| `MAXC_BROWSER_SUBSCRIPTION_LIMIT` | `32` | Per-session subscription cap used by browser and terminal subscriptions |
| `MAXC_BROWSER_RAW_RATE_LIMIT_PER_SEC` | `10` | Raw browser command rate limit |
| `MAXC_BROWSER_ALLOW_RAW_COMMANDS` | `true` | Enables `browser.raw.command` |
| `MAXC_BROWSER_ALLOWED_DOWNLOAD_ROOTS` | empty | Optional download allowlist |
| `MAXC_BROWSER_ALLOWED_UPLOAD_ROOTS` | empty | Optional upload source allowlist |
| `MAXC_BROWSER_ALLOWED_TRACE_ROOTS` | empty | Optional trace output allowlist |
| `MAXC_BROWSER_MAX_TABS_PER_SESSION` | `16` | Per-session tab cap |

## Terminal Runtime

| Env Var | Default | Meaning |
| --- | --- | --- |
| `MAXC_TERMINAL_RUNTIME` | `conpty` on Windows, `process-stdio` elsewhere | Preferred terminal runtime |
| `MAXC_TERMINAL_MAX_SESSIONS` | `32` | Global terminal session cap |
| `MAXC_TERMINAL_MAX_SESSIONS_PER_WORKSPACE` | `8` | Per-workspace terminal session cap |
| `MAXC_TERMINAL_MAX_HISTORY_EVENTS` | `512` | Buffered terminal event cap |
| `MAXC_TERMINAL_MAX_HISTORY_BYTES` | `262144` | Buffered terminal byte cap |
| `MAXC_TERMINAL_MAX_INPUT_BYTES` | `8192` | Max bytes accepted by one `terminal.input` |
| `MAXC_TERMINAL_MAX_ENV_BYTES` | `8192` | Max combined environment bytes for spawned terminals |
| `MAXC_TERMINAL_ALLOWED_CWD_ROOTS` | empty | Optional working-directory allowlist |
| `MAXC_TERMINAL_ALLOWED_PROGRAMS` | empty | Optional spawned-program allowlist |
| `MAXC_ENV_ALLOWLIST` | empty | Optional allowed env keys for terminal and agent launches |

## Agent and Artifact Control

| Env Var | Default | Meaning |
| --- | --- | --- |
| `MAXC_AGENT_ALLOWED_WORKSPACE_ROOTS` | empty | Optional agent working-directory allowlist |
| `MAXC_AGENT_ALLOWED_PROGRAMS` | empty | Optional agent launch-program allowlist |
| `MAXC_AGENT_MAX_WORKERS` | `8` | Concurrent worker cap |
| `MAXC_AGENT_MAX_TASKS_PER_WORKER` | `8` | Running-task cap per worker |
| `MAXC_ARTIFACT_MAX_FILES` | `256` | Global retained browser artifact file cap |
| `MAXC_ARTIFACT_MAX_TOTAL_BYTES` | `268435456` | Global retained browser artifact byte cap |
| `MAXC_ARTIFACT_TTL_MS` | `86400000` | Artifact retention TTL |
| `MAXC_ARTIFACT_MAX_FILES_PER_SESSION` | `64` | Per-browser-session artifact cap |
| `MAXC_DEFAULT_SESSION_SCOPES` | `diagnostics;runtime;agent` | Default scopes issued by `session.create` |

## Shutdown, Overload, and Breaker

| Env Var | Default | Meaning |
| --- | --- | --- |
| `MAXC_SHUTDOWN_DRAIN_TIMEOUT_MS` | `3000` | Graceful shutdown drain window |
| `MAXC_OVERLOAD_REJECT_THRESHOLD` | `1024` | Reject-new-work threshold |
| `MAXC_BREAKER_FAILURE_THRESHOLD` | `5` | Consecutive failures before breaker opens |
| `MAXC_BREAKER_COOLDOWN_MS` | `10000` | Breaker cooldown window |
| `MAXC_LOG_LEVEL` | `info` | Declared log verbosity |

## Operational Notes

- `system.readiness` depends on actual terminal runtime availability, browser runtime availability, artifact-root writability, and event-store writability.
- `MAXC_EVENT_DIR` affects both recovery and browser artifact storage, so unwritable paths degrade readiness.
- Browser upload, download, trace, and raw-command behavior should be considered unsafe-by-default for untrusted local workflows unless the matching allowlists and toggles are set.
- Narrower scopes through `MAXC_DEFAULT_SESSION_SCOPES` are useful for operator-only or diagnostics-only clients.
- Use terminal and agent allowlists before exposing backend execution to semi-trusted local automation.
