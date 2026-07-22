# Rank source-shape fidelity with AST facts

Status: resolved

## Question

Which deterministic corpus metrics make remaining output-shape work rankable
without adding text heuristics or a second source traversal?

## Acceptance direction

Extend the existing parsed-AST function signature with constructor shape,
declaration form, synthetic temporary exposure, and unnecessary control-flow
facts. Report both distinct file hits and aligned-function hits. Treat the
categories as triage signals rather than semantic proof, and keep the permanent
high-severity correctness gate unchanged.

## Evidence

The existing recursive `full_moon` AST signature traversal now counts table
constructors and fields, local and named function declarations,
function-valued assignments, and synthetic local targets. `compare_metrics`
derives four closed categories from those facts: `unnecessary_control_flow`,
`constructor_shape`, `declaration_sugar`, and `exposed_temporary`.

The report aggregates a category once per containing file even when several
functions hit it. This also fixes the former misleading zero file count for all
function-level metrics. A focused AST-signature test covers constructors,
declaration forms, and temporaries.

The deterministic 300-source oracle sweep processed every selected source with
zero compile, decompile, or parse errors. Across 4,236 aligned source functions:

- unnecessary control flow: 44 files / 59 functions (1.39% of functions);
- constructor shape: 103 files / 220 functions (5.19%);
- declaration sugar: 21 files / 28 functions (0.66%);
- exposed temporary: 75 files / 140 functions (3.31%).

All high-severity categories remained zero. `docs/DECOMPILE_FIDELITY.md` records
the complete command, table, methodology, and interpretation boundary.
