# Ratatui migration plan

**Session ID:** ses_200cb0364ffehzsIa6IzY2k1P3
**Created:** 5/7/2026, 12:32:35 AM
**Updated:** 5/7/2026, 12:54:10 AM

---

## User

Respond terse like smart caveman. All technical substance stay. Only fluff die.

Default: **full**. Switch: `/caveman lite|full|ultra`.

## Rules

Drop: articles (a/an/the), filler (just/really/basically/actually/simply), pleasantries (sure/certainly/of course/happy to), hedging. Fragments OK. Short synonyms (big not extensive, fix not "implement a solution for"). Technical terms exact. Code blocks unchanged. Errors quoted exact.

Pattern: `[thing] [action] [reason]. [next step].`

Not: "Sure! I'd be happy to help you with that. The issue you're experiencing is likely caused by..."
Yes: "Bug in auth middleware. Token expiry check use `<` not `<=`. Fix:"

## Intensity

| Level | What change |
|-------|------------|
| **lite** | No filler/hedging. Keep articles + full sentences. Professional but tight |
| **full** | Drop articles, fragments OK, short synonyms. Classic caveman |
| **ultra** | Abbreviate (DB/auth/config/req/res/fn/impl), strip conjunctions, arrows for causality (X → Y), one word when one word enough |
| **wenyan-lite** | Semi-classical. Drop filler/hedging but keep grammar structure, classical register |
| **wenyan-full** | Maximum classical terseness. Fully 文言文. 80-90% character reduction. Classical sentence patterns, verbs precede objects, subjects often omitted, classical particles (之/乃/為/其) |
| **wenyan-ultra** | Extreme abbreviation while keeping classical Chinese feel. Maximum compression, ultra terse |

Example — "Why React component re-render?"
- lite: "Your component re-renders because you create a new object reference each render. Wrap it in `useMemo`."
- full: "New object ref each render. Inline object prop = new ref = re-render. Wrap in `useMemo`."
- ultra: "Inline obj prop → new ref → re-render. `useMemo`."
- wenyan-lite: "組件頻重繪，以每繪新生對象參照故。以 useMemo 包之。"
- wenyan-full: "物出新參照，致重繪。useMemo .Wrap之。"
- wenyan-ultra: "新參照→重繪。useMemo Wrap。"

Example — "Explain database connection pooling."
- lite: "Connection pooling reuses open connections instead of creating new ones per request. Avoids repeated handshake overhead."
- full: "Pool reuse open DB connections. No new connection per request. Skip handshake overhead."
- ultra: "Pool = reuse DB conn. Skip handshake → fast under load."
- wenyan-full: "池reuse open connection。不每req新開。skip handshake overhead。"
- wenyan-ultra: "池reuse conn。skip handshake → fast。"

## Auto-Clarity

Drop caveman for: security warnings, irreversible action confirmations, multi-step sequences where fragment order risks misread, user confused. Resume caveman after clear part done.

Example — destructive op:
> **Warning:** This will permanently delete all rows in the `users` table and cannot be undone.
> ```sql
> DROP TABLE users;
> ```
> Caveman resume. Verify backup exist first.

## Boundaries

Code/commits/PRs: write normal. "stop caveman" or "normal mode": revert. Level persist until changed or session end.

ultra create a plan to migrate to newer version of ratatui

---

## Assistant

**Tool: bash**

**Tool: bash**

**Tool: glob**

---

## Assistant

**Tool: task**

**Tool: task**

---

## Assistant

**Tool: read**

**Tool: read**

**Tool: read**

---

## Assistant

**Tool: bash**

**Tool: grep**

---

## Assistant

**Tool: grep**

**Tool: read**

---

## Assistant

Plan ready. Here migration plan `ratatui 0.29 → 0.30`:

---

## Migration Plan: ratatui 0.29 → 0.30

**Current**: `ratatui = "0.29"`, `crossterm = "0.28"`, Rust 1.94.1 (edition 2024). MSRV 1.86 requirement met.

### Phase 1: Cargo.toml bump
- `ratatui = "0.29"` → `ratatui = "0.30"`
- Verify `tui-textarea = "0.7"` compatible w/ ratatui 0.30 (may need → `0.9`)
- Verify `tui-term = "0.2"` compatible w/ ratatui 0.30
- `cargo check` after bump → collect compile errors

### Phase 2: API fixes (ordered by impact)

| # | Change | Files affected | Action |
|---|--------|---------------|--------|
| 1 | `Cell::set_symbol()` → `Cell::set_char()` | `src/markdown.rs:273`, `src/graph/render.rs:916,922,931,940,965,971,982,988,1004` | Rename all calls |
| 2 | `Marker` → `#[non_exhaustive]` | `src/graf/config.rs:175-182` | Add `_ => Marker::Dot` wildcard arm in `From` impl |
| 3 | `Backend::Error` assoc type | `src/main.rs` (Terminal type), `src/canvas/app.rs`, `src/graf/app.rs` | Fn sigs returning `Result<_, io::Error>` → add `<B as Backend>::Error` or pin `CrosstermBackend<Stdout>` |
| 4 | `Block::title()` accepts `Into<Line>` | `src/graph/render.rs:575` (`title_style()`) | Verify title construction still works; `.title_style()` may be renamed |
| 5 | `List::highlight_symbol` accepts `Into<Line>` | `src/ui.rs` (highlight_style calls) | Remove `.into()` if present; string → `Line` auto-convert |
| 6 | `Style` const fn methods | `src/markdown.rs:275` (`Style::reset()`) | Verify `Style::reset()` still works — now static method |
| 7 | Custom `Widget` impl | `src/markdown.rs:249-305` (`ScrollablePseudoTerminal`) | Verify `Widget` trait signature unchanged for `fn render(self, area, buf)` — 0.30 keeps this |
| 8 | `Shape` trait for Canvas | `src/graph/render.rs:47-81,194+` | Verify `Shape::draw(&self, painter)` signature unchanged |

### Phase 3: Glob import audit
- `src/ui.rs:6` — `use ratatui::{prelude::*, widgets::*};` → compile will reveal renamed types
- `src/events.rs:5` — `use ratatui::prelude::*;` → same

### Phase 4: Dependent crate bumps
- `tui-textarea` 0.7 → likely needs 0.9.x for ratatui 0.30
- `tui-term` 0.2 → check compatibility
- Run `cargo update` + `cargo check` iteratively

### Phase 5: Build & test
1. `cargo check` — fix remaining compile errors
2. `cargo build` — full build
3. `cargo clippy` — lint pass
4. Manual test: run app, verify UI renders correctly (list view, edit view, canvas, graph, popups)

### Risk: Low → Medium
- No `Block::Title` usage (domain "Title" only)
- No `Flex::SpaceAround` usage
- No `Stylize` import for `Style`
- No `symbols::braille::BLANK/DOTS`
- Main risk: `tui-textarea`/`tui-term` compat + `Cell::set_symbol` rename

**Tool: question**

---

## Assistant

**Tool: task**

---

## Assistant

## Updated Plan with Compat Findings

**Blocker found: `tui-textarea` incompatible.**

### Crate Compat Matrix

| Crate | Current | Needs | Status |
|-------|---------|-------|--------|
| `ratatui` | 0.29 | 0.30 | Core migration |
| `crossterm` | 0.28 | 0.28 or 0.29 | OK, no change needed |
| `tui-term` | 0.2 | **0.3.4** | **Breaking** — `Widget` impl changed from `PseudoTerminal` to `&PseudoTerminal` (pass ref) |
| `tui-textarea` | 0.7 | **???** | **BLOCKED** — no released version supports ratatui 0.30. Repo inactive since Oct 2024. PR #117 only bumps crossterm, not ratatui |

### Options for tui-textarea blocker

1. **Fork tui-textarea** — update its `Cargo.toml` to use `ratatui-core`/`ratatui-widgets` + fix breaking changes. Use as git/path dep.
2. **Use git dep with PR #117** — but PR only updates crossterm, not ratatui 0.30. Insufficient.
3. **Delay migration** — wait for upstream tui-textarea to release 0.8 with ratatui 0.30 support.
4. **Replace tui-textarea** — swap for alternative (e.g., `ratatui-textarea 0.9.1` which appeared in `cargo search`).

Wait — `ratatui-textarea` 0.9.1 is a **different crate** (separate from `tui-textarea`). It may already support ratatui 0.30. Worth checking.

### Recommendation

1. Check if `ratatui-textarea` 0.9.1 supports ratatui 0.30 and is API-compatible
2. Bump `tui-term` → 0.3.4 (fix render ref change)
3. Apply ratatui 0.30 migration (Cell::set_symbol, Marker non_exhaustive, etc.)

Want me to research `ratatui-textarea` as a replacement, or take a different approach?

---

## User

make the research and then list the benefits of changing to ratatui 0.30

---

## Assistant

**Tool: task**

**Tool: task**

---

## Assistant

## Research Results

### ratatui-textarea = Drop-in replacement
- Official **ratatui org fork** of tui-textarea
- API **identical** — `TextArea`, `CursorMove`, `.input()`, `.lines()`, `.into_lines()`, `.set_block()`, `.set_style()`, all present
- Requires ratatui 0.30 (depends on `ratatui-core ^0.1`, `ratatui-widgets ^0.3`)
- Migration: swap `tui-textarea` → `ratatui-textarea = "0.9"`, rename imports

### tui-term = Compatible at 0.3.4
- Already supports ratatui 0.30
- Breaking: `Widget` impl moved from `PseudoTerminal` to `&PseudoTerminal` — pass ref

---

## Benefits of ratatui 0.30

### Ergonomics (less boilerplate)
| Feature | What it does |
|---------|-------------|
| `Rect::layout(&layout)` | Destructure splits: `let [top, bot] = area.layout(&layout);` — kills `Layout::default()...split(area)[0]` pattern |
| `Rect::centered()` | Center popups in one call — project has 10+ popup rect calcs |
| `Rect::outer()` | Expand rect by margin |
| `Style` const fn | `const MY_STYLE: Style = Style::new().blue();` — define theme styles at compile time |
| `border!` macro | No `Borders` import needed |
| `Color::from([r,g,b])` | Tuple/array RGB — cleaner than `Color::Rgb(r,g,b)` |

### Bug fixes hitting this project
- **Paragraph panic** on out-of-bounds render
- **Block title truncation** — left-aligned titles truncate right side now
- **Emoji clearing** — VS16 wide emoji overlay bug fixed
- **Canvas line clipping** — lines starting outside bounds now partially drawn (your graph edges affected)
- **Canvas coordinate rounding** — was rounding toward zero, caused off-by-one (your graph/canvas affected)
- **`padding_right()` was setting left padding** — direct bug fix
- **Paragraph wrapping** — extra line break on whitespace-only lines

### New Canvas markers
- `Marker::Quadrant` (2x2), `Sextant` (2x3), `Octant` (2x4) — alternatives to Braille with **no visible bands**. Your graf config exposes marker choice to users — they'd get 3 new options

### Architecture
- Crate split (`ratatui-core`, `ratatui-widgets`) — faster compile when only core types change
- `Cell::symbol` → `Option<CompactString>` — better memory for sparse screens
- Border merging — overlapping borders auto-join cleanly at intersections

### Dependency health
- `tui-textarea` abandoned since Oct 2024 — `ratatui-textarea` is actively maintained by ratatui team
- Stay on ratatui release train — future 0.31+ updates will be incremental instead of a massive jump later

---

### Migration cost summary
| Item | Effort |
|------|--------|
| `Cell::set_symbol()` → `set_char()` | 10 renames |
| `Marker` non_exhaustive wildcard | 1 match arm |
| `tui-textarea` → `ratatui-textarea` | 1 dep swap, import renames |
| `tui-term` 0.2 → 0.3.4 | render ref change |
| Glob import audit | compile errors guide fixes |
| `Backend::Error` type | fn signatures |
| Testing | manual verification of all views |

---

## User

double check everything and create a fully detailed plan to change to ratatui 0.30

---

## Assistant

Let me verify every claim against the actual source code.

**Tool: grep**

**Tool: grep**

**Tool: grep**

---

## Assistant

**Tool: read**

**Tool: read**

**Tool: read**

---

## Assistant

**Tool: read**

**Tool: read**

**Tool: read**

---

## Assistant

**Tool: grep**

**Tool: grep**

**Tool: grep**

---

## Assistant

**Tool: grep**

**Tool: grep**

**Tool: read**

---

## Assistant

**Tool: read**

**Tool: read**

---

## Assistant

**Tool: task**

**Tool: task**

---

## Assistant

Critical corrections found. Let me verify the remaining items.

**Tool: grep**

**Tool: grep**

**Tool: grep**

---

## Assistant

**Tool: grep**

**Tool: read**

**Tool: read**

---

## Assistant

**Tool: task**

**Tool: task**

---

## Assistant

**Tool: grep**

**Tool: grep**

---

## Assistant

**Tool: task**

---

## Assistant

**Tool: grep**

**Tool: grep**

---

## Assistant

All verification complete. Here's the **corrected**, fully detailed plan.

---

## Migration Plan: ratatui 0.29 → 0.30 (Verified)

### Verified: Most 0.30 "breaking changes" don't affect this project

| Claimed breaking | Reality | Action |
|---|---|---|
| `Cell::set_symbol` renamed | **False.** `set_char` is NEW, `set_symbol` unchanged | None |
| `Block::title_style` removed | **False.** Unchanged | None |
| `Widget::render(self,...)` changed to `&self` | **False.** Still `self` | None |
| `Shape::draw` changed | **False.** Unchanged | None |
| `Style::reset` changed | **False.** Unchanged | None |
| `Canvas::background_color` changed | **False.** Unchanged | None |
| `Backend::Error` breaks `CrosstermBackend<Stdout>` | **False.** `Error = io::Error`, unchanged | None |
| `terminal.show_cursor()` changed | **False.** Unchanged | None |
| `terminal.backend_mut()` changed | **False.** Unchanged | None |
| `Block::title("str")` breaks | **False.** `Into<Line>` accepts `&str` | None |
| `highlight_symbol("str")` breaks | **False.** `Into<Line>` accepts `&str` | None |
| `Alignment` renamed breaks code | **False.** Type alias exists | None |
| `portable-pty` conflicts with tui-term | **False.** Optional dep, not enabled | None |
| `vt100::contents()` returns `&str` not `String` | **True**, but `&screen_cell.contents()` auto-derefs `&&str` → `&str` | None needed |

---

### REQUIRED CHANGES (4 items)

#### 1. Cargo.toml — 3 dependency bumps

```toml
# Before
ratatui = "0.29"
tui-term = "0.2"
tui-textarea = { version = "0.7", features = ["crossterm"] }

# After
ratatui = "0.30"
tui-term = "0.3"
ratatui-textarea = "0.9"               # official ratatui org fork, crossterm is default feature
```

#### 2. Import renames — 6 files

`tui_textarea` → `ratatui_textarea` everywhere:

| File | Line | Change |
|---|---|---|
| `src/canvas/input.rs` | 5 | `use tui_textarea::TextArea` → `use ratatui_textarea::TextArea` |
| `src/palette.rs` | 4 | `use tui_textarea::TextArea` → `use ratatui_textarea::TextArea` |
| `src/events.rs` | 6 | `use tui_textarea::*` → `use ratatui_textarea::*` |
| `src/ui.rs` | 12 | `use tui_textarea::*` → `use ratatui_textarea::*` |
| `src/app.rs` | 24 | `use tui_textarea::TextArea` → `use ratatui_textarea::TextArea` |
| `src/canvas/app.rs` | 16 | `use tui_textarea::TextArea` → `use ratatui_textarea::TextArea` |

Also check test files:
| `test_scroll.rs` | 1 | `use tui_textarea::TextArea` → `use ratatui_textarea::TextArea` |
| `parse_debug_textarea.rs` | 1 | `use tui_textarea::{TextArea, CursorMove}` → `use ratatui_textarea::{TextArea, CursorMove}` |

#### 3. Marker `#[non_exhaustive]` — 1 file

`src/graf/config.rs:175-182` — add wildcard arm:

```rust
impl From<CanvasMarker> for ratatui::symbols::Marker {
    fn from(m: CanvasMarker) -> Self {
        match m {
            CanvasMarker::Braille => ratatui::symbols::Marker::Braille,
            CanvasMarker::HalfBlock => ratatui::symbols::Marker::HalfBlock,
            CanvasMarker::Dot => ratatui::symbols::Marker::Dot,
        }
    }
}
```
→
```rust
impl From<CanvasMarker> for ratatui::symbols::Marker {
    fn from(m: CanvasMarker) -> Self {
        match m {
            CanvasMarker::Braille => ratatui::symbols::Marker::Braille,
            CanvasMarker::HalfBlock => ratatui::symbols::Marker::HalfBlock,
            CanvasMarker::Dot => ratatui::symbols::Marker::Dot,
            // 0.30 added: Quadrant, Sextant, Octant — not mapped yet
        }
    }
}
```

Actually — since `CanvasMarker` is a non-exhaustive-like enum (you control it), the `match` on YOUR enum doesn't need a wildcard. The `#[non_exhaustive]` on `ratatui::symbols::Marker` means you can't match exhaustively on it. **This match IS exhaustive on `CanvasMarker` and returns `Marker` values — no wildcard needed on the return side.** The compiler won't complain because you're producing `Marker` values, not matching on `Marker` variants.

Wait — let me reconsider. The `match` is on `CanvasMarker` (your enum), and each arm returns a specific `Marker` variant. The `#[non_exhaustive]` on `Marker` means you can't do `match marker_value { Braille => ..., HalfBlock => ..., Dot => ... }` without a `_` arm. But here you're not matching on `Marker` — you're constructing `Marker` values. **No change needed.** The `#[non_exhaustive]` only affects pattern matching ON `Marker`, not construction.

**Correction: No code change needed for Marker.**

But — if you later want to support new markers (Quadrant, Sextant, Octant), you'd add them to `CanvasMarker` and the `From` impl. Optional enhancement.

#### 4. Compile and fix glob import issues

After the above changes, run `cargo check`. The glob imports at:
- `src/ui.rs:6` — `use ratatui::{prelude::*, widgets::*};`
- `src/events.rs:5` — `use ratatui::prelude::*;`

...should resolve identically since `ratatui` 0.30 re-exports everything from the sub-crates. If any name moved, compiler errors will show exactly what.

---

### FINAL TALLY

| Category | Count |
|---|---|
| Cargo.toml edits | 3 lines |
| Import renames | 8 files, 8 lines |
| Code changes | **0** |
| Total manual edits | ~11 lines |

### Execution order
1. Edit `Cargo.toml` — bump 3 deps
2. Rename imports — 8 files, `tui_textarea` → `ratatui_textarea`
3. `cargo check` — fix any remaining errors
4. `cargo build` — full build
5. `cargo clippy` — lint
6. Manual test — all views (list, edit, canvas, graph, popups, markdown preview)

### Post-migration optional enhancements
- Add `Quadrant`/`Sextant`/`Octant` to `CanvasMarker` enum in `src/graf/config.rs`
- Use `Rect::centered()` for popup positioning (10+ sites in `ui.rs`)
- Use `Rect::layout()` for cleaner layout destructuring
- Use `const` styles for theme colors

Ready to execute when you give the go-ahead.

---

