# Dependency compatibility

The lockfile also selects `rustls 0.23.46`, resolving
`RUSTSEC-2026-0346` through `RUSTSEC-2026-0349` without advisory ignores or an MSRV
change. This is a registry patch update, independent of the Mermaid fork below.
`rustls-webpki 0.103.16` also resolves `RUSTSEC-2026-0382` (acceptance of
distrusted intermediate certificates), preserving the same compiler minimum.

## Mermaid YAML backend

Clin depends on published Graf, Draw and Pinstar packages, with a minimum of
Pinstar 1.1.2 for the maintained Mermaid backend. Cargo.lock fixes exact versions
and registry checksums.

Pinstar uses published `merman-core-clin 0.6.0` from
[the maintained fork branch](https://github.com/reekta92/merman/tree/fix/maintained-yaml-msrv-1.90).
The upstream 0.6 parser API, library import, MIT/Apache-2.0 attribution and Rust
1.90 support remain. The backend reaches registry consumers without a root
`[patch]`. No Mermaid feature was disabled to make dependency checks pass.

The fork starts at the published 0.6.0 source revision
`91722fd1881ae37fd276788fe78a5cc7274d4cde`. Its only runtime change replaces
archived `serde_yaml` with YAML Organization's `yaml_serde 0.10.7`. clin's
historical `serde_yaml_ng` import name aliases that same maintained package;
there is no compatibility wrapper or second semantic value model.

`yaml_serde` uses maintained `libyaml-rs 0.3.0`. This backend still contains
translated libyaml unsafe code; removing the unmaintained dependency advisory
is not a claim that all parser memory-safety risks disappear. No new advisory
ignore was added. Source policy rejects unknown registries and Git dependencies.

### Compatibility and safety proof

New YAML contracts were run first against the old parser, then against the
replacement. The unchanged existing snapshot corpus also passes:

- 525 core unit tests and the complete existing semantic snapshot test.
- New contracts for tags, arbitrary mapping keys, aliases, numeric precision,
  quoted text, dates, multiline strings, CRLF, malformed/recursive aliases, and
  multi-document streaming.
- Rust 1.90 core tests, doctests, and xtask compilation.

clin's existing bounded YAML visitor and lossless `yaml-edit` CST edits remain
unchanged. Tests exercise duplicate-key rejection, expanded-node/depth bounds,
malformed frontmatter, opaque tagged values, aliases, LF/CRLF, comments, body
preservation, Base authoring, legacy opaque-data preservation, and native Mermaid
previews. New recovery records use version 2; legacy encrypted formats remain untouched.

Run `scripts/check.sh` for local Clin gates and `cargo package --locked` for
package compilation. Shared CI also checks policy against the extracted archive.
`cargo tree -i
yaml_serde` should show the shared maintained backend; `unsafe-libyaml`,
`serde_yaml`, and `serde_yaml_ng` packages must be absent from `Cargo.lock`.

### Updating or retiring the compatibility package

Keep the published compatibility package and its tests until an upstream release
supports the maintained backend without exceeding Rust 1.90. Do not substitute
a moving branch, remove parser limits, suppress advisories, or silently drop
Mermaid support. Update Pinstar first, then Clin's manifest, lockfile, policy and this note
together, then rerun the full gates and throwaway-vault smoke workflows.
