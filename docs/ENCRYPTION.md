# Encryption

Technical docs for the on-demand encryption system — encrypt/decrypt individual notes on demand using ChaCha20-Poly1305 AEAD.

---

## Overview

clin provides on-demand encryption for individual notes. Encrypted notes use the `.clin` extension and are decrypted back to `.md` on demand. The key never leaves the user's machine and is stored in the application data-local directory, outside the versioned vault.

**Source:** `src/storage.rs` (encrypt/decrypt methods) + `src/actions/encrypt.rs`, `src/actions/decrypt.rs`

---

## Algorithm

**ChaCha20-Poly1305** (via the `chacha20poly1305` crate).

- Symmetric stream cipher (ChaCha20) + authentication tag (Poly1305)
- AEAD (Authenticated Encryption with Associated Data) — tamper-proof
- 256-bit key (32 bytes)
- 96-bit nonce (12 bytes), randomly generated per encryption with `OsRng`

---

## Key Management

### Location

```text
<application-data-local-dir>/key.bin
```

`AppPaths::data_local_dir()` resolves this platform-specific directory.

### Key Generation

On first `encrypt_note()` call:

1. `ensure_key()` checks if `key.bin` exists
2. If not, generate 32 random bytes via `OsRng`
3. Write to `key.bin` with `0o400` permissions (Unix) — owner read-only
4. Store in `Storage::key: [u8; 32]`

### Security

- Key file permissions: `0400` (Unix) — only the owner can read
- Key is held in memory in `Storage::key` (zeroed on drop via `zeroize` crate)
- No password protection yet — key is plaintext on disk
- No key rotation
- No per-note keys — single key for all encrypted notes

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

**Nonce:** 12 random bytes, unique per encryption

**Ciphertext:** ChaCha20-Poly1305 encrypted output of bincode-serialized `Note`:

```rust
pub struct Note {
    pub title: String,
    pub content: String,
    pub updated_at: u64,
    pub tags: Vec<String>,
}
```

### Why Frontmatter is Plaintext

The YAML frontmatter in `.clin` files is **not encrypted**. This allows:

- **Fast summary loading** — `load_note_summary()` reads frontmatter directly without decryption
- **Search indexing** — titles and tags are visible without the key
- **Sorting** — `updated_at` is visible for sort operations

The frontmatter metadata (`pinned`, `links`) is preserved across encrypt/decrypt round-trips.

---

## Workflow

### Encrypt (`.md` → `.clin`)

```
User selects a note → Command Palette → Encrypt Note

encrypt_note(id):
  1. ensure_key() — generate key if missing
  2. load_note(id) — read plaintext .md
  3. Build frontmatter from note (title, tags, etc.)
  4. bincode::serialize(note) → bytes
  5. encrypt(bytes):
     a. OsRng → 12-byte nonce
     b. ChaCha20Poly1305::encrypt(nonce, bytes) → ciphertext
     c. CLIN1 magic + nonce + ciphertext → encrypted blob
  6. Prepend YAML frontmatter to encrypted blob
  7. Write to .clin file
  8. Delete original .md
```

### Decrypt (`.clin` → `.md`)

```
User selects a note → Command Palette → Decrypt Note

decrypt_note(id):
  1. ensure_key() — load existing key
  2. Read .clin file
  3. Load note (which decrypts the payload internally)
  4. Extract frontmatter from plaintext prefix
  5. Serialize frontmatter + content as .md
  6. Write to .md file
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
- Canvas (`.canvas`) and Draw (`.draw`) files are not encrypted
