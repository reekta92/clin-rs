# Changelog

All notable changes to clin are documented in this file.

## [0.10.0-beta.3] - 2026-07-15

### Added

- Replace DP+Chaikin smoothing with binomial filter
- Add image file support
- Render image nodes with filled block and icon
- Add mouse-drag panning and fix canvas text wrap
- Zoom previews toward cursor, fix scroll direction
- Scale snapshots to fill expanded pane
- Add draggable scrollbars to all scrollable panes

### CI

- Dispatch release issue revert
- Fix arm64 apt sources for release workflow
- Forced ipv4 in dispatch release
- Fixed format checks

### Changed

- Removed image support from the draw view
- Refactored source editor pane to use the same code as the edit view

### Documentation

- Update README.md, ROADMAP.md and new config references

### Fixed

- Separate node display title from internal ID
- Preserve pin status through encrypt/decrypt

### Miscellaneous

- Lock file regenerate
- Check fix
- Update readme about version bump
- Bump version to 1.90
- Cleanup project dir
- Sort Cargo.lock alphabetically

### Performance

- Decouple preview drag state from rendering
- Cache parsed data and re-render grids in-memory

### Release

- V0.10.0-beta.0
## [0.9.9] - 2026-07-14

### Added

- Redesigned title bar and merged it into the header bar
- Add mouse support to quick search
- Show recursive folder count in header bar
- Add inline_info toggle for notes list metadata
- Centralize popup mouse handling
- Add hover highlight to canvas context menu
- Add hover highlights to all interactive elements
- Rename backlinks to links pane and show forward links
- Add sidebars and wikilink previews
- Add find popup, insert-date, soft-wrap toggle
- Add images.enabled config option to disable pixel rendering
- Add pixel image rendering across Canvas, Notes, Draw
- Add ratatui-image rendering across canvas, draw, and notes views
- Add preset-aware tip caveats and expand tip pools
- Add popup accordion with n/N cycle in Notes info pane
- Add popup descriptions to Notes info pane
- Always render {hints} with powerline colors regardless of hint_bar_style
- Restructure help view into 3-pane layout
- Render live keybinds and styled markup in tips pane
- Auto-generate entries from exhaustive action meta

### CI

- Fix remaining format warnings in events and keybinds
- Fixed format warnings

### Changed

- Refactored edit view line highlight code
- Deduplicate layout, add UX features, context menu
- Remove dropdown border, add "Find:" label before input
- Rename QuickPopup→QuickSearch, render in header bar
- Apply fmt, canvas ocr image insertion, cleanup
- Unify popup footer rendering through PopupHints enum
- Unify UI helpers and improve safety
- Remove Accent style, rename Powerline variants, fix tab highlight
- Move page indicator to title bar via statusline

### Fixed

- Wire libgit2 auth callbacks into push/pull
- Fix selection styling and right-click jump
- Fixed jittering when moving nodes
- Merged inline_info with show_date_in_notes
- Fixed mouse interaction with quick search
- Resolve canvas overlap and template version
- Show file detail header in tree layout too
- Deduplicate tags on save
- Append tag from all_tags without double commas
- Add scroll/click/hover to trash, template, tags popups
- Fixed mouse scrolling breaks mouse accuracy in list popups
- Add scroll-offset to mouse row→index mapping in all list popups
- Fix mouse click off-by-one, add scroll, fix double-click
- Fix line numbers and mouse under soft-wrap
- Fix list hovers and sidebar click offset
- Account for scroll offset in command palette hover
- Correct sidebar mouse click offset
- Surface config validation errors in graph view overlay
- Use heading color for selection highlight row bg
- Fill full row width with selection highlight
- Add search glyph, improve readability on accent bg
- Use accent color for dropdown background, fix no-match fg
- Fill dropdown background with theme bg_style
- Clear header text before drawing search overlay
- Add preview-bg fill under small images
- 3 preview/canvas bugs — bg fill, pan drain, image indicator
- Render images in list view preview pane
- Include [image] section in shipped default config template
- Clear is_dragging_resize_handle on drop and resize-exit
- Clear resize state on Left Up so image renders after resize
- Wire image fields, fix file picker, add canvas image node
- Add universal q/Esc back/quit intercepts, override-proof
- Coalesce queued drag events; fix instant-apply popup cycle
- Hint bar preview dispatches on hint_bar_style
- Popup footers ignore hint_bar_style setting
- Split detail into powerline cells; fix right-side junction bg

### Miscellaneous

- Add support section to the readme

### Performance

- Render lightweight dot markers during pan/zoom transforms

### Release

- V0.9.9
## [0.9.8] - 2026-07-10

### Fixed

- Restore auto_refresh field lost in merge

### Miscellaneous

- Fix clippy warnings and formatting

### Performance

- Skip full filesystem rescan on note delete

### Release

- V0.9.8
## [0.9.7] - 2026-07-08

### Added

- Replace notify-debouncer-mini with raw notify for Access event filtering
- Auto-refresh notes list on external file changes
- Add 9 config-gated renderer features

### CI

- Fixed multiple checks issues

### Fixed

- Invalidate folder cache on auto-refresh

### Performance

- Reduce debounce to 50ms, throttle refresh to 250ms

### Release

- V0.9.7
## [0.9.6] - 2026-07-06

### Added

- Structured items, metric tables, text wrapping, border
- Add Show Info popup with note/folder metrics
- Add custom rules and grid layout tab
- Add sub-notes overlay manager
- Support smart/pinned folders, inline rename, drag-to-move
- Add expand-all, expand-to-level, and state persistence
- Add recursive counts and dim empty folders
- Add 9 config-gated renderer features

### CI

- Fixed multiple checks issues
- Fixed test checks

### Changed

- XOR obfuscate the entire subnotes database
- Secure subnotes file format and permissions

### Documentation

- Update ROADMAP.md

### Fixed

- Clear old name characters in inline rename overlay
- Fixed custom themes not having transparent option

### Release

- V0.9.6

### Revert

- Remove inline rename support, restore popup renames
## [0.9.5] - 2026-07-06

### Release

- V0.9.5
## [0.9.4] - 2026-07-06

### Added

- Add modular custom theme support
- Add 6 built-in themes
- Use builtin markdown renderer in preview
- Small changes for the setup wizard
- Add live preview panel and fix logo
- Replace multi-step wizard with single centered screen
- Overhaul setup wizard view
- Add show-all-files toggle and external open for non-notes

### CI

- Added overwrite release
- Fixed fmt and clippy checks
- Fixed deny check failing

### Documentation

- Fixes for config references

### Fixed

- Patch 8 bugs in wizard save, abort, display, and key handling
- Expand ~ and $VAR in storage paths
- Set uniform icon-row style for contiguous text runs

### Release

- V0.9.4
- V0.9.4
- V0.9.3
## [0.9.2] - 2026-07-01

### Added

- Added folders_first config and Ctrl + h shortcut to toggle it

### CI

- Fixed fmt check failing

### Release

- V0.9.2
## [0.9.1] - 2026-06-30

### CI

- Fixed clippy, format check warnings and errors
- Fixed clippy, format check warnings and errors
- Fixed CI check warnings and errors

### Release

- V0.9.1
- V0.9.0
## [0.8.32] - 2026-06-27

### Miscellaneous

- Bump anyhow from 1.0.102 to 1.0.103
- Bump uuid from 1.23.3 to 1.23.4

### Release

- V0.8.32
## [0.8.31] - 2026-06-23

### Release

- V0.8.31
## [0.8.30] - 2026-06-23

### Release

- V0.8.30
## [0.8.29] - 2026-06-23

### Release

- V0.8.29
## [0.8.28] - 2026-06-23

### Release

- V0.8.28
## [0.8.27] - 2026-06-23

### Testing

- Testing CI
- Testing CI

### Release

- V0.8.27
- V0.9.0-beta.4
- V0.9.0-beta.4
## [0.8.26] - 2026-06-21

### Added

- Reimplement view with git-style staging

### Changed

- Simplify command outputs and drop json arg
- Replace std Mutex/RwLock with parking_lot
- Remove custom logging and debug dump

### Documentation

- Update CLI Commands to match current CLI
- Reflect OverlayView trait architecture
- Sync docs with code changes

### Fixed

- Correct jump-to-top dispatch and add jump-to-bottom defaults
- Add #[serde(default)] to all config structs
- Prevent mouse movement from closing non-target popups
- Align UI consistency, navigation, and mouse targeting
- Restore terminal on stderr in panic hook

### Release

- V0.8.26
## [0.9.0-rc.3] - 2026-06-29

### Release

- V0.9.0-rc.3
## [0.9.0-rc.2] - 2026-06-29

### Added

- Improve renderer styling, layout, and performance
- Added a builtin markdown renderer to replace glow
- Replace month grid with rolling-weeks heatmap
- Add Draw and Graf strip sections, halfblock preview, centering
- Added a builtin markdown renderer to replace glow

### Fixed

- Initialize draw_state when creating new .draw file

### Performance

- Pre-warm graph preview at startup to avoid first-frame blink
- Progressive graph preview settle across frames
- Reduce graf preview freeze on first cycle

### Release

- V0.9.0-rc.2
## [0.9.0-rc.1] - 2026-06-28

### Added

- Add --vault, --json, notes cat, notes new --body/--no-tui
- Added config options for customizing preview pane command
- Expose preview paging and fix help tab digits
- Add vim-style count-prefix for motion keys

### Changed

- Unify overlay dispatch via OverlayView trait and collape popup god-function
- Consolidate 4 accessor families via keybind_scope! macro
- Route production writes through atomic_write
- Unify hex color parsing, clarify ThemeColors docs
- Split 2170-line config.rs into module tree
- Add list_state_selected helper
- Deduplicate overlay dispatch
- Consolidate format_relative_time

### Documentation

- Restructure install matrix and add visuals layer
- Sync README and docs/ with current code

### Fixed

- Ensure cursor line text contrast with transparent backgrounds
- Switch CONFIG_TEST_MUTEX to parking_lot::Mutex
- Set select_style on all TextArea editors

### Miscellaneous

- Remove stale Cargo.toml.bak
- Readme update

### Release

- V0.9.0-rc.1
## [0.9.0-rc.0] - 2026-06-25

### Added

- Handle folders in bulk move/copy/delete
- Accent-fill selected grid tiles, center mode badge, add footer hints
- Add presets with picker, per-preset overlays, and pending indicator
- Move tool buttons to header bar
- Add text labels to toolbar buttons

### Fixed

- Keep current base when switching pre-release id
- Incremental refresh on exit-edit instead of full rescan
- Preserve file extension when duplicating notes
- Keep graph state alive while Help is open
- Clear help_requested flag when consumed

### Release

- V0.9.0-rc.0
## [0.9.0-beta.6] - 2026-06-24

### Added

- Apply hint_bar_style to header details
- Add command palette option for hint bar style
- Add hjkl navigation to layout edit mode
- Improvements to the debug logging system
- Add comprehensive debug logging across all subsystems

### CI

- Aarch64 job fixes
- Cleanup-cancelled-release workflow for cancelled releases
- Fix for correct version calculating

### Changed

- Unify popup cancelation with context-aware keybinds

### Fixed

- Help view loop issue
- Correct help navigation and enable it in draw view
- Fallback to Quit when popup action shadows it
- Remove angle brackets from keybind display
- Add icon_mode to default generated config
- Notes openned from graph view not going back to graph view with builtin editor
- Handle mouse double-click result and rebuild graph state on return
- Add aur, crates-io, post-release to cleanup failure trigger
- Cleanup on build failure instead of cancel (cancelled() unreliable)
- Replace heredocs with printf to keep YAML valid, add cleanup-on-cancel job to dispatch-release
- Remove duplicated deb lines breaking YAML parse

### Release

- V0.9.0-beta.6
## [0.9.0-beta.5] - 2026-06-23

### Release

- V0.9.0-beta.5
## [0.9.0-beta.4] - 2026-06-23

### CI

- Fix for generate-rmp
- Ci fixes for nix and linux-package jobs
- Ci fixes for nix and linux-package jobs

### Release

- V0.9.0-beta.4
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

