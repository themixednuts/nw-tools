# Separate algorithm, source-style, and output oracles

Status: resolved

## Question

Which local and upstream corpora are authoritative for correctness, structure,
and New World idiom decisions?

## Resolution

Use three distinct oracles instead of treating any one corpus as original
source:

1. `Coldzer0/LuaDecompiler` is the reference for chunk decoding, semantic
   opcodes, SSA construction, control-flow recovery ideas, and differential
   validation. The inspected local checkout matches upstream `main` at
   `e75c48a73008187e88cc5a50a2dd06b884247923`.
2. `E:\Projects\DEMOJSON` is the source-style oracle. Its 1,290 Lua files use
   New World's module tables, PascalCase methods, lower-camel locals, and normal
   Lua constructor/declaration forms.
3. `E:\Projects\new-world\resources\lua` is a prior-output corpus: all 1,394
   Lua files contain the `Decompiled by cLuaDecompiler` header. It is useful for
   seeing what Coldzer0 recovers, but not for deciding what original source
   looked like.

The 1,203 shared relative paths are different content/version snapshots, so
path equality alone is not a source equivalence claim.

