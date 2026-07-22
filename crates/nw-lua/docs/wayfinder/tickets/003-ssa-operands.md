# Centralize SSA operand capabilities

Status: resolved

## Question

What small read/write operand capabilities should every analysis and transform
use so SSA references are not enumerated independently in rename, decompile
analysis, inlining, and future optimization passes?

## Acceptance direction

Define consumer-language capabilities for explicit value uses, mutable use
rewrites, definition enumeration, side effects, and control-flow roles. Keep
implicit register windows (calls, varargs, numeric/generic loops) explicit in
the IR rather than reconstructing versions from base-register integers.

## Decision

- `SsaOp::visit_uses` and `rewrite_uses` are the sole read/write operand
  traversal, and attach a `UseRole` to phi, mutation, capture, loop-control,
  and ordinary value uses.
- `SsaNode` owns every versioned definition produced by its instruction.
  `visit_defs` replaces the function-global implicit-definition table, while
  secondary `LOADNIL` definitions no longer require fabricated metadata nodes.
- Numeric and generic loop controls are stored as `LoopControl` containing
  three real `SsaRef` operands, so SSA rename versions the values in place.
- `OpEffects` and `ControlFlowRole` own motion/elimination and structural-flow
  classification for future passes.
- Evaluation-order recovery follows SSA dependency chains from each direct Lua
  operand. This preserves ordering when an effectful setup node feeds a later
  operand without introducing file- or opcode-sequence exceptions.

## Evidence

- Operand unit tests cover roles, mutable rewrites, loop versions,
  multi-register definitions, effects, and control roles.
- Removing `LOADNIL` pseudo nodes exposed and fixed secondary nil lowering.
- The fast 40-file New World corpus remains 100% decompile/recompile clean with
  zero undefined synthetic reads; focused checks cover the nested generic-loop
  setup shapes in `contractsdatahandler.lua` and `genericinvitecommon.lua`.
- `cargo test -p nw-lua` and
  `cargo clippy -p nw-lua --all-targets -- -D warnings` pass.
