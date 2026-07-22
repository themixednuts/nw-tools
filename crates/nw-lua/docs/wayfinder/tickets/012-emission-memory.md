# Reduce source-emission memory

Status: active

## Question

How can `nw-lua` reduce per-file and parallel peak memory without private token
layouts, unsafe representation tricks, New World-specific thresholds, or
weakening output validation?

## Acceptance direction

Measure peak live bytes at compiler-stage boundaries on the fixture set and the
five complex chunks. Preserve byte-identical formatted output, Lua 5.1 parse
validity, structural fidelity, deterministic batch results, and the public
decompiler API. Prefer changes at the abstraction that owns each allocation.

## Evidence

The measured five-file maximum working set is about 56 MiB for `nw-lua` versus
about 11 MiB for Coldzer0. Ending the SSA lifetime before source emission moved
mean peak working set only from 40.46 to 40.26 MiB and maximum peak from 56.57
to 56.41 MiB. The existing allocation benchmark also shows that direct typed
AST formatting allocates about 2.10 MB/file versus 1.65 MB/file for the former
serialize/reparse path because Full Moon's `update_positions()` rebuilds token
references and token payloads.

## Ordered work

1. Extend the stage benchmark with current and peak live-byte accounting so
   improvements target retained data rather than allocation traffic alone.
2. Pursue an upstream Full Moon position-update capability that mutates owned
   tokens in place. Its current public token fields prevent a clean downstream
   implementation; do not mirror the private representation or use unsafe.
3. If formatter trees still dominate, prototype a composable pretty-document
   renderer from the compact Lua AST directly into one output buffer, retaining
   the final independent Lua parse gate. Compare it against StyLua for syntax,
   byte stability, latency, and peak memory before adopting it.
4. Profile repeated identifier and literal storage. Introduce per-file symbol
   IDs, interning, or an arena only if retained-byte measurements justify the
   additional lifetime model.
5. Once per-file scratch cost is measurable, add a generic weighted resource
   budget to `nw-jobs` and let batch work consume permits by estimated cost.
   Worker count remains a CPU limit; the resource budget becomes the separate
   memory limit.

## Rejected shortcuts

- Skipping the final parse-validity gate.
- Maintaining a private clone of Full Moon's token representation.
- Selecting a global allocator before profiling fragmentation and live data.
- Treating input byte length or a New World filename as an unvalidated memory
  estimate.
