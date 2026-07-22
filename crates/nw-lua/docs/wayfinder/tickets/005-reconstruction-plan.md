# Replace mutable emission bookkeeping with a reconstruction plan

Status: resolved

## Question

How should materialization, node consumption, declaration ownership, and
expression inlining be decided before AST emission so `StatementBuilder` and
`ExprBuilder` do not coordinate through several mutable sets and name maps?

## Acceptance direction

Build one per-function reconstruction plan from SSA analyses and structured
regions. Give every SSA definition one disposition (inline, materialize,
constructor member, control-only, or dead) and every emitted declaration a
binding identity. AST construction consumes the plan monotonically; it does not
rediscover decisions while walking nodes.

## Evidence

`ReconstructionPlan` now assigns every SSA definition a `ValueDisposition`, a
stable optional `BindingId`, a materialization PC/name, and a declaration role.
It also owns the reusable table-constructor facts and a dense `NodeEmission`
schedule with four closed states: omitted by structured control flow,
standalone, multi-node owner, or owned member.

Multi-node recognition runs once while building the plan. Typed plans cover
table constructors, swaps, grouped local declarations, and Lua 5.1 fixed-call
result transfers. In particular, the compiler window `CALL R8 ...; MOVE R7,
R10; MOVE R6, R9; MOVE R5, R8` lowers directly to the New World idiom `L5, L6,
L7 = L5(L6)` instead of exposing call-frame registers. Planned owners feed
their target definitions back into value materialization before declaration
roles are finalized.

`StatementBuilder` no longer has a consumed-node set and never reruns a
multi-node recognizer. Its only declaration state is a monotonic cursor over
the exact binding identities supplied by the plan. The old lexical synthetic
scanner was removed; the binding-aware AST validator is authoritative.

Grouped-local planning also queries exact `BindingId` plus the shared dominator
analysis. A later assignment cannot be mistaken for a fresh local initializer
when a dominating `LOADNIL` already introduced that binding. This replaced the
former same-sequence/register heuristic and is covered by
`dominating_nil_declaration_is_not_replanned_as_a_multi_local_initializer` plus
the complete 38-opcode runtime matrix.

Validation after the final ownership cut:

- focused Phase 7 multi-value suite: 4/4 passed;
- reverse-call-result transfer runtime and shape regression: passed;
- Phase 10b hardening runtime suite: passed;
- fast corpus: 40/40 core and idiomatic decompile/recompile clean;
- heavy corpus: 300/300 core and idiomatic decompile/recompile clean, with
  4,223/5,068 exact prototypes (83.33%) and 59,318/72,183 matching opcodes
  (82.18%).

The implementation removes the emitter-time scanners and the obsolete
name-only validator; the replacement logic lives in the typed per-function
plan rather than parallel AST-emission paths.
