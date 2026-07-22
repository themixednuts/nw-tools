# nw-lua performance and Coldzer0 comparison

This report compares `nw-lua` with
[`Coldzer0/LuaDecompiler`](https://github.com/Coldzer0/LuaDecompiler) using the
same Lua 5.1 chunks. It distinguishes single-file latency from multi-file
throughput and keeps output validity beside speed; emitting invalid pseudo-Lua
faster is not a useful decompiler result.

## Compared builds

- Machine: Windows, AMD Ryzen 9 5900X (12 cores / 24 logical processors).
- `nw-lua`: Rust 1.97.1, portable `cargo build --release`.
- Coldzer0: official optimized Windows x86-64 v1.0.0 release.
- Coldzer0 source: `e75c48a73008187e88cc5a50a2dd06b884247923`.
  Commits after the v1.0.0 tag change only README/examples, so the release and
  current `main` use the same Pascal implementation.
- Date: 2026-07-22.

The five complex-file latency sample alternated tools after warm-up and
captured complete stdout. The 300-file sample compiled the same fixed, sorted
New World source roots with Lua 5.1 and wrote every result to disk.

## Single-file latency

Ten optimized CLI runs per file produced these means:

| Lua 5.1 chunk | Coldzer0 | nw-lua |
| --- | ---: | ---: |
| `options` | 260.69 ms | 422.04 ms |
| `landingscreenv2` | 124.64 ms | 338.53 ms |
| `loadingscreenmanager` | 400.68 ms | 336.23 ms |
| `uistyle` | 484.58 ms | 276.21 ms |
| `dungeonenterscreen` | 109.69 ms | 252.41 ms |
| **Mean** | **276.06 ms** | **325.09 ms** |

Coldzer0 is about 15% faster on the aggregate single-file latency sample.
Results vary by control-flow shape: `nw-lua` is faster on two of the five most
complex chunks and slower on three. `nw-lua` also performs idiomatic AST cleanup,
StyLua formatting, and a final Lua parse-validity check in this path.

The then-current ThinLTO `dist` profile narrowed the aggregate gap in a
separate affinity-pinned paired run: 302.24 ms/file for `nw-lua` versus 271.07
ms/file for Coldzer0, or about 11.5%.

## Parallel corpus throughput

Coldzer0 accepts one input per process. Its parallel figures therefore use up
to N independent official CLI processes. `nw-lua` uses one process and its
bounded, ordered `nw_jobs::JobRunner` batch path.

| Tool | Workers | 300 files | Throughput |
| --- | ---: | ---: | ---: |
| Coldzer0 | 1 | 29.63 s | 10.12 files/s |
| Coldzer0 | 8 | 7.65 s | 39.24 files/s |
| nw-lua | 1 | 6.52 s | 46.02 files/s |
| nw-lua | 8 | 1.33 s | 225.11 files/s |

The batch path gives `nw-lua` 4.89x scaling from one to eight workers and 5.74x
the measured eight-process Coldzer0 throughput. Results are collected in input
order and output-name collisions are rejected before work begins.

Automatic worker selection is capped at eight. On the measurement machine, 24
workers caused memory pressure and reduced the 24-file test from about 90
files/s to under 5 files/s. More threads are not automatically more throughput;
`--jobs` remains available for workload-specific tuning.

## Output validity

Every output was checked with Lua 5.1 `luac -p`:

| Tool | Valid outputs | Invalid outputs |
| --- | ---: | ---: |
| nw-lua | 300 | 0 |
| Coldzer0 | 269 | 31 |

This is a syntax/recompile check, not a claim that recompilation alone proves
semantic equivalence. `nw-lua` additionally retains its runtime, structural,
and high-severity fidelity gates.

## Measured optimization

The existing allocation/stage benchmark showed that chunk parsing and SSA
construction were below 1% of total time; source emission dominated:

| Stage | Before | After | Change |
| --- | ---: | ---: | ---: |
| Full decompile | 2,232 us/file | 1,601 us/file | -28.3% |
| Source emission | 1,691 us/file | 1,141 us/file | -32.5% |

The old path lowered into a Full Moon AST, serialized it, made StyLua parse the
serialization back into the same AST type, formatted it, then ran the final
validity parse. The optimized path materializes token positions and passes the
typed AST directly to StyLua. The final parse gate remains, and all five complex
outputs are byte-identical to the pre-optimization output.

This time improvement currently trades for more allocator traffic in the
microbenchmark (about 2.10 MB/file versus 1.65 MB/file) because Full Moon's
position materialization rewrites tokens. Avoiding that traffic cleanly would
require upstream support for positioned programmatic AST construction; it is a
remaining target, not a reason to add an unsafe local token representation.

## Memory and SIMD decision

On the five complex chunks, mean peak working set was 43.7 MiB for `nw-lua`
(56.0 MiB maximum) and 9.2 MiB for Coldzer0 (10.7 MiB maximum). Most of the gap
comes from the structured Full Moon/StyLua trees and the final validation parse.
This is why batch concurrency is bounded.

SSA reconstruction now ends at an explicit compiler-stage boundary before
source emission starts. A paired complex-file run reduced mean peak working set
from 40.46 to 40.26 MiB and maximum peak from 56.57 to 56.41 MiB. The small
change confirms that SSA retention was not the main memory cost: Full Moon
position materialization, StyLua's formatted tree, and final validation remain
the targets. The tracked follow-up is
[`wayfinder/tickets/012-emission-memory.md`](wayfinder/tickets/012-emission-memory.md).

A one-codegen-unit, `target-cpu=native` build improved the same complex sample
by about 4% (307.24 to 294.94 ms/file in the paired run). Hand-written
SIMD is not justified: byte parsing is about 0.1% of runtime, while recursive AST
formatting is the dominant work and has no useful contiguous numeric kernel.
The workspace provides a portable optimized distribution profile instead. A
five-round paired run over the 300-file corpus selected fat LTO over ThinLTO:
fat was about 0.7% faster with one worker, 2.9% faster with eight, and produced
a 3.7% smaller executable. Its clean build was about 41% slower, which is an
acceptable distribution-build tradeoff rather than a development-profile cost.

```powershell
cargo build -p nw-lua --bin nw-lua --profile dist
```

Use `RUSTFLAGS="-C target-cpu=native"` only for a machine-local build where the
portability tradeoff is intentional.

## Reproduce the in-process stage benchmark

```powershell
$env:NW_LUA_BENCH_ITERS = 200
cargo bench -p nw-lua --bench decompile_hot_path
```

The benchmark reports time, allocation count, allocated bytes, reallocations,
and reallocated bytes for parse, SSA, core/idiomatic AST reconstruction, source
emission, and the complete pipeline.
