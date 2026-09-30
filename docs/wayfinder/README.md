# Global format-aware find across whole paks

## Destination

A locked design for one global `find` over whole pak archives that decodes
each entry by its catalog-determined format and fff-matches names plus decoded
content over the jobs pool — entry point, call-stack handoff, TDD seams, and
performance budget all decided, ready to implement test-first. Decisions, not
code; the map is done when nothing is left to decide before implementation.

## Notes

- Domain: `nw-tools` global search. Existing per-format finds stay as they
  are; this effort decides the unified path, not their internals.
- Every session consults the `tdd` and `call-stack-design` skills. Test seams
  are agreed with the human before any test is written (per `tdd`); API shape
  follows the call-stack handoff (per `call-stack-design`).
- Standing preferences: the fff/frizbee matcher is fixed — tune its use, never
  replace it. Parallelism goes through the jobs API (`nw-jobs` `JobRunner`,
  `RunCtx::map_results_compact`), not bespoke worker threads. The content
  index extracts on that same pool (`from_jobs` never opens a second one).
  Reuse existing format decoders; building new decoders is out of scope.
- This map is planning-only.

## Decisions so far

<!-- one line per closed ticket: enough to judge relevance, then zoom the link -->

- [Inventory catalog-to-format dispatch](tickets/001-catalog-format-dispatch.md)
  — dispatch is extension globs + magic bytes, not catalog types
  (`AssetType` is an opaque UUID); reuse `classify_entry`/`path_family`
  lineage, keep the catalog to path/GUID loading.
- [Lock the find entry point and result contract](tickets/002-entry-point-and-result-contract.md)
  — new `nw-tools grep`: flat `(pak, entry, format, location, snippet,
  score)` rows, fuzzy default, every human-text-searchable format, repeatable
  OR queries over names+content, streaming with on-the-fly ranking.
- [Design the search pipeline call stack](tickets/003-search-pipeline-call-stack.md)
  — registry of `Box<dyn SearchAdapter>`; routing never reads bytes;
  live TTY on reused `TableView`; skip-and-count, no aborts, no panics.
  Handoff: [003-search-pipeline-call-stack.handoff.md](tickets/003-search-pipeline-call-stack.handoff.md).
- [Agree the TDD seams](tickets/004-tdd-seams.md)
  — outer lib seam (`grep::run`), adapter port seam (`&dyn SearchAdapter`
  on hand literals), pure merger seam, injected-token cancellation;
  order/membership asserted, never numeric scores.
- [Set the whole-pak performance budget](tickets/005-performance-budget.md)
  — name-only p95 ≤10s; full-content first hits <5s; complete ≤10min
  over the 1.45M-entry local install.
- [Persist a compact relational content index](tickets/006-content-index.md)
  — interned tokens + dual `Crc32` (original and lowercase) + integer
  facts in the drizzle cache; file paths always dual-CRC'd; build
  starts on a side thread beside any command.

## Frontier

Grep implementation is in progress (tickets 001–005). The persisted
index (ticket 006) is the next vertical slice: intern, CRC lookup,
pak build, and parallel start.

## Not yet specified

- How results render and stream (report/table/JSON, incremental output).
- How scores merge across formats with different hit vocabularies.
- Fallback for entries no decoder covers (raw-byte search vs skip vs path-only).
- Whether the TUI picker consumes the same pipeline, and when.
- `--jobs` autotuning and cancellation/progress UX for huge archives.

## Out of scope

- Replacing or reworking the frizbee/fff matcher itself.
- Building new format decoders (adb, animation, audio, …) — reuse only.
- Changing existing per-format `find` commands' behavior.
- Asset extraction, repack, or catalog-write paths.
