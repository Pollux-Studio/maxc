# CLI

The backend ships a local CLI crate named `maxc-cli`.

Run it through Cargo:

```bash
cargo run -p maxc-cli -- health --pretty
```

## Transport

- On Windows, the CLI connects to the local named pipe at `\\.\pipe\maxc-rpc`.
- On non-Windows platforms, the named-pipe transport is not implemented.
- The CLI prints JSON. Use `--pretty` for formatted output.

## Commands

### Health

```bash
cargo run -p maxc-cli -- health --pretty
```

### Readiness

```bash
cargo run -p maxc-cli -- readiness --token <token> --pretty
```

### Diagnostics

```bash
cargo run -p maxc-cli -- diagnostics --token <token> --pretty
```

### Session

Create:

```bash
cargo run -p maxc-cli -- session create
```

Refresh:

```bash
cargo run -p maxc-cli -- session refresh --token <token>
```

Revoke:

```bash
cargo run -p maxc-cli -- session revoke --token <token>
```

### Terminal

Spawn:

```bash
cargo run -p maxc-cli -- terminal spawn --token <token> --workspace-id ws-1 --surface-id sf-1
```

Input:

```bash
cargo run -p maxc-cli -- terminal input --token <token> --workspace-id ws-1 --surface-id sf-1 --terminal-session-id ts-123 --input "echo hello"
```

Resize:

```bash
cargo run -p maxc-cli -- terminal resize --token <token> --workspace-id ws-1 --surface-id sf-1 --terminal-session-id ts-123 --cols 140 --rows 40
```

Kill:

```bash
cargo run -p maxc-cli -- terminal kill --token <token> --workspace-id ws-1 --surface-id sf-1 --terminal-session-id ts-123
```

### Browser

Create:

```bash
cargo run -p maxc-cli -- browser create --token <token> --workspace-id ws-1 --surface-id sf-1
```

Open tab:

```bash
cargo run -p maxc-cli -- browser tab-open --token <token> --workspace-id ws-1 --surface-id sf-1 --browser-session-id bs-123 --url https://example.com
```

Navigate:

```bash
cargo run -p maxc-cli -- browser goto --token <token> --workspace-id ws-1 --surface-id sf-1 --browser-session-id bs-123 --tab-id tab-123 --url https://example.com/dashboard
```

Close:

```bash
cargo run -p maxc-cli -- browser close --token <token> --workspace-id ws-1 --surface-id sf-1 --browser-session-id bs-123
```

## Proper Usage

- Create a session first and reuse the returned token until refresh or revoke.
- Prefer `--pretty` for manual inspection and default compact JSON for scripting.
- Treat the CLI as a thin RPC client. It does not add retries, queueing, or client-side caching.
- Use shell-safe quoting for `--input` and `--url`.
