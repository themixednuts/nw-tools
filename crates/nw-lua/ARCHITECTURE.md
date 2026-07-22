# nw-lua — Lua bytecode disassembler & decompiler

A version-aware Lua bytecode (`.luac`) disassembler and SSA-based decompiler,
derived algorithmically from
[`Coldzer0/LuaDecompiler`](https://github.com/Coldzer0/LuaDecompiler) at commit
`e75c48a73008187e88cc5a50a2dd06b884247923` (the local fixed clone is
`E:\Projects\LuaDecompiler`). The Free Pascal reference is AGPL-3.0; see the
licensing note below.

**Complete target: Lua 5.1** (New World / Lumberyard-O3DE ScriptContext ships PUC-Rio
Lua 5.1 `lundump` bytecode). `LuaVersion` recognizes 5.1–5.5 only at the input
boundary. `LuaTarget` admits a release to chunk decoding, SSA, and decompilation
only after that entire pipeline is implemented; today it contains only 5.1.

> **Licensing:** The upstream reference is AGPL-3.0. The repository owner has chosen to
> keep `nw-lua` under the workspace's MIT license anyway. Do **not** add AGPL headers.
> This is a deliberate decision by the owner.

---

## 1. Design principles (how this port differs from the reference)

The reference is a faithful, working decompiler but it is **string-oriented**: expressions
are cached as `AnsiString` (`TExprMap = array of AnsiString`) and statements are produced
by `EmitLine('local ' + LHS + ' = ' + Expr)`. There is no structured output.

This port keeps the reference's *algorithms* (chunk format, opcode semantics, SSA
construction, control-flow reconstruction heuristics) but replaces the string-building
back end with a real **compiler pipeline**:

```
bytes
  │  chunk::parse            version-aware lundump reader            [LuaChunk.pas]
  ▼
Chunk { header, root: Proto (tree of nested Protos) }
  │  bytecode::decode        raw u32 → Instruction via OpcodeTable   [LuaOpcodes.pas + LuaDis.pas]
  ▼
disasm::disassemble  ──►  textual disassembly (--dis)                [LuaDis.pas]
  │  ir::lift + ir::ssa      CFG → dominators → φ → rename           [LuaSSA.pas / LuaSSAPasses.pas]
  ▼
SsaFunction { blocks, nodes, φ }  ──►  ssa dump (--ssa-dump)
  │  analyses + structuring  SSA → ControlComponent set + RegionTree + ReconstructionPlan
  ▼
ControlComponent set + RegionTree + ReconstructionPlan { values, bindings, node schedule }
  │  lower                   planned ownership → **Lua AST** (NOT strings)
  ▼
ast::Block  (structured Stmt/Expr tree)
  │  codegen                 AST → Lua source (pretty-printer)       [replaces EmitNode string concat]
  ▼
String  (--dec)
```

**Core rules**

1. **AST, not string pushing.** Decompilation builds a structured tree and materializes it
   into a `full_moon::ast::Block` (the spec-compliant Lua 5.1–5.4 AST) via a thin `emit`
   builder adapter — never by string concatenation. The completed AST is passed directly to
   `stylua` after token positions are materialized, avoiding a redundant stringify/reparse
   boundary. Every formatted result is still re-parsed by `full_moon` as a validity gate. Decision + evidence:
   `docs/AST_DECISION.md`. The heavy tree manipulation (inlining, De Morgan, control-flow
   structuring) happens on the **SSA / region** layer; the Lua AST is only the final
   materialization target, built once, fully-formed.
2. **Use crates instead of hand-rolling classic algorithms** — see §4.
3. **Idiomatic Rust**: owned trees (`Vec<Proto>` not raw pointers), `enum` with data for
   sum types (SSA ops, AST nodes), `Result<_, LuaError>` (thiserror) not sentinels,
   iterators, `&[u8]`/`bstr` for Lua's byte-strings (Lua strings are **not** UTF-8).
4. **Complete targets, not partial versions.** Input detection produces a
   `LuaVersion`; the single boundary conversion to `LuaTarget` rejects releases
   without a complete parse-to-source pipeline. Everything afterward routes
   through `LuaTarget` + `OpcodeTable`. Do not hardcode instruction field widths;
   read them from the active table.
5. **Clippy-clean** under the workspace lints (correctness/suspicious = deny).
6. **Small, single-responsibility files — no monoliths.** Strive for small, focused files
   (a few hundred lines); **hard cap ~1000 lines** — split before you hit it, don't sit at it.
   When a unit outgrows a clean single responsibility, split it into a directory module of
   focused submodules.
   The reference's large files MUST become multi-file modules, e.g.: `LuaSSA.pas` (2757) →
   `ir/{cfg,dom,ssa,lift,passes,dump}.rs`; `LuaDecompCF.inc` (4246) →
   `decompile/control_flow/{mod,regions,conditionals,loops,switch}.rs`; `LuaDecompBoolean.inc`
   (2255) and `LuaDecompMulti.inc` (2545) likewise become directories. One file, one job.
7. **Compiler architecture, not a god-object.** The pipeline is discrete passes with explicit,
   typed inputs/outputs (`bytes → Chunk → [Instruction] → SsaFunction → RegionTree →
   full_moon::Block → String`). Do **not** port the reference's ~2000-line `TDecompiler` god
   class: each pass is its own module with a narrow API; prefer free functions + small focused
   pass structs over one mega-struct threaded with mutable state. Data flows one direction —
   later passes don't reach back and mutate earlier ones.
8. **One source of truth; compute once; earn every pass.** Encode each rule / semantic / policy
   exactly once and have all consumers query it. Op properties (is-branch / terminator / call,
   defines-dest, RK operand slots, field widths, version quirks) live in ONE op-info /
   `OpcodeTable` layer read by disasm, CFG, lift, and SSA — never scatter `op == JMP || op ==
   FORLOOP …` matches across modules, never sprinkle `if version == V51` checks. Analyses (CFG
   edges, dominators, dominance frontiers, use-counts, def-sites, liveness) are computed **once**
   into typed, indexed results that later passes look up in O(1) — not re-derived by re-walking
   the IR. Extra passes are welcome when each is a *distinct transformation*; they are a **smell**
   when they exist to recompute a fact that should have been stored, or to patch emitted text
   after the fact. Prefer a **fast, few-pass** design: fix the IR / region facts at the source, do
   not band-aid downstream. (Multi-pass by design = good; multi-pass to paper over weak data
   modeling = reject.)

---

## 2. Crate layout

```
crates/nw-lua/
  Cargo.toml
  ARCHITECTURE.md            ← this file (the spec every phase builds against)
  src/
    lib.rs                   public API: parse / disassemble / decompile / build_ssa
    error.rs                 LuaError (thiserror)
    version.rs               recognized LuaVersion + complete LuaTarget capability
    chunk/                   [LuaChunk.pas]
      mod.rs                 Chunk, Proto, Constant, LocVar, UpvalDesc, Header
      reader.rs              ByteReader: endian, sizes, varints, per-version string formats
      header.rs              parse_header (per version)
      proto.rs              parse_proto (per version), code/constants/upvals/debug
    bytecode/                [LuaOpcodes.pas + instruction decode from LuaDis.pas]
      mod.rs
      semantic.rs            SemanticOp enum (~90, version-independent) + names + parse
      table.rs               OpcodeTable (field widths + raw→SemanticOp map),
                             exhaustive target selection + custom-table loader
      instruction.rs         Instruction (decoded A/B/C/Bx/sBx/Ax/sJ/k, RK helpers)
    disasm/
      mod.rs                 disassemble(proto/chunk) -> String   [LuaDis.pas]
    ir/                      [LuaSSA.pas / LuaSSAPasses.pas]
      mod.rs                 SsaFunction, BasicBlock, SsaNode, SsaRef, SsaOp, BinOp/UnOp/RelOp
      cfg.rs                 leader detection, basic-block split, succ/pred edges
      dom.rs                 dominators (petgraph simple_fast) + dominance frontiers
      ssa.rs                 φ placement (Cytron) + rename
      lift.rs                Instruction (SemanticOp) → SsaNode lifting
      operands.rs            authoritative use/def roles, effect facts, loop-control operands
      table.rs               typed NEWTABLE floating-byte allocation/source-count hints
      passes.rs              typed pass pipeline, schedules, reports, analysis cache
      passes/simplify.rs     conservative SSA cleanup transforms [LuaSSAPasses.pas]
      dump.rs                DumpSSA equivalent (--ssa-dump)
    decompile/               [LuaDecomp*.inc] — SSA → decompiler IR (never strings)
      mod.rs                 Decompiler driver, DecompOptions, per-proto
      ast/                   compact decompiler IR (the working tree passes build + rewrite).
        mod.rs               re-exports; Block, Name, BinOp, UnOp, TableField, FuncBody
        bindings.rs          one identity-aware usage/collision/rename traversal
        stmt.rs              Stmt
        expr.rs              Expr
      control_flow/          typed region detection and lowering (linear/if/while/repeat/for)
        regions/assembly.rs  CFG + control components → RegionTree
        conditionals.rs      branch polarity, reachability, merge, and PHI facts
        loops.rs             numeric, generic, and natural loop facts
      boolean/               one indexed ControlComponent set for conditions and values
        short_circuit/       typed condition/value plans and guarded composition
      expr_build/            planned SSA value → ast::Expr lowering
      reconstruction.rs      immutable value, declaration, constructor, and node ownership plan
      reconstruction/regions.rs  region projections used by reconstruction planning
      stmt_build/            monotonic RegionTree + ReconstructionPlan → ast::Stmt lowering
      multi/                 [LuaDecompMulti.inc] multi-value and constructor recovery
        plan.rs              typed omitted/standalone/owner/member node-emission schedule
        assign.rs            planned tuple declaration, swap, and call-result transfer lowering
        table_constructor.rs one reusable nested constructor plan from SSA + NEWTABLE hints
        table_list.rs        constructor fields and SETLIST multi-result semantics
      closure.rs             [LuaDecompClosure.inc] nested funcs / upvalues
      naming.rs              [LuaDecompNaming.inc] local/arg/global naming heuristics
    emit/                    the ONLY module that produces Lua text / touches full_moon
      mod.rs                 public: to_source(&decompile::ast::Block) -> Result<String>
      builder.rs             full_moon node builders (token trivia/spacing owned here)
      lower.rs               decompile::ast → full_moon::ast::Block (precedence/parens),
                             then stylua_lib::format_code + full_moon reparse gate
    bin/
      nw-lua.rs              CLI: --dis --dec --ssa-dump --annotate --lua-version --opcode-table
  tests/                     integration tests + fixtures
```

Reference-file → module mapping is authoritative for algorithm recovery: read
the mapped `.pas`/`.inc` file in the fixed Coldzer0 clone, then express its facts
through the Rust boundaries above rather than porting its string-oriented object
shape.

---

## 3. Key type sketches (refine against the reference; these fix the boundaries)

Byte-strings: Lua constants/names are raw bytes. Use `bstr::BString` for owned,
`&bstr::BStr` for borrowed. Never assume UTF-8.

```rust
// version.rs
pub enum LuaVersion { V51, V52, V53, V54, V55 }
pub enum LuaTarget { V51 }

// chunk/mod.rs
pub enum Constant { Nil, Boolean(bool), Number(f64), Integer(i64), Str(BString) }
pub struct LocVar   { pub name: BString, pub start_pc: i32, pub end_pc: i32 }
pub struct UpvalDesc{ pub in_stack: bool, pub idx: u8, pub kind: u8, pub name: BString }
pub struct Proto {
    pub source: BString,
    pub line_defined: i32, pub last_line_defined: i32,
    pub code: Vec<u32>,                 // raw instruction words
    pub line_info: Vec<i32>,
    pub constants: Vec<Constant>,
    pub upvalues: Vec<UpvalDesc>,
    pub protos: Vec<Proto>,            // owned nested functions (no raw pointers)
    pub loc_vars: Vec<LocVar>,
    pub max_stack: u8, pub num_params: u8, pub is_vararg: u8,
    pub version: LuaTarget,
}

// bytecode/semantic.rs  — one variant per abstract op across all versions
pub enum SemanticOp { Move, LoadK, LoadBool, LoadNil, GetUpval, GetGlobal, GetTable,
    /* … ~90 … */ Return, ForPrep, ForLoop, TForLoop, SetList, Close, Closure, VarArg,
    Unknown /* raw ordinal with no mapping */ }

// bytecode/table.rs
pub struct OpcodeTable {
    pub version: LuaTarget,
    pub op_bits: u8, pub a_bits: u8, pub b_bits: u8, pub c_bits: u8,
    // + Bx/sBx/Ax/sJ/K derivation; RK bit position
    pub map: Vec<SemanticOp>,          // raw opcode ordinal → SemanticOp
}
impl OpcodeTable {
    pub fn decode(&self, raw: u32) -> Instruction { /* … */ }
}

// ir/mod.rs
pub enum SsaRef { None, Reg { reg: u16, ver: u32 }, Const(u32) }
pub enum BinOp { Add, Sub, Mul, Div, Mod, Pow, IDiv, BAnd, BOr, BXor, Shl, Shr }
pub enum UnOp  { Neg, Not, Len, BNot }
pub enum RelOp { Eq, Lt, Le, Test, TestSet }
// SSA node: common fields + an op enum carrying variant data (idiomatic; replaces the
// reference's flat TSSANode record with kind tag). Multi-register instructions own their
// versioned secondary definitions; no function-global implicit-def side table or pseudo nodes.
pub struct SsaNode { pub pc: i32, pub line: i32, pub dest: SsaRef, pub op: SsaOp, … }
pub enum SsaOp { Nop, Phi { operands: Vec<SsaRef>, blocks: Vec<usize> }, Move(SsaRef),
    LoadK(u32), NewTable { array_hint: TableSizeHint, hash_hint: TableSizeHint },
    /* … */ Branch { rel: RelOp, a: SsaRef, b: SsaRef, invert: bool,
    t_true: i32, t_false: i32 }, Call { … }, /* … */ }

// Every analysis and transform uses these capabilities rather than matching SsaOp again:
// SsaOp::visit_uses / rewrite_uses (with UseRole), SsaNode::visit_defs,
// SsaOp::effects, and SsaOp::control_flow_role. Numeric/generic loop control windows are
// LoopControl values containing three versioned SsaRefs, not unversioned base arithmetic.

// SSA construction owns a typed PassPipeline. PassChange declares preserved analyses;
// PassContext lazily caches value facts and invalidates them centrally. Each pass runs once
// or to a bounded fixpoint and produces a deterministic report. SSA construction runs no
// simplifying transform: even safe folds can erase provenance needed for source/debug bindings.
// The cleanup pipeline is explicit and opt-in.

// Preserve NEWTABLE's encoded floating-byte operand. It is an exact source
// field count only where Lua 5.1's int2fb mapping is injective; larger values
// are allocation capacities and must not be treated as constructor boundaries.
pub struct TableSizeHint(u16);

// decompile/ast — the decompiler's COMPACT WORKING IR. This is NOT "a second Lua AST":
// full_moon remains the sole spec-compliant AST and the output. This IR is the medium the
// decompile passes build and rewrite (inline by use-count, De Morgan, restructure) before
// `emit` materializes full_moon ONCE. Minimal, decompiler-shaped, grows only as passes need.
pub struct Block(pub Vec<Stmt>);
pub enum Stmt { Local{names,attribs,values}, Assign{targets,values}, Call(Expr), Do(Block),
    While{cond,body}, Repeat{body,cond}, If{arms:Vec<(Expr,Block)>, else_:Option<Block>},
    NumericFor{var,start,stop,step,body}, GenericFor{names,exprs,body},
    Function{name,body,local}, Return(Vec<Expr>), Break }
pub enum Expr { Nil, True, False, VarArg, Number(f64), Integer(i64), Str(BString),
    Name(Name), Global(BString), Index{obj,key}, Field{obj,name},
    Call{func,args,method:Option<Name>}, Function(FuncBody), Table(Vec<TableField>),
    Binary{op,lhs,rhs}, Unary{op,operand}, Paren(Box<Expr>) }

// emit — the decompilation OUTPUT AST is full_moon's spec-compliant tree; emit is the ONLY
// module that touches full_moon:
//   decompile::ast::Block -> full_moon::ast::Block -> to_string()
//                         -> stylua_lib::format_code -> full_moon reparse gate
// `builder` owns token trivia/spacing; `lower` owns precedence/parens (full_moon Display
// invents neither). Reuse the proven patterns from the scratchpad `fullmoon_spike`.
```

---

## 4. Dependencies (research current versions in Phase 0; prefer workspace deps)

| Purpose                              | Crate            | Notes |
|--------------------------------------|------------------|-------|
| Errors                               | `thiserror`      | already a workspace dep |
| Lua byte-strings                     | `bstr`           | `BString`/`BStr`; Lua strings are bytes |
| Endian byte reading                  | `byteorder`      | header-driven endianness/sizes |
| CFG + dominators                     | `petgraph`       | `algo::dominators::simple_fast` **is** Cooper-Harvey-Kennedy — matches the reference's cited algorithm exactly |
| Ordered maps (naming, dedup)         | `indexmap`       | deterministic iteration |
| **Lua AST + serialization (output)** | `full_moon`      | `=2.2.0`, `default-features=false`, `features=["lua54"]`. The spec-compliant Lua 5.1–5.4 AST; `emit` builds a `full_moon::ast::Block` and also re-parses output as a validity gate. See `docs/AST_DECISION.md`. MPL-2.0 — link only, do not copy source. |
| **Formatting**                       | `stylua`         | `=2.5.2`, `default-features=false`, `features=["lua54"]`. Library crate `stylua_lib::format_code(...)`; normalizes the emitted source so `emit` only needs parseable trivia. |
| Runtime-equivalence validation (dev) | `mlua`/PUC lua   | later: run original vs decompiled and diff output |

Add new crates to `[workspace.dependencies]` in the root `Cargo.toml` and reference them
with `workspace = true`. Confirm the crate exists and pick the latest stable version
(`cargo add` / crates.io) during Phase 0 research.

---

## 5. Phase plan (each phase = one `codex exec` run, reviewed before the next)

Vertical, tracer-bullet slices. Each phase must `cargo build -p nw-lua`,
`cargo clippy -p nw-lua` clean, and `cargo test -p nw-lua` green before it is considered done.

- **P0 — Scaffold + 5.1 chunk parsing.** Crate skeleton, workspace wiring, `LuaVersion`,
  `LuaError`, `ByteReader`, header + proto parsing for 5.1. Milestone: parse
  `tests/*.luac` + a freshly compiled `fib.luac`; a `parse` API/test dumps the proto tree.
- **P1 — Opcodes + instruction decode + disassembler (5.1).** `SemanticOp`, 5.1
  `OpcodeTable`, `Instruction` decode, `--dis`. Validate against reference `--dis`.
- **P2 — SSA (5.1).** CFG, dominators (petgraph), dominance frontiers, φ placement,
  rename, semantic-op lifting, `--ssa-dump`.
- **P3 — `emit` adapter (full_moon + stylua).** Region/SSA → `full_moon::ast::Block` builder
  adapter (owns trivia/spacing + precedence parens) → `to_string()` → `stylua_lib::format_code`
  → `full_moon` reparse gate. Round-trip test on hand-built blocks. See `docs/AST_DECISION.md`.
- **P4 — Decompile: expressions + linear statements.** `expr.rs` + straight-line
  `EmitNode`-equivalent producing `ast::Stmt`. Milestone: decompile branch-free protos.
- **P5 — Control-flow reconstruction** (`control_flow.rs`, `region.rs`): if/elseif/else,
  while, repeat, numeric/generic for, break.
- **P6 — Boolean / short-circuit** (`boolean.rs`): and/or, De Morgan, comparison chains.
- **P7 — Multi-assign / multi-return** (`multi.rs`).
- **P8 — Closures / upvalues** (`closure.rs`), recursive proto decompile.
- **P9 — Naming heuristics** (`naming.rs`). Debug identifiers are authoritative.
  Anonymous bindings use deterministic role prefixes (`aN` parameter, `lN` local,
  `uN` upvalue), while receiver proof may rename a synthetic first parameter to
  `self`. Physical register reuse is not binding identity: initializer ownership
  follows the SSA value entering the debug-local lifetime, so a call initializer
  emits `local x = f(...)` instead of naming the pre-call function temporary `x`.
- **P9b — Idiomatic clean-code emitter.** A **semantics-preserving** AST→AST cleanup applied by the
  driver *before* `emit`. Built as ONE clean abstraction, **not a bloated rulebook**:
  - a single generic **bottom-up AST rewriter** (fold/visitor over `Block`/`Stmt`/`Expr`) that
    applies a list of small, orthogonal rewrite rules **to fixpoint**;
  - each rule is a tiny pure matcher for one local shape (≈ a few lines); adding a cleanup = adding
    one rule, never touching the engine. No giant match, no per-pattern special-casing.

  Rules, all semantics-preserving, in two groups:
  - **structure / clean code:** early-return guard clauses (invert a wrapping `if` so the body
    de-nests), drop `else` after a branch that returns/`break`s, flatten nesting, remove redundant
    `do…end`, `else`+`if` → `elseif` chains, drop empty/dead branches.
  - **idiom / style:** function-declaration sugar (`M.f = function…` → `function M.f(…)`,
    `local f = function…` → `local function f(…)`), method sugar (`function M.f(self,…)` →
    `function M:f(…)` when the first param is `self`), module-pattern (`local M = {} … return M`),
    idiomatic **synthetic-name** styling.

  Driven by `docs/IDIOMATIC_STYLE.md` (StyLua / luacheck / LuaLS / EmmyLua + **New World's own Lua
  style**, verdict: PascalCase tables/methods, lowerCamel locals, UPPER_SNAKE constants). Every
  rule is validated by runtime-equivalence (behavior must not change); P9b **never renames
  recovered debug identifiers** (fidelity). Lives in `decompile/idiomatic/` — `engine.rs` (the
  generic rewriter + fixpoint driver) plus small rule modules; keep each rule tiny.
  Rules: casing uses the **`heck`** crate (`ToUpperCamelCase`=PascalCase, `ToLowerCamelCase`,
  `ToShoutySnakeCase`=UPPER_SNAKE) — never hand-rolled. All renames are **binding-aware** (rewrite a
  binding's chosen name + every use, never text replacement) and apply to **synthetic names only**.
  Aggressiveness = **conservative** (keep opaque `lN` unless a stronger role is provable); **do** rename a
  recognized synthetic module table to PascalCase from the chunk source/file stem when reliable.
- **P9c — Corpus hardening** (done): real NW corpus 80%→99%, 0 crashes; residual ~1% are honest `Err`.
- **P10 — CLI + validation harness** parity with the reference; wire options; fixtures.
- **P10b — Hardening round 2 → 100% corpus.** Fix the residual ~1%: cyclic/irreducible-CFG region
  structuring (the goto-like cases), mixed multi-result targets, open-ended `SETLIST`. Derive a
  **general** structuring algorithm from the **Lua 5.1.5 compiler/VM source** (`lparser.c` /
  `lcode.c` / `lvm.c` / `lopcodes.h`) — how `luac` actually emits break/loop/short-circuit control
  flow — plus reference `LuaDecompCF.inc`. **Do NOT tune to NW-specific patterns**; the decompiler
  must stay general for any Lua 5.1 bytecode. The NW corpus is **validation only** (diverse test
  bytecode), never a heuristic source. **Validate**: self-contained runtime-equivalence
  reproductions of each *generic* control-flow pattern + real files decompile→reparse→
  **recompile→decompile idempotent** (fixpoint proxy, since NW files can't run standalone).
  **Bar recalibrated (owner decision):** the goal is **100% recompile-cleanly** (luac accepts the
  decompiled output) + **comprehensive runtime-equivalence** on generic constructs — NOT
  instruction-identity (equivalent source compiles differently; that's an unachievable/meaningless
  bar for a decompiler). The core (`--no-idiomatic`) structural opcode comparison is a **soft
  bug-finder** (report the match rate; fix genuine control-flow bugs it surfaces; ignore benign
  compilation diffs). **High-priority correctness fix:** expression inlining / local-lifetime —
  the decompiler over-materializes (one `local` per SSA temp → 200-local overflow that won't
  recompile, plus drift + ugly temps); inline single-use, **evaluation-order-safe** values so the
  local count ≈ the original. Hard gate: 100% decompile-OK, 0 crashes, **100% recompile-clean**.
- **P10c — Lua 5.1.5 spec-completeness audit.** Verify full 5.1.5 coverage (all 38 opcodes, constant
  types + number formats, chunk-format edge cases, arg modes, language constructs, string escapes)
  vs the 5.1.5 manual + `lundump.c`/`lopcodes.h`/`lvm.c`. Report in `docs/LUA51_COVERAGE.md`; finish
  any gaps cleanly before P11.
- **P11+ — Extend one complete target at a time**: for each later release, land
  its chunk format, opcode table, semantics, source AST/emitter support, runtime
  tests, and corpus gate before adding its `LuaTarget` variant.

New World specifics to keep in mind: NW bytecode is standard PUC 5.1 `lundump`, but may
use a **custom/shuffled opcode table** — the `--opcode-table` loader (P1/P10) is important
for NW. Validate early against `tests/shopcommon.luac` and `examples/*.decompiled.lua`.

---

## 6. Validation

- Unit tests per module (decode, dominators, φ placement, codegen precedence).
- Golden disassembly vs. reference `--dis` on shared fixtures.
- Runtime-equivalence (later): compile a `.lua` fixture with `luac5.1`, decompile, recompile
  / re-run, compare output — mirrors the reference `run_decompiler_validation.py`.
- `full_moon` re-parse of every `--dec` output as a syntactic gate.

## 7. Hard constraints for implementers (codex)

- **Never** run `git checkout`, `git restore`, `git reset`, or any command that discards
  working-tree changes. Do not `git commit` unless explicitly asked.
- Touch only `crates/nw-lua/` and the root `Cargo.toml` `[workspace.dependencies]` table.
  Do not modify other crates.
- Stay within the current phase's scope. Leave clearly-marked `todo!()` /
  `// PHASE N:` stubs for later phases so the crate keeps compiling.
- Read the mapped reference file(s) in `E:\Projects\LuaDecompiler` before porting a module,
  and the matching PUC Lua source (`E:\Projects\lua-5.1.5\src\lundump.c`, `lopcodes.h`,
  `lvm.c`) when opcode/format semantics are unclear.
- **No monolithic files / god-objects** (see §1 rules 6–7). Strive for small, focused files;
  **hard cap ~1000 lines** (split before hitting it). When a ported reference file would blow
  past a clean single responsibility, split it into a directory module of focused submodules
  *up front*, not as an afterthought. Each pass is its own module
  with a narrow, typed API. Reviews will reject oversized files and mega-structs.
- **DRY rules, compute-once analyses, few-pass** (see §1 rule 8). One source of truth per
  rule/semantic/policy — no duplicated `op == …` / `version == …` logic across modules. Compute
  each analysis once into a typed, indexed result and look it up; do not re-walk the IR to
  re-derive it. Do not add a pass to compensate for missing data or to patch output text — fix the
  IR/region. Reviews will reject duplicated rule logic and recompute-by-rewalk.
```
