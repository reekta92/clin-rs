# Popup implementation report

Date: 2026-10-25. Branch: `fix/popup-design-consistency`.

This report records the implementation following [the original visual audit](POPUP_DESIGN_AUDIT.md). It covers every inventoried popup, including surfaces that receive changes through shared renderers and surfaces intentionally retained. Both reports live at the repository root, outside the official wiki in `docs/`.

## Result

The main improvements are reachable content, fitted instructions, consistent keyboard selection, clearer destructive actions, and shared dialog titles and borders. Info and confirmations now scroll; Subnotes adapts to narrow terminals; header dropdowns and plugin menus respect their available frame. Calendar, Go to Line, drawing text editing, and Canvas Rename follow the existing detached-banner convention.

This is not a claim of identical layouts or complete visual acceptance. Forms, contextual menus, inspectors, and header overlays retain layouts appropriate to their tasks. Automated rendering establishes geometry and selected style contracts; actual terminal colors, glyphs, and native picker appearance still need interactive review.

## Shared changes

- Popup footers prioritize primary actions and dismissal before secondary shortcuts. They use the existing terminal-width fitter, keep the classic popup hint style independently of the main footer setting, and indicate omitted hints with an ellipsis.
- A shared selected style supplies the theme's highlight foreground and background with bold text. Palette and Find selection use this pair; pointer hover remains distinct. Selected palette glyphs and descriptions no longer override it with low-contrast foregrounds. Color swatch previews retain their actual colors.
- Popup text areas receive explicit theme text and cursor styles. This reaches short text prompts and other callers of the shared text-area constructor.
- `PopupSize::Content` supports bounded content-sized value dialogs. Multiple-choice palette workflows use the same stable Large shell as Theme, with a scrollable results section. Standalone option lists also use Large geometry with an overflow scrollbar. Rendering and mouse routing use the same sizing rules.
- Display-width and grapheme-aware fitting replace several character/byte-count assumptions. This improves wide-character paths, combining characters, menu padding, and wrapping.
- Shared confirmation rendering wraps message and detail text into a scrollable viewport while keeping action buttons separate. Keyboard and mouse scrolling work for global, palette-owned, and nested confirmations.

At extremely small sizes, a terminal still cannot display every title, instruction, and control simultaneously. The changes prevent several geometry failures and preserve essential controls where space permits; they do not establish a usable one-cell application.

## All 20 global popup variants

“Shared” below means the popup inherits the common footer, text-area, or sizing changes rather than receiving a separate redesign.

| Popup | What changed | What remains task-specific |
| --- | --- | --- |
| `Template` | Fitted footer; explicit themed input/cursor; normalized “No matching templates” state. | Filter/results stack, template previews, deletion workflow. |
| `Theme` | Shared footer fitting and input conventions. Its palette entry receives the palette selection and focus fixes. | Theme preview, custom marker, background toggles, and setup-owned toggle restrictions. |
| `Info` | Replaced fixed section layouts with wrapped themed content, scroll state, and a scrollbar. Arrows/j/k, PageUp/PageDown, Home/End, wheel, and scrollbar handling expose long metrics, paths, tags, and final sections. Active border uses heading color. | Note and folder information retain their metrics and sections. |
| `Tag` | Shared input/cursor and footer improvements. Layered tag deletion inherits wrapped, scrollable confirmation text. | Single/batch tagging, suggestions, and all-tags list. |
| `IconMode` | Stable Large option-list shell; mouse selection uses the same geometry and scroll offset. Shared footer fitting applies. Palette entry matches Theme's size and scrollable section. | Nerd Font, Unicode, None, and Emoji choices. |
| `HintBarStyle` | Large shared option list with overflow scrollbar and matching mouse routing; fitted popup instructions remain classic regardless of the selected main-footer style. Palette entry matches Theme's size. | All hint styles and palette previews remain available. |
| `RemoveTags` | Explicit `[x]`/`[ ]` marks separate multiselection from the standard keyboard cursor highlight. Nested confirmation can wrap and scroll. | Counts, batch scope, and destructive action semantics. |
| `KeybindPreset` | Large option list, matching mouse routing, and shared footer improvements. Palette entry matches Theme's size and scrolling design. | Default, Helix, Vim, and Emacs presets; existing title wording. |
| `Sort` | Large option list, matching mouse routing, and shared footer improvements. Palette entry matches Theme's size and scrolling design. | Four existing sort choices. |
| `Folder` | Shared themed text input/cursor and fitted create/rename instructions. | Existing compact create/rename prompt. |
| `FolderPicker` | Normalized “No matching folders” state; shared footer/input improvements. | Move/copy operations for notes, folders, bulk and mixed selections; existing filter/results layout. |
| `NoteRename` | Shared themed input/cursor and fitted instructions. | Existing target-filename preview and collision handling. |
| `VaultRename` | Shared themed input/cursor and fitted instructions. Palette rename is also content-sized. | Existing vault display-name behavior. |
| `CreateNote` | Shared input/cursor and fitted create/cancel instructions across Markdown, text, drawing, canvas, and Bases prompts. | Markdown/text filename previews and format-specific creation behavior. |
| `Import` | Standalone input changed from Large to Prompt size; its active border now uses heading color. Shared themed input and fitted import/cancel instructions. | Source/target handling, append behavior, native file selection, and clipboard flow. |
| `CreateFormat` | Five-choice list uses the same Large standalone option shell and shared render/mouse geometry. | All five file formats. |
| `Search` | Added focus-aware `Enter close` for input and `Enter jump` for results, plus Esc cancellation, Tab focus switching, and result-navigation guidance. Primary action/cancel survive before filter syntax when width is limited. | Title/path, folder/pinned/tag, grep and subnote modes, groups, truncation notices, and results layout. |
| `TrashView` | Added an explicit “Trash is empty” state without highlighting a nonexistent item. Shared footer fitting applies. | Populated list, scrollbar, restoration, and permanent-deletion flows. |
| `Goals` | Shared text-input/cursor and footer improvements; palette alternatives become content-sized value workflows. | Daily word and note goal validation and behavior. |
| `Subnotes` | Replaced the fixed 34-column split with shared responsive geometry. Narrow popups show the focused list or editor; wider ones allocate 35% to the list while protecting editor space. Mouse routing uses the same geometry. Empty state gives creation guidance; footer fits. | Title/content editing, existing save behavior, and Esc return to list. |

## All 10 confirmation variants

Every row below inherits wrapped/scrollable message and detail content, separate visible buttons, fitted action/cancel/scroll hints, and shared rendering/mouse button geometry. Action identity, safety text, destructive classification, and initial button selection are preserved; no blanket change to default selection was made.

| Action | Retained consequence and action |
| --- | --- |
| `DeleteNote` | Recoverable Trash action. |
| `DeleteFolder` | Recoverable Trash action for the folder and its contents. |
| `DeleteTag` | Destructive Delete action; existing Tag-layer Enter/Esc behavior and other yes/no handling. |
| `DeleteTemplate` | Destructive Delete action and permanent-removal explanation. |
| `DeleteFromTrash` | Delete Forever action and irreversible warning. |
| `EmptyTrash` | Destructive Empty Trash action, count, and warning. |
| `BulkDeleteItems` | Recoverable Trash action with note/folder counts. |
| `QuitApp` | Non-destructive Quit action. |
| `RemoveVault` | Remove action with path and explanation that files on disk are not touched. |
| `RemoveAllTagsFromSelected` | Destructive Remove All action, including the confirmation nested in Remove Tags. |

## Command palette and every workflow

The root palette now uses the standard selected color pair, distinct pointer hover, heading-colored active borders, and fitted instructions. Errors wrap into up to three status rows and appear even when no workflow is active. Empty status rows are omitted rather than leaving a dark strip between the input/current value and Apply. Nonempty status rows use the theme's surface background. This applies to URL import, both goals, document naming, vault rename, and Bases workflows through their shared renderer. Input, controls, status, and result geometry are shared with mouse routing.

| Workflow | What changed |
| --- | --- |
| `Theme` | Standard selection and focus treatment while preserving actual swatches and background controls. Large browsing geometry remains stable while filtering. |
| `Features` | Inherits selection, focus, hints, and error treatment; retains Large browsing geometry. |
| `Options` | Stable Large shell matching Theme, including filter, current value, selected rows, previews, and scrolling. Size does not shrink with option count or filtering. Covers `Sort`, `Icon`, `Hint`, `Preset`, and `Layout`. |
| `Goal` | Compact value workflow with space for status and Apply; covers word and note goals. |
| `Create` | Compact naming/value presentation with space for Apply and errors; covers drawing, canvas, and Bases continuations. |
| `Import` | Compact URL/value presentation with status and Apply; source, target, and progress handling remain intact. |
| `Vault` | Standard selection/focus/error treatment with stable Large browsing geometry. |
| `VaultName` | Content-sized rename continuation with status and Apply. |
| `Bases` | Shared palette sizing, selection, status, and mouse geometry; typed clearing/removal prompts receive destructive borders. Preparing, Running, choices, and multiline Prompt states remain supported. |

Bases configuration commands continue to reuse their existing prompt/choice owner: view rename/layout, filters, properties, sort/grouping/manual group order, limits/heights/widths, list formatting, card/kanban and cover options, summaries/formulas/labels, view creation/removal, new note, and export. Commands that act immediately or open the source editor do not acquire unnecessary dialogs.

## Editor, header overlays, Calendar, setup, and Bases

| Surface | What changed |
| --- | --- |
| Editor save/open error | Shared detached banner and destructive border; themed wrapped error text. Existing dismissal/lifetime behavior remains. |
| Editor link preview | Inherits fitted shared footer; loading, document, error, and embedded content layouts remain. |
| Go to Line | One standard content border, detached banner, heading focus, ordinary text/placeholder colors, and one classic jump/cancel footer replace nested borders and custom keycaps. |
| Editor Find | Shared Quick Search uses terminal widths, frame-bounded dropdown height, standard selection, and a visible selected window after resize; mouse hit testing uses the same offset. |
| Help search | Inherits the same Quick Search geometry and selection fixes. |
| Graph search | Inherits the same Quick Search geometry and selection fixes; host selected text follows the shared color contract. |
| Bases search | Inherits Quick Search fixes; retains pending/results/count behavior. |
| F2 keybind reference | Width-aware header and row fitting; bounded columns; safe tiny-frame arithmetic; “More shortcuts in Help” when not all columns fit. |
| F3 Messages | Dropdown width is bounded by the frame. Ratatui wrapping replaces hand-split character wrapping; the message viewport scrolls across wrapped paragraphs. Severity colors and stripes remain. |
| F4 Vaults | Frame-bounded width/height and display-width path truncation that retains the path suffix. Ordinary rows retain the accent background. Selection swaps the accent foreground/background pair so it stays distinct even when the theme's highlight background equals accent; selected text and path/details use accent on the contrasting background. A `›` cursor identifies selection independently of color, including the Add row. The active-vault dot remains separate. Renderer and mouse share the visible window. |
| Calendar `Form` | Shared banner/frame/footer; heading focus labels and themed selected cursor; errors have a separate wrapped destructive row. Existing new/edit event fields and form scrolling remain. |
| Calendar `Path` | Compact shared path prompt for import/export, with the same focus, cursor, and separate error treatment. |
| Calendar `Notes` | Inherits Quick Search changes; linked markers and link/open behavior remain. |
| Calendar `Delete` | Shared destructive frame and readable wrapped series-deletion text with keyboard/wheel scrolling. Existing confirmation keys remain. |
| Calendar `Conflict` | Shared consequential styling and a scrollable wrapped comparison; existing keep-local/use-remote choices remain. |
| Calendar `Help` | Shared banner/frame/footer plus scrollable wrapped text and scrollbar. Closing dialogs resets the dialog scroll state. |
| Setup `PathInput` | Inherits shared footer and text-area styling; retains the existing short explanatory/input layout. |
| Setup `ConfirmNonEmpty` | Inherits fitted shared confirmation hints; keeps its existing wrapped directory explanation and confirmation behavior. It did not receive a new scrolling viewport. |
| Setup exit | Three visible role-colored Save/Exit, Discard, and Back actions replace prose-only choices. Mouse hitboxes come from the same button layout as rendering. Existing y/q/n/Enter/Esc behavior remains. |
| Bases `Prompt` | Standard bordered frame, wrapped instructional hint, fitted save/cancel/paste footer, and destructive styling for typed CLEAR/view-removal safeguards. Input remains multiline where required. |
| Bases `Image` | Inherits shared footer fitting; retains its large image viewport. |
| Embedded Bases dialogs | Reuse the same updated search and prompt renderers in note/editor/link previews and Canvas embeds. |

The latest main-branch Calendar improvements were merged before final validation. This report describes the popup-specific delta against that updated main, not unrelated Calendar changes.

Vault switching also fixes a state-loss bug: before a persistent switch, Clin now retains both the current storage path and destination in the configured vault list, deduplicating expanded paths. This preserves an implicit starting/default vault after adding and switching to a new vault, so switching back does not require re-adding it. The existing save-before-switch checks, rollback configuration, and session-only `--vault` behavior remain intact.

## Drawing, Graph, and Canvas dependency surfaces

These changes live in their owning libraries, with host adapters supplying the necessary theme tokens. All three library branches are named `fix/popup-design-consistency` and have been pushed.

| Surface | What changed |
| --- | --- |
| Drawing context menus | Frame-bounded menus, selected-row scrolling and matching mouse offset, terminal-width padding, selected shortcut contrast, and hover that does not erase selection. Covers action sets for empty/text/other targets and Color submenu. |
| Drawing shape selector | Bounded visible window follows selection; mouse uses the same offset; hints sit beside the selector. Covers rectangle, ellipse, diamond, line, and arrow. |
| Drawing color selector | Same bounded selector behavior; labels retain readable selection foreground while swatches keep actual colors. |
| Drawing text editor | Native-style short prompt dimensions, detached EDIT TEXT banner, heading border, and adjacent classic instructions. |
| Graph context menu | Bounded geometry, including nonzero frame origins; selected window and matching mouse offset; width-aware padding and readable selected shortcuts. Host hover style is distinct from keyboard selection. Covers node/no-node and multiselection action sets. |
| Graph config errors | Shared destructive banner/frame, wrapped diagnostic/fix text, scroll keys/wheel, and closing instructions replace the bespoke rounded box. |
| Graph reload notice | Shared titled frame, content-measured wrapped height, and destructive treatment on failure replace the untitled three-row box. |
| Graph looking glass | Existing contextual inspector retained; no forced conversion into a centered dialog. |
| Canvas context menus | Bounded visible window, selected-row scrolling and matching mouse offset, width-aware labels/shortcuts, and readable selected shortcuts. Covers `Canvas`, `Editor`, `ColorPicker`, `ShapePicker`, `EdgeMenu`, `EdgeColorPicker`, `EdgeStylePicker`, and `OrientationPicker`. |
| Canvas Rename | Half-width/max-80 short prompt with detached banner, heading border, and themed cursor. Clin renders one classic footer from actual remapped bindings; duplicate literal Enter/Esc hints are suppressed. Standalone library hints still honor `show_hints`. |
| Canvas floating editor | Existing node-anchored editor retained; its contextual placement is intentional. |
| Canvas edge inspector | Dimensions bounded to available canvas; wheel scrolling, matching click offset, scrollbar, and corrected first-row hover keep long edge lists usable. |

| Dependency | Pinned commit | Version resolved in the source build |
| --- | --- | --- |
| [draw-rs](https://github.com/reekta92/draw-rs/tree/fix/popup-design-consistency) | `e475c841911886f23052ca79f4efb900fc126997` | 0.1.3 |
| [graf](https://github.com/reekta92/graf/tree/fix/popup-design-consistency) | `8ee98a768c99f59a570f558a08c3800dbaca9df8` | 1.1.1 |
| [pinstar](https://github.com/reekta92/pinstar/tree/fix/popup-design-consistency) | `490fb6e3acf8c3aede483b5124ddd5bbdaa206b2` | 1.1.4 |

`Cargo.toml` and `Cargo.lock` pin these exact Git revisions. Dependency policy continues to deny unknown Git sources and allows only the three named repositories.

## Retained boundaries and remaining criticism

- OS file and directory pickers retain platform-native behavior. No native picker redesign or cross-platform screenshot comparison was performed.
- The unused `draw_header_dropdown` helper is not counted as a new active popup. Upstream standalone Graph help/search/config and Canvas help remain outside normal Clin help/search routing.
- Graph legend/minimap, outline panes, inline frontmatter editing, image surfaces, and selection rectangles remain contextual view content rather than being restyled as dialogs.
- Long secondary shortcut lists still cannot all fit into one footer. The fitter prioritizes essential actions and signals omission; it does not introduce a new expanded-hints interface.
- Some short explanatory surfaces remain fixed-height: setup path/nonempty-directory messages and editor errors did not receive the Info/confirmation scrolling implementation. Very long paths or error strings can still exceed those viewports. Palette status messages intentionally reserve at most three wrapped rows.
- Entry-point differences remain where the palette provides filtering/previews and a standalone popup provides direct editing. Existing naming differences such as Keybind Preset versus Navigation Preset were not broadly rewritten.
- Theme tests verify assigned foreground/background contracts, not measured WCAG contrast or the user's actual terminal palette. A consistent token pair does not guarantee every custom theme is readable.

## Verification

Initial implementation source checks, before the follow-up corrections below:

| Check | Result |
| --- | --- |
| Isolated `cargo test --offline --locked --tests -- --test-threads=4` | 830 passed, 0 failed, 11 ignored: 809 library tests and 21 integration tests. The signal-handling test also launches a child test run; its repeated result is excluded from this total. |
| `cargo clippy --offline --locked --all-targets -- -D warnings` | Passed. |
| `cargo +1.90.0 check --offline --locked --all-targets` | Passed on the declared minimum Rust toolchain. |
| `cargo deny --locked --all-features check` | Advisories, bans, licenses, and sources passed; duplicate-dependency and unused-license-allowance warnings remain. |
| `cargo fmt --all -- --check` and `git diff --check` | Passed. |
| Draw tests and all-target Clippy | 49 tests passed; Clippy passed. |
| Graph tests and all-target Clippy | 55 tests passed; Clippy passed. |
| Pinstar all-feature tests and all-target/all-feature Clippy | 60 tests passed; Clippy passed. |

Tests exercise representative renderer families at 80×24, 120×40, and 40×12, with tiny header-overlay cases at 1×1 and 8×4. Coverage includes long Unicode content, narrow Subnotes editing, scrolling Info/confirmations/Calendar help, setup mouse targets, visible selection after resizing, and palette Apply-row/status geometry. Theme coverage checks all 19 built-in themes with solid/transparent backgrounds. Canvas Rename integration tests cover remapped F7/F8 bindings across all 13 hint styles and both background modes.

These are family and behavior tests, not an independently captured screenshot of every variant. The 11 existing ignored tests were not enabled. No interactive terminal walkthrough or Windows/macOS native-dialog review was performed. Full tests used temporary XDG config/data directories to isolate application state.

### Follow-up acceptance checks

The reported selector sizes, empty status strip, missing starting vault, and invisible F4 selection were reproduced with failing regression tests before their fixes. The follow-up changes passed:

| Check | Result |
| --- | --- |
| Palette suite | 70 passed, including five new regression tests. |
| UI design suite | 31 passed. |
| Existing vault-switcher event test | 1 passed. |
| All-target Clippy with warnings denied | Passed. |
| Formatting and diff whitespace checks | Passed. |

The five regression tests cover all five palette option families and stable filtering geometry; overflow and mouse alignment in the standalone hint selector; omission of empty status rows and themed error backgrounds for URL import, both goals, drawing, canvas, and Bases creation; preservation of an implicit starting vault through rebootstrap and a return switch without duplicates; and F4 accent preservation plus cursor/selection contrast on the vault and Add rows across all 19 built-in themes with solid and transparent backgrounds. The palette suite and Clippy also passed after the subsequent F4 correction restoring accent-colored ordinary rows. No interactive terminal acceptance was performed for this follow-up. The earlier full suite and dependency/toolchain checks were not repeated because the focused affected suites and Clippy passed.

## Packaging limitation

`cargo package --offline --locked --allow-dirty` creates the package but fails its verification build. Cargo replaces Git dependencies with registry dependencies for that build. Published Draw/Graph versions lack the new `draw::ThemeColors.heading` and `graf::ThemeColors.menu_hover` fields required by the host adapters.

The checked-out branch builds and tests against exact Git pins. A registry-distributable release still requires coordinated library releases and updating Clin to those published versions. No library release or application release was published as part of this work.

## Commit trail

Clin changes were split into conventional commits:

1. `12e01f2a` — record the source-based popup audit.
2. `2ea0f00f` — make popup content and essential controls reachable.
3. `aee01657` — unify dialog chrome and content-based prompt sizing.
4. `ed52c192` — wrap/scroll long confirmations without changing actions.
5. `f0825790` — bound header overlays in narrow terminals.
6. `356b0437` — integrate pinned drawing, Graph, and Canvas popup fixes.
7. `c9258c36` — preserve selected contrast and wrap palette errors.
8. `59d5cf75` — merge current Calendar improvements before validation.
9. `d7cb362` — expose Search actions before secondary filter hints.
10. `fd9229ab` — record the initial implementation report and validation.
11. `76b446c` — match option workflows to Theme and remove empty status rows.
12. `2422a34` — retain the starting vault and make F4 selection visible.

Library work was also split into conventional commits: Draw `f2b2a4f` and `e475c84`; Graph `1460a60` and `8ee98a7`; Pinstar `c806e3a` and `490fb6e`. This follow-up report update is committed separately from implementation.
