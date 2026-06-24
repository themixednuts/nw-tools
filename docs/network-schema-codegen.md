# Network Schema Codegen

## End Goal

Generate the public `nw-network` Rust API from a derived network schema instead
of hand-porting each network type. The current `typeregistry.json` remains a
read-only input; every enrichment step writes a separate artifact with evidence
and confidence.

The target generated surface is:

- network messages and replicated states with stable type IDs and type indices
- field order, group, and handler evidence for replicated-state fields
- support structs from the serialize model when a network type references them
- evidence reports for unsupported, ambiguous, or low-confidence shapes
- capture-validation hooks that show which wire values are fully parsed

## Phases

1. **Static extraction**
   - Run the Ghidra extractor against `NewWorld 3-26`.
   - Emit every TypeRegistry row, handler vtable functions, AZ RTTI provider
     evidence, registration-hook names, and `RegisterField` call evidence.
   - Output: `newworld.network_schema.static.v1`.

2. **Normalization**
   - Convert the static report into `newworld.network_schema.v1`.
   - Preserve all raw source addresses and confidence-ranked evidence.
   - Attach SerializeContext kind/name/dependency evidence from the existing
     compiler IR.
   - Optionally validate type indices against the game `typeindex.json` asset.
   - Do not mutate `typeregistry.json`.

3. **Merge and enrichment**
   - Merge normalized Ghidra evidence with `typeindex.json`, `serialize.json`,
     module descriptors, and capture observations.
   - Resolve field handler shapes, support-type references, roots, and module
     placement.
   - Emit a report for every unresolved or conflicting type.

4. **Rust planning**
   - Build a Rust IR from the normalized schema, following the existing
     serialize codegen flow.
   - Use `syn`, `quote`, and `prettyplease` for source emission.
   - Generate descriptor/evidence modules before generating full field structs;
     full field structs require handler-type recovery.
   - Prefer derives and generated support types over handwritten impls.

5. **Validation**
   - Parse capture bundles through `nw-network` and report exact unsupported
     type IDs, field handlers, and byte spans.
   - Keep fixtures small but real: one happy path per common message/state
     family, plus regression cases for missing or overflowed fields.

6. **Publication**
   - Generate only the public, self-contained `nw-network` surface.
   - Keep private project paths and engine implementation details out of
     generated docs and public APIs.

## Current Command

```powershell
cargo run -p nw-serialize-codegen -- network-schema `
  --ghidra-report E:\Projects\new-world\resources\network-schema.static.json `
  --typeindex "E:\Games\steamapps\common\New World\typeindex.json" `
  --out tmp\network-schema.v1.json

cargo run -p nw-serialize-codegen -- network-rust `
  --schema tmp\network-schema.v1.json `
  --out tmp\network-schema.rs `
  --report tmp\network-schema.rust-report.json
```
