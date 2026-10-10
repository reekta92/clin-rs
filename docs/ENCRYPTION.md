# Encryption

Technical docs for the on-demand encryption system — encrypt/decrypt individual notes on demand using ChaCha20-Poly1305 AEAD.

---

## Overview

clin provides on-demand encryption for individual notes. Encrypted notes use the `.clin` extension and are decrypted to their recorded original extension (`.md` when absent). The application stores a shared plaintext key in the application data-local directory, outside the vault; clin does not transmit it as part of normal vault backup.

**Source:** `src/storage.rs` (encrypt/decrypt methods) + `src/actions/encrypt.rs`, `src/actions/decrypt.rs`

---

## Algorithm

**ChaCha20-Poly1305** (via the `chacha20poly1305` crate).

- Symmetric stream cipher (ChaCha20) + authentication tag (Poly1305)
- AEAD (Authenticated Encryption with Associated Data) — detects modification of the encrypted payload; plaintext frontmatter is **not** authenticated
- 256-bit key (32 bytes)
- 96-bit nonce (12 bytes), freshly generated per encryption with `rand::rng().fill(...)`

---

## Key Management

### Location

```text
<application-data-local-dir>/key.bin
```

`AppPaths::data_local_dir()` resolves this platform-specific directory.

### Key Generation

During `Storage` initialization (and again before operations that need the key):

1. `ensure_key()` checks if `key.bin` exists
2. If not, generate 32 random bytes via `rand::rng().fill(...)`
3. Write to `key.bin` with `0o400` permissions (Unix) — owner read-only
4. Store in `Storage::key: [u8; 32]`

### Security

- Key file permissions: `0400` (Unix) — only the owner can read
- Key is held in memory in `Storage::key` (zeroed on drop via `zeroize` crate)
- No password protection yet — key is plaintext on disk
- No key rotation
- No per-note keys — one application-wide key is shared across vaults
- Keep a secure separate backup of `key.bin`; copying the vault alone does not copy the key, and losing it makes encrypted content unrecoverable
- Anyone who can read the key and vault can decrypt content; this is not password protection or a zero-knowledge service

---

## File Format

Encrypted `.clin` files have two sections:

### Layout

```
┌──────────────────────────────────────────┐
│  Frontmatter (YAML, plaintext)           │
│  ---                                     │
│  title: "My Note"                        │
│  updated_at: 1746814780                  │
│  tags: [work, journal]                   │
│  pinned: false                           │
│  links: [[other note]]                   │
│  ---                                     │
├──────────────────────────────────────────┤
│  Encrypted payload:                      │
│  ┌────────┬──────────┬─────────────────┐ │
│  │ CLIN1  │ 12-byte  │ ciphertext      │ │
│  │ magic  │ nonce    │ (bincode-serial-│ │
│  │ (5B)   │          │ ized Note)      │ │
│  └────────┴──────────┴─────────────────┘ │
└──────────────────────────────────────────┘
```

**Magic:** `CLIN1` (5 bytes) — identifies the encrypted payload start

**Nonce:** 12 fresh random bytes per encryption (uniqueness is probabilistic)

**Ciphertext:** ChaCha20-Poly1305 encrypted output of bincode-serialized `Note`, including the 16-byte authentication tag:

```rust
pub struct Note {
    pub title: String,
    pub content: String,
    pub updated_at: u64,
    pub tags: Vec<String>,
}
```

### Why Frontmatter is Plaintext

The YAML frontmatter in `.clin` files is **neither encrypted nor authenticated**. Titles, tags, timestamps, pinned state, links, original extension, text alignment, and other metadata remain visible and can be modified independently of the encrypted payload. Do not store secrets in frontmatter. This allows:

- **Fast summary loading** — `load_note_summary()` reads frontmatter directly without decryption
- **Search indexing** — titles and tags are visible without the key
- **Sorting** — `updated_at` is visible for sort operations

The frontmatter metadata (`pinned`, `links`, and any unknown keys such as Obsidian Properties) is preserved across save, encrypt, decrypt, and duplicate round-trips.

---

## Workflow

### Encrypt (plaintext note → `.clin`)

```
User selects a note → Command Palette → Encrypt Note

encrypt_note(id):
  1. ensure_key() — generate key if missing
  2. load_note(id) — read plaintext .md
  3. Build frontmatter from note (title, tags, etc.)
  4. bincode::serialize(note) → bytes
  5. encrypt(bytes):
     a. rand::rng().fill(...) → 12-byte nonce
     b. ChaCha20Poly1305::encrypt(nonce, bytes) → ciphertext
     c. CLIN1 magic + nonce + ciphertext → encrypted blob
  6. Prepend YAML frontmatter to encrypted blob
  7. Atomically write to a collision-safe .clin path
  8. Delete original plaintext file (not a secure erase)
```

### Decrypt (`.clin` → original extension)

```
User selects a note → Command Palette → Decrypt Note

decrypt_note(id):
  1. ensure_key() — load existing key
  2. Read .clin file
  3. Load note (which decrypts the payload internally)
  4. Extract frontmatter from plaintext prefix
  5. Serialize frontmatter + content using recorded original extension (default .md)
  6. Write to a collision-safe plaintext path
  7. Delete original .clin
```

### Loading a `.clin` Summary

```
load_note_summary(id):
  1. Read .clin file
  2. Extract frontmatter from plaintext YAML prefix
     → Get title, updated_at, tags, pinned, links
  3. Return NoteSummary (no decryption needed)
```

### Loading Full `.clin` Content

```
load_note(id):
  1. Read .clin file
  2. Extract frontmatter (plaintext YAML)
  3. Payload begins at the frontmatter boundary → validate CLIN1 magic at offset 0
  4. decrypt(payload):
     a. Parse magic, nonce, ciphertext
     b. ChaCha20Poly1305::decrypt(nonce, ciphertext) → bytes
     c. bincode::deserialize(bytes) → Note
  5. Return Note
```

---
## Editor Draft Recovery

Editor mutations attempt to synchronously write a recovery draft for plaintext notes.
- Located at `<vault>/.clin/editor_draft.bin`.
- Contains a bincode-serialized tuple: `(note_id, title, content)`.
- Encrypted using the same application-wide ChaCha20-Poly1305 key.
- Bootstrap attempts to decrypt and restore it into the note; `.clin` IDs are excluded from draft writes.
- Recovery is best effort: write/save errors are not all propagated, and the current recovery path deletes a readable draft even after decoding or save failures. Drafts are not a replacement for backups.


## Key Rust Types

```rust
pub struct Storage {
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub notes_dir: PathBuf,
    pub templates_dir: PathBuf,
    pub key: [u8; 32],  // zeroized on drop
}

impl Storage {
    pub fn ensure_key(&mut self) -> Result<()>;
    pub fn encrypt_note(&mut self, id: &str) -> Result<String>;
    pub fn decrypt_note(&mut self, id: &str) -> Result<String>;
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
    pub fn decrypt(&self, payload: &[u8]) -> Result<Vec<u8>>;
}
```

---

## Action Integration

Encrypt/Decrypt are available via the command palette (Ctrl+P):

- `EncryptNoteAction` — `note.encrypt`
- `DecryptNoteAction` — `note.decrypt`

See [COMMAND_PALETTE.md](COMMAND_PALETTE.md) for the action system.

---

## Limitations

- Key is plaintext on disk (`key.bin`)
- No password-derived key (yet)
- No key rotation
- No per-note keys
- Bulk encrypt/decrypt not available (individual notes only)
- Images are rejected by Encrypt Note. Canvas/Draw views require plaintext JSON; the generic action does not reject their extensions, but note-style conversion can add frontmatter and break JSON loading. Do not treat this as supported canvas/draw encryption.
- Plaintext frontmatter is visible and unauthenticated
- `.clin` editing is blocked; decrypt through the palette before editing
- Subnotes saved under plaintext parents are only XOR-obfuscated, not encrypted; see [SUBNOTES.md](SUBNOTES.md)
- Encrypting deletes the current plaintext file, not previous copies, OS/editor caches, backups, or Git history. Existing backup history can still expose plaintext. Review backups before pushing a vault to any remote.
