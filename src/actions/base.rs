use crate::actions::ActionCategory;
use crate::simple_action;

simple_action!(
    NewBaseAction,
    "base.new",
    "New Base",
    "Create a new .base (Obsidian-compatible) file and open it",
    ActionCategory::Views,
    "\u{f0c5}",
    "\u{1f4c4}",
    begin_create_base
);

simple_action!(
    OpenBaseAction,
    "base.open",
    "Open Base",
    "Open an existing .base file from the vault",
    ActionCategory::Views,
    "\u{f1c0}",
    "\u{1f5c2}",
    begin_open_base
);
