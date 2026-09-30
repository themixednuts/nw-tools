# Design the search pipeline call stack

Status: resolved
Kind: `wayfinder:grilling`
Blocked by: [Inventory catalog-to-format dispatch](001-catalog-format-dispatch.md), [Lock the find entry point and result contract](002-entry-point-and-result-contract.md)

## Question

What is the typed entrypoint-to-effect call stack for global find, per the
`call-stack-design` skill? Produce the handoff: entrypoint, decode-dispatch
ownership, jobs-pool fan-out shape, per-format adapter seam (what typed value
crosses it, who owns decode failure), result merge and ranking, cancellation
and error paths, and file/module ownership. Compare at least two ownership
shapes where the seam is genuinely open (e.g. central dispatcher vs
per-format adapters behind a port).

## Resolution

Approved as drafted in [003-search-pipeline-call-stack.handoff.md](003-search-pipeline-call-stack.handoff.md):
registry of `Box<dyn SearchAdapter>` behind a port (predicate + decode +
extract per adapter, name matching as a `Tier::Name` adapter); routing never
reads bytes; matcher-per-worker via a thin `RunCtx` addition; live TTY on
reused `TableView::streaming` with zero-padded score sort + exact two-tier
settle pass; skip-and-count with no abort paths; **no panics** (adapters
report skips as values, never panic on untrusted data — this dissolved the
worker-panic open question instead of answering it).

## Direction (grilling rounds 1–2, locked)

- Ownership: registry of per-format adapters behind a port trait;
  `Box<dyn>` is fine. Adding a format never touches shared code.
- Failure: skip-and-count everywhere, no abort paths at all; everything
  funnels to the issues summary (entry-level and pak-level alike).
- Renderer: ratatui live ranked view on TTY reusing
  `tui::browse`/`TableView::streaming` + `RowFeed` (merger posts rank-ordered
  snapshots); piped output streams arrival-ordered with the ranked set.
  Reuse `RunCtx` progress, `CancellationToken`, existing signal handling.
- Volume: soft per-entry hit cap with Enter-to-expand (constrains the live
  view: needs an expand affordance, whether native to `TableView` or bespoke).
- Ordering: picker convention — name tier above content tier, score-desc
  within tiers (scores aren't calibrated across formats).
