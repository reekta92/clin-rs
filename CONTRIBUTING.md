# Contributing to clin

Thank you for considering contributing to clin! Here's how to get started.

## Getting Started

1. Fork and clone the repository
2. Install Rust 1.88.0+ via [rustup](https://rustup.rs)
3. Build: `cargo build`
4. Run: `cargo run`

## Releasing

Releases are fully automated via **Actions → Dispatch Release → Run workflow**.

1. Select bump type: `patch`, `minor`, `major`, or `pre-patch`
2. Click **Run workflow**

This produces:
- **Platforms:** Linux x86_64, Windows x86_64
- **Linux packages:** `.deb` (Debian/Ubuntu), `.rpm` (Fedora/RHEL/openSUSE), AppImage, AUR
- **Registries:** crates.io, AUR (`clin-rs-bin`)
- **Artifacts:** changelog, .tar.xz, .zip

### Required repository secrets

| Secret | Source | Used by |
|---|---|---|
| `CARGO_REGISTRY_TOKEN` | [crates.io/settings/tokens](https://crates.io/settings/tokens) — publish scope | crates.io publish |
| `AUR_SSH_PRIVATE_KEY` | SSH keypair registered at [aur.archlinux.org](https://aur.archlinux.org/account) | AUR package push |


## Development

- **Code style**: Run `cargo fmt` before committing
- **Linting**: Run `cargo clippy -- -D warnings` and fix all warnings
- **Testing**: Run `cargo test` to verify nothing is broken
- **Architecture**: See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the system overview
- **Configuration reference**: See [docs/CONFIG_REFERENCE.md](docs/CONFIG_REFERENCE.md)

## Pull Requests

1. Create a feature branch from `main`
2. Make your changes with clear, descriptive commits
3. Ensure `cargo fmt`, `cargo clippy`, and `cargo test` pass
4. Open a PR with a description of what changed and why

## Bug Reports

Use the [Bug Report](https://github.com/reekta92/clin-rs/issues/new?template=bug_report.md) issue template.

## Feature Requests

Use the [Feature Request](https://github.com/reekta92/clin-rs/issues/new?template=feature_request.md) issue template.

## License

By contributing, you agree that your contributions will be licensed under the [GPL-3.0 License](LICENSE).
