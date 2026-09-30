# Lock the find entry point and result contract

Status: resolved
Kind: `wayfinder:grilling`
Blocked by: (none — informed by Inventory catalog-to-format dispatch when it lands)

## Question

Where does global find live and what does a hit look like? Decide: new
top-level `find` command vs extending `asset search` vs TUI-first surface;
the result contract (per-format hit vocabulary normalized how far, score
semantics, exact-vs-fuzzy default, JSON/table output); and which formats are
in scope for the first implementation. Conditional answers are fine —
record what changes if the dispatch inventory surprises us.

## Resolution

Decided over two grilling rounds (all recommendations accepted, with Q4
upgraded by the human).

- **Entry point:** new top-level command, named `grep` (`nw-tools grep`).
  `asset search` keeps meaning path hits.
- **Result contract:** one flat row per hit —
  `(pak, entry, format, location, snippet, score)` — with format-native
  locations (`row 12 · column Damage`, `fragment Ability_X`, …) and a trimmed
  matched-text snippet always present. JSON comes from the existing global
  `--format` flag.
- **Match default:** fff-fuzzy, `--exact` opt-out (existing convention).
- **Scope — no phases:** every human-text-searchable format from day one:
  entry paths, datasheet cells + columns, objectstream payloads, catalog
  text, mannequin fragments/tags/controller XML, animation names/events,
  audio string tables, lua decompiled text. Binary metadata (dds/model
  headers) is out; lua raw-bytes fallback on decompile failure is skipped,
  not half-matched.
- **Queries:** repeatable args, OR semantics, matched against names and
  content together, with `--name-only` / `--content-only` scoping flags.
- **Output:** streaming *with* on-the-fly sort/ranking (not append-only).

### Glossary (effort-scoped)

- **hit** — one matched occurrence.
- **location** — a format-specific address: pak + entry + where-inside.
- **score** — the fff value ordering hits.
- **searchable** — a format counts iff it carries human-readable text
  (decoded or raw); pixel/binary blobs never do.
- **snippet** — the trimmed matched decoded text shown per hit.

### Tension handed to the pipeline ticket

Streaming × global ranking is contradictory in the general case — a
perfect rank needs the full set. Ticket 003 must resolve how literal
"on-the-fly ranking" gets (bounded ranked buffer, approximate merge,
re-sorted render ticks, …) rather than assuming both properties fully.
