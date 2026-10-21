# Contributing to clin

Thank you for considering contributing to clin! Here's how to get started.

## Getting Started

1. Fork and clone the repository
2. Install Rust 1.90.0+ via [rustup](https://rustup.rs)
3. Build: `cargo build`
4. Run: `cargo run`

## Releasing

Releases are fully automated via **Actions → Release → Run workflow**.

1. Select bump type: `patch`, `minor`, or `major`. Use the separate `prerelease` flag and identifier for pre-releases.
2. Click **Run workflow**

This produces:
- **Platforms:** Linux x86_64, Linux aarch64, Windows x86_64, macOS aarch64 (Apple Silicon)
- **Linux packages:** `.deb` (x86_64 + aarch64), `.rpm` (x86_64 + aarch64), AppImage (x86_64). The workflow updates PKGBUILD references for downstream AUR maintainers.
- **Other package managers:** crates.io; Nix consumes the repository flake (no separate release publish step)
- **Artifacts:** changelog, .tar.xz, .tar.gz, .zip, .dmg

### Application icons

`assets/icon.svg` is the source for the application icon, including the README logo.
After editing it, run `python3 .github/scripts/generate_icons.py` (requires
`rsvg-convert` from librsvg and Python Pillow) and commit the generated
`assets/clin.png`, `clin.ico`, and `assets/clin.icns` together.

Linux packages install the SVG and PNG into the `hicolor` icon theme, and Windows
builds embed the ICO in `clin.exe`. macOS archives and DMGs include `clin.app`,
which uses the ICNS and launches the bundled terminal executable in Terminal.
The shared release workflow in `reekta92/.github` includes these assets in release
archives so the AUR package can install the same desktop launcher and icons.

### Required repository secrets

| Secret | Source | Used by |
|---|---|---|
| `BOT_TOKEN` | Repository bot token with Git write access | Wiki publication; optional release authentication |
| `CARGO_REGISTRY_TOKEN` | [crates.io/settings/tokens](https://crates.io/settings/tokens) — publish scope | crates.io publish |


## Development

- **Local CI runner**: `scripts/check.sh` runs formatting, Clippy, tests, dependency policy, and the declared minimum Rust check locally. CI additionally runs Windows/macOS tests and verifies the extracted registry package; `cargo package --locked` reproduces package compilation. Install stable Rust with rustfmt/Clippy, the toolchain declared by `rust-version`, cargo-deny, and Python 3.11+. Missing tools fail the check. Use `scripts/check.sh clippy test` for a subset; `scripts/check.sh check` is an optional separate stable build check.
- **Code style**: Run `cargo fmt --all` before committing
- **Linting**: Run `cargo clippy --locked --all-targets -- -D warnings` and fix all warnings
- **Testing**: Run `cargo test --locked`; ignored performance tests are opt-in
- **Commits**: Use Conventional Commits (`docs:`, `fix:`, `feat:`, etc.); the generated changelog filters other messages. Do not hand-edit `CHANGELOG.md` or bump `Cargo.toml` for releases.
- **Architecture**: See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the system overview
- **Configuration reference**: See [docs/CONFIG_REFERENCE.md](docs/CONFIG_REFERENCE.md)

### Checking upstream libraries

Run an existing adapter contract against a candidate library checkout:

```sh
python3 scripts/check_upstream.py ../pinstar
python3 scripts/check_upstream.py ../graf
python3 scripts/check_upstream.py ../draw-rs
```

This requires Python 3.12+ and stable Rust. The script archives the current
committed Clin revision into a temporary directory, patches the dependency's
actual registry or Git source, and resolves the candidate there. It verifies that Cargo
selected the candidate as Clin's direct dependency before running the matching
`*_plugin_e2e` tests with `--locked`. The original manifest and lockfile remain
untouched. Build artifacts reuse `target/`, or `CARGO_TARGET_DIR` when set.
Commit paired Clin changes before running this command; uncommitted Clin edits
are not included. Candidate library edits are read from its working tree.

Library CI uses the same command through the shared `clin-compat.yml` workflow
and pins the supported Clin commit to a full SHA. For a coordinated API change,
link the library PR and the paired Clin PR, test that committed Clin revision,
then release the compatible library and update Clin's dependency pin in a linked
PR. An incompatible candidate version fails instead of silently testing the old
dependency. See [dependency compatibility](docs/DEPENDENCY_PATCHES.md)
for the published Mermaid backend; consumers need no root Cargo patch.

Test the development tools without building Clin:

```sh
python3 -m unittest discover -s .github/scripts -p 'test_check_*.py' -v
```

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

See [the shared development process](docs/DEVELOPMENT.md) for repository roles,
workspace setup, coordinated changes and [release acceptance](docs/RELEASE_CHECKLIST.md).

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
