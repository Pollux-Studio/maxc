# CLI

The backend ships a local command-line client crate named `maxc-cli`. It is a thin JSON-RPC client for manual checks, scripts, and smoke testing.

## Transport and Output

- Windows transport: local named pipe at `\\.\pipe\maxc-rpc`
- Non-Windows transport: not implemented in the CLI
- Default output: compact JSON
- Optional output: `--pretty` for formatted JSON

Example:

```bash
cargo run -p maxc-cli -- --pretty health
```

## Command Families

### Health and diagnostics

```bash
cargo run -p maxc-cli -- health
cargo run -p maxc-cli -- readiness --token <token>
cargo run -p maxc-cli -- diagnostics --token <token>
```

### Session

```bash
cargo run -p maxc-cli -- session create
cargo run -p maxc-cli -- session refresh --token <token>
cargo run -p maxc-cli -- session revoke --token <token>
```

### Terminal

```bash
cargo run -p maxc-cli -- terminal spawn --token <token> --workspace-id ws-1 --surface-id sf-1 --cols 120 --rows 30
cargo run -p maxc-cli -- terminal input --token <token> --workspace-id ws-1 --surface-id sf-1 --terminal-session-id ts-123 --input "echo hello"
cargo run -p maxc-cli -- terminal resize --token <token> --workspace-id ws-1 --surface-id sf-1 --terminal-session-id ts-123 --cols 140 --rows 40
cargo run -p maxc-cli -- terminal history --token <token> --workspace-id ws-1 --surface-id sf-1 --terminal-session-id ts-123 --from-sequence 10 --max-events 20
cargo run -p maxc-cli -- terminal kill --token <token> --workspace-id ws-1 --surface-id sf-1 --terminal-session-id ts-123
```

### Browser

```bash
cargo run -p maxc-cli -- browser create --token <token> --workspace-id ws-1 --surface-id sf-1
cargo run -p maxc-cli -- browser tab-open --token <token> --workspace-id ws-1 --surface-id sf-1 --browser-session-id bs-123 --url https://example.com
cargo run -p maxc-cli -- browser goto --token <token> --workspace-id ws-1 --surface-id sf-1 --browser-session-id bs-123 --tab-id tab-123 --url https://example.com/dashboard
cargo run -p maxc-cli -- browser history --token <token> --workspace-id ws-1 --surface-id sf-1 --browser-session-id bs-123 --from-sequence 4 --max-events 50
cargo run -p maxc-cli -- browser close --token <token> --workspace-id ws-1 --surface-id sf-1 --browser-session-id bs-123
```

### Agent

```bash
cargo run -p maxc-cli -- agent worker create --token <token> --workspace-id ws-1 --surface-id sf-1
cargo run -p maxc-cli -- agent worker list --token <token> --workspace-id ws-1 --surface-id sf-1
cargo run -p maxc-cli -- agent worker get --token <token> --workspace-id ws-1 --surface-id sf-1 --agent-worker-id aw-123
cargo run -p maxc-cli -- agent worker close --token <token> --workspace-id ws-1 --surface-id sf-1 --agent-worker-id aw-123
cargo run -p maxc-cli -- agent task start --token <token> --workspace-id ws-1 --surface-id sf-1 --agent-worker-id aw-123 --prompt "run tests"
cargo run -p maxc-cli -- agent task list --token <token> --workspace-id ws-1 --surface-id sf-1
cargo run -p maxc-cli -- agent task get --token <token> --workspace-id ws-1 --surface-id sf-1 --agent-task-id at-123
cargo run -p maxc-cli -- agent task cancel --token <token> --workspace-id ws-1 --surface-id sf-1 --agent-task-id at-123 --reason "user cancel"
cargo run -p maxc-cli -- agent attach terminal --token <token> --workspace-id ws-1 --surface-id sf-1 --agent-worker-id aw-123 --terminal-session-id ts-123
cargo run -p maxc-cli -- agent detach terminal --token <token> --workspace-id ws-1 --surface-id sf-1 --agent-worker-id aw-123
cargo run -p maxc-cli -- agent attach browser --token <token> --workspace-id ws-1 --surface-id sf-1 --agent-worker-id aw-123 --browser-session-id bs-123
cargo run -p maxc-cli -- agent detach browser --token <token> --workspace-id ws-1 --surface-id sf-1 --agent-worker-id aw-123
```

## Usage Notes

- Create a session first and reuse the returned token until refresh or revoke.
- The CLI does not add client-side retries, caching, or state repair.
- Each mutating CLI invocation generates a fresh `command_id`; idempotent replay only happens if a caller intentionally reuses an ID.
- The CLI is backend-focused. It is suitable for smoke checks and operator workflows, not for streaming terminal rendering.
- `terminal input` sends the exact provided value. Add newline characters explicitly when needed.
- Use shell-safe quoting for `--input`, `--prompt`, and `--url`.
- Use `--pretty` for manual inspection and default compact output for scripting.
