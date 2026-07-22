# Recover idiomatic New World Lua through compiler facts

## Destination

`nw-lua` reconstructs behaviorally faithful, idiomatic New World Lua through a
typed compiler pipeline: versioned bytecode semantics, SSA and reusable
analyses, structured reconstruction, a compact Lua AST, and source emission.
Output-shape improvements come from recovered compiler facts rather than text
patches or isolated pattern exceptions.

## Notes

- This map includes implementation and verification; it is not planning-only.
- `Coldzer0/LuaDecompiler` at commit `e75c48a73008187e88cc5a50a2dd06b884247923`
  is the algorithm and validation reference. Its string-oriented `TDecompiler`
  shape is not a Rust architecture to port.
- `E:\Projects\DEMOJSON` is the New World source-style oracle.
- `E:\Projects\new-world\resources\lua` contains 1,394 prior
  `cLuaDecompiler` outputs. Use it as a reference-output corpus, not as original
  source.
- Fix facts at the earliest owning layer. Emission only lowers the final AST.
- Preserve runtime behavior, evaluation order, debug names, and raw Lua string
  bytes. Every output change needs focused shape tests plus runtime or corpus
  validation.
- Keep the crate's `ARCHITECTURE.md`, this map, and the relevant system note in
  sync when a boundary changes.

## Decisions so far

- [Separate algorithm, source-style, and output oracles](tickets/001-reference-oracles.md)
  — use Coldzer0 for compiler recovery, DEMOJSON for idioms, and the old resource
  tree for differential output examples.
- [Recover constructor boundaries from bytecode facts](tickets/002-table-constructors.md)
  — model `NEWTABLE` sizing as a typed compiler hint and let one reusable plan
  drive both expression and statement reconstruction.
- [Centralize SSA operand capabilities](tickets/003-ssa-operands.md) — every
  analysis and transform now consumes one typed use/definition/effect contract;
  multi-register definitions belong to their node and loop controls are real
  versioned operands.
- [Introduce a composable SSA pass pipeline](tickets/004-ssa-passes.md) — typed
  pass changes drive explicit cache invalidation and bounded fixpoints; the
  reconstruction path preserves source provenance while every simplifying
  transform remains explicit and opt-in.
- [Replace mutable emission bookkeeping with a reconstruction plan](tickets/005-reconstruction-plan.md)
  — every SSA value and node has planned ownership; AST lowering consumes
  binding declarations and multi-node schedules monotonically.
- [Unify condition and short-circuit value recovery](tickets/006-control-dependence.md)
  — one typed component set owns condition/value control dependence, guarded
  value composition, and classified branch continuations.
- [Make AST idiom passes binding-aware](tickets/007-binding-aware-idioms.md) —
  one `BindingId` traversal owns local usage, collision checks, and rewrites
  across closures and shadowing.
- [Rank source-shape fidelity with AST facts](tickets/008-source-shape-ranking.md)
  — the existing AST signature owns constructor, declaration, temporary, and
  excess-control metrics at function and file granularity.
- [Separate recognized releases from complete compiler targets](tickets/009-version-boundary.md)
  — `LuaTarget` is the proof required to enter the pipeline; future release
  tags remain input-boundary facts until implemented end-to-end.
- [Complete the compiler-pipeline audit](tickets/010-completion-audit.md) — all
  standard, Clippy, maintainability, structural-corpus, and source-oracle gates
  pass on the final implementation.
- [Measure and optimize the decompiler](tickets/011-performance.md) — remove the
  redundant AST serialization boundary, add bounded deterministic batch
  parallelism, and reject SIMD or unbounded worker counts when measurements do
  not support them.
- [Reduce source-emission memory](tickets/012-emission-memory.md) — measure live
  bytes per compiler stage, eliminate token-position rebuilding at its owning
  abstraction, and add workload-aware memory budgeting only from measured cost.

## Frontier

- Source-emission peak memory is the remaining performance frontier. Ticket 012
  defines the ordered, non-adhoc work without weakening the parse-validity gate.

## Not yet specified

- Nothing inside the New World Lua 5.1 output goal. Lua 5.2-5.5 are recognized
  releases but intentionally require separate end-to-end target tickets.

## Out of scope

- Reproducing original whitespace or comments; bytecode does not retain them.
- Porting Coldzer0's string concatenation, `TDecompiler` god object, fixed-size
  register sets, or proximity thresholds.
- Re-casing recovered identifiers, localizing globals, or moving effects merely
  for style.
