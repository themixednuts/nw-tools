# Lua 5.1.5 Coverage Matrix

Authoritative references:

- Lua 5.1 Reference Manual: <https://www.lua.org/manual/5.1/>
- Local PUC-Rio Lua 5.1.5 source: `E:\Projects\lua-5.1.5\src`
  - chunk format: `lundump.c`
  - opcode layout/modes: `lopcodes.h`, `lopcodes.c`
  - VM semantics: `lvm.c`
  - parser/codegen shapes: `lparser.c`, `lcode.c`
  - number/string formatting/parsing: `luaconf.h`, `lobject.c`, `lstrlib.c`

Status legend:

- `COVERED+tested`: implemented and covered by a direct unit/integration/runtime-equivalence test.
- `COVERED-untested`: implemented but no direct test. Current count: `0`.
- `GAP`: not implemented. Current count: `0` for standard Lua 5.1.5 double-number chunks.

## Summary

| Area | COVERED+tested | COVERED-untested | GAP |
|---|---:|---:|---:|
| Lua 5.1 opcodes | 38 | 0 | 0 |
| Constant kinds / literal round-trip | 5 | 0 | 0 |
| Chunk-format rows | 15 | 0 | 0 |
| Language construct rows | 38 | 0 | 0 |

Direct opcode tests lacking runtime-equivalence coverage: none.

## Opcode Coverage

All opcode rows use:

- P1 decode: `tests/spec_5_1.rs::opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes` asserts the opcode appears in disassembly.
- P2 lift: the same test runs `nw_lua::ssa_dump` for each opcode source case.
- P4-P9 reconstruction: the same test decompiles and runs original vs decompiled Lua through `tests/support`.

| Opcode | P1 decode | P2 SSA lift | P4-P9 source reconstruction | Direct runtime-equivalence case |
|---|---|---|---|---|
| `MOVE` | COVERED+tested | COVERED+tested | COVERED+tested | `linear_registers_and_globals` |
| `LOADK` | COVERED+tested | COVERED+tested | COVERED+tested | `linear_registers_and_globals` |
| `LOADBOOL` | COVERED+tested | COVERED+tested | COVERED+tested | `linear_registers_and_globals` |
| `LOADNIL` | COVERED+tested | COVERED+tested | COVERED+tested | `linear_registers_and_globals` |
| `GETUPVAL` | COVERED+tested | COVERED+tested | COVERED+tested | `upvalues_and_close` |
| `GETGLOBAL` | COVERED+tested | COVERED+tested | COVERED+tested | `linear_registers_and_globals` |
| `GETTABLE` | COVERED+tested | COVERED+tested | COVERED+tested | `table_fields_and_self` |
| `SETGLOBAL` | COVERED+tested | COVERED+tested | COVERED+tested | `linear_registers_and_globals` |
| `SETUPVAL` | COVERED+tested | COVERED+tested | COVERED+tested | `upvalues_and_close` |
| `SETTABLE` | COVERED+tested | COVERED+tested | COVERED+tested | `table_fields_and_self` |
| `NEWTABLE` | COVERED+tested | COVERED+tested | COVERED+tested | `table_fields_and_self` |
| `SELF` | COVERED+tested | COVERED+tested | COVERED+tested | `table_fields_and_self` |
| `ADD` | COVERED+tested | COVERED+tested | COVERED+tested | `upvalues_and_close` |
| `SUB` | COVERED+tested | COVERED+tested | COVERED+tested | `arithmetic_and_unary` |
| `MUL` | COVERED+tested | COVERED+tested | COVERED+tested | `arithmetic_and_unary` |
| `DIV` | COVERED+tested | COVERED+tested | COVERED+tested | `arithmetic_and_unary` |
| `MOD` | COVERED+tested | COVERED+tested | COVERED+tested | `arithmetic_and_unary` |
| `POW` | COVERED+tested | COVERED+tested | COVERED+tested | `arithmetic_and_unary` |
| `UNM` | COVERED+tested | COVERED+tested | COVERED+tested | `arithmetic_and_unary` |
| `NOT` | COVERED+tested | COVERED+tested | COVERED+tested | `arithmetic_and_unary` |
| `LEN` | COVERED+tested | COVERED+tested | COVERED+tested | `arithmetic_and_unary` |
| `CONCAT` | COVERED+tested | COVERED+tested | COVERED+tested | `concat_and_compare` |
| `JMP` | COVERED+tested | COVERED+tested | COVERED+tested | `concat_and_compare` |
| `EQ` | COVERED+tested | COVERED+tested | COVERED+tested | `concat_and_compare` |
| `LT` | COVERED+tested | COVERED+tested | COVERED+tested | `concat_and_compare` |
| `LE` | COVERED+tested | COVERED+tested | COVERED+tested | `concat_and_compare` |
| `TEST` | COVERED+tested | COVERED+tested | COVERED+tested | `boolean_tests` |
| `TESTSET` | COVERED+tested | COVERED+tested | COVERED+tested | `boolean_tests` |
| `CALL` | COVERED+tested | COVERED+tested | COVERED+tested | `linear_registers_and_globals` |
| `TAILCALL` | COVERED+tested | COVERED+tested | COVERED+tested | `vararg_and_tailcall` |
| `RETURN` | COVERED+tested | COVERED+tested | COVERED+tested | `linear_registers_and_globals` |
| `FORLOOP` | COVERED+tested | COVERED+tested | COVERED+tested | `numeric_and_generic_loops` |
| `FORPREP` | COVERED+tested | COVERED+tested | COVERED+tested | `numeric_and_generic_loops` |
| `TFORLOOP` | COVERED+tested | COVERED+tested | COVERED+tested | `numeric_and_generic_loops` |
| `SETLIST` | COVERED+tested | COVERED+tested | COVERED+tested | `numeric_and_generic_loops` |
| `CLOSE` | COVERED+tested | COVERED+tested | COVERED+tested | `upvalues_and_close` |
| `CLOSURE` | COVERED+tested | COVERED+tested | COVERED+tested | `upvalues_and_close` |
| `VARARG` | COVERED+tested | COVERED+tested | COVERED+tested | `vararg_and_tailcall` |

## Constants And Literals

| Constant / literal area | Status | Tests | Notes |
|---|---|---|---|
| `nil` | COVERED+tested | `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes` (`LOADNIL`) | Reconstructs local nil ranges and nil expressions. |
| boolean | COVERED+tested | `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes`, `phase6_boolean::runtime_equivalence_phase6_boolean_cases` | Covers `true`, `false`, `not`, `and`, `or`, tests. |
| `lua_Number` double | COVERED+tested | `number_literals_recompile_to_exact_lua_51_number_bits` | Emitted finite literals use Rust's shortest round-trip `f64` formatting; recompilation by Lua 5.1 `strtod` must reproduce identical `f64::to_bits()`. Includes integers, >2^53 values, negatives, fractions, precision-sensitive values, `1e308`, normal minimum, subnormal minimum, and infinities. |
| Lua strings as bytes | COVERED+tested | `string_literals_recompile_to_exact_lua_51_bytes` | Emits quoted Lua 5.1 string literals with ASCII escapes. Recompiled constants are byte-exact for empty strings, quotes, backslashes, NUL, control bytes, CR/LF/CRLF, 0x7F-0xFF, all 256 byte values, and long strings. |
| Integral `lua_Number` build flag | COVERED+tested | `chunk_reader_handles_lua_51_layout_variants_and_rejects_bad_header_fields` | 32-bit integral values and exactly representable 64-bit integral values are parsed. 64-bit integral values not exactly representable by the crate's `f64` constant model are explicitly rejected instead of rounded. |

Non-finite note: `+inf` and `-inf` round-trip through `1e9999` / `-1e9999` with the local Lua 5.1 runtime. NaN has no exact Lua 5.1 source literal, so `to_source` rejects NaN constants (`nan_number_literals_are_rejected_instead_of_mis_emitted`) rather than emitting a different value.

## Chunk Format

| `lundump.c` field / behavior | Status | Tests | Notes |
|---|---|---|---|
| Signature `\x1bLua` | COVERED+tested | `chunk_parse::parses_shopcommon_lua_51_chunk`, `chunk_reader_handles_lua_51_layout_variants_and_rejects_bad_header_fields` | Bad signatures return `LuaError::BadMagic`. |
| Version byte `0x51` | COVERED+tested | `chunk_parse::parses_shopcommon_lua_51_chunk`, `cli_phase10::cli_rejects_future_lua_version_override_cleanly` | 5.2-5.5 are version-aware stubs for P11 and are rejected in P10c. |
| Format byte | COVERED+tested | `chunk_reader_handles_lua_51_layout_variants_and_rejects_bad_header_fields` | Official format `0` only; nonzero is rejected. |
| Endianness flag | COVERED+tested | `chunk_reader_handles_lua_51_layout_variants_and_rejects_bad_header_fields`, `chunk::reader::tests::reads_big_endian_int_and_size_t` | Both little and big endian scalar reads are tested. |
| `sizeof(int)` | COVERED+tested | `chunk_reader_handles_lua_51_layout_variants_and_rejects_bad_header_fields` | Sizes 1, 2, 4, and 8 are accepted and tested. Other sizes are rejected. |
| `sizeof(size_t)` | COVERED+tested | `chunk_reader_handles_lua_51_layout_variants_and_rejects_bad_header_fields` | Sizes 1, 2, 4, and 8 are accepted and tested. Other sizes are rejected. |
| `sizeof(Instruction)` | COVERED+tested | `chunk_reader_handles_lua_51_layout_variants_and_rejects_bad_header_fields`, `bytecode_phase1::decodes_lua_51_instruction_fields` | Lua 5.1 4-byte instructions are supported. Other instruction sizes are rejected. |
| `sizeof(lua_Number)` | COVERED+tested | `chunk_reader_handles_lua_51_layout_variants_and_rejects_bad_header_fields` | 4-byte float and 8-byte double are handled. Other sizes are rejected. |
| Integral-number flag | COVERED+tested | `chunk_reader_handles_lua_51_layout_variants_and_rejects_bad_header_fields` | See constant matrix for exactness limits. |
| Proto header (`source`, lines, `nups`, params, vararg, max stack) | COVERED+tested | `chunk_parse::parses_shopcommon_lua_51_chunk`, `edge_protos_runtime_equivalence` | Source `NULL` inherits parent source as in `LoadFunction`. |
| Code vector | COVERED+tested | `bytecode_phase1::standard_lua_51_table_decodes_shopcommon_without_unknown_opcodes`, `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes` | Raw instructions are stored and decoded through `OpcodeTable`. |
| Constant vector | COVERED+tested | `chunk_parse::parses_shopcommon_lua_51_chunk`, number/string tests in `spec_5_1.rs` | Tags nil, boolean, number, string are handled; unknown tags are rejected. |
| Nested protos | COVERED+tested | `phase8_closures::runtime_equivalence_phase8_closure_cases`, `edge_protos_runtime_equivalence` | Recursive proto tree parsing and closure decompilation covered. |
| Debug line info and locals | COVERED+tested | `chunk_parse::parses_shopcommon_lua_51_chunk`, `phase9_naming::stripped_accumulator_loop_uses_one_loop_var_name` | Line info and local ranges are parsed; naming uses them when valid and safe. |
| Upvalue names | COVERED+tested | `phase8_closures::runtime_equivalence_phase8_closure_cases`, `edge_protos_runtime_equivalence` | Child upvalue names are reconciled with parent emitted binding names to avoid stale synthetic names. |
| Lua 5.1 string serialization (`size_t` length includes trailing NUL) | COVERED+tested | `chunk::reader::tests::reads_lua_51_string_without_trailing_nul`, `string_literals_recompile_to_exact_lua_51_bytes` | Returned `BString` excludes the serialized trailing NUL. |

## Language Constructs

| Construct | Status | Tests |
|---|---|---|
| Chunk / block sequencing | COVERED+tested | `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes`, corpus recompile gate |
| Local declaration | COVERED+tested | `decompile_phase4::local_add_decompiles_with_debug_local_names`, `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes` |
| Assignment | COVERED+tested | `phase7_multi::swap_decompiles_as_single_multiple_assignment`, `recovered_reserved_and_invalid_names_are_emitted_safely` |
| Call statement | COVERED+tested | `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes` |
| `do ... end` block | COVERED+tested | `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes` (`upvalues_and_close`) |
| `while` | COVERED+tested | `decompile_phase5_control_flow::control_flow_fixtures_decompile_and_reparse`, `corpus::runtime_equivalence_phase10b_hardening_cases` |
| `repeat ... until` | COVERED+tested | `decompile_phase5_control_flow::control_flow_fixtures_decompile_and_reparse` |
| `if` / `elseif` / `else` | COVERED+tested | `decompile_phase5_control_flow::control_flow_fixtures_decompile_and_reparse`, `phase9b_idiomatic::runtime_equivalence_idiomatic_declaration_sugar_cases` |
| Numeric `for` | COVERED+tested | `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes`, `decompile_phase5_control_flow::control_flow_fixtures_decompile_and_reparse` |
| Generic `for` | COVERED+tested | `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes`, `decompile_phase5_control_flow::control_flow_fixtures_decompile_and_reparse` |
| Function declaration | COVERED+tested | `phase8_closures::runtime_equivalence_phase8_closure_cases`, `phase9b_idiomatic::runtime_equivalence_idiomatic_declaration_sugar_cases` |
| Local function | COVERED+tested | `phase8_closures::runtime_equivalence_phase8_closure_cases`, `edge_protos_runtime_equivalence` |
| Return with no values | COVERED+tested | `edge_protos_runtime_equivalence` |
| Return with values / multiret | COVERED+tested | `phase7_multi::runtime_equivalence_phase7_multi_cases`, `vararg_and_tailcall` |
| `break` | COVERED+tested | `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes` (`numeric_and_generic_loops`) |
| `nil` expression | COVERED+tested | `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes` |
| `true` / `false` expressions | COVERED+tested | `phase6_boolean::runtime_equivalence_phase6_boolean_cases` |
| Numerals | COVERED+tested | `number_literals_recompile_to_exact_lua_51_number_bits` |
| Strings | COVERED+tested | `string_literals_recompile_to_exact_lua_51_bytes` |
| Vararg `...` in main chunk and function | COVERED+tested | `phase7_multi::runtime_equivalence_phase7_vararg_cases`, `vararg_and_tailcall`, `edge_protos_runtime_equivalence` |
| Table constructor list fields | COVERED+tested | `phase7_multi::runtime_equivalence_phase7_multi_cases`, `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes` |
| Table constructor named/keyed fields | COVERED+tested | `decompile_phase4::table_field_decompiles_as_table_assignment_and_read`, `emitter_brackets_invalid_ast_field_names_defensively` |
| Table constructor `[expr]` fields | COVERED+tested | `emitter_brackets_invalid_ast_field_names_defensively`, `recovered_reserved_and_invalid_names_are_emitted_safely` |
| Table constructor trailing multiret | COVERED+tested | `phase7_multi::runtime_equivalence_phase7_multi_cases` (`multiret_in_table`) |
| Name/global access | COVERED+tested | `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes`, `recovered_reserved_and_invalid_names_are_emitted_safely` |
| Index and field access | COVERED+tested | `table_fields_and_self`, `recovered_reserved_and_invalid_names_are_emitted_safely` |
| Function call expression | COVERED+tested | `phase7_multi::runtime_equivalence_phase7_multi_cases` |
| Method call expression | COVERED+tested | `table_fields_and_self`, `phase8_closures::method_case_keeps_self_receiver` |
| Function expression / empty function | COVERED+tested | `edge_protos_runtime_equivalence` |
| Binary arithmetic `+ - * / % ^` | COVERED+tested | `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes` |
| Concatenation `..` | COVERED+tested | `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes` |
| Comparisons `== ~= < <= > >=` | COVERED+tested | `phase6_boolean::runtime_equivalence_phase6_boolean_cases`, `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes` |
| Logical `and` / `or` | COVERED+tested | `phase6_boolean::runtime_equivalence_phase6_boolean_cases`, `boolean_tests` |
| Unary `-`, `not`, `#` | COVERED+tested | `opcode_runtime_equivalence_matrix_exercises_all_lua_51_opcodes` |
| Parenthesization and precedence | COVERED+tested | `emit::tests::precedence_add_mul`, `precedence_parens_add_before_mul`, `precedence_unary_power`, `precedence_power_unary_rhs`, `precedence_concat_right_assoc`, `precedence_not_comparison`, `precedence_and_or`, `precedence_or_before_and` |
| Deeply nested closures | COVERED+tested | `edge_protos_runtime_equivalence` |
| Max-ish locals/upvalues | COVERED+tested | `edge_protos_runtime_equivalence` (`maxish_upvalues`, 55 captures) |

Lua 5.1 has no integer subtype, bitwise operators, `goto`, labels, `_ENV`, attributes, or hex-float literals. Those are intentionally outside the Lua 5.1.5 source reconstruction matrix and belong to later-version phases where applicable.

## Subtle Edge Audit Results

- Number literal faithfulness: finite doubles and infinities now recompile to the exact same `f64` bit pattern. NaN is rejected because Lua 5.1 source cannot express a specific NaN payload. Integral 64-bit values that cannot fit exactly in `f64` are rejected at chunk parse time.
- String literal faithfulness: no code bug found; the new all-byte test validates the `full_moon`/StyLua path preserves byte strings through escaped ASCII Lua string literals.
- Reserved words / invalid identifiers: defensive lowering now brackets invalid `Global`, `Field`, method fallback, and named table-field forms. Patched bytecode tests cover keyword globals, invalid fields, and keyword method keys.
- Edge protos: fixed stale upvalue-name precedence so child closures use the parent emitted capture names. Fixed future debug-local binding heuristics so temporary table-constructor values are not mistaken for later generic-for locals.

## P11 Readiness

For standard Lua 5.1.5 chunks using the default double `lua_Number`, the crate is fully covered by this matrix and ready to layer 5.2-5.5 work. Remaining explicitly unsupported inputs are not Lua 5.1.5 double chunks: later-version chunk formats/opcodes, non-4-byte instruction words, unknown constant tags, unsupported scalar sizes, NaN source emission, and inexact 64-bit integral-number constants.
