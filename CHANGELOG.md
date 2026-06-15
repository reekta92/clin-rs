# Changelog

All notable changes to clin are documented in this file.

## [0.8.12] - 2026-06-15

### Miscellaneous

- Cliff.toml changes
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

