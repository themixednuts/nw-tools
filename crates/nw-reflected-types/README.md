# nw-reflected-types

Behavior-free Rust source mirror for the New World/Lumberyard types present in
the captured `SerializeContext` schema. The checked-in source is generated from
`codegen/selection.json`; character, animation, Mannequin, material, texture,
and audio crates consume these definitions but own parsing and conversion
behavior themselves.

Regenerate from the repository root:

```powershell
cargo run -p nw-serialize-codegen -- generate `
  --serialize-context resources/serialize.json `
  --modules resources/modules `
  --selection explicit `
  --selection-file crates/nw-reflected-types/codegen/selection.json `
  --language rust `
  --rust-layout vendored `
  --rust-package nw-reflected-types `
  --out-dir crates/nw-reflected-types
cargo fmt -p nw-reflected-types
```

Generated types are schema evidence, not behavior. Binary/XML parsing semantics
remain grounded in New World Ghidra evidence and the matching Lumberyard source.
