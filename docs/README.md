# Documentation

This directory is the source of truth for backend behavior, usage, operations, and development workflow.

## Start Here

- `backend-overview.md`: what the backend does today.
- `architecture.md`: crate layout and request/data flow.
- `rpc-api.md`: JSON-RPC methods, auth, examples, and errors.
- `frontend-integration.md`: frontend-facing contract, screen flows, polling/subscription expectations, and UI mapping.
- `cli.md`: CLI commands, flags, examples, and transport behavior.
- `configuration.md`: environment variables, defaults, and tuning.
- `operations.md`: health checks, diagnostics, shutdown, recovery, and perf guardrails.
- `testing.md`: fmt, clippy, tests, coverage, and perf verification.
- `development.md`: how to extend the backend safely.

## Quick Usage

Create a session:

```bash
cargo run -p maxc-cli -- session create
```

Check health:

```bash
cargo run -p maxc-cli -- health --pretty
```

Run the backend validation suite:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features --offline -- -D warnings
cargo test --workspace --all-features --offline
cargo llvm-cov --workspace --all-features --fail-under-lines 85
```

## Frontend Implementation Note

The backend docs are intended to be sufficient for frontend implementation. Start with:

1. `frontend-integration.md` for screen behavior and data flow.
2. `rpc-api.md` for exact request and error expectations.
3. `operations.md` for readiness and diagnostics behavior.
