---
name: dojo-contributing
description: Contributor workflow for dojoengine/dojo. Use when implementing or reviewing changes in this repository, especially Rust workspace edits, Cairo fixture refreshes, and CI-parity formatting/lint/test checks.
---

# Dojo Contributing

Use this skill to make scoped changes in `dojoengine/dojo` and validate them with repository-standard commands.

## Core Workflow

1. Identify affected crates and binaries from `git status` and touched files.
2. Prepare local test databases:
   - `bash scripts/extract_test_db.sh`
3. If Cairo examples, policies, or core behavior changed, refresh artifacts:
   - `POLICIES_FIX=1 cargo nextest run --all-features --build-jobs 20 --workspace --nocapture policies`
   - `bash scripts/rebuild_test_artifacts.sh`
4. Run baseline validation:
   - `cargo build --workspace`
   - `cargo nextest run --all-features --build-jobs 20 --workspace`
5. If Katana behavior is involved, test with a local Katana binary:
   - `cargo build -r --bin katana`
   - `KATANA_RUNNER_BIN=./target/release/katana cargo nextest run --all-features --build-jobs 20 --workspace`
6. Run style and lint checks:
   - `bash scripts/rust_fmt.sh --fix`
   - `bash scripts/clippy.sh`

## PR Checklist

- Keep the diff focused to the requested behavior.
- Include fixture/artifact updates when Cairo-driven behavior changes.
- Record exact validation commands and outcomes in the PR body.
