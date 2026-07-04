# nw-lua

Lua 5.1 bytecode parsing, disassembly, SSA dumping, and decompilation for New World tools.
Lua 5.2 through 5.5 are recognized by the roadmap but are not supported until P11.

## CLI

Build the command:

```powershell
cargo build -p nw-lua
```

Usage:

```text
nw-lua [options] <file.luac>
  --dis                 disassemble bytecode
  --dec                 decompile to Lua source (default)
  --ssa-dump            dump SSA IR for all protos
  --annotate            prepend disassembly as Lua comments during decompilation
  --lua-version <VER>   override detected version: 51|52|53|54|55 (only 51 supported now)
  --opcode-table <F>    load a custom opcode-table mapping from file F
  --debug               emit debug trace to stderr
  -o, --output <F>      write to file F instead of stdout
  -h, --help            print help
  -V, --version         print version
```

Examples:

```powershell
cargo run -p nw-lua --bin nw-lua -- --dec crates/nw-lua/tests/fixtures/shopcommon.luac
cargo run -p nw-lua --bin nw-lua -- --dis --opcode-table crates/nw-lua/tests/fixtures/idle_heroes.txt crates/nw-lua/tests/fixtures/shopcommon.luac
cargo run -p nw-lua --bin nw-lua -- --ssa-dump crates/nw-lua/tests/fixtures/shopcommon.luac -o shopcommon.ssa.txt
```
