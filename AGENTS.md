# Repository Guidelines

## Project Structure & Module Organization
- Describe top-level modules/directories and how they relate.

## Build, Test, and Development Commands
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo test --workspace --all-features`

## Coding Style & Naming Conventions
- Run the repo formatters/linters; do not hand-format.
- Match existing naming and module layout.

## Testing Guidelines
- Add tests for behavior changes and regressions.
- Prefer fast unit tests; add integration tests for cross-module behavior.

## Commit & Pull Request Guidelines
- Use concise, imperative commit subjects; link issues where applicable.
- PRs must include: summary, rationale, and testing notes.

## Agent Tooling

- **Pre-commit hooks:** run `bin/setup-githooks` (configures `core.hooksPath` for this repo).

- **Source of truth:** `.agents/`.
- **Symlinks:** `CLAUDE.md` is a symlink to this file (`AGENTS.md`). Editor/agent configs should symlink skills from `.agents/skills`.
- **Skills install/update:**

```bash
npm_config_cache=/tmp/npm-cache npx -y skills add https://github.com/cartridge-gg/agents   --skill create-pr create-a-plan   --agent claude-code cursor   -y
```

- **Configs:**
  - `.agents/skills/` (canonical)
  - `.claude/skills` -> `../.agents/skills`
  - `.cursor/skills` -> `../.agents/skills`

## Code Review Invariants

- No secrets in code or logs.
- Keep diffs small and focused; avoid drive-by refactors.
- Add/adjust tests for behavior changes; keep CI green.
- Prefer check-only commands in CI (`format:check`, `lint:check`) and keep local hooks aligned.
- For Starknet/Cairo/Rust/crypto code: treat input validation, authZ, serialization, and signature/origin checks as **blocking** review items.
