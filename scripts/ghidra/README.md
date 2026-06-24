# Ghidra Reflection Renamer

`AzReflectionRenamer.java` renames AZ/Lumberyard reflection artifacts in a
loaded New World program.

Run it from Ghidra's Script Manager and select:

```text
resources/serialize.json
```

The script automatically loads sibling evidence when present:

- `resources/modules/*.json`
- `resources/behavior-context.json`
- `resources/serialize-class-registration.jsonl`
- `resources/serialize-field-registration.jsonl`

`resources/behavior-context.7z` contains the optional behavior context evidence.
The script reads `resources/behavior-context.json` when present; otherwise it
streams `behavior-context.json` directly from the `.7z` archive.

By default the script runs in dry-run mode and writes:

```text
resources/serialize.renames.json
```

Set `AZ_SERIALIZE_RENAME_APPLY=true` before launching Ghidra to apply renames.

## Replicated-State Schema Extractor

`ReplicatedStateSchemaExtractor.java` builds a static JSON report for replicated
state registration evidence. Run it against the loaded `NewWorld 3-26` program
and point it at `typeregistry.json`.

Useful environment variables:

```text
NW_REPLICATED_STATE_TYPEREGISTRY_JSON=E:\Projects\new-world\resources\typeregistry.json
NW_REPLICATED_STATE_SCHEMA_OUT=E:\Projects\new-world\resources\replicated-state-schema.static.json
```

The script emits every `typeregistry.json` row, recovers
`MB::ReplicatedState::RegisterField` callers from Ghidra where available, and
adds constructor field order, groups, handler offsets, instance vtables, and
decoded AZ RTTI provider evidence to rows that can be statically mapped. Native
type names are recovered from actual AZ/Hub registration helper tables or AZ RTTI
providers; TypeRegistry names remain the raw TypeRegistry/debug-name field.
