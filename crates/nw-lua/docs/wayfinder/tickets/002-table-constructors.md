# Recover constructor boundaries from bytecode facts

Status: resolved

## Question

How should table-constructor recovery distinguish fields belonging to the
literal from later mutations without proximity limits or output cleanup?

## Decision

Represent Lua 5.1's `NEWTABLE` B/C operands as typed floating-byte size hints.
When the compiler encoding identifies an exact source field count, a pure
constructor analysis scans the SSA node stream until that many list and record
fields have been observed. Nested constructors are handled as stack/register
windows and the completed plan is shared by expression inlining and statement
reconstruction.

If a count is not exact or the stream violates constructor invariants, decline
the richer shape and retain ordinary assignments. Do not use instruction-gap
thresholds. In particular, a following `CLOSURE`/`SETTABLE` method assignment is
not absorbed once the original literal's exact record-field count is complete.

## Verification

- Unit-test floating-byte hint exactness and decoding.
- Runtime-compile a nested hash-only module constructor followed by a method.
- Require one `local M = { ... }` declaration, nested constructor fields, and a
  separate idiomatic method declaration.
- Run all `nw-lua` tests, Clippy, and the fixed fidelity gate.

## Result

- Added `TableSizeHint`, preserving `NEWTABLE`'s encoded floating-byte facts and
  exposing exact counts only through the injective range.
- Added one `TableConstructorPlan` shared by expression and statement recovery.
  Exact nested constructors use a register-frame stack; open/large list
  constructors use the semantic `SETLIST` boundary.
- Removed the duplicate constructor scanners from `ExprBuilder` and
  `table_list`; the implementation is a net code reduction.
- Added runtime-equivalence coverage for a nested module literal followed by a
  method declaration and strengthened the CLI `shopcommon` shape assertion.
- `cargo test -p nw-lua` and `cargo clippy -p nw-lua --all-targets -- -D warnings`
  pass.
- The 300-file fidelity sweep has zero compile/decompile/parse failures and zero
  high-severity hits. Divergent files fell from 279 to 263; `function_count`
  file hits fell from 17 to 9.
- The separate ignored 300-file structural stress sweep recompiles 299/300
  files and exposed a separate function-wide materialization limit tracked
  by [Replace mutable emission bookkeeping with a reconstruction plan](005-reconstruction-plan.md).
