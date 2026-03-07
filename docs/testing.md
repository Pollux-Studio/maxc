# Testing

## Standard Validation

Run the full backend quality gate:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features --offline -- -D warnings
cargo test --workspace --all-features --offline
cargo llvm-cov --workspace --all-features --fail-under-lines 85
```

## What Each Layer Covers

- `backend/automation/src/server.rs` tests:
  - auth
  - request validation
  - terminal flows
  - browser flows
  - reliability behavior
  - diagnostics and observability
- `backend/automation/tests/rpc_compatibility.rs`:
  - additive JSON-RPC stability
  - error-code stability
- `backend/cli/src/main.rs`:
  - command parsing
  - request construction
  - in-process smoke tests
- `backend/telemetry/src/lib.rs`:
  - collector and latency snapshot behavior
- `backend/storage/src/lib.rs`:
  - append, replay, checksum, and compaction

## Perf Verification

```bash
cargo run -p maxc-automation --bin perf-harness --offline -- --mode synthetic --profile ci --json
```

Use the perf harness when changing:

- request dispatch
- storage replay or persistence behavior
- subscription fan-out
- overload controls
- diagnostics or instrumentation that touches hot paths

Use real-runtime validation on a Windows machine with local terminal and browser dependencies available:

```bash
cargo run -p maxc-automation --bin perf-harness --offline -- --mode real-runtime --profile ci --json
```

- `--mode synthetic` is the deterministic CI baseline.
- `--mode real-runtime` validates ConPTY, real browser startup, screenshots, and agent worker startup against `perf-baseline-real.json`.
- The real-runtime suite is expected to run on Windows only.

## Offline Notes

- Use `--offline` whenever the lockfile already contains all needed dependencies.
- If you introduce a new dependency, the first lockfile update may require network access outside the sandbox.

## Proper Example Validation

When updating docs or examples:

1. Verify the CLI command exists.
2. Verify the RPC method exists.
3. Verify required auth and IDs match the implementation.
4. Re-run at least the targeted crate tests for the touched surface.

## Release Gate

For a release-quality backend change, run:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features --offline -- -D warnings
cargo test --workspace --all-features --offline
cargo run -p maxc-automation --bin perf-harness --offline -- --mode synthetic --profile ci --json
cargo llvm-cov --workspace --all-features --fail-under-lines 85
```

Then run the Windows-only real-runtime perf suite before calling the backend release-ready.
