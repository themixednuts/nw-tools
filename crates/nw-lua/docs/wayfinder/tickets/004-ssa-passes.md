# Introduce a composable SSA pass pipeline

Status: resolved

## Question

How should `ir/passes.rs` become a real, testable pass pipeline aligned with
Coldzer0's trivial-phi, copy-propagation, constant-folding,
strength-reduction, and dead-code passes without losing debug bindings or
control-flow facts?

## Acceptance direction

Use a small pass trait returning a typed change result, explicit analysis
invalidation/preservation, deterministic fixpoint bounds, and a pipeline owned
by SSA construction. Port semantics, not Coldzer0's global change flag. First
prove each pass against focused IR tests and output equivalence before enabling
it in the default decompile pipeline.

## Decision

`PassPipeline` owns an ordered set of heterogeneous `SsaPass` implementations.
Each invocation returns `PassChange`, including the analyses it preserves, and
`PassContext` lazily caches value-use and definition-position facts. A pass can
run once or to an explicit, bounded fixpoint; `PipelineReport` records changes,
iterations, and exhausted bounds deterministically.

SSA construction preserves the compiler-shaped IR exactly. Constant folding,
trivial-phi elimination, copy propagation, and effect-aware dead-code
elimination are available through the opt-in cleanup pipeline. A full-suite
audit proved that even semantically safe constant folding can erase the
expression provenance needed to reconstruct debug locals (`a + b`, `x * x`),
so it does not run before source reconstruction. All transforms consume the
operand/effect capabilities from ticket 003.

Coldzer0's pass ordering and fixpoint behavior remain the algorithm reference,
but its algebraic strength reductions are not portable as written. For example,
`x * 0 -> 0` can suppress `__mul`, while `not not x -> x` changes a boolean into
the original value. Those rewrites require type and metamethod proofs that this
IR does not yet possess, so they are deliberately absent rather than patched
with opcode-specific exceptions.

## Evidence

- Focused IR tests cover constants reached through SSA definitions, non-finite
  results, sequential copy chains, self-edge trivial phis, effectful unused
  operations, and bounded non-convergence.
- Phase 4 shape tests prove the construction boundary retains debug-local
  initializer expressions while bytecode constants still reconstruct directly.
- The 40-file New World fast corpus remains 40/40 decompile/recompile clean with
  zero undefined synthetic values after enabling the construction pipeline.
- Structural diagnostics improved to 314/449 exact prototypes (69.93%) and
  4673/6524 matching opcodes (71.63%) on that sample.
- `cargo test -p nw-lua` and Clippy are the closure gates for this decision.
