# Repository Guidelines

## Project Structure & Module Organization

Clin is a Rust terminal application for Markdown vaults. `src/bin/clin.rs` is the executable entry point; `src/lib.rs` hosts application orchestration. Feature modules include `src/app/`, `src/events/`, `src/ui/`, `src/bases/`, and `src/calendar/`; persistence lives in `src/storage.rs`. Keep changes in the module that owns the behavior.

Unit tests sit beside implementation code; `tests/` contains CLI and plugin integration tests. `assets/` holds icons and desktop integration, `examples/` contains examples, `docs/` holds architecture and user documentation, and `.github/` contains automation. Start with `CONTRIBUTING.md` and `docs/ARCHITECTURE.md`.

## Build, Test, and Development Commands

Use Rust 1.90.0 or newer, edition 2024.

- `cargo build --locked`: build the application with committed dependency versions.
- `cargo run -- --vault /tmp/clin-dev-vault`: launch against a disposable vault.
- `cargo fmt --all`: format Rust sources.
- `cargo clippy --locked --all-targets -- -D warnings`: lint all targets; warnings fail.
- `cargo test --locked`: run the standard test suite.
- `scripts/check.sh`: run formatting, Clippy, tests, dependency policy, and minimum Rust checks. Requires stable Rust with rustfmt/Clippy, the minimum toolchain, cargo-deny, and Python 3.11+.
- `scripts/check.sh clippy test`: run selected gates.
- `cargo package --locked`: verify package compilation.

## Coding Style & Naming Conventions

Follow `.editorconfig` and `rustfmt.toml`: four-space Rust indentation, two-space TOML/YAML indentation, LF endings, and a 100-column Rust width. Use `snake_case` for functions/modules, `PascalCase` for types, and `SCREAMING_SNAKE_CASE` for constants. Reuse existing helpers and preserve validation and error handling; heed the manifest's Clippy rules.

## Testing Guidelines

Use Rust's `#[test]` framework with descriptive `snake_case` names stating expected behavior. Add focused regression tests for behavioral fixes. Use `tempfile` and isolated configuration for filesystem tests; never target personal vaults. Run focused tests with `cargo test --locked <name_filter>`. Ignored performance tests are opt-in; interactive terminal behavior requires manual verification.

## Commit & Pull Request Guidelines

Create feature branches from `main`. Follow existing Conventional Commits, such as `fix(storage): preserve vault files` or `feat: improve calendar navigation`. Explain what changed, why, and validation in PRs; link related issues and paired library PRs. Required checks must pass before merging. Release automation owns version bumps and `CHANGELOG.md` updates.
