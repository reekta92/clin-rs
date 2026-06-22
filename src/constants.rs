pub const FILE_MAGIC: &[u8; 5] = b"CLIN1";
pub const NONCE_LEN: usize = 12;
pub const LIST_HELP_HINTS: &str = "j/k move · Enter open · ? help · q quit";
pub const EDIT_HELP_HINTS: &str = "Tab focus · Esc back · Ctrl+Q quit · Ctrl+P preview";
pub const HELP_PAGE_HINTS: &str = "← → switch tab · ↑ ↓ scroll · / search · Esc close";
pub const GRAPH_HELP_HINTS: &str = "Esc: back · +/-: zoom · L: labels · a: fit";
pub const DRAW_HELP_HINTS: &str =
    "Esc: back · d: draw · s: shape · t: text · e: erase · Ctrl+S: save";
pub const CANVAS_HELP_HINTS: &str = "Esc: back · Space: pin · /: filter · Backspace: delete";
pub const BACKUP_HELP_HINTS: &str = "s: commit · p: push · r: refresh · Ctrl+P: settings · Esc: ←";
pub const CONTENT_TREE_HELP_HINTS: &str = "j/k move · Tab fold · Enter jump · Esc back · ? help";
