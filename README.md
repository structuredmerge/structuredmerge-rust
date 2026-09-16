# StructuredMerge Rust native layer

This repository contains [`kettle-rusty`](crates/kettle-rusty), the Rust project
tooling built on the [StructuredMerge kernel](https://github.com/structuredmerge/structuredmerge).
Kernel crates, generated language bindings, and `smorg` development now live in
that repository. This repository retains its complete pre-split history so old
commit and package-provenance links continue to resolve.

## Development

Run `mise run check` for formatting, Clippy, and workspace tests. Tests use the
sibling [fixtures repository](https://github.com/structuredmerge/structuredmerge-fixtures)
at `../fixtures`.

During migration, `kettle-rusty` pins the kernel to its published history-split
commit over SSH. Cargo.lock records the resolved dependency graph. A future
release will use the corresponding registry version after package validation;
the pin is not a claim that the existing crates.io version contains current
kernel behavior.

Ruby host development must use the kernel checkout's `packages/ruby`, not this
repository. Point `STRUCTUREDMERGE_RUST_DEV` at that checkout when using the
existing Ruby development switch. Existing local bundles may need to be
resolved again through their owning Bundler workflow.

## Behavioral authority

Ruby remains the behavioral golden master for migrated capabilities until an
explicit reviewed authority decision. Passing the bounded `kettle-rusty`
suite does not establish complete `kettle-jem` parity. Native-layer tooling and
kernel releases have separate lifecycles.

## Release

`mise run release -- --no-push` validates the `kettle-rusty` package locally.
Publishing remains gated on validating its registry-resolved kernel dependency
and the declared Rust-project capability set. Kernel crates are released by
`structuredmerge/structuredmerge`.

See [LICENSE.md](LICENSE.md) for the existing license choices.
