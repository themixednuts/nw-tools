# Make AST idiom passes binding-aware

Status: resolved

## Question

How should declaration sugar, method recovery, and synthetic module-table
styling operate on stable binding identity instead of byte-name comparisons and
hand-maintained scope scans?

## Acceptance direction

Treat `BindingId` as the semantic identity of every local, parameter, and
upvalue reference in the compact AST. Give idiom passes reusable traversal
capabilities that select and rewrite one binding while naturally respecting
shadowing and nested function boundaries. Textual `Name` bytes remain an
emission property, never the key used to prove aliasing or scope.

Remove `count_scope_declarations`, byte-slice rename walkers, and other
name-based shadow tracking once their consumers use identity. Preserve globals,
labels, method paths, and deliberately unbound names. Add focused nested-scope,
shadowing, recursive-local-function, method-sugar, and module-table style tests,
then keep both corpus gates clean.

## Starting evidence

The AST and validator already carry `FunctionId`/`BindingId`, but
`idiomatic/naming_style.rs` still counts declarations by byte spelling and
renames through a custom shadow-aware walker. `idiomatic/sugar.rs` contains a
second byte-name rewrite path for declaration/function sugar. Those passes are
duplicating a scope problem already solved by binding identity.

## Evidence

`ast::bindings` now owns one binding-aware traversal capability for usage,
collision checks, and renaming across blocks and nested functions. It follows
`BindingId` declarations and references through closures while deliberately
ignoring fields, labels, globals, and same-spelled shadow bindings.

`naming_style` and declaration sugar consume that capability. The former
name-counting and byte-spelling rename walkers, receiver scans, and duplicate
scope-declaration scans were removed. Module binding recognition, local and
recursive function sugar, and method receiver removal now prove identity with
`BindingId`; textual names are only selected after collision availability has
been checked.

The same capability now collects receiver and ordinary-value read counts for
all bindings in one AST walk. A synthetic table with receiver mutations and one
value read into a named field can therefore inherit that field's lower-camel
name (`Options.Properties = l4` becomes `Options.Properties = properties`). The
rule is consumer- and binding-driven: it has no New World field-name list, and
numeric or dynamic index consumers remain anonymous because they provide no
stable semantic spelling.

The owning reconstruction fixes exposed by the stronger tests were made at
their source: captured bindings force all SSA definitions of that binding to be
materialized; loop regions retain the actual versioned loop-variable
definitions; and nested constructor scheduling excludes its own owner when
checking whether field setup must stand alone.

Focused evidence includes nested closure/shadow rename tests,
binding-availability and receiver-usage tests, named-field inference plus
numeric/dynamic refusal tests,
`local_function_sugar_distinguishes_shadowed_binding_identity`, the Phase 8
closure suite (3/3), Phase 9 naming suite (5/5), and Phase 9b declaration and
constructor regressions. Final corpus gates remained clean: 40/40 fast and
300/300 heavy decompile/recompile, with 4,223/5,068 exact prototypes (83.33%)
and 59,318/72,183 matching opcodes (82.18%) on the heavy structural sweep. The
300-file fidelity gate also retains zero hits in all four high-severity classes.
