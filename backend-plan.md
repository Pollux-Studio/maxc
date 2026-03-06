# Backend Implementation Plan v2

## Overview
This plan keeps the completed Phase 1-6 backend as the baseline and tracks the remaining work to reach a real Windows-first terminal, browser, and multi-agent backend.

Each phase ends with two mandatory closing tasks in this order:
1. `Phase Quality Gate (Mandatory)`
2. `Commit Phase Changes (Mandatory)`

Global validation commands for every quality gate:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features --offline -- -D warnings
cargo test --workspace --all-features --offline
cargo llvm-cov --workspace --all-features --fail-under-lines 85
```

---

## Phase 1: Foundation and Contracts

### Tasks
- [x] Create backend module layout: `automation`, `core`, `browser`, `terminal`, `storage`, `security`, `telemetry`.
- [x] Define JSON-RPC contracts, error codes, typed IDs, and core config.
- [x] Add terminal and browser contract models.
- [x] Add CI baseline for format, lint, tests, and coverage reporting.
- [x] **Phase Quality Gate (Mandatory)**
- [x] **Commit Phase Changes (Mandatory)**

## Phase 2: IPC RPC Server and Security

### Tasks
- [x] Implement authenticated local JSON-RPC server and dispatcher.
- [x] Add session APIs, rate limits, validation, timeouts, and error mapping.
- [x] Add browser and terminal route registration with audit fields.
- [x] **Phase Quality Gate (Mandatory)**
- [x] **Commit Phase Changes (Mandatory)**

## Phase 3: Event Store and State Recovery

### Tasks
- [x] Implement append-only event store, projections, snapshots, and replay.
- [x] Add command idempotency and compaction.
- [x] Extend durable state for browser and terminal flows.
- [x] **Phase Quality Gate (Mandatory)**
- [x] **Commit Phase Changes (Mandatory)**

## Phase 4: Execution Core

### Tasks
- [x] Implement terminal and browser lifecycle RPC methods.
- [x] Implement subscription fan-out, buffering, scheduler rules, and lifecycle cleanup.
- [x] Implement simulated browser and terminal execution state.
- [x] **Phase Quality Gate (Mandatory)**
- [x] **Commit Phase Changes (Mandatory)**

## Phase 5: Performance and Reliability Hardening

### Tasks
- [x] Add benchmark harness, overload rejection, breaker, graceful shutdown, and fault injection.
- [x] Add regression guardrails and performance baselines.
- [x] **Phase Quality Gate (Mandatory)**
- [x] **Commit Phase Changes (Mandatory)**

## Phase 6: Operability and Release Readiness

### Tasks
- [x] Add logs, metrics, traces, diagnostics, readiness, and CLI support.
- [x] Add compatibility coverage and operational documentation.
- [x] **Phase Quality Gate (Mandatory)**
- [x] **Commit Phase Changes (Mandatory)**

---

## Phase 7: Real Terminal Runtime

### Tasks
- [x] Replace the remaining simulated terminal execution path with Windows ConPTY-backed execution and resize behavior, with `process-stdio` retained as the non-Windows/test fallback.
- [x] Implement real local process spawning behind `terminal.spawn`.
- [x] Support arbitrary programs, optional args, working directory, and environment injection for spawned terminal sessions.
- [x] Implement true stdin writes for `terminal.input`.
- [x] Implement stdout/stderr streaming and real `terminal.output` subscription events.
- [x] Implement terminal process exit tracking with `pid`, `status`, and `exit_code` metadata.
- [x] Implement in-process terminal kill signaling and lifecycle cleanup for shutdown.
- [x] Keep the existing `terminal.*` RPC method names stable while adding additive real-runtime response fields.
- [x] Update terminal tests and docs to reflect the Windows ConPTY runtime and non-Windows `process-stdio` fallback.
- [x] Add explicit terminal history/readback RPC for reconnect-safe frontend redraw.
- [x] Add terminal quotas and runtime policy controls.
- [x] **Phase Quality Gate (Mandatory)**
- [x] **Commit Phase Changes (Mandatory)**

## Phase 8: Real Browser Runtime

### Tasks
- [x] Replace the simulated browser runtime with a real Chromium-backed engine, with synthetic fallback only when the environment cannot launch a browser.
- [x] Implement real page/tab lifecycle and navigation.
- [x] Implement real browser automation, downloads, screenshots, tracing, and network controls.
- [x] Implement real browser event streaming and runtime cleanup.
- [x] Separate durable browser metadata from live runtime handles.
- [x] Update diagnostics, telemetry, and docs for the real browser runtime.
- [x] **Phase Quality Gate (Mandatory)**
- [x] **Commit Phase Changes (Mandatory)**

## Phase 9: Frontend Runtime Contract

### Tasks
- [ ] Finalize terminal and browser event payload contracts with sequence/cursor support.
- [ ] Add reconnect-safe runtime status and readback APIs for frontend rendering.
- [ ] Finalize session/action gating and degraded-state contracts for the frontend.
- [ ] Add CLI and integration coverage for reconnect-safe flows.
- [ ] **Phase Quality Gate (Mandatory)**
- [ ] **Commit Phase Changes (Mandatory)**

## Phase 10: Multi-Agent Orchestration

### Tasks
- [ ] Add agent worker, task, and delegation models.
- [ ] Add RPCs for worker lifecycle, task routing, inspection, and cancellation.
- [ ] Allow multiple concurrent agent workers to own real terminal and browser resources safely.
- [ ] Expose worker state in diagnostics and frontend-facing status payloads.
- [ ] **Phase Quality Gate (Mandatory)**
- [ ] **Commit Phase Changes (Mandatory)**

## Phase 11: Security and Isolation

### Tasks
- [ ] Add allow/deny policy for spawned programs, cwd roots, env forwarding, and raw browser access.
- [ ] Add per-session quotas and artifact retention rules.
- [ ] Add redaction and stronger audit behavior for runtime and diagnostics data.
- [ ] Add token-scope and ownership controls for runtime and agent actions.
- [ ] **Phase Quality Gate (Mandatory)**
- [ ] **Commit Phase Changes (Mandatory)**

## Phase 12: Real Runtime Release Gate

### Tasks
- [ ] Replace synthetic benchmark assumptions with real terminal/browser latency benchmarks.
- [ ] Add stress, crash, restart, and cleanup coverage for real runtimes.
- [ ] Finalize readiness, shutdown, and release checks for real dependencies.
- [ ] Update all runtime docs and examples to match shipped behavior.
- [ ] **Phase Quality Gate (Mandatory)**
- [ ] **Commit Phase Changes (Mandatory)**
