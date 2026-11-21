# Dependency compatibility

## Dependency footprint

Clin uses bincode's Serde API without its derive macros, shares `directories` 6
with Graf, and reuses TLS's `ring` dependency for streaming SHA-256 file digests.
Stored digests and recovery record formats remain unchanged.

Image features explicitly retain the existing decoders and PNG output while
omitting the unused AVIF encoder. Image 0.25's `avif` feature enables encoding;
AVIF decoding requires the separate `avif-native` feature, which Clin did not
enable. Pinstar separates `image-defaults` from image-node support, so Clin
disables its defaults and selects codecs itself. Older Pinstar releases force
all image defaults.

The lockfile also selects `rustls 0.23.46`, resolving
`RUSTSEC-2026-0346` through `RUSTSEC-2026-0349` without advisory ignores or an MSRV
change. This is a registry patch update, independent of the Mermaid parser.
`rustls-webpki 0.103.16` also resolves `RUSTSEC-2026-0382` (acceptance of
distrusted intermediate certificates), preserving the same compiler minimum.

## Mermaid YAML backend

Pinstar 1.1.6 and `merman-core-clin 0.6.1` are published on crates.io.
Pinstar provides native flowchart, sequence, ER and state previews alongside
independent image codec selection. It enables the parser features those
previews use. The standalone parser still enables Gantt by default.

The [maintained fork](https://github.com/reekta92/merman/tree/fix/maintained-yaml-msrv-1.90)
retains the upstream 0.6 parser API, MIT/Apache-2.0 attribution and Rust 1.90
support. It handles delimiters in quoted subgraph titles and UTF-8 ER attribute names.

The fork starts at the published 0.6.0 source revision
`91722fd1881ae37fd276788fe78a5cc7274d4cde`. Its YAML backend replaces
archived `serde_yaml` with YAML Organization's `yaml_serde 0.10.7`. clin's
historical `serde_yaml_ng` import name aliases that same maintained package;
there is no compatibility wrapper or second semantic value model.

`yaml_serde` uses maintained `libyaml-rs 0.3.0`. The translated libyaml backend
still uses unsafe code. Dependency policy rejects unknown registries and Git
sources and retains the existing maintenance exceptions for bincode
(`RUSTSEC-2025-0141`) and paste (`RUSTSEC-2024-0436`).

## Date and calendar dependencies

Clin uses Jiff for civil dates and timestamps and `calcard-next` for iCalendar
parsing and recurrence. Graf 1.1.3 and Trash 5.4 use Jiff. Pinstar
selects only the Mermaid parsers required for its previews. The lockfile
excludes `chrono` and `chrono-tz`, resolving `RUSTSEC-2026-0543` and
`RUSTSEC-2026-0555`. The migration preserves stored timestamp and calendar
formats.

### Compatibility checks

The maintained parser package owns its semantic snapshots and YAML contracts.
When updating it, run those tests on the declared minimum Rust version, including
tags, arbitrary mapping keys, aliases, numeric precision, quoted text, dates,
multiline strings, CRLF, malformed/recursive aliases, and multi-document streaming.

Clin uses a bounded YAML visitor and lossless `yaml-edit` CST edits. Tests exercise duplicate-key rejection, expanded-node/depth bounds,
malformed frontmatter, opaque tagged values, aliases, LF/CRLF, comments, body
preservation, Base authoring, legacy opaque-data preservation, and native Mermaid
previews. New recovery records use version 2; legacy encrypted formats remain untouched.

Run `scripts/check.sh` for local Clin gates and `cargo package --locked` for
package compilation. Shared CI also checks policy against the extracted archive.
`cargo tree -i yaml_serde` should show the shared maintained backend; `unsafe-libyaml`,
`serde_yaml`, and `serde_yaml_ng` packages must be absent from `Cargo.lock`.

### Updating or retiring the compatibility package

Keep the published compatibility package and its tests until an upstream release
supports the maintained backend without exceeding Rust 1.90. Do not substitute
a moving branch, remove parser limits, suppress advisories, or silently drop
Mermaid support. Update Pinstar first, then Clin's manifest, lockfile, policy and this note
together, then rerun the full gates and throwaway-vault smoke workflows.
