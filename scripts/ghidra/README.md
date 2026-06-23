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
