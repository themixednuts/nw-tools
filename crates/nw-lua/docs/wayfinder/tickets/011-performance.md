# Measure and optimize the decompiler

Status: resolved

## Question

How does `nw-lua` compare with Coldzer0 on latency, throughput, memory, and
output validity, and which optimizations are justified by measured hot paths?

## Acceptance direction

Benchmark optimized builds on identical Lua 5.1 chunks. Separate single-file
latency from corpus throughput, preserve deterministic output and every
validity/fidelity gate, and measure before introducing concurrency, native CPU
requirements, SIMD, unsafe code, or specialized storage.

## Evidence

The stage benchmark identified source emission as roughly 76% of the optimized
small-file pipeline; chunk parsing and SSA construction together were below 1%.
Emission no longer serializes the completed Full Moon AST merely for StyLua to
parse it back into the same type. It materializes token positions and passes the
AST directly to StyLua while retaining the final parse-validity gate. This cut
the measured full pipeline from 2,232 to 1,601 microseconds per file and source
emission from 1,691 to 1,141 microseconds per file. The five complex outputs are
byte-identical before and after the change.

The CLI now accepts multiple inputs with deterministic output names and bounded
`nw_jobs::JobRunner` workers. Results and failures are collected in input order,
duplicate output names are rejected before work begins, and `--jobs` makes
scheduling an explicit user capability. Automatic selection is capped at eight:
eight workers reached about 90 files/s on the 24-file scaling sample, while 24
workers caused memory pressure and fell below 5 files/s.

On the fixed 300-file comparison corpus, `nw-lua` processed 46.02 files/s with
one worker and 225.11 files/s with eight (4.89x scaling). Coldzer0 accepts one
input per process; one and eight externally scheduled processes measured 10.12
and 39.24 files/s. Lua 5.1 recompilation accepted 300/300 `nw-lua` outputs and
269/300 Coldzer0 outputs.

Coldzer0 retains the lower memory footprint: its five-complex-file mean peak
working set was 9.2 MiB versus 43.7 MiB for `nw-lua`. Direct AST formatting also
trades higher allocator traffic for lower latency because Full Moon position
materialization rewrites tokens. `docs/PERFORMANCE.md` records this remaining
cost rather than hiding it.

Native CPU targeting improved the complex sample by only about 4%.
No hand-written SIMD or unsafe path was added because byte parsing is about 0.1%
of runtime and recursive AST formatting has no suitable contiguous numeric
kernel. A portable `dist` profile supplies fat LTO, one codegen unit, and symbol
stripping without requiring the build machine's ISA. Fat LTO was selected over
ThinLTO after a five-round paired 300-file benchmark: it was faster at one and
eight workers and produced a smaller executable, at the expected cost of a
slower distribution build.

Final verification passes formatting and diff hygiene, Clippy with warnings
denied, the complete all-target suite, 300/300 structural decompile/recompile
with unchanged 83.33% exact prototypes and 82.18% matching opcodes, and the
300-file zero-high-severity fidelity gate.
