# Changelog

All notable changes to clin are documented in this file.

## [0.9.0-beta.4] - 2026-06-23

### CI

- Fix for generate-rmp
- Ci fixes for nix and linux-package jobs
- Ci fixes for nix and linux-package jobs
## [0.9.0-beta.3] - 2026-06-22

### CI

- Fixed simple syntax errors

### Release

- V0.9.0-beta.3
## [0.9.0-beta.2] - 2026-06-22

### CI

- Fixed simple syntax errors

### Release

- V0.9.0-beta.2
## [0.9.0-beta.1] - 2026-06-22

### Added

- Add Ctrl+P as default command palette keybind
- Add IconMode with Nerd Font/Unicode/None glyph switching
- Add inline search popup with row highlighting
- Hard-abort on Ctrl+C, bypass graceful shutdown
- Add command palette actions to set daily goals
- Add daily word and note goal system
- Add month calendar widget to list view
- Add editor presets and multi-key sequences

### CI

- Fixed simple syntax errors
- Fixed linux-package release
- Added a preview job
- Updated dispatch-release to have a approval phasei
- Updated dispatch-release to have an option for overriding the version
- Updated dispatch-release to have an option for pre-releases
- Updated dispatch-release to have an option for target branch

### Changed

- Extract TUI event loop into library crate
- Strip animation engine, fix tab centering, fix calendar border
- Huge refactor splitting app.rs, keybinds.rs and rendering.rs

### Fixed

- Bullet only CLI items in About tab, not config/info items
- Match calendar vertical centering
- Invert resize arrow direction when preview is on right
- Clear orphaned plaintext temp files after crash
- Interrupt stuck quit-time flush on second signal
- Zeroize transient decrypted buffers
- Reap zombie glow child on cancel render
- Fixed clippy and build warnings
- Rebuild display list cache on reload_theme

### Miscellaneous

- Bumped version back to 0.8.26
- Bumped version back to 0.8.26
- Bump version to 0.8.26
- Bump version to 0.8.26
- Bump version to 0.8.26
- Bump version to 0.8.26

### Performance

- Use opt-level=3 and mimalloc allocator

### Release

- V0.9.0-beta.1
- V0.9.0-beta.0
- V0.10.0-beta.0
- V0.9.0
## [0.8.25] - 2026-06-20

### Release

- V0.8.25
## [0.8.24] - 2026-06-20

### Added

- Add Toggle Preview Word Wrap action
- Add word-wrap and fullscreen toggles

### Release

- V0.8.24
## [0.8.23] - 2026-06-20

### Fixed

- Guard SIGHUP/SIGQUIT with #[cfg(unix)]

### Release

- V0.8.23
## [0.8.22] - 2026-06-20

### Added

- Add background backup worker thread

### Fixed

- Set git user config in perform_creates_commit test
- Disable git backups by default

### Release

- V0.8.22
## [0.8.21] - 2026-06-20

### Added

- Preview pane rendering optimizatons

### Miscellaneous

- Fixed fmt CI failing
- Bump toml_edit from 0.22.27 to 0.25.12+spec-1.1.0
- Bump toml from 0.8.23 to 1.1.2+spec-1.1.0
- Bump portable-pty from 0.8.1 to 0.9.0
- Bump signal-hook from 0.3.18 to 0.4.4
- Bump actions/checkout from 4 to 7
- Bump cachix/install-nix-action from 27 to 31

### Release

- V0.8.21
## [0.8.20] - 2026-06-20

### Added

- Add tab_icons_only mode with overlap-safe title bars

### Fixed

- Move cut/copy/paste to Ctrl+Shift to free bare Ctrl

### Miscellaneous

- Fixed fmt CI failing
- Fixed fmt CI failing

### Performance

- Cache arboard clipboard in thread-local

### Release

- V0.8.20
## [0.8.19] - 2026-06-20

### Fixed

- Restore terminal on SIGINT/SIGTERM and panic
- Use slugified note id from save_note for external editor
- Confirm text-input popups on Enter only
- Replace deprecated xorg.libX11/xorg.libxcb with libx11/libxcb

### Miscellaneous

- Fixed fmt CI failing

### Performance

- Cache per-item list display formatting
- Defer initial note load to background thread

### Styling

- Fix formatting in test code

### Release

- V0.8.19
## [0.8.18] - 2026-06-18

### Fixed

- Vault detection logic changed to fix an issue

### Miscellaneous

- Fixed fmt CI failing

### Release

- V0.8.18
## [0.8.17] - 2026-06-18

### Added

- Add toggle to show hidden files/folders
- Handle vault-mode source and target paths
- Detect existing vaults as notes root

### Release

- V0.8.17
## [0.8.16] - 2026-06-17

### CI

- Add manual macOS DMG upload workflow
- Add macOS DMG creation to release workflow

### Changed

- Fix CI failures and upgrade git2
- Remove dead and broken options

### Release

- V0.8.16
## [0.8.15] - 2026-06-16

### Added

- Add macOS ARM (aarch64-apple-darwin) build to release workflow
- Add 'q' as universal back/exit key
- Added more CLI options and fixed config bugs

### Changed

- Migrate to std::sync::LazyLock and remove regex dependency
- Use Prompt size for commit popup
- Unify popup sizing with tiers and constraints
- Unify sub-app run-loops and popups
- Config and CLI code refactor

### Documentation

- Sync documentation with current application state

### Fixed

- Restrict tagging to .md, .txt, and .clin files
- Restore note creation and rename popup heights to 12

### Miscellaneous

- Fix unnecessary_sort_by clippy warning
- Run cargo fmt
- Fix clippy warnings and remove dead code
- Code quality and dead code removal

### Release

- V0.8.15
## [0.8.14] - 2026-06-15

### Added

- Commit only the selected unstaged changes
- Add "Create new…" tile to grid layout
- Route list "Create new" item through format chooser
- Format chooser popup for new md/txt/draw/canvas
- Persist notes layout (tree/grid) to config

### Fixed

- Drop redundant remote URL/name border titles
- Stop diff pane scrolling past content
- Stop status/history lists scrolling past content
- Always land on Vault tab when toggling to grid

### Miscellaneous

- Fixed clippy warnings

### Release

- V0.8.14
## [0.8.13] - 2026-06-15

### Miscellaneous

- Readme update about nix support
- Nix support and automation via dispatch release

### Release

- V0.8.13
## [0.8.12] - 2026-06-15

### Miscellaneous

- Cliff.toml changes

### Release

- V0.8.12
## [0.8.11] - 2026-06-15

### Fixed

- Dispatch-release appending wrong version number

### Release

- V0.8.11
## [0.8.10] - 2026-06-15

### Fixed

- Dispatch-release fix for appending wrong changelog

### Release

- V0.8.10
## [0.8.9] - 2026-06-14

### Documentation

- Align MSRV references

### Fixed

- Mouse clicking not on point in backup view

### Miscellaneous

- Clippy and formatting

### Release

- V0.8.9
## [0.8.8] - 2026-06-14

### Added

- Added toggle settings to the command palette
- Improved the UI of the command pallette, added glyphs and categories

### Fixed

- CI formatting and clippy issues
- Backup settings popup not interactable with mouse
- Resolve clippy warnings for msrv 1.88.0
- Mouse clicking outside popup closes it
- Preview pane not rendering and rendering slowly
- Mouse clicking precision in canvases
- Fix changelog history overwrite in dispatch-release workflow
- Fixed Shift + Tab not working in tab switchers
- Mouse support not working on many elements
- External editor toggle not working

### Miscellaneous

- Format codebase and fix clippy warnings
- README.md update
- CI finding fixes
- Update README.md
- Update ROADMAP.md

### Testing

- Fix markdown test failure when glow is not installed

### Release

- V0.8.8
## [0.8.7] - 2026-06-14

### Miscellaneous

- Changelog automation fix
- Changes for dispatch release
- Changes for dispatch release
- Fix dispatch release automation
- Appimage automation fix
- Changelog automation
- Fixed icon issue for automation
- Removed dist remainings
- Removed dist dependency
- Even more CI fixes

### Assets

- Add 256x256 PNG icon for AppImage

### Release

- V0.8.7
## [0.8.6-1] - 2026-06-13

### Miscellaneous

- More CI fixes
- More CI fixes
## [0.8.6] - 2026-06-13

### Added

- Insert/append actions
- Content tree for notes
- Content tree for notes
- Grid view in notes view
- Title bar for views
- Backup system using git

### Documentation

- Update installation versions to v0.7.0-61
- Update installation versions to v0.7.0-43

### Fixed

- Pinstar and draw view mouse click precision
- Modularized popup system so all the popups share the same template
- Preview pane not scales with the window size
- Lifelong bug of markdown preview is fixed
- Mouse selecting doesn't trigger the preview

### Miscellaneous

- CI fixes
- Publishing CI and even more automation
- CI and contributing documentation
- Refactoring
- Comment updates
- Update PKGBUILD

### Add

- Preview pane for graph view and preview pane position customization, bug fix for graph view scaling
- More QOL, title bar for popups
- More QOL and UX/UI improvements, changes
- Popup improvements for notes view, lots QOL additions, reworked search engine
- Popup improvements for notes view, lots QOL additions, reworked search engine
- QOL improvements to notes view, grep search, improved popups and more
## [0.7.0-61] - 2026-05-10

### Fixed

- Canvas file doesn't show up in the notes view

### Miscellaneous

- Updated documentation
- Updated documentation
- Updated documentation
- Comment removals for proper documentation later

### Add

- Pinstar final touches and code structure refactorings
- Pinstar view improvements, enhancements, Obsidian import and more
- Pinstar view mimicing Obsidian canvas feature

### Overhaul

- Complete overhaul of the UI, added demo paint like canvas, lots of QOL changes, migrated to ratatui 0.30 and more
- Complete overhaul of the UI, added demo paint like canvas, lots of QOL changes, migrated to ratatui 0.30 and more

### Qol

- Shift + tab to cycle backwards in graph view
- Enter now closes the theme switcher
## [0.7.0-43] - 2026-05-09

### Fixed

- Default theme colors
- More unaffected background colors
- Background color of command palette input field
- Background color for notes view

### Miscellaneous

- Doc updates
- Migrated to ratatui 0.30

### Add

- Added demo canvas using the same logic as graf
- Button for solid backgrounds
- Universal theme and switch theme option

### Overhaul

- Complete overhaul of the UI, added demo paint like canvas, lots of QOL changes, migrated to ratatui 0.30 and more
## [0.5.2-1] - 2026-05-04

### Fixed

- Default config fixed, root label missing from legend fixed

### Miscellaneous

- Update readme
- Documentation
- Documentation
- Documentation
- Readme update
- Readme update

### Add

- Custom keybinds for graf
- Custom keybinds for graf
## [0.5.2] - 2026-05-03

### Fixed

- Default config fixed, root label missing from legend fixed
- Default config fixed, root label missing from legend fixed

### Miscellaneous

- Readme update
## [0.5.0-2] - 2026-05-03

### Documentation

- Update installation versions to v0.5.0-2
- Update installation versions to v0.4.4

### Miscellaneous

- Update version for crates.io
- Update version for crates.io
- Update PKGBUILD
- Version numbering

### Add

- Integrated project graf as the graph view
## [0.4.4] - 2026-05-01

### Documentation

- Update installation versions to v0.3.6

### Fixed

- Markdown rendering is 10x faster now
- Markdown rendering is 10x faster now

### Add

- More graph view features, color coding, user configs for title rendering
- Graph view
- Preparations for graph view with reworked encryption
## [0.3.4-5] - 2026-04-05

### Miscellaneous

- Automation
- Automation
- Automation
## [0.3.4-1] - 2026-04-05

### Documentation

- Update installation versions to v0.3.4-2
- Update installation versions to v0.3.4-1
- Update installation versions to v0.0.0
## [0.3.3] - 2026-04-01

### Added

- Restore external editor support
- Implement Command Palette and OCR Paste

### Fixed

- Zero sensitive data from memory on drop
- Restrict encryption key file permissions
- Sanitize user strings for terminal display
- Validate paths to prevent directory traversal
- Secure temp file permissions and deletion
- Prevent shell injection in external editor

### Miscellaneous

- Add cargo-deny and CI security scanning
## [0.2.1.2] - 2026-03-31

