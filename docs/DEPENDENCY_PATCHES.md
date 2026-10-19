# Mermaid YAML compatibility

clin-rs preserves its Rust 1.90 minimum and existing Mermaid integration by
patching `merman-core 0.6.0` to the immutable compatibility revision
`ba16ce6a43a5f0d2d070bd40cf91b958fd7e22e0` of
[`reekta92/merman`](https://github.com/reekta92/merman).

The fork replaces archived `serde_yaml` with maintained `yaml_serde 0.10.7`.
clin's existing `serde_yaml_ng` imports alias that same package. Both paths now
use `libyaml-rs` instead of `unsafe-libyaml`, resolving `RUSTSEC-2024-0449`
without suppressing the advisory. The replacement still contains unsafe parser
code; maintenance does not guarantee memory safety.

Cargo only applies patches from the root manifest. Keep this patch even if the
pinstar dependency declares the same one. Cargo also drops patches during
crates.io publishing; applications installing the published crate must supply
this root patch to use the maintained backend.

Update the manifest, lockfile, source allowlist and this note together. Retire
the patch once upstream offers the maintained backend within the supported
Rust minimum. `merman-core 0.7` and later require Rust 1.95 and an API migration.

Verify with `cargo +1.90.0 test --locked`, `cargo deny --locked --all-features
check`, and `cargo tree --locked -i yaml_serde`. `serde_yaml`, `serde_yaml_ng`
and `unsafe-libyaml` packages must remain absent from the lockfile.
