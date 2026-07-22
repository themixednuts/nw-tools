# nw-lua

Lua 5.1 bytecode parsing, disassembly, SSA dumping, and decompilation for New World tools.
Lua 5.2 through 5.5 release tags are recognized at the input boundary, but only
the complete Lua 5.1 target may enter the compiler pipeline.

## CLI

Build the command:

```powershell
cargo build -p nw-lua
```

Usage:

```text
nw-lua [options] <file.luac>...
  --dis                 disassemble bytecode
  --dec                 decompile to Lua source (default)
  --ssa-dump            dump SSA IR for all protos
  --annotate            prepend disassembly as Lua comments during decompilation
  --no-idiomatic        skip idiomatic AST cleanup during decompilation
  --lua-version <VER>   override the detected complete compiler target (currently 51)
  --opcode-table <F>    load a custom opcode-table mapping from file F
  --debug               emit debug trace to stderr
  -j, --jobs <N>        maximum parallel workers for multiple input files
  -o, --output <F>      write one result to F, or multiple results under directory F
  -h, --help            print help
  -V, --version         print version
```

Examples:

```powershell
cargo run -p nw-lua --bin nw-lua -- --dec crates/nw-lua/tests/fixtures/shopcommon.luac
cargo run -p nw-lua --bin nw-lua -- --dis --opcode-table crates/nw-lua/tests/fixtures/idle_heroes.txt crates/nw-lua/tests/fixtures/shopcommon.luac
cargo run -p nw-lua --bin nw-lua -- --ssa-dump crates/nw-lua/tests/fixtures/shopcommon.luac -o shopcommon.ssa.txt
cargo run -p nw-lua --bin nw-lua -- --jobs 8 --output decompiled one.luac two.luac three.luac
```

Multiple inputs are processed in parallel with deterministic names under the
output directory; failures are reported in input order. Automatic parallelism is
bounded at eight workers because source formatting is memory-intensive; use
`--jobs` to tune a known workload. See [docs/PERFORMANCE.md](docs/PERFORMANCE.md)
for the Coldzer0 comparison and reproducible measurements.
