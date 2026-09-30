# Set the whole-pak performance budget

Status: resolved
Kind: `wayfinder:grilling`
Blocked by: [Lock the find entry point and result contract](002-entry-point-and-result-contract.md)

## Question

What does "performant whole-pak fff search" mean measurably? Decide the
reference corpus (which install/paks, approximate entry count), the latency
budget (p50/p95 for name-only vs full-content search), batch vs streaming
result expectations, `--jobs` scaling behavior, and what gets measured to
prove it. The budget must be checkable in CI or by a stated manual procedure.

## Resolution

Decided over two grilling rounds (Q5 clarified Q2).

- **Reference corpus:** the full local install — 72G, 130 paks,
  **1,446,650 entries** — on this rig. CI never sees game data; it gates on
  a small synthetic pak corpus instead.
- **Datum (dev profile, this rig):** full classify sweep 44s; fuzzy name
  search over all 1.45M names 6.8s / 67,876 matches. Heaviest content:
  `.dynamicslice` 248k entries / 8.35GB unpacked, `.meta` 317k entries.
- **Budgets (dev profile, reference corpus, default `--jobs`):** name-only
  p95 ≤10s; full-content first hits streaming <5s; full completion ≤10min.
  Release builds re-baselined before done (≈2–4× faster expected).
- **No-skipping rule:** budgets hold over *complete* searches. The safety
  caps stay exactly as designed — `--max-entry-size` and the soft per-entry
  hit cap with overflow counts — with full counting; nothing is dropped to
  meet a number.
- **Scaling:** near-linear to physical cores on the decode-bound middle,
  flattening at merge/render. CI asserts strictly-faster (2 vs 1 worker) and
  nothing tighter; the real curve is measured once on the reference rig.
- **Proof:** CI synthetic gate (exact counts, deterministic order, zero
  panics, skip accounting balances) + manual reference run with numbers
  pasted here at implementation time (datum above is the pre-implementation
  baseline). No bench harness until someone asks twice.

## Reference run (2026-09-14)

Release `time_index`, 24 threads, 130 paks, dedicated
`%LOCALAPPDATA%\nw-tools\index-bench.sqlite`. This run started before leftover
XML became a complete extract (ADR-0001); leftover `.xml` was one `TextLine`
fact per non-empty line.

| step | result |
|---|---|
| cold | Complete 130 paks in **8740.62s** (~2.4 h) |
| lookup `ironsword` | **0 hits in 1.55ms** (CRC miss path is fast; a miss on this corpus is wrong) |
| search `kickout` | 378 hits in **24.33s** |
| db | **17.04 GB** (prior cold without leftover XML lines: 507s / 861 MB) |
| resume | Complete 130 paks in **7.16s** |

Over the 10-minute complete budget. Resume is fine. Re-run after the complete-XML extract before treating these as the release baseline.
