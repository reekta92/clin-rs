pub const FILE_MAGIC: &[u8; 5] = b"CLIN1";
pub const NONCE_LEN: usize = 12;
pub const LIST_HELP_HINTS: &str = "Up/Down move   Enter open/new   Del/d delete   f location   ? help   Tab change focus   q quit";
pub const EDIT_HELP_HINTS: &str = "Esc back   Tab change focus   Ctrl+Q quit";
pub const HELP_PAGE_HINTS: &str = "Esc/q/?/F1 close help";
pub const _HELP_PAGE_TEXT: &str = "clin help\n\
\n\
Core features\n\
- Encrypted local notes stored as binary .clin files\n\
- Notes shown in a list, open/edit directly in terminal UI\n\
- Continual Auto-save\n\
- Save+quit from editor with Ctrl+Q\n\
- Delete notes with confirmation (d/Delete then y or Enter)\n\
- Open note file location in file manager (f in notes view)\n\
\n\
Notes view shortcuts\n\
- Up/Down: move selection\n\
- Enter: open selected note or create new\n\
- d/Delete: request note delete\n\
- y or Enter: confirm delete\n\
- n or Esc: cancel delete\n\
- f: open selected note location\n\
- Tab: switch focus between note list and Encryption toggle button
- Enter/Space on Encryption button: toggle Encryption
- ?: open help page
- q: quit app

Editor shortcuts
- Tab: change focus (Title, Content, Encryption toggle button)
- Esc: return to notes view
- Ctrl+Q: quit app
- Mouse selection in content area
- Clipboard: Ctrl+C/Ctrl+X/Ctrl+V, Ctrl+Insert, Shift+Insert, Shift+Delete
- Edit: Ctrl+A, Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z
- Word edit: Ctrl+Backspace, Ctrl+Delete

Help page
- Esc/q/?/F1: close help and return to notes\n";
