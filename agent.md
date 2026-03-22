# clin-rs-vim-reimplement

## Project Overview
This is a Rust-based project, seemingly a reimplementation or rework of a tool named `clin`, heavily associated with Vim workflows.

## AI Agent Instructions
Future AI agents should use this file to understand the project architecture and the history of actions taken.
All significant workflow steps, decisions, and structural changes should be documented here.

## Actions Taken
- **Initial Setup (March 22, 2026):** 
  - Created `.gitignore` to prevent committing unnecessary files (e.g., `/target` directory, swap files).
  - Created this `agent.md` file to act as a living ledger of actions and project context for AI agents.

- **Vim Mode Overhaul (March 22, 2026):**
  - **Issue Identified:** The project used `vim-line 4.0.3` which is a line-oriented vim parser that does not handle multiline natively. `tui-textarea` elements were flattened to strings, processed by `vim-line`, and entirely recreated on every keystroke. This destroyed scroll state, performance on large texts, layout states, and undo history natively managed by `tui-textarea`.
  - **Solution Implemented:** Removed the `vim-line` dependency. We implemented a native, lightweight, state-machine driven Vim adapter (`src/vim.rs`) wrapper over `tui-textarea` translating standard Vim motions and operators (including inner objects, `ciw`, `di[`, etc.) natively into `tui-textarea` editing commands (`cursor_move`, `start_selection`, `cut`, `copy`, etc.).
  - **Benefits:** Retained lightweight behavior without taking on massive text editor library dependencies like `edtui`. Completely fixed cursor jumping, multiline destruction, and allowed native `undo`/`redo` (u/Ctrl+r) functionality inside `tui-textarea` to seamlessly map to vim commands.
  - **New Architecture:** 
    - `src/vim.rs` has the `Vim` struct that holds current state (`Normal`, `Insert`, `OperatorPending`, `Visual`, etc.).
    - `src/helpers.rs` handles internal index lookups for specific vim features like inner objects on the text buffer.
- **Bug Fix (March 22, 2026):**
  - **Issue:** Pressing Escape while in Visual or Visual Line modes inside the Vim adapter inadvertently saved the file and exited to the Notes view, rather than returning to Vim Normal mode.
  - **Fix:** In `src/main.rs`, the `handle_edit_keys` event handler originally let the Escape key bypass Vim interception unless it was in `Insert` mode. Changed the check to trap `Esc` locally when `mode != VimMode::Normal`, delegating back to `vim.transition()` to correctly clear Visual mode state, remove selection, and return to `Normal` mode.
- **Vim Return Status & Commands**: `src/vim.rs` has been refactored to change the return type of `transition` from `bool` to a new `VimOutput` enum (`Handled`, `Unhandled`, `Command(String)`). The `VimMode::Command` state has been fully implemented.
- **Escape Key Intercept Context**: `src/main.rs` `handle_edit_keys` was refactored so that when `app.vim_enabled` is true, the `Escape` key purely transitions vim states (or goes back) and *does not* quit or save the editor.
- **Vim Quit Actions**: `:wq` inherently saves and quits. `:w` saves. `:q` or `:q!` simply quits out of the modal edit screen without saving. All other escapes require explicit vim commands.
- **Dynamic Help Text for Vim Mode**: Updated `src/main.rs` to dynamically switch the status bar help hints. When `app.vim_enabled` is true, the `Esc autosave...` text changes to show `:wq save+quit   :q quit   :w save   Tab change focus` instead via a new `VIM_EDIT_HELP_HINTS` constant.
- **Normal Mode Input Sinking**: Changed `VimOutput::Unhandled` to `VimOutput::Handled` for default patterns in `Normal`, `Visual`, and `VisualLine` modes so that unmapped arbitrary typing doesn't leak into the base `tui-textarea` to type characters while outside of `Insert` mode. Also explicitly forwarded `PageUp`, `PageDown`, `Home`, and `End` properly in the vim motion blocks.
- **Vim Vertical Navigation**: Added support for `gg` (jump to start of file) and `G` (jump to end of file). Also implemented prefix multipliers tracking (`count_buffer`), enabling users to jump to specific lines using `<LINE_NUMBER>gg` or `<LINE_NUMBER>G` (e.g., `5gg` or `12G`) across `Normal`, `Visual`, and `VisualLine` modes.
- **Vim Vertical Navigation Bugfix**: Fixed an issue where using `gg` or `G` (or their numbered variants) in `Normal` mode was unintentionally maintaining anchor states from standard visual modes and drawing selection highlights. Forced `textarea.cancel_selection()` immediately before execution.
- **Vim `<NUMBER>g` Update**: Modified the `g` motion logic so that `<NUMBER>g` executes a line jump immediately, rather than waiting for `<NUMBER>gg`. Empty `g` inputs still wait for the second `g` appropriately (`pending_g`).
- **Cleanup and Dev Scripts (March 22, 2026)**:
  - Created a `dev_scripts/` folder at the root.
  - Moved all unstructured or temporary build/test files (`*.py`, `temp_vim_src.rs`, `test_vim` artifacts) into this folder.
  - **New Rule**: Future AI agents or contributors should use the `dev_scripts/` folder for any scratch files, experimental scripts, python manipulators, test bins, or temporary logs rather than littering the project root.
  - This folder (`/dev_scripts/`) has been explicitly added to `.gitignore`.
- **Continuous Autosave (March 22, 2026)**:
  - Addressed user feedback to strip explicit manual saving mechanisms.
  - Rewrote saving infrastructure so the note writes to disk continually on every TUI edit event frame in `run_app`.
  - Replaced explicit `app.save_current()` calls bounded to "Esc"/"Ctrl+Q" with passive `app.autosave()` behavior.
  - Wrapped `run_tui_session` in `std::panic::catch_unwind` with an `AssertUnwindSafe`/pointer combination to intercept panics and force-save the text buffer directly before exiting upon a crash.
  - Updated all bottom bar help prompts, README entries, and Vim mode handlers to reflect that `:q`, `:wq`, `Ctrl+Q` and `Esc` just quit or exit the view while relying on continuous autosave backend infrastructure.
- **Code Quality & Linting Improvements (March 22, 2026)**:
  - Performed a comprehensive static analysis pass using `cargo clippy --all-targets --all-features`.
  - Fixed multiple collapsible `if` statement expressions.
  - Eliminated `.unwrap()` panics in visual visual mode highlighting (`visual_line_anchor`) within `vim.rs`, replacing them with safe unwraps/rebinding default values to prevent unexpected UI crashes.
  - Enforced a `cargo fmt` standard pass across the entire workspace.
- **UI Tweaks - Editor Title Bar & Vim Indicator (March 22, 2026)**:
  - Removed the static `clin` header bar entirely from the active Editor View layout, optimizing terminal screen space for editing.
  - Repositioned the active `[ Vim ]` mode status badge to align directly onto the left-hand corner of the bottom Help/Status footer.
  - The Vim toggle button/indicator now dynamically displays the active internal machine state (e.g. `[ Vim: NORMAL ]`, `[ Vim: VISUAL LINE ]`, `[ Vim: INSERT ]`) directly leveraging `VimMode::name()`.

## Future Reference For Agents
- **Script Handling:** Always write temporary patches, text extraction scripts, or intermediate compile tasks into `dev_scripts/`. Do not pollute the root directory.
- **Save Mechanisms:** Do not re-implement discrete `save` button functions. The software runs a continuous loop leveraging `app.autosave()`, writing to disk safely without user prompts. Focus directly on text buffer manipulations.
- **Panics:** Use `unwrap_or(...)` or safe defaults handling rather than `.unwrap()`. The system acts as a native text editor; crashes destroy unsaved text buffers. We have panic isolation interceptors (`catch_unwind`), but safe unwraps are highly prioritized to skip crashing in the first place!
- **Code Formatting:** Before completing an architectural logic task, run `cargo fmt` and `cargo clippy` to ensure Rust best practices and clean integrations.
