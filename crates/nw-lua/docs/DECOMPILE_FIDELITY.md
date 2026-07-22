# nw-lua decompile fidelity status

This report tracks source-vs-decompile fidelity, not just whether emitted Lua
parses or recompiles. The analyzer compares original Lua source against
`nw-lua` output after compiling the original source with Lua 5.1 `luac.exe`.

## Permanent gate

`tests/fidelity_gate.rs` is the permanent regression lock for the high-severity
classes. It shells the existing `nw-lua-fidelity` analyzer over the fixed sorted
corpus roots:

- `E:\Projects\az-rs\resources\fixtures\lua\good-lua`
- `E:\Projects\DEMOJSON`

The default test runs `--limit 80 --examples 0` and skips cleanly if `luac.exe`
or the corpus roots are absent. A heavier `--limit 300` sweep is available as an
ignored test. The gate asserts both file and function hits are zero for:

- `dropped_return`
- `empty_decompiled_branch`
- `bogus_not_number`
- `undefined_synthetic_read`

Validation run:

```text
cargo test -p nw-lua --test fidelity_gate
```

Result: default 80-file gate passed in the final full-suite audit.

## Current 300-file sweep

Command:

```text
cargo run -p nw-lua --bin nw-lua-fidelity -- \
  --luac E:\Projects\lua-5.1.5\src\luac.exe \
  --root E:\Projects\az-rs\resources\fixtures\lua\good-lua \
  --root E:\Projects\DEMOJSON \
  --limit 300 --examples 12
```

Result after typed reconstruction ownership, unified control dependence,
binding-aware idiom passes, and source-shape ranking:

- Files seen: 1296
- Files processed: 300
- Source compile errors: 0
- Decompile errors: 0
- Parse errors: 0
- Divergent files: 275 / 300 (91.67%)
- Divergent functions: 1390 / 4236 (32.81%)

| Category | File hits | File pct | Function hits | Function pct |
| --- | ---: | ---: | ---: | ---: |
| `function_count` | 20 | 6.67% | 74 | 1.75% |
| `dropped_return` | 0 | 0.00% | 0 | 0.00% |
| `statement_count` | 266 | 88.67% | 1091 | 25.76% |
| `assignment_count` | 259 | 86.33% | 758 | 17.89% |
| `assignment_target_mismatch` | 260 | 86.67% | 779 | 18.39% |
| `control_flow_count` | 105 | 35.00% | 221 | 5.22% |
| `empty_decompiled_branch` | 0 | 0.00% | 0 | 0.00% |
| `unnecessary_control_flow` | 44 | 14.67% | 59 | 1.39% |
| `constructor_shape` | 103 | 34.33% | 220 | 5.19% |
| `declaration_sugar` | 21 | 7.00% | 28 | 0.66% |
| `exposed_temporary` | 75 | 25.00% | 140 | 3.31% |
| `short_circuit_loss` | 66 | 22.00% | 88 | 2.08% |
| `short_circuit_gain` | 142 | 47.33% | 271 | 6.40% |
| `bogus_not_number` | 0 | 0.00% | 0 | 0.00% |
| `number_short_circuit` | 146 | 48.67% | 0 | 0.00% |
| `undefined_synthetic_read` | 0 | 0.00% | 0 | 0.00% |

The high-severity classes are all zero on the 300-file sweep and are enforced by
the default gate.

File hits now include files containing a function-level difference. Earlier
reports counted only lexical/file-level categories in that column, so the old
zero file counts for statement, assignment, and control-flow categories were
not meaningful. The expanded source-shape categories also make the aggregate
divergent-file count deliberately stricter; the comparable function-level total
fell from 1,769 to 1,390 while all correctness gates remained zero.

## Source-shape ranking

The analyzer now ranks four directly actionable output-quality signals in
addition to broad statement and assignment drift:

- `unnecessary_control_flow`: the decompile has more structured control nodes
  than the source;
- `constructor_shape`: table-constructor or constructor-field counts differ;
- `declaration_sugar`: local functions, named function declarations, and
  function-value assignments use a different declaration form;
- `exposed_temporary`: the decompile introduces more synthetic local bindings.

These metrics are collected by the existing recursive AST signature traversal;
there is no second child scanner or text-pattern reconstruction path. They are
ranking signals, not proof of incorrect behavior, and direct work toward the
owning reconstruction or binding fact.

## Typed constructor-boundary recovery

Lua 5.1's compiler records table-constructor list and record counts in the B/C
operands of `NEWTABLE` using its floating-byte encoding. `TableSizeHint` now
retains that encoding and reports an exact source field count only across the
injective range. A shared, stack-based `TableConstructorPlan` uses those exact
counts for nested constructors and falls back to the semantic `SETLIST`
boundary for larger/open list constructors.

This replaces separate expression/statement scans and prevents later table
mutations—especially method closure assignments—from being absorbed into an
earlier literal. On `abilitiescommon.lua`, output now reconstructs the original
five-field nested module constructor followed by a separate colon method.

## R3 fixes from residual sampling

R3 sampled residual `short_circuit_loss`, `control_flow_count`, and
`function_count` examples. Three real correctness bugs were found and fixed with
generic repro tests:

- A value-chain parser path dropped comparison operands in
  `a and b == c or d == e`, producing only the first guard. Covered by
  `r3_preserves_boolean_chain_operands`.
- A long type-guarded boolean chain could materialize a PHI from the value arm
  in fallback arms, or leave the fallback as `nil`; visible boolean PHIs now
  initialize from real `LOADBOOL false` operands. Covered by
  `runtime_equivalence_r3_boolean_chain_residuals`.
- A loop branch whose empty arm jumps back to the active while header was emitted
  as `break`, dropping the non-empty sibling body. Natural-loop selection now
  uses the widest loop for a header and continue detection recognizes active
  loop headers. Covered by
  `runtime_equivalence_preserves_loop_if_body_with_empty_continue_arm`.

## Residual classification

- `short_circuit_loss`: remaining sampled cases are structural after the R3
  fixes. Examples include boolean materialization such as `local v = false; if
  guard then v = expr end; return v`, nested `if` expansion of compound
  predicates, or boolean normalization with `or false`. These are noisier than
  the original expression but preserve the sampled behavior.
- `control_flow_count`: sampled residuals are mostly `if` vs `elseif`,
  guard-clause inversion, de-nesting, and expression-to-control-flow lowering.
  The real loop/backedge bug found during sampling was fixed.
- `function_count`: sampled hits are mostly analyzer alignment limitations. For
  example, methods decompiled as table-constructor function fields are complete,
  but the analyzer does not collect function expressions inside table
  constructors, so they appear as missing aligned functions. Over-count examples
  are also dominated by callback/function-expression alignment noise. No
  duplicated or invented function body was confirmed in the bounded sample.
- `statement_count`, `assignment_count`, and `assignment_target_mismatch` remain
  high-volume temp exposure / under-inlining signals. They are useful for
  triage, but are not treated as correctness failures without a concrete dropped
  or wrong effect.
- `number_short_circuit` remains a lexical smell because Lua source often uses
  numeric operands in `x and 1 or 2`-style idioms. `bogus_not_number` is the
  correctness version of that smell and is zero.

## Analyzer methodology

The analyzer:

1. Compiles original source with Lua 5.1 `luac.exe`.
2. Decompiles the resulting bytecode with `nw_lua`.
3. Parses original and decompiled source with `full_moon`.
4. Builds approximate per-function signatures.
5. Compares function/proto counts, statement counts, return counts, assignment
   counts and targets, control shape, constructor shape, declaration form,
   synthetic temporary exposure, and lexical smells.

Function alignment is best effort: exact function name first, then
source/decompile order. The analyzer is a deterministic divergence signal, not a
semantic proof; residual categories require sampling before they are treated as
decompiler bugs.
