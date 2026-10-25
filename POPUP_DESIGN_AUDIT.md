# Popup visual consistency audit

Audited on 2026-10-25 against commit `5da1c1e9`.

## Verdict

Clin has a recognizable popup palette, but only a partially consistent popup design. Most native dialogs share uppercase banners, theme colors and borders. Their sizing, focus indicators, selection treatment, information density and instructions are much less coherent. Calendar and several editor/plugin dialogs visibly follow different conventions.

The most serious problems are practical: instructions are clipped, information panels cannot expose all their content, and fixed layouts leave too little room for editing. These deserve attention before cosmetic border changes.

| Dimension | Assessment | Reason |
| --- | --- | --- |
| Theme vocabulary | Mostly consistent | Native dialogs and plugin adapters reuse theme colors. Some selection foregrounds use the wrong semantic pair. |
| Titles and dialog anatomy | Mixed | Detached uppercase banners coexist with titles inside borders, full-width header bars and untitled floating editors. |
| Focus and selection | Weak | Heading borders, accent borders, muted borders, highlight fills, hover fills and reversed cursors all communicate active state. |
| Spacing and sizing | Weak | Generic percentage sizes coexist with fixed widths, percentage-only plugin prompts and large mostly empty value dialogs. |
| Instructions and errors | Weak | Different hint formats, locations, lengths and error treatments; several hints cannot fit. |
| Context menus | Relatively consistent | Graph, drawing and canvas menus share borderless rows, padding, shortcuts and theme highlights. Hover behavior still differs. |

Different task types should retain different layouts. A context menu, an image preview and an event form do not need identical dimensions. The inconsistency is in their shared visual language and their handling of limited space.

## Method and limits

This is a source-based design audit, not a screenshot review or interactive acceptance run. Findings describe implemented styling and layout constraints. Terminal palette appearance, actual contrast, glyph rendering and platform-native dialogs were not visually measured.

Coverage includes all 20 `ActivePopup` variants, all 10 `ConfirmAction` variants, every palette workflow, calendar and setup dialogs, editor overlays, header dropdowns, native file/directory picker paths and the popup renderers in the locked drawing, graph and canvas dependencies. Shared renderers are assessed once; their variants are enumerated below. Definitions without normal host callers are marked separately.

The source scan covered all 188 Rust files under `src/`, then followed plugin render calls into cached sources for `draw-rs 0.1.2`, `graf-rs 1.1.0` and `pinstar 1.1.4`, matching `Cargo.lock`. A temporary Graphify structural map helped locate relationships; source reads established the findings. Graph extraction used no semantic LLM calls, so its reported semantic token cost was zero. No application code was changed.

## Findings, in priority order

### 1. High: popup instructions are routinely too wide to display

`draw_popup_footer` puts the complete hint line into a centered, single-row `Paragraph`. It neither wraps nor fits the hints to the available width. Meanwhile, `Large` popups occupy only 60% of the terminal and stop growing at 100 columns.

The Templates hint line is **88 terminal columns**. The Subnotes hint line is **105 columns**, so it cannot fit even at the maximum popup width. At an 80-column terminal, both get only 48 columns. Navigation, cancellation and editing instructions disappear through clipping. Search also spends its footer on filter syntax rather than explaining navigation or dismissal.

This makes otherwise polished dialogs feel unfinished. The main application footer already has width-fitting logic; popup footers do not get equivalent handling. Reuse the fitting policy where appropriate, keeping essential confirmation/cancellation hints visible and moving secondary instructions to help.

Evidence: [popup footer and frame](src/ui/popups.rs#L1185), [size rules](src/ui/popups.rs#L563), [Templates hints](src/ui/popups.rs#L17), [Subnotes hints](src/ui/popups.rs#L1400), [Search hints](src/ui/mod.rs#L1214), [existing footer fitter](src/ui/popups.rs#L919).

### 2. High: Info is too small for its own content and has no scrolling

At 80×24, the Medium popup is 40×12. Removing its footer and content border leaves only nine content rows. Note Info requests six metrics, a spacer, three or four more metrics, another spacer, file path, folder path, tags and top words. Even the first two metric groups exceed that budget.

The renderer assigns fixed row requirements to every section; there is no scroll offset or scroll control in `InfoPopup`. Content must be compressed or clipped when its required height exceeds the panel. Long paths, Unicode text and many tags make this worse. The panel looks like a complete information card while concealing information.

Make the information reachable with a content viewport or a suitably adaptive layout. Merely changing Medium to Large will not solve arbitrary content length.

Evidence: [Info renderer](src/ui/popups.rs#L146), [actual note information](src/actions/info.rs#L77), [Info state](src/popups.rs#L300).

### 3. High: Subnotes wastes scarce editor space on a fixed list width

Subnotes reserves 34 columns for the left list regardless of popup width. At 80 terminal columns, the popup is 48 columns wide: the editor gets 14 columns, or **12 usable columns after borders**. At 40 terminal columns, the popup is only 30 columns wide, so the requested list width alone exceeds the entire popup.

The result is a cramped content editor beside a disproportionately large list. The three bordered regions and oversized footer consume more space. Use a minimum viable editor width and change the split when space is insufficient.

Evidence: [Subnotes split and fields](src/ui/popups.rs#L1416), [popup size rules](src/ui/popups.rs#L563).

### 4. Medium: selection means different things in different popups

Most ordinary lists use `highlight_fg` on `highlight_bg`, with bold text. Quick Search instead paints the selected row with `heading`; editor, help and graph callers explicitly give its selected text `fg`. With the default theme that means White text on Yellow rather than the standard Black on Cyan. Exact contrast depends on the terminal palette, but this is plainly a different color contract.

The command palette uses the hover background for keyboard selection and adds bold; pointer hover uses the same background without bold. It also leaves input and results borders muted. Users have to distinguish active selection primarily by typography. Remove Tags adds another selection vocabulary using accent/heading fills and underline. Drawing color selectors apply the chosen color to the label itself, overriding the normal selected foreground.

Use a stable foreground/background pair for keyboard selection, a distinguishable pointer hover and an independent mark for multiselection. Preserve color swatches without requiring the whole label to carry the swatch color.

Evidence: [Quick Search selected fill](src/ui/quick_search.rs#L228), [editor result foreground](src/ui/edit_view.rs#L441), [help result foreground](src/ui/help.rs#L324), [graph result foreground](src/graf_adapter.rs#L1622), [palette selection](src/ui/mod.rs#L1016), [Remove Tags states](src/ui/mod.rs#L609), [default colors](src/app_theme.rs#L59). Drawing dependency: `src/ui/mod.rs:102`, `draw_selector`.

### 5. Medium: Calendar uses a separate dialog design without a clear benefit

Event forms, import/export, deletion, conflict resolution and calendar help bypass the shared popup frame. They use sentence-case titles inside an accent border, plus two left-aligned footer rows inside the frame. Standard dialogs use uppercase detached banners and one centered footer row outside the content border.

The event form reasonably needs multiple labeled fields. Its different title, border and instruction styling does not follow from that need. Calendar note selection then switches back to the global Find header, making one feature span three visual patterns.

Retain the form layout and field scrolling; reuse shared title, footer and focus conventions around it.

Evidence: [all Calendar dialog rendering](src/calendar/view.rs#L1596).

### 6. Medium: destructive and consequential choices lack a consistent presentation

Generic confirmations provide a useful convention: destructive border/action colors, explicit action/cancel buttons and Cancel selected initially. Calendar's Delete dialog instead has the same accent border as safe forms and only a textual `y: delete entire series` instruction. Conflict resolution similarly relies on footer prose to distinguish replacing local data from keeping it.

Bases property clearing and view removal use typed prompts rather than the generic confirmation layout. That can be a valid safety mechanism, but visually these remain ordinary editing/value-input surfaces. Setup exit uses the confirmation frame but presents three actions as prose, while generic confirmations render two visible buttons.

Keep each operation's safety semantics. Standardize the visual indication of consequence and make available actions comparably visible. Trashing recoverable files versus permanently deleting them is already intentionally distinguished; preserve that distinction.

Evidence: [generic confirmation](src/ui/popups.rs#L1297), [initial Cancel and action classifications](src/app/popups.rs#L371), [Calendar deletion/conflict](src/calendar/view.rs#L1665), [Bases clearing prompt](src/bases/authoring.rs#L549), [Bases removal prompt](src/bases/palette.rs#L165), [setup exit](src/ui/setup.rs#L1058).

### 7. Medium: equivalent tasks change size and appearance depending on entry point

Theme, sort, icon mode, hint style and navigation preset can use compact standalone selection dialogs or the Large command-palette workflow. Goals can use a five-row prompt or that same large palette shell. Drawing/canvas/Bases naming and URL import also have standalone and palette presentations.

The palette adds useful filtering, current-value indicators and previews. However, a single value can sit in a large shell with mostly unused result space. Standalone Import also uses Large even though the normal text-prompt action is a URL. Worse, its only input border is muted while ordinary naming prompts use heading to indicate activity.

Entry-point differences should explain additional capability, not change the meaning of focus or impose unnecessary empty space. Share the task presentation rules while preserving palette navigation.

Evidence: [popup-to-palette routing](src/palette/workflows.rs#L329), [palette frame and input](src/ui/mod.rs#L867), [palette input sizing](src/palette/managers.rs#L16), [standalone Import](src/ui/mod.rs#L1191), [actual import entry paths](src/actions/import.rs#L119).

### 8. Medium: Go to Line and Canvas Rename look like older, separate widgets

Go to Line uses a title inside an outer border, a second bordered input, an accent-filled value and custom keycap-style hints inside the dialog. Other short text prompts use one content border, a detached banner and classic footer hints. The extra nesting is especially expensive in a five-row popup.

Canvas Rename uses a percentage-sized upstream prompt with its title and literal Enter/Esc instructions inside the border. Clin then adds another hint line below it using configurable main-footer hint formatting. The duplicate instructions can disagree after key remapping; their styling also changes with `hint_bar_style`, unlike normal popup footers.

Use one title and one authoritative hint presentation per prompt. Keep key display owned by the host that resolves those bindings.

Evidence: [Go to Line](src/ui/edit_view.rs#L463), [Canvas host hints](src/pinstar_adapter.rs#L455), [fixed popup hint style](src/ui/popups.rs#L1174). Pinstar dependency: `src/render.rs:2051`, `render_rename_popup`.

### 9. Medium: resize and Unicode handling are inconsistent across families

The shared banner correctly measures terminal display width. Several other overlays use character counts or byte lengths for titles, wrapping and geometry. Info estimates wrapping with character counts; Messages splits text by characters; Vaults truncates paths by character count; Graph diagnostics use byte lengths. Wide characters and combining sequences therefore do not receive equivalent space handling.

Messages and Vaults clamp their width to a minimum of 30 without also capping it to the terminal width. Quick Search caps the number of results requested by the caller, but does not itself cap dropdown height to the frame. Bases supplies a height-aware limit; editor/help/calendar use fixed limits. These differences produce inconsistent behavior in small windows.

Use terminal-column measurements and frame-bounded geometry at the owning renderer. This is a readability issue, not merely alignment polish.

Evidence: [correct banner width](src/ui/popups.rs#L540), [Info wrapping estimates](src/ui/popups.rs#L173), [message wrapping and minimum width](src/ui/message_overlay.rs#L78), [Vaults width and truncation](src/ui/vault_switcher.rs#L55), [Quick Search geometry](src/ui/quick_search.rs#L195), [Bases height-aware caller](src/bases/search.rs#L32), [Graph diagnostic sizing](src/graf_adapter.rs#L1673).

### 10. Medium: error presentation does not have a common visual hierarchy

Editor save/open errors use a plain bordered `Error` box with no shared banner, themed destructive border or dismissal footer. Graph configuration errors use a rounded `Config Error` border and inline closing instructions. Graph reload notices use a third, untitled three-row box. Calendar errors occupy the footer, color both error and instructions warning, and compete for limited instruction space. Palette errors use a dedicated destructive-colored workflow status row.

Users should recognize an error before reading it. Use the existing destructive/warning vocabulary consistently and keep recovery instructions legible. The right content layout can still differ between a brief notice and a list of configuration problems.

Evidence: [editor error](src/ui/edit_view.rs#L425), [Graph config error](src/graf_adapter.rs#L1648), [Graph reload notice](src/graf_adapter.rs#L1709), [Calendar footer](src/calendar/view.rs#L1673), [palette error row](src/ui/mod.rs#L974).

### 11. Low: empty states and field labeling vary unnecessarily

Templates and folders say `(no matching …)`; Search says `No results` or `Type to search notes`; Quick Search says `No matches`; Remove Tags says `No tags to remove`; Subnotes uses centered italic instructional prose; empty Trash has no explicit empty-state item. These are related states but vary in tone, alignment and helpfulness.

Many native text fields rely on their banner or placeholder for context. Calendar uses persistent field labels; palette value inputs say `Value`; standalone Bases uses an unbordered text area below an instructional line. These differences should follow task needs, with one consistent way to present labels, placeholders and recovery advice.

Evidence: [Templates](src/ui/popups.rs#L54), [Search](src/ui/mod.rs#L1543), [Quick Search](src/ui/quick_search.rs#L223), [Remove Tags](src/ui/mod.rs#L595), [Subnotes](src/ui/popups.rs#L1432), [Trash](src/ui/mod.rs#L1623), [Bases prompt](src/bases/authoring.rs#L124).

## Complete surface inventory

“Aligned” means the surface uses the dominant native pattern. It does not imply that clipping, accessibility or theme behavior passed runtime checks. Shared problems above apply even to aligned surfaces.

### Global popup registry

Registry: [ActivePopup](src/popups.rs#L383). Main rendering: [draw_ui](src/ui/mod.rs#L401), plus [ActivePopup::draw](src/popups.rs#L407).

| Variant | All visual modes covered | Design assessment |
| --- | --- | --- |
| `Template` | Filter input; results; empty results | Aligned two-pane stack; footer far too long. |
| `Theme` | Theme list; custom marker; general/graph background toggles; setup-owned version hides graph toggle | Aligned shell; three bordered regions consume height; ON/OFF semantics use success/destructive. Palette alternative differs. |
| `Info` | Note information; folder information; metrics/text/tags/spacers | Aligned banner, neutral content border; overfull unscrollable content is a major defect. |
| `Tag` | Single/batch editing; suggestions; all-tags list | Aligned shell/focus borders; variable suggestion height competes with list space. |
| `IconMode` | Nerd Font, Unicode, None, Emoji | Shared option list; consistent with its peers, unnecessarily tall for four choices. |
| `HintBarStyle` | All `HintBarStyle::ALL` choices | Same option list; palette adds previews, standalone does not. |
| `RemoveTags` | Cursor; marked rows; counts; empty list; nested confirmation | Aligned shell; selection/marking vocabulary differs from other lists. |
| `KeybindPreset` | Default, Helix, Vim, Emacs | Same option list; palette calls it Navigation Preset, standalone Keybind Preset. |
| `Sort` | Title ascending/descending; modified newest/oldest | Same option list; coherent with other selectors. |
| `Folder` | Create folder; rename folder | Aligned five-row text prompt. |
| `FolderPicker` | Move/copy note, folder, bulk notes, bulk folders, mixed selection | Aligned filter/results stack, focus borders and scrollbar. Operation title is generic MOVE/COPY. |
| `NoteRename` | File rename with target-path preview | Aligned prompt; preview is embedded in the bottom border and may truncate. |
| `VaultRename` | Vault display-name editing | Aligned prompt; related palette rename uses a different shell. |
| `CreateNote` | Markdown, plain text, drawing, canvas, Bases | Aligned prompts; Markdown/text include filename preview, others use a plain input. Palette alternatives for drawing/canvas/Bases differ. |
| `Import` | File, CSV/TSV, JSON, URL, clipboard title branches; new/append target | Large, muted-border input. Normal UI prompts for URL; file/CSV/JSON use native pickers and clipboard imports directly. Other branches exist but are not separate normal prompt flows. |
| `CreateFormat` | All five file-format choices | Same Medium option list; consistent selection style. |
| `Search` | Title/path search; folder/pinned/tag filters; grep groups; subnote results; empty/truncated results | Aligned shell and focus borders; syntax-heavy footer clips and omits action guidance. |
| `TrashView` | Populated list; empty list | Aligned list and scrollbar; no explicit empty-state text. |
| `Goals` | Daily word goal; daily note goal | Aligned short prompts; palette version is much larger. |
| `Subnotes` | Empty/populated list; title/content focus; hover | Aligned shell; fixed split and excessive hints make it unsuitable for narrow terminals. |

### Confirmation actions

All ten variants use [draw_confirm_popup](src/ui/popups.rs#L1297). Action text, consequence colors and initial selection come from [show_confirm](src/app/popups.rs#L371). Tag deletion and Remove Tags also construct this same popup through their own paths.

| Action | Presentation and critique |
| --- | --- |
| `DeleteNote` | Recoverable Trash action; non-destructive frame. |
| `DeleteFolder` | Recoverable Trash action; long folder messages can clip. |
| `DeleteTag` | Destructive Delete action; Enter/Esc when layered over Tag, y/n otherwise. |
| `DeleteTemplate` | Destructive Delete action with permanent-removal explanation. |
| `DeleteFromTrash` | Destructive Delete Forever action with irreversible warning. |
| `EmptyTrash` | Destructive Empty Trash action with item count and warning. |
| `BulkDeleteItems` | Recoverable Trash action with note/folder counts. |
| `QuitApp` | Non-destructive Quit action. |
| `RemoveVault` | Non-destructive Remove action; detail includes path and “Files on disk are not touched.” |
| `RemoveAllTagsFromSelected` | Destructive Remove All action; also used as nested confirmation in Remove Tags. |

The family is visually coherent. All share fixed 12-row sizing, non-wrapping message/detail paragraphs and potential long-label/message clipping. Setup exit, Calendar deletion and Bases typed confirmations are separate surfaces, not extra variants of this renderer.

### Command palette and continuations

All use the Large palette shell in [draw_ui](src/ui/mod.rs#L835). Workflow definitions: [Workflow](src/palette/workflows.rs#L12). Some tasks stay in the palette; other actions open their dedicated popup.

| Surface | Modes covered | Design assessment |
| --- | --- | --- |
| Root command palette | Tabs, search, two-row results, descriptions, usage ordering | Consistent internal layout; selection resembles hover; borders do not convey active focus. |
| `Theme` | Swatches, custom themes, current theme, background toggles, setup-owned mode | Useful preview; different anatomy from standalone Theme. |
| `Features` | Feature list and toggle state | Reuses palette; appropriate for browsing a larger list. |
| `Options` | `Sort`, `Icon`, `Hint`, `Preset`, `Layout` | Consistent within palette; current-value row and some previews; differs from standalone selectors. |
| `Goal` | `WordGoal`, `NoteGoal` | Same palette shell for a single numeric value; excess empty space. |
| `Create` | Drawing, canvas and Bases naming; shared state also carries file format; creation progress/error | Common Value input; larger than standalone prompt. Markdown/text normal creation uses dedicated prompts. |
| `Import` | URL value, new-note/append target, progress/error | Consistent palette input; other normal import sources bypass this prompt. |
| `Vault` | List, filtering, active/current value, adding/removing vaults | Coherent manager page; F4 Vaults uses a separate header-dropdown layout. |
| `VaultName` | Rename continuation | Same Value input; standalone rename is a compact prompt. |
| `Bases` | `Preparing`, `Running`, `Prompt`; choices or multiline values; errors | Consistent palette ownership; typed confirmations still need consequence styling. |

Bases commands reuse the same prompt/choice renderer rather than defining dozens of independent dialog designs. Covered configuration entries: rename view, layout, global/view filters, displayed properties, sort, grouping, manual group order, row limit/height, column widths, list markers/indent/separator, card/kanban widths, cover property/fit/aspect ratio, column summaries, formulas, custom summaries, property labels, add/remove view, new note and export. Group-order actions and source editing are also traced; commands that mutate immediately or open the source editor are not additional popup designs. See [configuration command catalog](src/bases/palette.rs#L14).

### Editor, headers, Calendar, setup and Bases

| Surface | Modes covered | Renderer and critique |
| --- | --- | --- |
| Editor error | Save failure; open failure | [edit_view.rs:425](src/ui/edit_view.rs#L425). Standalone Error border; inconsistent warning anatomy. |
| Editor link preview | Loading, document, error; embedded Bases content | [edit_view.rs:636](src/ui/edit_view.rs#L636). Shared banner/footer, borderless padded preview; appropriate content variant. |
| Editor Find | Results, no matches, match count | [quick_search.rs:142](src/ui/quick_search.rs#L142). Header bar, different selection color pair. |
| Go to Line | Placeholder; entered number | [edit_view.rs:463](src/ui/edit_view.rs#L463). Nested borders and custom hints. |
| Help search | Results/no matches | [help.rs:321](src/ui/help.rs#L321). Same Find renderer and color concern. |
| Graph search | Results/no matches | [graf_adapter.rs:1615](src/graf_adapter.rs#L1615). Same Find renderer; caller additionally truncates results to a separate width budget. |
| Bases search | Pending results; results/no matches; count | [bases/search.rs:32](src/bases/search.rs#L32). Same Find renderer; height-aware caller is better bounded. |
| Calendar `Notes` | Link/unlink chooser; open linked note | [calendar/view.rs:1605](src/calendar/view.rs#L1605). Same Find renderer with linked markers. |
| F2 keybind overlay | Every application view; multiple columns | [quick_keybinds.rs:55](src/ui/quick_keybinds.rs#L55). Accent header and alternating rows; right-anchored, passive reference surface. |
| F3 Messages | Warning, fatal, mixed, pinned, empty pinned header | [message_overlay.rs:15](src/ui/message_overlay.rs#L15). Severity-colored header, wrapped striped rows; sensible exception, inconsistent bounds/width handling. |
| F4 Vaults | Active vault, other vaults, add row, scrolling | [vault_switcher.rs:16](src/ui/vault_switcher.rs#L16). Same broad header family; solid accent rows, different selection fill and no local footer hints. |
| Calendar `Form` | New event; edit event series; focused field; validation error | [calendar/view.rs:1596](src/calendar/view.rs#L1596). Labeled form, separate title/footer/focus language. |
| Calendar `Path` | Import; export | Same Calendar renderer. Large editing area for a path compared with normal naming prompts. |
| Calendar `Delete` | Entire-series deletion | Same Calendar renderer. Destructive consequence lacks generic destructive frame/action treatment. |
| Calendar `Conflict` | Keep local; use remote; remote deletion | Same Calendar renderer. Comparison prose is appropriate; consequential choices are only footer text. |
| Calendar `Help` | Calendar-specific help text | Same Calendar renderer. Wrapped text without a scrolling viewport in this renderer. |
| Setup `PathInput` | Vault path; notice/error | [setup.rs:293](src/ui/setup.rs#L293). Shared Prompt banner/footer; instructions and input in a small borderless body. |
| Setup `ConfirmNonEmpty` | Nonempty-directory confirmation | Same setup vault modal. Wrapped explanation/path in four body rows; inherits generic confirm/cancel footer. |
| Setup exit confirmation | Save/exit; discard; back | [setup.rs:1058](src/ui/setup.rs#L1058). Shared confirmation frame but prose actions instead of buttons. |
| Bases `Prompt` | Property edit/clear; configuration; new note; export; move group; group ordering | [authoring.rs:124](src/bases/authoring.rs#L124). Shared Medium banner, plain text hints, borderless multiline input; palette alternative also covered. |
| Bases `Image` | Public image preview | [authoring.rs:99](src/bases/authoring.rs#L99). Shared Large banner, plain close hint; image content appropriately fills body. |
| Embedded Bases dialogs | Notes preview, editor preview, link preview, Canvas embeds | [embedding.rs:122](src/bases/embedding.rs#L122). Reuses Bases search/dialog renderers; no independent visual design. |

### Dependency-rendered surfaces

These are part of the application's visible UI even though Clin delegates rendering. Sources were read locally at the locked versions; the links provide corresponding package source references.

| Surface | Variants covered | Design assessment and ownership |
| --- | --- | --- |
| Drawing context menu | `Actions` for empty space/text/nontext targets; `Color` submenu | `draw-rs`, `src/ui/canvas_menu.rs:102`. Borderless themed rows with shortcuts/swatches; close to Graph/Canvas menus. Hover is painted even over the selected row. |
| Drawing shape selector | Rect, Ellipse, Diamond, Line, Arrow | `draw-rs`, `src/render.rs:80`, `src/ui/mod.rs:102`. Narrow top-centered borderless list; hints at view bottom, away from selector. |
| Drawing color selector | Entire fixed color palette | Same selector, `src/render.rs:107`. Full label gets swatch foreground, including selection. |
| Drawing text editor | Text edit textarea | `draw-rs`, `src/render.rs:136`. Centered width up to 60, height up to five; in-border EDIT TEXT title, bottom-of-view hints. |
| Graph context menu | No-node/node-selected and multiselection action sets | `graf-rs`, `src/render.rs:1630`. Borderless themed list; selection colors supplied by [Clin adapter](src/graf_adapter.rs#L178). Hover uses highlight pair rather than native muted hover background. |
| Graph looking glass | Selected node visual, link count, tags | `graf-rs`, `src/render.rs:1420`. Bordered contextual inspector; graph-specific styling is reasonable. Uses graph background/border tokens rather than native dialog chrome. |
| Graph config errors | Configuration problem list and suggested fixes | Host-owned [graf_adapter.rs:1648](src/graf_adapter.rs#L1648). Rounded in-border title, separate geometry and text styling. |
| Graph reload notification | Success/failure message | Host-owned [graf_adapter.rs:1709](src/graf_adapter.rs#L1709). Untitled centered three-row box, no common popup footer. |
| Canvas context menus | `Canvas`, `Editor`, `ColorPicker`, `ShapePicker`, `EdgeMenu`, `EdgeColorPicker`, `EdgeStylePicker`, `OrientationPicker` | `pinstar`, `src/menu.rs:19,290`. Same borderless menu family across all eight types; shared selected pair, hover skips selected row. |
| Canvas Rename | Rename node | `pinstar`, `src/render.rs:2051`, plus [host hints](src/pinstar_adapter.rs#L455). In-border mixed title/instructions; redundant host footer; percentage sizing differs from native Prompt. |
| Canvas floating editor | In-node text editing | `pinstar`, `src/render.rs:1710`. Accent border fitted to selected node; context anchoring is justified, but no shared prompt title/footer. |
| Canvas edge list | Selected node's incoming/outgoing edge rows | `pinstar`, `src/render.rs:1790`. Bottom-right bordered EDGES inspector with hover; content-derived dimensions are not capped to the canvas dimensions in this renderer. |

Dependency sources: [Draw render](https://docs.rs/crate/draw-rs/0.1.2/source/src/render.rs), [Draw UI](https://docs.rs/crate/draw-rs/0.1.2/source/src/ui/mod.rs), [Draw menus](https://docs.rs/crate/draw-rs/0.1.2/source/src/ui/canvas_menu.rs), [Graph render](https://docs.rs/crate/graf-rs/1.1.0/source/src/render.rs), [Canvas render](https://docs.rs/crate/pinstar/1.1.4/source/src/render.rs), [Canvas menus](https://docs.rs/crate/pinstar/1.1.4/source/src/menu.rs).

### Native dialogs, inactive helpers and boundaries

| Surface or definition | Disposition |
| --- | --- |
| OS directory picker | [pick_directory](src/ui/mod.rs#L1819): Zenity/KDialog, macOS chooser or Windows folder browser, with setup text-input fallback. Native appearance is intentional; cross-platform screenshots were not checked. |
| OS file picker | [pick_file](src/ui/mod.rs#L1883): file/CSV/JSON imports, image insertion and other file-selection callers. Native appearance is intentional; not an inconsistency to solve with TUI styling. |
| `draw_header_dropdown` | [Definition](src/ui/popups.rs#L1622) has no caller under `src/`. Accent header, darkened accent selected/hovered rows, content-sized unbounded dropdown. It is not the renderer of the current drawing selectors; do not count it as an additional active popup. |
| Upstream Graph help/search/config UI | The dependency's standalone `ui.rs` contains these renderers. Clin's adapter uses its own graph UI plus shared Help/Find, so those standalone widgets are not extra normal Clin popups. |
| Upstream Canvas help overlay | `pinstar::render` contains a tabbed 80%×85% Help overlay when `show_help` is set. Clin's normal Help action routes to [the application Canvas help tab](src/pinstar_adapter.rs#L420). The library renderer exists, but its standalone help layout is not rated as a normal host help flow. |
| Frontmatter property editing | [editor/frontmatter.rs:392](src/editor/frontmatter.rs#L392). Inline pane, not a floating popup. Checked to avoid missing a field editor; excluded from popup consistency ratings. |
| Outline content pane | [outline/render.rs:215](src/outline/render.rs#L215). Permanent split pane, not a popup. |
| Graph legend/minimap; selection rectangles; preview image surfaces; status/mode hints | Contextual visualizations or ordinary view chrome, not dialogs. Their drawing/clear calls were checked during inventory so they were not mistaken for missing modal surfaces. |

## Sizing evidence

These dimensions are calculated directly from `centered_rect`, not measured screenshots. Width and height include the footer where the renderer reserves one; the detached banner sits one row above.

| Terminal | Small | Medium | Large | Prompt | Confirm |
| --- | --- | --- | --- | --- | --- |
| 80×24 | 32×9 | 40×12 | 48×14 | 40×5 | 40×12 |
| 120×40 | 48×16 | 60×20 | 72×24 | 60×5 | 60×12 |
| 40×12 | 30×5 | 30×6 | 30×7 | 30×5 | 30×12 |

The shared placement function handles frame bounds better than several bespoke overlays, but percentage sizing does not establish a usable content minimum. On a short terminal, a full-height Confirm also has no row available above it for the detached banner, which the banner renderer deliberately skips at `y == 0`.

## What should stay

- Shared theme tokens and adapter mappings already provide a good foundation; replacing them would add complexity without fixing the layout defects.
- Shared banner display-width handling, border-integrated scrollbars and default Cancel selection are worth keeping.
- Distinct shells for centered dialogs, header search/reference overlays, contextual menus and contextual inspectors are reasonable. Standardize their shared states and sizing rules, rather than forcing every surface into one box.
- Native file and directory pickers should retain platform behavior.

## Recommended repair order and proof

1. Make content reachable: Info viewport, responsive Subnotes split, frame-bounded dropdowns and visible essential hints.
2. Standardize selection/focus/hover colors and consequential-action styling, including Calendar and Bases prompts.
3. Bring Calendar, Go to Line, Canvas Rename and error dialogs onto the existing title/footer conventions while preserving their content layouts and owners.
4. Consolidate display-width measurements and remove duplicated or contradictory hints. Normalize empty states afterward.

For implementation, use existing `ratatui::TestBackend` patterns to render each shared family at 80×24, 120×40 and a narrow/short size, including long Unicode titles/paths, empty/populated/overflowing lists, pointer hover, keyboard focus, destructive actions and inline errors. Check every variant in the inventory for routing and labels; renderer-family snapshots can share coverage. Add real terminal checks for light/dark/transparent themes and custom keybindings before calling the visual audit complete.

Existing checks inspected, but not executed: [confirmation hover/click geometry](src/ui/design_tests.rs#L714), [popup scrollbar borders and mouse targets](src/ui/design_tests.rs#L1220), [Unicode banner background preservation](src/ui/popups.rs#L1721), and Bases embedded-dialog workflow tests. These cover useful pieces, not all popup designs.

No build, automated tests, interactive popup walkthrough or cross-platform visual checks were run for this documentation-only audit. The verified results are source coverage, renderer relationships, literal hint widths and calculated layout budgets; the remaining visual risks need rendered acceptance evidence.
