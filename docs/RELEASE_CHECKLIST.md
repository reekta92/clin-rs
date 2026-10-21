# Release acceptance

The `Release readiness` milestone tracks current blockers. Record candidate SHA,
OS, terminal/version, image protocol and results in its acceptance issue.
Incomplete or failed required items block stable publication.

## Automated proof

- Required stable, minimum-Rust, dependency policy, Windows, macOS and extracted
  registry-package checks pass for the candidate.
- Graf, Pinstar and Draw adapter contracts pass against the committed consumer.
- Development-tooling and wiki tests pass when affected.
- No root Cargo patch is needed for the maintained Mermaid YAML backend.
- The release dry run passes without changing main, tags or registry versions.
- Actual publication validates the exact candidate and reports its successful
  checks before the protected push. Concurrent changes to main abort safely.

## Interactive acceptance on Linux, Windows and macOS

Use a disposable vault and isolated configuration. Preserve real vaults and keys
before checking upgrade behavior.

1. Start, resize to a small terminal and back, and open Help. Verify readable
   focus, keyboard input, paste, mouse input and configured palette keys.
2. Create a note, edit its title/body/properties, save, quit and reopen. Verify
   content/filename. Change the file externally and confirm a stale save retains
   the unsaved buffer and reports a conflict. Check read-only behavior.
3. Leave unsaved text, terminate, restart and verify recovery. Check ordinary
   exit, Ctrl+C and error exits restore raw mode, mouse and alternate screen.
4. Open Canvas and Draw, change content, save, leave and reopen. Verify external
   changes reject stale saves and failed saves retain the live model.
5. Open Graph and a public `.base` file, navigate and return to an edited note.
   Verify the pending buffer and expected history are retained.
6. Check clipboard and images in a supported terminal. Record protocol-specific
   skips explicitly; hosted rendering tests exclude terminal I/O.
7. On macOS test both Finder's packaged app and its terminal binary. On Windows
   test the packaged executable in a normal terminal. Verify startup and exit.

Unix tests cover mode-bit preservation. Windows recovery inherits ACLs; replacement
does not promise preservation of arbitrary security descriptors, extended
attributes, ownership or ACLs. Keep the limits in [UPGRADING.md](UPGRADING.md).

## After publication

Verify registry installation and selected artifacts against recorded checksums.
Confirm documentation/PKGBUILD reference updates pass CI before their normal
push. Successful runs remove their temporary branches. Inspect failed runs
before removing retained branches; published versions and tags stay immutable.
Record the release URL and close acceptance only after these checks.
