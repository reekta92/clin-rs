# Obsidian Bases in clin-rs

clin-rs supports Obsidian's **Bases** feature as native table and list views. A Base is a `.base` YAML file in your vault that defines filtered, sorted, and grouped database-like views over note frontmatter and file metadata.

## Compatibility and Syntax

The `.base` files must follow the Obsidian Bases specification.

### Top-Level Fields

A `.base` file contains the following sections:
- `filters`: An expression tree (`and`, `or`, `not`, or raw expression string) used to filter matching files globally.
- `formulas`: A map of named computed properties. E.g. `total: price * qty`.
- `properties`: A map of key-to-display-configuration. E.g., setting custom display names for table columns.
- `summaries`: A map of custom summary formulas.
- `views`: A list of view definitions.

### Views Configuration

Each view under `views` can specify:
    - `type`: View type. `table`, `list`, `cards`, and `map` are all supported.
- `limit`: Max rows to display.
- `filters`: View-specific filter tree.
- `groupBy`: Object containing `property` and `direction` (`ASC` or `DESC`).
- `order`: A list of property keys determining column order.
- `summaries`: A map of column-key to summary-type (e.g. `Average`, `Sum`).

### Expression Language

The expression engine supports:
- Operators: `+ - * / %`, `== != > < >= <=`, `! && ||`, parenthesis `( )`, member access `.`, and method calls.
- Types: String, Number, Boolean, Date, List, Object.
- Date Arithmetic: E.g., `date + "1 day"` or `now() - "2 weeks"`. Case-sensitive short units (`y M w d h m s`) and case-insensitive long units (`year/month/week/day/hour/minute/second`) are supported.
- Functions:
  - Global: `if(cond, then, else)`, `now()`, `today()`, `date(s)`, `link(target)`, `max(a, b)`, `min(a, b)`.
  - String: `contains(hay, needle)`, `replace(s, from, to)`, `split(s, sep)`, `lower(s)`, `title(s)`, `length(s)`.
  - Number: `round(n)`, `ceil(n)`, `floor(n)`, `abs(n)`, `toFixed(n, digits)`.
  - Date: `format(d, fmt)`, `relative(d)`, `time(d)`.
  - List: `sort(l)`, `join(l, sep)`, `unique(l)`, `mean(l)`, `length(l)`, `filter(l, pred)`, `map(l, expr)`.

### Column Summaries

The following built-in column summaries are supported:
- `Average`, `Sum`, `Min`, `Max`, `Range`, `Median`, `Stddev` (Numbers)
- `Earliest`, `Latest` (Dates)
- `Checked`, `Unchecked` (Booleans)
- `Empty`, `Filled`, `Unique` (Any)
- Custom summaries: Evaluates a custom summary expression mapped under the top-level `summaries` section (with the column values bound to `values`).

## View layouts

Four layout types are fully implemented:
- **Table** (default): Columnar grid with sortable columns, horizontal scrolling, summary rows. Supports cell editing inline.
- **List**: One `ListItem` per row. The primary column is shown with a marker (`• value`) followed by indented `propertyName: value` lines for each additional column. Press `m` to cycle the marker style: bullet → numbered (`1.`, `2.`, …) → none (indented only) → bullet. Group labels are rendered as `── label ──` separators. Horizontal navigation (MoveLeft/MoveRight/EditCell/SortAsc/SortDesc) is a no-op in List view — only row-level operations (MoveUp/MoveDown/Open/CycleView) are active.
- **Cards**: Text-tile grid. Each tile shows the primary value as a bordered title, up to 4 property `key: value` lines as body. Fixed tile metrics: 28 columns × 7 rows. Click a tile to select it; `j`/`k` page the grid. If a note has a `color` or `cover` property set to a hex color or CSS name, the card renders a colored banner strip at the top of the tile matching that color.
- **Map**: ASCII lat/long scatter plot. Reads the `coordinates` property (`"lat, lon"` string or `[lat, lon]` list); also checks `coords` and `location` fallback keys. Notes without a parseable coordinate are skipped (no marker). Each pin can show a custom color and icon: set `marker_color` or `color` (hex or CSS name) for the pin color, and `marker_icon` or `icon` to a Lucide icon name (supported: `map-pin`/`pin`, `star`, `heart`, `flag`, `bookmark`, `home`, `circle`, `square`, `diamond`) for the glyph. Keyboard-only navigation — mouse is not supported in Map view.
## Editing bases in-app

Press `E` to open a full-screen raw YAML editor showing the current `.base` file content. This is a `TextArea` covering the entire terminal — not a small popup.

- `Ctrl+S` — attempt to parse and save the YAML. On success the base is re-evaluated and the view re-renders. On parse error the editor stays open and a status message shows the error.
- `Esc` — discard changes and close the editor without saving.
- Standard text-editing shortcuts (copy, paste, select-all, undo, redo) work inside the editor.

## Export

Press `x` to export the current view as CSV. The file is written as `{base_stem}.csv` in the same folder as the `.base` file. All matched rows are exported (not just the visible window). A status message shows the export path.

Press `y` to copy the current view as tab-separated values (TSV) to the system clipboard. A status message shows the row count. The clipboard format is suitable for pasting into spreadsheets.

## New note from base

Press `N` (shift-N) to create a new blank note in the same folder as the `.base` file. The note starts with the title "Untitled note" and opens in the editor. After saving, the base view is restored and refreshed — the new note appears if it matches the base's active filter. Notes created at the vault root (`.base` file in root) are created there too.

## Current Limitations

- **In-Memory Sort**: Sorting by clicking columns (SortAsc/SortDesc) is in-memory only and is lost when exiting the base view. Persistent sorting is stored in Obsidian's private cache and is not supported in Phase 1.
- **Simplified List Closures**: List functions like `filter` and `map` support path and literal predicates referencing `this` (e.g. `filter(tags, this == "work")` or `map(items, this.price)`), but do not support arbitrary multi-argument lambda expressions.
- **Embedded Bases**: Embedding a base view inside a markdown note is a Phase 2 goal.

## Keybindings (Default Mode)

- `k` / `Up`: Move cursor up
- `j` / `Down`: Move cursor down
- `h` / `Left`: Move cursor left (table only; no-op in list)
- `l` / `Right`: Move cursor right (table only; no-op in list)
- `o` / `Enter`: Open selected note
- `e`: Edit selected cell inline (table only; no-op in list)
- `E`: Edit base raw YAML (full-screen editor)
- `Ctrl+S`: Save base (inside raw YAML editor)
- `N` (shift-N): New note in base folder
- `x`: Export view as CSV
- `y`: Copy view as TSV to clipboard
- `Tab`: Cycle view layout configurations
- `s`: Sort selected column ascending (in-memory)
- `S`: Sort selected column descending (in-memory)
- `r`: Refresh / reload files
- `q` / `Esc`: Back to previous view
