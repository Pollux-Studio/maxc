# Testing

## Standard Backend Quality Gate

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features --offline -- -D warnings
cargo test --workspace --all-features --offline
cargo llvm-cov --workspace --all-features --fail-under-lines 85
```

## What the Current Tests Cover

- `backend/automation/src/server.rs`
  - auth and scope checks
  - request validation and error mapping
  - terminal runtime flows
  - browser runtime flows
  - agent worker and task flows
  - overload, breaker, shutdown, recovery, and fault injection
  - readiness, diagnostics, metrics, and logs
- `backend/automation/tests`
  - JSON-RPC compatibility and contract stability
- `backend/cli/src/main.rs`
  - command parsing, request construction, and in-process smoke flows
- `backend/storage/src/lib.rs`
  - append, replay, checksum, snapshots, and compaction
- `backend/core/src/lib.rs`
  - configuration parsing and validation
- `backend/telemetry/src/lib.rs`
  - collector and latency snapshot behavior

## Perf Harness Modes

### Synthetic mode

```bash
cargo run -p maxc-automation --bin perf-harness --offline -- --mode synthetic --profile ci --json
```

Use this for deterministic CI validation and hot-path regression checks.

Profiles cover:
- `rpc_health`
- `session_lifecycle`
- `terminal_interactive`
- `browser_navigation`
- `browser_fanout`
- `restart_recovery`

### Real-runtime mode

```bash
cargo run -p maxc-automation --bin perf-harness --offline -- --mode real-runtime --profile ci --json
```

Use this on Windows to validate the shipped runtime path.

Profiles cover:
- `terminal_spawn_latency`
- `terminal_interactive`
- `browser_create_latency`
- `browser_navigation_latency`
- `browser_screenshot_latency`
- `agent_worker_start_latency`

Synthetic mode is the normal CI baseline. Real-runtime mode is the release/runtime validation pass.

## When to Re-run Which Checks

Run the full quality gate when changing:
- RPC dispatch or request validation
- storage or recovery behavior
- readiness, diagnostics, metrics, or logs
- terminal, browser, or agent runtime behavior
- policy, quota, or ownership enforcement

Run perf harness when changing:
- dispatch hot paths
- history or subscription buffering
- storage replay or persistence path
- terminal, browser, or agent startup behavior
- observability code that touches every request

## Documentation Verification

When changing docs or examples:
1. Verify the method or CLI command exists.
2. Verify required scopes and required IDs match the code.
3. Verify example fields are actually returned today.
4. Re-run at least the touched crate tests if the docs describe behavior that recently changed.

## Offline Notes

- Use `--offline` when dependencies are already in the lockfile.
- The first dependency addition or lockfile refresh may require network access outside restricted environments.
