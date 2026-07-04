# Lua AST representation decision

Date: 2026-07-03

## Recommendation

Use `full_moon` as the final Lua syntax materialization target, followed by
`stylua` formatting and `full_moon` reparse validation. Do not roll a custom AST
for the decompiler output path right now.

Recommended dependencies:

```toml
full_moon = { version = "=2.2.0", default-features = false, features = ["lua54"] }
stylua = { version = "=2.5.2", default-features = false, features = ["lua54"] }
```

For 5.1-only output, use `full_moon::LuaVersion::lua51()` and
`stylua_lib::LuaVersion::Lua51` during validation/formatting. Keep the `lua54`
feature enabled in the crate so the builder can also materialize 5.2-5.4 syntax
when later bytecode versions are added.

This fits the pipeline in `ARCHITECTURE.md`: SSA and region structuring do the
hard transformations, then a small `LuaAstBuilder` adapter constructs a complete
`full_moon::ast::Block` once, serializes it, formats it, and reparses the result.
The Lua AST is not a rewrite arena.

## Survey

Latest versions were checked with crates.io/cargo on 2026-07-03.

| Crate | Latest / status | Lua coverage | Printer back to source | Synthesis API | Decision |
|---|---:|---|---|---|---|
| `full_moon` | 2.2.0, updated 2026-04-15, active, MPL-2.0 | Lua 5.1 always; feature flags `lua52`, `lua53`, `lua54`, `luau`, `luajit`, `cfxlua`; no 5.5 | Yes: `Display` on AST nodes; lossless CST printer | Mixed: many `new`/`with_*` builders, but identifiers/literals/punctuation require `TokenReference` | Recommended |
| `stylua` | 2.5.2, updated 2026-05-16, active, MPL-2.0 | Formatter syntax flags for Lua 5.1-5.4, LuaJIT, Luau, CfxLua | Yes: `stylua_lib::format_code` and `format_ast` | Not an AST representation; formats parsed `full_moon` AST/source | Use after `full_moon` Display |
| `lua-ast` | Not found by `cargo info lua-ast` | N/A | N/A | N/A | Not an option |
| `hematita` | 0.1.0, updated 2021-08-11, GPL-3.0, inactive | Interpreter parser subset, not versioned 5.1-5.5 | Some `Display` impls on parser AST | Public enums/structs, but interpreter-oriented | Reject: stale, GPL, incomplete |
| `luau-ast-rs` | 0.1.29, updated 2023-08-02, MIT | Luau and Lua 5.1 | No source printer found; debug AST output only | Arena-like `Chunk`, some internals `pub(crate)` | Reject: Luau-oriented, no printer |
| `luaur-ast` | 0.1.7, updated 2026-07-02, MIT | Luau faithful Rust port | Has pretty-printer functions for Luau AST blocks | Port-style AST, raw-pointer flavored internals; not ergonomic for external synthesis | Reject: Luau, not PUC Lua 5.1-5.5 |
| `luaparse-rs` | 0.1.1, updated 2026-03-11, MIT/Apache-2.0 | Feature-gated Lua 5.1-5.4 and Luau | No source printer found | Public AST structs with spans and synthetic constructors | Reject for output: parser AST only |
| `valua-ast` | 0.1.0, updated 2026-05-20, MIT, very new | Claims Lua 5.5 AST, Lua 5.4 subset | No `Display`/printer in crate | Plain public fields, easy to synthesize | Monitor only: AST definitions without printer/parser |
| `oak-lua` | 0.0.11, updated 2026-03-30, MPL-2.0, young | Claims Lua 5.x, no clear per-version model | Internal `to_source`, optional pretty-print feature | Red/green parser AST; not clearly designed as emitter target | Reject for now: immature/unclear compliance |
| `tree-sitter-lua` + `treesitter-types-lua` | 0.5.0 / 0.2.0, active grammar/types | tree-sitter Lua grammar | No source printer | Parse-tree wrappers, not owned emitter AST | Reject: parser tooling only |

## `full_moon` Findings

`full_moon` 2.2.0 feature flags from `cargo info`:

```text
default = [serde]
serde = [dep:serde]
cfxlua = [lua54]
lua52 = []
lua53 = [lua52]
lua54 = [lua53]
luajit = []
luau = [roblox]
roblox = [luau]
```

It covers Lua 5.1-5.4, Luau, LuaJIT, and CfxLua. It does not cover Lua 5.5.

Lua 5.5 source gap:

- Covered already by `lua54`: `goto`, labels, `//`, bitwise operators,
  `<const>`, `<close>`, ordinary loops/functions/tables.
- Not covered: `global` declarations and the Lua 5.5 named vararg-table syntax.
- Mostly semantic/no new source form: read-only for-loop control variables,
  compact arrays, GC changes, bytecode/string reuse.

For this decompiler, the 5.5 gap is acceptable for the current 5.1-first
roadmap if 5.5 bytecode is lowered to executable 5.4-compatible Lua source
instead of exact 5.5 source. `global` declarations are compile-time declaration
syntax and can usually be omitted or lowered to ordinary global access in
decompiled output. Named vararg tables can be lowered to a generated local
table from `...` if needed. If exact 5.5 source reconstruction becomes a goal,
`full_moon` must be extended/upstreamed or revisited.

Synthesis behavior:

- A synthesized `Block` can be displayed and reparsed. `Ast` itself has no
  public `Ast::new`; constructing a top-level `Block` and serializing it is the
  clean path. Parsing an empty string just to get an `Ast` shell is possible but
  was intentionally not used in the spike.
- Minimum trivia: every identifier/literal is a `TokenReference`; syntax tokens
  are `TokenReference::symbol(...)`; comma-separated lists are
  `Punctuated<T>` with `Pair::Punctuated`. Whitespace must be attached to tokens
  where Lua needs separation (`" then "`, `" = "`, `", "`, etc.).
- `Display` does not pretty-print or invent missing spaces. If you produce
  `thenlocal`, it is invalid. If you omit precedence parentheses, `stylua` will
  format the wrong parse tree. Our builder must own spacing and paren decisions.
- Many statement nodes have useful builders: `If::new`, `ElseIf::new`,
  `NumericFor::new`, `LocalAssignment::new`, `LocalFunction::new`,
  `FunctionBody::new`, `Return::new`, `FunctionCall::new`, plus `with_*`
  setters. Expressions are mostly direct enum variants.

`stylua` integration:

- The package is `stylua`; the library crate is `stylua_lib`.
- Confirmed API: `stylua_lib::format_code(code, config, range,
  OutputVerification::Full)` parses with `full_moon`, formats, reparses, and
  can verify AST equivalence.
- In the spike, raw `full_moon` output was intentionally minimal and StyLua
  normalized it into conventional multi-line formatting.

## Spike

Path:

```text
C:\Users\jonfo\AppData\Local\Temp\claude\E--Projects-nw-tools\a181c50c-d3a2-42d0-a00e-dbe3b657da76\scratchpad\fullmoon_spike
```

Commands run:

```text
cargo add full_moon@2.2.0 --features lua54 --no-default-features --dry-run
cargo add stylua@2.5.2 --features lua54 --no-default-features --dry-run
cargo add full_moon@2.2.0 --features lua54 --no-default-features
cargo add stylua@2.5.2 --features lua54 --no-default-features
cargo fmt
cargo run
cargo doc --no-deps
```

`cargo doc` for the whole dependency graph exceeded the command timeout; the
local `cargo doc --no-deps` build completed. The spike keeps its own
`Cargo.lock`.

Important constraint: snippets were constructed programmatically from
`full_moon` nodes. Parsing is used only as an assertion after Display/StyLua
output.

Actual `cargo run` output:

```text
=== local x = f(a, b) ===
construction_loc: 4
full_moon_display:
local x = f(a, b)
stylua:
local x = f(a, b)

=== if / elseif / else ===
construction_loc: 26
full_moon_display:
if c then local x = f(a, b) elseif d then local x = f(d, b) else local x = f(c, b) end
stylua:
if c then
    local x = f(a, b)
elseif d then
    local x = f(d, b)
else
    local x = f(c, b)
end

=== numeric for ===
construction_loc: 10
full_moon_display:
for i = 1, n do local x = f(i, n) end
stylua:
for i = 1, n do
    local x = f(i, n)
end

=== local function + call ===
construction_loc: 27
full_moon_display:
local function g(a, b) return a + b end
g(1, 2)
stylua:
local function g(a, b)
    return a + b
end
g(1, 2)

=== local y = a and b or c ===
construction_loc: 3
full_moon_display:
local y = a and b or c
stylua:
local y = a and b or c
```

Construction LOC is counted inside each snippet function and excludes the
shared helper layer. The helper layer in the spike is intentionally small:
identifier/literal token constructors, `symbol`, `punctuated`, `block`,
`local_assign`, `call`, `call_expr`, `binary_expr`, and formatter/parse
assertions. A production builder should be roughly 150-250 LOC initially,
because it must also own string literal escaping, numeric literal policy, target
version switches, and precedence/parentheses.

## Pros and Cons

### `full_moon` + `stylua`

Pros:

- Complete, spec-oriented Lua 5.1-5.4 syntax surface maintained by a real Lua
  tooling ecosystem.
- Avoids owning a hand-written Lua grammar, precedence printer, and version
  matrix.
- Can validate every decompile result by reparsing with the same parser.
- StyLua gives stable formatting, so the materializer only needs parseable
  minimal trivia.
- Fits the architecture because the Lua AST is built once and not rewritten.

Cons:

- Direct construction is noisy. We need a local builder adapter.
- `Display` preserves exactly what we attach. It is not a pretty-printer and
  will not protect us from missing spaces or missing parentheses.
- No public `Ast::new`; use `Block` as the synthesized root or add a tiny
  wrapper that serializes a `Block`.
- No Lua 5.5 syntax support today.
- MPL-2.0 dependency. This is fine for linking as a dependency, but do not copy
  source into the MIT crate.

### Custom complete AST

A custom AST would be more ergonomic for construction, but the owner's hard
rule makes it a large language-front-end project. To be acceptable it would
need, at minimum:

- Version-gated statement coverage: assignment, local declaration, global
  declaration for 5.5, do/while/repeat, if/elseif/else, numeric and generic for,
  function declaration, local function, function-call statement, return, break,
  goto, label, empty statements where valid, and attributes.
- Full expression coverage: nil/booleans/numerals/strings, varargs and 5.5
  named vararg table lowering or syntax, names, field/index access, function
  calls and method calls, anonymous functions, tables, unary/binary operators,
  parentheses, and exact precedence/associativity.
- Table constructors: list fields, name fields, expression-key fields,
  separators, and multi-return tail behavior.
- Function bodies: parameter lists, varargs, local scoping implications,
  method/self syntax, attributes/type syntax only if target dialect requires it.
- Version-aware printers for Lua 5.1, 5.2, 5.3, 5.4, and 5.5, plus tests
  against the official grammar.
- A parser-backed validation dependency anyway, likely `full_moon` for 5.1-5.4.

That is not justified when the output tree is only a final materialization
target.

## Integration Pattern

1. Keep all expression inlining, boolean rewriting, control-flow structuring,
   and naming in SSA/region layers.
2. Add an internal `lua_syntax` or `emit_ast` adapter around `full_moon`:
   `name`, `number`, `string`, `symbol`, `punctuated`, `call`, `local_assign`,
   `if_stmt`, `for_stmt`, `function_body`, `return_stmt`, and
   `paren_if_needed`.
3. Materialize `RegionTree`/SSA output into a `full_moon::ast::Block`.
4. Serialize raw source with `Block::to_string()`.
5. Reparse raw source with `full_moon::parse_fallible(raw, target_version)`.
6. Format with `stylua_lib::format_code(raw, config, None,
   OutputVerification::Full)`.
7. Reparse formatted source with the same target version and return it.

Use target-version validation:

- `Lua51` for New World / primary path.
- `Lua54` when output contains 5.2-5.4 constructs such as `goto`, bitwise ops,
  `//`, or `<close>`.
- For future 5.5 bytecode, emit 5.4-compatible source unless exact 5.5 source
  becomes a formal requirement.

## Sources

- `full_moon` crates.io/docs/source: https://crates.io/crates/full_moon,
  https://docs.rs/full_moon/2.2.0/full_moon/
- `stylua` crates.io/source: https://crates.io/crates/stylua,
  https://docs.rs/stylua/2.5.2/stylua/
- Lua 5.5 version history and manual:
  https://www.lua.org/versions.html,
  https://www.lua.org/manual/5.5/manual.html,
  https://www.lua.org/manual/5.5/readme.html
- Other crates surveyed:
  https://crates.io/crates/hematita,
  https://crates.io/crates/luau-ast-rs,
  https://crates.io/crates/luaur-ast,
  https://crates.io/crates/luaparse-rs,
  https://crates.io/crates/valua-ast,
  https://crates.io/crates/oak-lua,
  https://crates.io/crates/tree-sitter-lua,
  https://crates.io/crates/treesitter-types-lua
