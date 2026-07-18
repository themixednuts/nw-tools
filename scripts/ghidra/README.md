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

## Network Schema Extractor

`NetworkSchemaExtractor.java` builds a static JSON report for network type and
field registration evidence. The file in Ghidra's script directory is generated;
do not edit `$HOME/ghidra_scripts/NetworkSchemaExtractor.java` directly. Edit
the small source files in this directory instead, then run the sync script.

Run the generated script against the loaded `NewWorld 3-26` program and point it
at:

```text
resources/typeregistry.json
```

The extractor source is split across helper files and ordered fragments for
maintainability, but Ghidra should run a single bundled
`NetworkSchemaExtractor.java`. Use:

```powershell
scripts/ghidra/Sync-NetworkSchemaExtractor.ps1
```

The sync script writes the bundled file to `$HOME/ghidra_scripts` by default and
removes copied helper `.java` files from that output directory. Keeping helper
`.java` files in the Ghidra script root makes the script manager treat them as
standalone scripts, which can prevent `NetworkSchemaExtractor` from loading.
The sync script also rejects network-schema source modules over 1000 lines.

Compile the bundled script without running binary analysis:

```powershell
scripts/ghidra/Test-NetworkSchemaExtractor.ps1 -GhidraHome D:\.ghidra
```

- `NetworkSchemaAddressFormatter.java` — address formatter callback shared by models.
- `NetworkSchemaModels.java` — leaf model/data holders.
- `NetworkSchemaPcode.java` — p-code evidence model holders.
- `NetworkSchemaStack.java` — stack-state and constructor evidence models.
- `NetworkSchemaText.java` — deterministic text/literal helpers.
- `NetworkSchemaTypeModels.java` — nested type and container-shape model holders.
- `NetworkSchemaX86.java` — x86 operand, register, memory, and offset helpers.
- `NetworkSchemaJson.java` — JSON helpers.
- `network_schema_extractor/NetworkSchemaExtractor.*.javafrag` — ordered
  fragments for the generated Ghidra script entrypoint.

Interactive runs present an analysis-mode dialog. `Full schema` remains the default.
Focused type, handler-vtable, and function-trace runs ask for their addresses or type
indices directly and write distinct report files, so they cannot replace the complete
schema report.

The script emits every `typeregistry.json` row, recovers
`MB::ReplicatedState::RegisterField` callers from Ghidra where available, and
adds constructor field order, groups, handler offsets, instance vtables, and
decoded AZ RTTI provider evidence to rows that can be statically mapped. Native
type names are recovered from actual AZ/Hub registration helper tables or AZ RTTI
providers; TypeRegistry names remain the raw TypeRegistry/debug-name field.

For types with no replicated-state field registration, the extractor follows their
unmarshal call graph and records only field, storage, codec, and nested-type evidence
proven by P-code data flow. Delegation to a polymorphic fragment codec remains an
explicit schema operation instead of being flattened into synthetic fields.
