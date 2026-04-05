pub const FILE_MAGIC: &[u8; 5] = b"CLIN1";
pub const NONCE_LEN: usize = 12;
pub const LIST_HELP_HINTS: &str = "j/k move  Enter open  r rename  d delete  p pin  s sort  Ctrl+f search  P preview  ? help  q quit";
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
- Up/Down/j/k: move selection\n\
- gg: jump to top, G: jump to bottom\n\
- Ctrl+u/d: page up/down\n\
- Enter: open selected note or create new\n\
- r: rename note or folder\n\
- y: duplicate note\n\
- p: pin/unpin note\n\
- s: cycle sort options\n\
- Ctrl+f: search notes by title\n\
- Shift+P: toggle preview pane\n\
- Shift+T: open trash\n\
- d/Delete: move note to trash\n\
- y or Enter: confirm delete\n\
- n or Esc: cancel delete\n\
- f: open selected note location\n\
- m: move note or folder\n\
- .: manage tags (with autocomplete, Shift+D to delete)\n\
- /: filter by tags (with autocomplete)\n\
- Tab: switch focus between note list and toggle buttons\n\
- Enter/Space on toggle button: switch state\n\
- ?: open help page\n\
- q: quit app\n\
\n\
Editor shortcuts\n\
- Tab: change focus (Title, Content, Encryption toggle button)\n\
- Esc: return to notes view\n\
- Ctrl+Q: quit app\n\
- Mouse selection in content area\n\
- Clipboard: Ctrl+C/Ctrl+X/Ctrl+V, Ctrl+Insert, Shift+Insert, Shift+Delete\n\
- Edit: Ctrl+A, Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z\n\
- Word edit: Ctrl+Backspace, Ctrl+Delete\n\
\n\
Help page\n\
- Esc/q/?/F1: close help and return to notes\n";
