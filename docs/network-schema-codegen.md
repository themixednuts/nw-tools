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

## Pipeline Shape

Treat this as a small compiler, not a pile of special-case generators:

1. **Ingestors** read one source each and emit evidence records only. The
   Ghidra static report, type registry dump, `typeindex.json`, serialize
   model, source-header hints, captures, and manual overrides should not mutate
   the final schema directly.
2. **Normalization** turns raw evidence into canonical facts with provenance:
   type identity, AZ RTTI identity, registration hooks, handler vtables,
   field-registration calls, message-unmarshal stores, native source names, and
   wire-codec observations.
3. **Resolution** is the only stage that chooses winners. It ranks evidence,
   diagnoses conflicts, and produces typed plans for messages, replicated
   states, support structs, descriptors, and blocked items.
4. **Emission** consumes only resolved plans. Rust output should not inspect raw
   Ghidra JSON or apply late ad hoc overrides.
5. **Reporting** explains every blocked item in terms of missing evidence or a
   concrete conflict, so the next Ghidra/source/capture pass has a precise
   target.

Use these core concepts consistently:

- `WireShape`: the bytes/codecs recovered from Ghidra or captures.
- `NativeShape`: source-style semantics such as source wrapper, storage type,
  key/value types, and key/value marshaller policies.
- `ResolvedFieldTy`: the concrete Rust type to emit after wire evidence and
  native/source evidence agree.

Wire evidence can reject a bad semantic guess, but it must not invent source
semantics by itself.

## Source-Style Replicated Containers

Do not flatten generated replicated containers to aliases such as
`ReplicatedMap<K, V>`. Handwritten code may keep aliases where they help, but
generated network code should preserve the source concept and emit explicit
policy types.

The source shape is:

```cpp
MB::ReplicatedContainer<K, V, KMarshaller, VMarshaller>
```

with source wrappers such as `ReplicatedMapFieldHandler` supplying the storage
container. Delta marshal/unmarshal writes:

- a `VlqU32` change count
- the key with `KMarshaller`
- a `SequenceNumber` per change, encoded with VLQ64 semantics
- the value with `VMarshaller` for add/update changes

Generated Rust should therefore target policy-explicit shapes:

```rust
pub synced_timers: ::nw_network::serialize::ReplicatedContainer<
    ::std::collections::HashMap<::nw_network::Crc32, u64>,
    { ::nw_network::serialize::WIRE_VEC_CAP },
    ::nw_network::serialize::DefaultMarshaler<::nw_network::Crc32>,
    ::nw_network::serialize::VlqU64Marshaler,
>;
```

This preserves the source semantics: the native value is `u64`, and the
marshaller policy is `VlqU64Marshaler`. Do not model that as
`HashMap<Crc32, VlqU64>` unless the native type itself is a VLQ wrapper.

Useful resolver diagnostics:

- `missing-handler-vtable`: a registered field exists but its handler vtable was
  not recovered.
- `missing-semantic-type`: the wire codec is known but the native/Rust semantic
  type is not.
- `container-codec-only`: container delta/full wire evidence exists, but
  storage/key/value/policy evidence is incomplete.
- `wire-semantic-mismatch`: semantic evidence projects to a different wire
  codec than Ghidra/capture evidence.
- `placeholder-field-name`: a message body is shaped, but the public field name
  is still synthetic.
- `fixed-state-flow-unmodeled`: a fixed replicated-state registration path has
  not been ingested.
- `override-unmatched`: an override entry no longer matched a field.

## Migration Slices

1. Preserve richer wire shapes in `nw-tools` (`vlq-u64`,
   `sequence-number`, and `replicated-container<key,value>`) while keeping Rust
   generation conservative.
2. Move field overrides into the `nw-tools` pipeline as an override ingestor and
   report stale overrides.
3. Resolve scalar fields from `WireShape` plus native/source evidence.
4. Resolve replicated containers from native source wrapper/storage/key/value
   evidence plus Ghidra wire-codec evidence, then emit full
   `ReplicatedContainer<Storage, CAP, KeyCodec, ValueCodec>` types.
5. Add raw-message tiers for high-confidence message bodies whose field names
   are still placeholders, keeping them out of the stable public API until names
   are known.

## Current Command

```powershell
cargo run -p nw-serialize-codegen -- network-schema `
  --ghidra-report E:\Projects\new-world\resources\network-schema.static.json `
  --typeindex "E:\Games\steamapps\common\New World\typeindex.json" `
  --field-overrides E:\Projects\nw-network\codegen\network-field-overrides.json `
  --out tmp\network-schema.v1.json

cargo run -p nw-serialize-codegen -- network-rust `
  --schema tmp\network-schema.v1.json `
  --out tmp\network-schema.rs `
  --report tmp\network-schema.rust-report.json
```
