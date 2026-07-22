# Complete the compiler-pipeline audit

Status: resolved

## Question

Does the final implementation satisfy the architectural, fidelity,
maintainability, and validation gates as one coherent Lua 5.1 decompiler?

## Acceptance direction

Keep every prior ticket resolved, remove stale frontier work, verify all Rust
sources stay below the project file-size cap, run formatting and diff hygiene,
compile every target, pass Clippy with warnings denied, pass the complete
standard test matrix, and rerun both heavy corpus measurements on the final
build.

## Evidence

- Every Rust source file is below 900 lines. Region-tree projection was split
  from reconstruction ownership into `reconstruction/regions.rs` rather than
  leaving a new oversized coordinator.
- `cargo fmt -p nw-lua -- --check` and `git diff --check -- crates/nw-lua`
  pass.
- `cargo check -p nw-lua --all-targets` passes.
- `cargo clippy -p nw-lua --all-targets -- -D warnings` passes.
- `cargo test -p nw-lua --all-targets -j 1` passes the final post-split unit,
  integration, runtime-equivalence, CLI, corpus, Lua 5.1 specification, and
  benchmark-target matrix. Only the two deliberately ignored heavy tests are
  skipped by the standard command.
- The explicitly run structural heavy test is 300/300 decompile/recompile
  clean, with 4,223/5,068 exact prototypes (83.33%) and 59,318/72,183 matching
  opcodes (82.18%).
- The final 300-source oracle ranking has zero source compile, decompile, or
  parse errors; all four high-severity categories are zero; and 1,390/4,236
  aligned functions (32.81%) retain at least one measured source-shape
  difference.

The audit also caught two provenance issues that narrow gates had hidden:
default SSA folding erased debug-local expressions, and a later assignment
could be regrouped after a dominating nil declaration. The former was removed
from the reconstruction path; the latter now uses exact binding identity plus
the shared dominator analysis. Both have focused regressions.
