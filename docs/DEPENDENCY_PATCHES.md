# Dependency compatibility patches

## Mermaid YAML backend

clin-rs pins `merman-core 0.6.0` to compatibility fork
[`reekta92/merman`](https://github.com/reekta92/merman), revision
`ba16ce6a43a5f0d2d070bd40cf91b958fd7e22e0`, through `[patch.crates-io]`.
Pinstar, Mermaid rendering, package versions, and clin's Rust 1.90 minimum stay
unchanged. No Mermaid feature was disabled to make dependency checks pass.

The fork starts at the published 0.6.0 source revision
`91722fd1881ae37fd276788fe78a5cc7274d4cde`. Its only runtime change replaces
archived `serde_yaml` with YAML Organization's `yaml_serde 0.10.7`. clin's
historical `serde_yaml_ng` import name aliases that same maintained package;
there is no compatibility wrapper or second semantic value model.

`yaml_serde` uses maintained `libyaml-rs 0.3.0`. This backend still contains
translated libyaml unsafe code; removing the unmaintained dependency advisory
is not a claim that all parser memory-safety risks disappear. No new advisory
ignore was added. The source policy allows only the specific approved fork URL.

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
preservation, Base authoring, encrypted-note round trips, and native Mermaid
previews. Encrypted `Note` and bincode schemas are unchanged.

Run `development/scripts/check.sh` for the complete clin gates. `cargo tree -i
yaml_serde` should show the shared maintained backend; `unsafe-libyaml`,
`serde_yaml`, and `serde_yaml_ng` packages must be absent from `Cargo.lock`.

### Updating or retiring the patch

Keep the immutable revision and compatibility tests until an upstream release
supports the maintained backend without exceeding Rust 1.90. Do not substitute
a moving branch, remove parser limits, suppress advisories, or silently drop
Mermaid support. Update `Cargo.toml`, `Cargo.lock`, source policy and this note
together, then rerun the full gates and throwaway-vault smoke workflows.
