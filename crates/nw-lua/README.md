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
  --mode <dis|dec|ssa>  rendering operation (default: dec)
  --annotate            prepend disassembly as Lua comments during decompilation
  --no-idiomatic        skip idiomatic AST cleanup during decompilation
  --lua-version <VER>   override the detected complete compiler target (currently 51)
  --opcode-table <F>    load a custom opcode-table mapping from file F
  --debug               emit debug trace to stderr
  -j, --jobs <N>        maximum parallel workers for multiple input files
  -o, --out <FILE>      write one input's rendered result to FILE
      --out-dir <DIR>   write multiple inputs' rendered results beneath DIR
      --force           replace existing output files
      --dry-run         read and render inputs without writing output files
  -h, --help            print help
  -V, --version         print version
```

Examples:

```powershell
cargo run -p nw-lua --bin nw-lua -- --mode dec crates/nw-lua/tests/fixtures/shopcommon.luac
cargo run -p nw-lua --bin nw-lua -- --mode dis --opcode-table crates/nw-lua/tests/fixtures/idle_heroes.txt crates/nw-lua/tests/fixtures/shopcommon.luac
cargo run -p nw-lua --bin nw-lua -- --mode ssa crates/nw-lua/tests/fixtures/shopcommon.luac -o shopcommon.ssa.txt
cargo run -p nw-lua --bin nw-lua -- --jobs 8 --out-dir decompiled one.luac two.luac three.luac
```

Multiple inputs are processed in parallel with deterministic names under the
output directory; failures are reported in input order. Automatic parallelism is
bounded at eight workers because source formatting is memory-intensive; use
`--jobs` to tune a known workload. See [docs/PERFORMANCE.md](docs/PERFORMANCE.md)
for the Coldzer0 comparison and reproducible measurements.
