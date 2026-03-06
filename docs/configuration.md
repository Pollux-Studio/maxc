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
| `MAXC_SHUTDOWN_DRAIN_TIMEOUT_MS` | `3000` | Graceful shutdown wait window |
| `MAXC_OVERLOAD_REJECT_THRESHOLD` | `1024` | Reject-new-work threshold |
| `MAXC_BREAKER_FAILURE_THRESHOLD` | `5` | Consecutive failure count before opening breaker |
| `MAXC_BREAKER_COOLDOWN_MS` | `10000` | Breaker cooldown |
| `MAXC_LOG_LEVEL` | `info` | Declared log verbosity |

## Recommended Usage

- Use defaults for local development first.
- Raise `MAXC_EVENT_DIR` to a persistent path in CI or long-lived runs.
- Configure `MAXC_TERMINAL_ALLOWED_CWD_ROOTS` and `MAXC_TERMINAL_ALLOWED_PROGRAMS` before exposing terminal spawn to untrusted local workflows.
- Lower `MAXC_OVERLOAD_REJECT_THRESHOLD` only when testing overload behavior.
- Keep `MAXC_BREAKER_FAILURE_THRESHOLD` and `MAXC_BREAKER_COOLDOWN_MS` conservative in development so failures are visible quickly.
