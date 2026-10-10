# Contributing to clin

Thank you for considering contributing to clin! Here's how to get started.

## Getting Started

1. Fork and clone the repository
2. Install Rust 1.90.0+ via [rustup](https://rustup.rs)
3. Build: `cargo build`
4. Run: `cargo run`

## Releasing

Releases are fully automated via **Actions → Dispatch Release → Run workflow**.

1. Select bump type: `patch`, `minor`, `major`, or `pre-patch`
2. Click **Run workflow**

This produces:
- **Platforms:** Linux x86_64, Linux aarch64, Windows x86_64, macOS aarch64 (Apple Silicon)
- **Linux packages:** `.deb` (x86_64 + aarch64), `.rpm` (x86_64 + aarch64), AppImage (x86_64), AUR (`clin-rs-bin`)
- **Other package managers:** crates.io, Nix
- **Artifacts:** changelog, .tar.xz, .tar.gz, .zip, .dmg

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

## Documentation and Wiki

The [GitHub Wiki](https://github.com/reekta92/clin-rs/wiki) is generated from all root-level Markdown files and `docs/**/*.md`. Update these source files through the usual pull request process; do not edit generated wiki pages directly, because the next sync overwrites those edits. Local `development/` files and graphify artifacts are not published.

`.github/workflows/wiki.yml` tests the generator on documentation pull requests and publishes after relevant changes reach `main`. Maintainers can also run **Actions → Sync Wiki → Run workflow** on `main`. New documentation files are discovered automatically; removed files are removed from the wiki. Handwritten pages outside the generated page names are preserved.

Publication uses the existing `BOT_TOKEN` repository secret, which must have Git write access to `reekta92/clin-rs.wiki.git`. Keep this credential in Actions secrets, never in source files. The workflow never exposes it to pull request jobs.

Run the generator tests locally:

```bash
python3 -m unittest discover -s .github/scripts -p 'test_sync_wiki.py' -v
```

To preview generated pages in a separate wiki checkout without publishing:

```bash
git clone git@github.com:reekta92/clin-rs.wiki.git /tmp/clin-wiki
python3 .github/scripts/sync_wiki.py /tmp/clin-wiki --revision "$(git rev-parse HEAD)"
git -C /tmp/clin-wiki diff
```

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
