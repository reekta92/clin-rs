# Upgrading to the plaintext-only app

Backup and encryption features have been removed. clin no longer creates Git repositories, commits, pushes, generates keys, encrypts notes, or decrypts legacy records. Existing vault Git history, encrypted files, and key files are not automatically removed or converted.

## Before upgrading an encrypted vault

1. Securely copy the complete vault, including hidden `.clin` metadata, and separately preserve the original application key files. A vault copy alone does not include the key stored outside it.
2. Use the previous release against a copy, with the original key, to recover drafts and decrypt notes. The previous recovery implementation can discard failed records, so never run this migration against your only surviving copy.
3. Verify recovered plaintext content and supported canvas/drawing formats before opening the copy in this release.
4. Keep originals until the migration and independent backups have been verified.

Legacy `.clin` notes, `.clin/editor_draft.bin`, old `.clin/document-recovery` journals, and opaque encrypted subnote payloads remain untouched by automatic recovery. Startup reports detected legacy notes/recovery records. Encrypted subnotes cannot be read or changed here; use the previous release first. Old encrypted catalog caches are not reused or automatically deleted.

## Plaintext recovery and privacy

Recovery now uses version-2 records under `<vault>/.clin/document-recovery-v2`. The directory is private (`0700`) and records use `0600` on Unix. Windows uses inherited filesystem ACLs; review access permissions on shared locations.

These records contain plaintext note contents, including prior contents used for transaction rollback. File permissions are **not encryption**. Choose a vault/filesystem with suitable access controls. A filesystem backup, sync tool, or another process running as your user may copy or read these records.

Draft writes are coalesced off the input thread, with a 150 ms scheduling window. Save and orderly session transitions flush pending writes. Abrupt process or machine failure can lose changes not yet persisted. Failed writes, malformed records, and recovery conflicts are retained and reported, not discarded.

## Safe note saving

Loaded notes and public documents share raw-source baseline checks, read-only checks, permission-preserving atomic replacement, and a native lock for cooperating clin writers. A disk change already present when saving is checked causes a conflict rather than silently replacing newer content. Other editors do not honor clin's lock; no cross-application guarantee is made for a write racing inside the final check/publication interval.

Ordinary title/body saves retain the existing filename. New notes still derive an initial filename from their title. Explicit file rename is separate and warns that incoming links are not rewritten. Existing `rename_on_title_change` settings are retired.

## Frontmatter editing and recovery

F7 opens a hidden-by-default properties region above Body for Markdown/text notes. Set `[editor] frontmatter_mode = "raw_yaml"` for raw editing in the same region. `[notes.frontmatter]` controls automatic managed output; disabled fields are retained and explicit edits/removals win for that save. `original_ext` is accepted as a compatibility no-op and existing extension metadata is preserved.

Version-2 transaction/source-draft records remain readable. New repair-only editor records retain invalid YAML, unfinished property inputs, and conflicting Title/YAML input without automatically writing them into a note. Startup opens the newest repair draft; additional repair sessions remain on disk. Repair and save it using the original baseline; external changes still block replacement. The new enum variant is not readable by older releases, which retain unsupported records. Recovery remains private plaintext under the same permissions and size limits.

Keep stable filename-based wikilinks such as `[[CL-01]]`: ordinary body/property/title saves retain `CL-01.md`. Existing broken links are not rewritten or migrated.

## Retired settings

`[backup]`, `[features].backup`, `[features].encryption`, `[list].preview_encryption`, and backup statusline overrides are removed from configuration. Cleanup preserves unrelated preferences and comments; failures are reported. Keybind files no longer expose a backup scope. These config changes do not touch vault `.git` history or key files.

Use external version control or filesystem backup tools if needed. Removing clin's backup dashboard does not remove the need for independent backups.

Explicit file rename changes only the filename; it preserves source bytes and
does not change the frontmatter title. Plaintext subnotes move with the parent;
opaque legacy subnotes and destination-record collisions block the operation.
An interrupted rename may retain both parent records for manual inspection.

Permission preservation covers Unix mode bits. Replacement does not preserve
owner/group, ACLs, extended attributes, or Windows security descriptors. Review
shared-directory access controls before editing files that depend on those
features. Templates use source conflict checks but do not have note recovery
records.
