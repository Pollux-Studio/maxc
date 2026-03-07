# Repository Guidelines

## Project Structure & Module Organization
This repository is currently documentation-first. Top-level files:

- `README.md`: product vision, architecture, and planned module layout.
- `CONTRIBUTION.md`: contributor workflow, Rust setup, style, and testing basics.
- `BRANCHING.md`: branch model and release flow.
- `LICENSE`: licensing terms.
- `docs/`: detailed backend documentation, usage, operations, testing, and development workflow.

As implementation code is added, keep modules aligned with the architecture in `README.md` (`core/`, `terminal/`, `browser/`, `automation/`, `ui/`, `notifications/`, `cli/`, `config/`).

## Build, Test, and Development Commands
Use Rust tooling (documented in `CONTRIBUTION.md`):

- `cargo build`: compile the project.
- `cargo run`: run the local app.
- `cargo test`: execute all tests.
- `cargo fmt`: format code before commit.
- `cargo clippy`: run lints and catch common issues.

Recommended pre-PR check:
`cargo fmt && cargo clippy && cargo test`

## Coding Style & Naming Conventions
- Follow idiomatic Rust and keep functions small and readable.
- Use descriptive names for modules, types, and variables.
- Document public APIs with Rust doc comments.
- Branch names must follow:
  - `feature/<name>`
  - `bugfix/<name>`
  - `hotfix/<name>`
  - `release/vX.X.X`

Examples: `feature/browser-surfaces`, `bugfix/terminal-input-freeze`.

## Testing Guidelines
- Add unit tests for new logic and bug fixes.
- Cover edge cases; avoid flaky timing-dependent tests.
- Run `cargo test` locally before pushing.
- Place tests near the code they verify (module tests or dedicated test files when added).

## Commit & Pull Request Guidelines
Commit messages should be clear, imperative, and specific (as seen in history), e.g.:

- `Add README, branching, and contribution docs`
- `Fix terminal input freeze`

Avoid vague messages like `update code`.

PR requirements:
- Open PRs into `develop` (not direct commits to `main`/`develop`).
- Ensure build, format, lint, and tests pass.
- Include a concise description, linked issue(s), and docs updates when behavior changes.
- For UI/UX changes, include screenshots or short recordings when applicable.

For detailed implemented backend behavior, CLI/RPC usage, operations, and contributor workflow, refer to `docs/README.md`.
