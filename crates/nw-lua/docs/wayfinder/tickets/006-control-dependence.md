# Unify condition and short-circuit value recovery

Status: resolved

## Question

How should one control-dependence representation recover both branch conditions
and value-producing short-circuit expressions without separate condition-chain,
value-select, and region exception paths?

## Acceptance direction

Build one typed control-dependence graph/forest from CFG, dominance, post-
dominance, branch polarity, and phi sources. Classify each recovered component
by consumer capability (control condition, value expression, or statement
region), not by a second pattern recognizer. Region structuring and expression
reconstruction must query the same component and agree on block/value ownership.

Remove duplicated condition/value scans and the empty/dead control regions they
produce. Preserve Lua 5.1 evaluation order and loop-carried values. Add focused
runtime and output-shape tests for nested `and`/`or`, short-circuit returns,
assignments, loop conditions, and branch-local effects, then keep both corpus
gates clean.

## Starting evidence

`BooleanAnalysis` currently stores `condition_chains` and `value_plans`
separately, while region assembly/lowering contains additional tests for phi
coverage and value-only branches. For example, an inline return expression can
still leave an empty `if creatorMemberBlob then end` region because expression
ownership and statement-region ownership are decided independently.

## Evidence

`BooleanAnalysis` now owns one `Vec<ControlComponent>`, where the closed
consumer capability is either `Condition(ConditionChain)` or
`Value(ValuePlan)`. Start-block and PHI indexes are projections rebuilt from
that authoritative component set; region assembly, expression reconstruction,
and PHI consumers query those projections rather than maintaining parallel
facts.

Short-circuit value recognition runs exactly once per basic block. Condition
recovery receives a narrow immutable `ConditionContext` containing CFG,
expression, loop-header, and precomputed value-start capabilities, so recursive
condition discovery no longer reruns every value recognizer.

The analyzer composes a condition prefix and nested value select into
`ValuePlanKind::Guarded` only after proving that both share a merge/PHI and the
condition's failure edge supplies Boolean `false` to that PHI. The composed
value owns the complete block range, and redundant suffix condition components
are removed before indexing. This recovers, as one expression, chains such as
`type(v) == "number" and ... and (v ~= 0 or fallback(v))` without leaving an
empty statement region.

Region-only continuation behavior is classified before mutation through one
closed `BranchRegionPlan` (`LoopBreak`, `LoopContinue`, `TerminalGuard`, or
`FinalEmptySibling`). One lowering path owns block consumption and one-arm `if`
construction; the former sequence of recognition-and-emission exception
methods was removed.

Validation after the ownership cut:

- Phase 5 control-flow suite: 9/9 passed;
- Phase 6 Boolean suite: 11/11 passed, including a new full guarded-prefix
  output-shape regression;
- Phase 7 multi-value suite: 4/4 passed;
- Phase 10b hardening runtime suite: passed;
- fast corpus: 40/40 core and idiomatic decompile/recompile clean, with 341/449
  exact prototypes and 5,076/6,146 matching opcodes;
- heavy corpus: 300/300 core and idiomatic decompile/recompile clean, with
  4,203/5,069 exact prototypes (82.92%) and 59,103/72,236 matching opcodes
  (81.82%).
