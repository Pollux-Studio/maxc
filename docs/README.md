# Documentation

This directory is the backend documentation source of truth. It describes the shipped backend contract, runtime behavior, operations, and development workflow.

## Start Here

- `backend-overview.md`: backend capabilities and current runtime behavior.
- `frontend-integration.md`: screen-level frontend contract, reconnect rules, and UI gating.
- `rpc-api.md`: method-level JSON-RPC contract, scopes, IDs, and error behavior.
- `operations.md`: readiness, diagnostics, shutdown, recovery, and release checks.
- `configuration.md`: every `BackendConfig` environment variable and operational meaning.
- `cli.md`: `maxc-cli` commands, flags, and example backend workflows.
- `testing.md`: validation commands, perf harness modes, and release gate checks.
- `architecture.md`: crate responsibilities and request/runtime/persistence flow.
- `development.md`: how to extend backend behavior and keep docs aligned.

## Recommended Reading Order

1. Frontend teams: `frontend-integration.md` -> `rpc-api.md` -> `operations.md`
2. Backend contributors: `backend-overview.md` -> `architecture.md` -> `development.md`
3. Operators: `operations.md` -> `configuration.md` -> `testing.md`

## Quick Backend Checks

```bash
cargo run -p maxc-cli -- health --pretty
cargo run -p maxc-cli -- session create
cargo run -p maxc-cli -- readiness --token <token> --pretty
```

## Validation Gate

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features --offline -- -D warnings
cargo test --workspace --all-features --offline
cargo llvm-cov --workspace --all-features --fail-under-lines 85
```

Use `docs/` pages instead of top-level files for detailed backend behavior. Top-level docs should stay summary-level and point here.
