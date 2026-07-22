# Separate recognized releases from complete compiler targets

Status: resolved

## Question

How can later Lua releases remain recognizable at the input boundary without
letting a partially implemented version enter chunk decoding, SSA, or source
reconstruction?

## Acceptance direction

Represent recognized release tags separately from complete parse-to-source
compiler targets. Require the target capability at every internal versioned
boundary. Adding a target variant must make version-sensitive compiler matches
non-exhaustive until that release is implemented end-to-end. Remove scattered
support checks and duplicated version labels.

## Evidence

`LuaVersion` recognizes and labels Lua 5.1 through 5.5 input tags.
`LuaTarget` is the narrower proof that the complete pipeline supports a release;
today it has only `V51`. Header parsing performs the single release-to-target
conversion. `Header`, `Proto`, `OpcodeTable`, and `SsaFunction` then carry only
`LuaTarget`, so unsupported releases cannot become valid compiler IR.

Built-in opcode selection is now infallible for a supported target. The CLI and
custom opcode-table loader resolve the same target capability, and three local
copies of version-label matching plus scattered `V51` support checks were
removed. Version-sensitive opt-in constant folding matches `LuaTarget`
exhaustively, so adding the next complete target produces compiler-directed
work rather than a silent fallback.

Unit tests prove that recognized 5.2-5.5 releases are not complete targets and
that release/target labels have one owner. Chunk, opcode-table, CLI, and
all-target compilation tests pass; `--lua-version 54` remains a clean,
non-panicking rejection. Later chunk formats and language semantics remain an
explicit future expansion, outside the New World Lua 5.1 output goal.
