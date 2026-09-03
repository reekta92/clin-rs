# Changelog

All notable changes to clin are documented in this file.

## [0.12.0] - 2026-09-03

### CI

- Applied fixes for clippy warnings
- Fixed clippy warnings
- Fmt fixes
- Fixed a false positive

### Changed

- Deduplicate input dispatch and update missing docs
- Trim over-engineering (drop dead deps and newtype wrappers)
- Cut dead code and duplicated machinery
- Remove dead code and consolidate shared helpers
- Collapse over-engineered code paths (~2400 lines)
- Cut dead code and collapse duplicated logic
- Cleaned up general deadcode left from previous releas

### Miscellaneous

- Prune dead code and slim dependencies
- Wrap binary with runtime dependencies
- Include desktop files the package
- Put devshell and package in separate files

### Styling

- Cargo fmt
- Cargo fmt
- Cargo fmt
## [0.11.3] - 2026-08-22

### Fixed

- Don't push Kitty keyboard enhancement flags on Windows

### Release

- V0.11.3
## [0.11.2] - 2026-08-20

### CI

- Cargo fmt fixes

### Changed

- Changed default vault directory to 'Documents/clin Vault'

### Release

- V0.11.2
## [0.11.1] - 2026-08-18

### Added

- Added todo.txt incompleted tasks widget
- Add todo widget to notes view bottom section
- Added todo.txt parsing to the edit view
- Add builtin todotxt parsing and rendering support

### CI

- Fixed clippy warnings
- Fixed clippy and fmt warnings

### Documentation

- Mark todo.txt widget complete in roadmap

### Fixed

- Fixed draw widget bg color
- Fixed calendar widget activity tracking

### Miscellaneous

- Update ROADMAP.md
- ROADMAP.md update
- ROADMAP.md updates

### Release

- V0.11.1
## [0.11.0] - 2026-08-18

### CI

- Fixed fmt warnings
- Remove custom codeql workflow
- Applied cargo fmt

### Fixed

- Fixed resize confirming with right click
- Fixed resize confirming with right click

### Miscellaneous

- Bump rand from 0.8.7 to 0.10.2

### Comp

- Compatibility changes for rand bump

### Release

- V0.11.0
## [0.11.0-rc.1] - 2026-08-17

### Added

- Restrict marquee selection of groups to titlebar
- Constrain group node interaction to titlebar
- Implement robust draft recovery and ensure safe autosave
- Add Ctrl+s manual save shortcut
- Tweak autosave indicators per review
- Implement autosave with visual indicators

### CI

- Updated to codeql v4
- Custom codeql configuration without autobuild
- Use central workflows from .github repo
- Fixed fmt warnings

### Changed

- Cleanup dead code blocks and files

### Documentation

- Add editor draft recovery details to architecture and help

### Fixed

- Add zoom-scaled margin to hit testing to fix fractional coordinate truncation
- Use dynamic zoom-scaled hit height to match visual titlebar
- Increase group titlebar hit area to 60.0
- Preserve multi-selection when dragging nodes

### Miscellaneous

- Add beta labeler workflow caller
- Moved SECURITY.md and CODE_OF_CONDUCT.md
- Bump sha2 from 0.10.9 to 0.11.0
- Bump strum from 0.26.3 to 0.28.0

### Release

- V0.11.0-rc.1
- V0.11.0-rc.1

### Ui

- Remove text from saved/unsaved indicator
## [0.11.0-rc.0] - 2026-08-15

### Added

- Added color previews to color picker
- Added color picker for draw
- Use header dropdown for shape popup
- Simplify cursor paste bindings
- Show active transform modes
- Pan empty cursor drags
- Wire editing action help
- Unify live grid rendering
- Render selection transform overlays
- Add element action menus
- Add cursor selection moves
- Add history selection clipboard state
- Add precise affine hit testing
- Migrate files to stable v2 items
- Render orbiting tags as varied geometric shapes
- Looking glass expands downward with tags, visual stays fixed
- Show link count above tags in looking glass
- Title on looking-glass border, tags list below visual, drop meta line
- Add looking glass, context menu, and multi-select
- Tint no-title overlay text with edge color
- Edge overlay shows titles, edge colors, hover highlight
- Edge-list legend overlay with keyboard/mouse edge access
- Move mode status to header bar, add shortcut hint padding
- Dynamic key resolution for context menus
- Direct-letter canvas actions + menu shortcut hints
- Orthogonal edge routing toggle (Ctrl+O)
- Edge segments, edge context menu, edge color/style
- Multi-select rectangle with bulk color/delete
- Undo/redo with 20-slot snapshot stack
- Expand color palette from 6 to 10 colors
- Add EdgeStyle enum and style field to CanvasEdge

### CI

- Fixed fmt and clippy warnings
- Fixed fmt and clippy warnings
- Fixed clippy warnings
- Fixed fmt warning
- Fixed fmt warnings

### Changed

- Trim footer hints
- Unify graf and pinstar canvas features
- Rebuild simulation for local/group mode instead of hiding nodes

### Fixed

- Fixed shift + l conflicting with movement
- Shift + g shortcut not toggling grid
- Fixed small visual bug in set color
- Fixed middle click drag not panning
- Use muted accent for hovered/selected popup items
- Make header popup draw over the title bar
- Fixed links section not updating in graf view
- Fixed graf view overlays bleeding through the canvas
- Fixed grid rendering logic so it's uniforn 1:1
- Fixed grid rendering when zoom in/out
- Strengthen hover highlight
- Limit header mode notices
- Stabilize shared grid density
- Prefer selected item shortcuts
- Reserve clipboard shortcuts
- Orient selection controls
- Satisfy strict clippy
- Restore complete help suggestions
- Unify context menu shortcut background
- Match graph marquee color to pinstar
- Remove marquee selection border
- Tint marquee selection borders
- Correct graph view mouse targeting
- Link_count = total degree (forward + backlinks)
- Keep orphan nodes in local/group focus builds
- Deselect on empty click and preserve mode banner on box-select
- Unbind context-menu open key, restore minimap toggle
- Looking glass without selection ring, opaque background
- Render looking glass identical to simulation nodes
- Render looking glass as outlined node with edges
- Fix context menu height, hover, and right-aligned hints
- Fix context menu click offset and outside dismiss
- Apply connection changes without restarting simulation
- Show mode banner over header bar
- Show shortcut hints in set-color (EdgeColorPicker) menu
- Remove k/j from canvas MenuUp/MenuDown
- Enter completes delete-connection mode too
- Enter completes connection; delete connection works both ways
- Add-node shortcuts in menu, multi-select drag, set-color guard, orthogonal notify
- Select-rect overlay over nodes, click-deselect, add-node shortcuts
- Context menu UX and right-drag fixes
- Clippy lints for pinstar merge features

### Miscellaneous

- Gitignore update
- Gitignore update
- Cleanup project dir
- Clean up proj dir
- Updated roadmap
- Formatting fixes
- Cleanup graph tests
- Updated gitignore
- Updated gitignore
- Updated gitignore

### Styling

- Remove brackets from context menu shortcut hints

### Release

- V0.11.0-rc.0
## [0.10.1] - 2026-08-12

### Added

- Add scrollbar pan mode for notes list
- Add colored Tags section to note info popup

### CI

- Fixed formatting issues

### Fixed

- Remove redundant footer bar from confirm popups
- Remove full date from selected tree row
- Ignore key.bin and state.json in is_existing_vault
- Use help_tabs with glyphs for mouse tab hit-testing

### Release

- V0.10.1
## [0.10.0] - 2026-08-11

### Added

- Added clin ascii art to the about page
- Add in-session vault selection

### CI

- Removed x86_64-darwin
- Fixed ci warnings
- Routed all ci/cd channels to the central repo

### Documentation

- Synchronize docs with implementation

### Fixed

- Preserve markdown styles and compact headers
- Changed ascii art in the setup wizard
- Show form popup keys
- Label text popup shortcuts
- Reserve text popup keys
- Use character highlight offsets
- Reserve literal yes and no
- Preserve content line structure
- Align scrollbar with viewport
- Resolve pane and clipboard regressions
- Refresh previews and streamline text selection

### Miscellaneous

- Remove redundant repository files and folders
- .gitignore formatting

### Performance

- Add dedicated editor session pipeline

### Release

- V0.10.0
- V0.10.0
## [0.10.0-rc.7] - 2026-08-05

### Added

- Add Ctrl+d batch tag removal from select mode
- Split quick-keybinds into columns when terminal too short
- Add Compact style
- Add Hexagon hint bar style
- Add gradient powerline hint bar styles

### Changed

- Merge batch tagging mode into select mode

### Fixed

- Show_all_files = true now shows all files with '?'
- Use Ctrl+. for remove tags (Ctrl+r collides with RefreshNotes)
- Use Ctrl+r for batch remove tags (Ctrl+d collides)
- Show pinned folders in grid with correct colors

### Miscellaneous

- .gitignore and project structure changes

### Performance

- Batch input and cache highlight to kill O(N²) per-keystroke work

### Release

- V0.10.0-rc.7
## [0.10.0-rc.6] - 2026-07-26

### Added

- Add Ctrl+C/X/V, bracket-paste routing, notifier
- Add 4 hint bar styles (Bubbles, Blurred, Chips, Brackets)
- Add global F1 (help) and F5 (redraw) keybinds
- Bold key column and message labels in overlays
- Route all silent failures into message overlay
- Added error catching and notification system for warnings/fatals
- Standardize footer hints across all views
- Add Shift+Tab reverse cycle for grid layout tabs
- Add 15 palette actions for runtime-safe config toggles

### Changed

- Host-agnostic core for clin-gui

### Documentation

- Add Find in File, Outline, Links to editor popup help
- Sync all docs to current codebase state

### Fixed

- Route keyboard shortcuts through system clipboard path
- Reorder footer hints to put quit before help
- Duplicate push re-freshens existing entry instead of vanishing
- Fixed fatal error message for storage init
- Prevent header/footer overlap at small terminal widths
- Fixed colors with powerline themes repeating
- Revert draw preview to per-axis fill
- Match subnotes color and glyphs across layouts
- Render groups behind children, parse hex node colors
- Fit .canvas and .draw previews with uniform aspect
- Confine tag-popup textarea border to input area
- Check virtual paths in tree layout color logic
- Expand subnotes, fix graph bg, add sort indicator
- Run NaN reset before drag skip, scatter coincident nodes

### Performance

- Eliminate O(E·N) edge scan and per-frame allocations

### Release

- V0.10.0-rc.6
## [0.10.0-rc.5] - 2026-07-24

### Added

- Add QuickKeybinds overlay toggled by F2
- Preview pane scroll syncs with the textarea
- Add dynamic md coloring
- Add ghost syntax (conceal method) for markdown

### CI

- Fix actions/checkout and resolve clippy -D warnings

### Changed

- Removed READ mode and modal typing method
- More changes to the markdown style to make it's output unified
- Rewrite built-in layout engine

### Fixed

- Scale offset-range positions to reach track bottom
- Wrapping behaviour fixes
- Potential fix for graph view nodes flinging around
- Potential fix for graph view causing crash
- Codeblocks color fix so it uses a color from the theme
- Ctrl + f not launching the quick search
- Preview pane scroll sync fixes when wrap is on
- Reorganize hint items
- Fixed preview pane and textarea sync
- Tab not inserting indentation
- Fixed left margin for h1 headers
- Trim unsaved note whitespace
- Remove dead code warnings

### Performance

- Reduced the poll rate with frozen layout

### Release

- V0.10.0-rc.5
## [0.10.0-rc.4] - 2026-07-22

### Added

- Add deterministic angular jitter to static layout
- Add circular layout for static graphs
- Continuous scrolling READ preview
- Refine Graph layout to use double-click opening and respect preview pane
- Add top-level Graph layout option for notes
- Add graf.max_node setting
- Reorganize generated file layout
- Add `clin cache reset` command

### Changed

- Replace static circle layout with degree-ranked disk
- Implement simulated annealing graph physics

### Fixed

- Make Windows release build compile
- Drop isolated nodes when show_orphan is false
- Create directory before saving summaries
- Tolerate spaces and lowercase in Shift+letter binds

### Miscellaneous

- Warning cleanup

### Performance

- Optimize GFM renderer using run-based layout and async worker
- Optimize graph rendering and physics
- Cache preview codeblocks and simplify edit view
- Add LOD, spatial index, adaptive physics, and node cap
- Optimize view for large 10k note vaults
- Persist note summary cache to disk, defer graph rebuilds

### Release

- V0.10.0-rc.4
- V0.10.0-rc.3
- V0.10.0-rc.2
## [0.10.0-rc.1] - 2026-07-18

### Performance

- Cache folder list in FolderGraph preview, drop per-frame O(N) scan

### Release

- V0.10.0-rc.1
## [0.10.0-rc.0] - 2026-07-18

### Added

- Render FolderGraph note card with markdown
- Scroll note content by page
- Extend help view to 10 tabs (ContentTree, Setup)

### Changed

- Rename "Content tree" → "Outline" across code, config, and docs
- Remove redundant full-redraw clears from event handlers

### Fixed

- Track visual width, fit tables to pane, fix CJK in snapshot
- Cycle pages on middle-click, fix page indicator
- Remove Phase 2 render gate that caused ~500ms hover lag

### Performance

- Coalesce Moved events and gate redundant hover draws

### Release

- V0.10.0-rc.0

### Revert

- Remove ContentTree and Setup tabs
## [0.10.0-beta.5] - 2026-07-18

### Added

- Extended graph preview for all the folders
- Tweaks to the subnotes tab/folder and graph preview
- Draw inter-subnote wikilink edges in subnote graph
- Add browsable Subnotes view with grid tab and tree folder
- Perf-fix mouse selection, add read-mode select + clipboard, mode header
- Add READ/EDIT mode indicator to title bar
- Add modal READ/EDIT modes to Edit view
- Added scrollbar to related sections of the project

### Changed

- Replace subnote graph physics with static braille radial diagram
- Sync scroll cache on explicit scroll, add source-line map for READ↔EDIT
- Remove custom cursor-line and selection highlights
- Unify all list hit-tests through list_index_at

### Fixed

- Correct viewport math, pan direction, and add focus mode
- Fixed h1 title position in markdown renderer
- Align edit view hit-test layout with render layout
- Scale scrollbar position from offset to selection range
- Render help tab icons, coalesce mouse events, skip CreateNew in select
- Replace textarea search with own impl to fix wrap panic

### Release

- V0.10.0-beta.5
- V0.10.0-beta.4
## [0.10.0-beta.3] - 2026-07-15

### Added

- Replace DP+Chaikin smoothing with binomial filter
- Add image file support
- Render image nodes with filled block and icon
- Add mouse-drag panning and fix canvas text wrap
- Zoom previews toward cursor, fix scroll direction
- Scale snapshots to fill expanded pane
- Add draggable scrollbars to all scrollable panes
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

- Modernize arm runners to use native arm
- Dispatch release issue revert
- Fix arm64 apt sources for release workflow
- Forced ipv4 in dispatch release
- Fixed format checks
- Fix remaining format warnings in events and keybinds
- Fixed format warnings
- Migrate to a central workflow system

### Changed

- Removed image support from the draw view
- Refactored source editor pane to use the same code as the edit view
- Refactored edit view line highlight code
- Deduplicate layout, add UX features, context menu
- Remove dropdown border, add "Find:" label before input
- Rename QuickPopup→QuickSearch, render in header bar
- Apply fmt, canvas ocr image insertion, cleanup
- Unify popup footer rendering through PopupHints enum
- Unify UI helpers and improve safety
- Remove Accent style, rename Powerline variants, fix tab highlight
- Move page indicator to title bar via statusline

### Documentation

- Update README.md, ROADMAP.md and new config references

### Fixed

- Separate node display title from internal ID
- Preserve pin status through encrypt/decrypt
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
- Coalesce queued drag events; fix instant-apply popup cycle
- Hint bar preview dispatches on hint_bar_style
- Popup footers ignore hint_bar_style setting
- Split detail into powerline cells; fix right-side junction bg
- Restore x86_64-darwin support

### Miscellaneous

- Lock file regenerate
- Check fix
- Update readme about version bump
- Bump version to 1.90
- Cleanup project dir
- Sort Cargo.lock alphabetically
- Gitignore cleanup
- Modernize nix packaging

### Performance

- Decouple preview drag state from rendering
- Cache parsed data and re-render grids in-memory
- Render lightweight dot markers during pan/zoom transforms

### Release

- V0.10.0-beta.3
- V0.10.0-beta.3
## [0.9.10] - 2026-08-03

### Fixed

- Use XDG config dir on macOS instead of Library/Application Support

### Miscellaneous

- Revert version to 0.9.9 to allow 0.9.10 release
- Bump toml from 1.1.3+spec-1.1.0 to 1.1.4+spec-1.1.0
- Bump clap from 4.6.2 to 4.6.4
- Bump serde_json from 1.0.150 to 1.0.151
- Bump anyhow from 1.0.103 to 1.0.104
- Bump uuid from 1.23.5 to 1.24.0
- Bump clap from 4.6.1 to 4.6.2

### Styling

- Fix line break in config_dir assignment

### Release

- V0.9.10
- V0.10.0-beta.2
- V0.10.0-beta.0
## [0.9.9] - 2026-07-14

### Fixed

- Wire libgit2 auth callbacks into push/pull
- Add universal q/Esc back/quit intercepts, override-proof

### Miscellaneous

- Add support section to the readme

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

