//! Lua bytecode parsing and decompilation support.
//!
//! The public API exposes parsing, disassembly, SSA dumps, and decompilation
//! for Lua 5.1 binary chunks.

pub mod bytecode;
pub mod chunk;
pub mod disasm;
pub mod error;
pub mod ir;
pub mod version;

pub mod decompile;
pub mod emit;
pub(crate) mod number;

pub use decompile::DecompOptions;
pub use emit::to_source;
pub use error::LuaError;

/// Parse a Lua binary chunk.
///
/// # Errors
///
/// Returns [`LuaError`] when the input is truncated, has an invalid header, uses
/// an unsupported Lua version, or contains malformed chunk data.
pub fn parse_chunk(bytes: &[u8]) -> Result<chunk::Chunk, LuaError> {
    chunk::parse(bytes)
}

/// Parse and disassemble a Lua binary chunk.
///
/// # Errors
///
/// Returns [`LuaError`] when parsing fails or the chunk version has no built-in
/// opcode table in this phase.
pub fn disassemble(bytes: &[u8]) -> Result<String, LuaError> {
    let (chunk, table) = parse_with_builtin_table(bytes)?;
    Ok(disassemble_chunk_with_table(&chunk, &table))
}

/// Parse and disassemble a Lua binary chunk with a caller-supplied opcode table.
///
/// # Errors
///
/// Returns [`LuaError`] when parsing fails or the table version does not match
/// the chunk version.
pub fn disassemble_with(bytes: &[u8], table: &bytecode::OpcodeTable) -> Result<String, LuaError> {
    let chunk = parse_chunk(bytes)?;
    ensure_compatible_table(&chunk, table)?;
    Ok(disassemble_chunk_with_table(&chunk, table))
}

/// Parse a Lua binary chunk and dump Phase 2 SSA for every prototype.
///
/// # Errors
///
/// Returns [`LuaError`] when parsing fails or the chunk version has no built-in
/// opcode table in this phase.
pub fn ssa_dump(bytes: &[u8]) -> Result<String, LuaError> {
    let (chunk, table) = parse_with_builtin_table(bytes)?;
    Ok(ssa_dump_chunk_with_table(&chunk, &table))
}

/// Parse a Lua binary chunk and dump SSA with a caller-supplied opcode table.
///
/// # Errors
///
/// Returns [`LuaError`] when parsing fails or the table version does not match
/// the chunk version.
pub fn ssa_dump_with(bytes: &[u8], table: &bytecode::OpcodeTable) -> Result<String, LuaError> {
    let chunk = parse_chunk(bytes)?;
    ensure_compatible_table(&chunk, table)?;
    Ok(ssa_dump_chunk_with_table(&chunk, table))
}

/// Parse and decompile a Lua binary chunk into formatted Lua source.
///
/// # Errors
///
/// Returns [`LuaError`] when parsing, SSA construction, reconstruction, or
/// source emission fails.
pub fn decompile(bytes: &[u8]) -> Result<String, LuaError> {
    decompile_with_options(bytes, DecompOptions::default())
}

/// Parse and decompile a Lua binary chunk into core, bytecode-shaped Lua source.
///
/// This runs the full correctness pipeline but skips the idiomatic AST cleanup
/// pass so validation can compare bytecode structure without style rewrites.
///
/// # Errors
///
/// Returns [`LuaError`] when parsing, SSA construction, reconstruction, or
/// source emission fails.
pub fn decompile_core(bytes: &[u8]) -> Result<String, LuaError> {
    decompile_with_options(bytes, DecompOptions::core())
}

/// Parse and decompile a Lua binary chunk with explicit decompile options.
///
/// # Errors
///
/// Returns [`LuaError`] when parsing, SSA construction, reconstruction, or
/// source emission fails.
pub fn decompile_with_options(bytes: &[u8], options: DecompOptions) -> Result<String, LuaError> {
    decompile_with_options_and_module_stem(bytes, options, None)
}

/// Parse and decompile a Lua binary chunk with an optional file-stem fallback.
///
/// The fallback is used only when the chunk's own source name is empty.
///
/// # Errors
///
/// Returns [`LuaError`] when parsing, SSA construction, reconstruction, or
/// source emission fails.
pub fn decompile_with_options_and_module_stem(
    bytes: &[u8],
    options: DecompOptions,
    fallback_module_stem: Option<&str>,
) -> Result<String, LuaError> {
    let (chunk, table) = parse_with_builtin_table(bytes)?;
    decompile_chunk_with_table_options_and_module_stem(
        &chunk,
        &table,
        options,
        fallback_module_stem,
    )
}

/// Parse and decompile a Lua binary chunk with a caller-supplied opcode table.
///
/// # Errors
///
/// Returns [`LuaError`] when parsing fails, the table version does not match the
/// chunk version, SSA/decompilation fails, or source emission fails.
pub fn decompile_with(bytes: &[u8], table: &bytecode::OpcodeTable) -> Result<String, LuaError> {
    decompile_with_table_options(bytes, table, DecompOptions::default())
}

/// Parse and decompile a Lua binary chunk with explicit options and opcode table.
///
/// # Errors
///
/// Returns [`LuaError`] when parsing fails, the table version does not match the
/// chunk version, SSA/decompilation fails, or source emission fails.
pub fn decompile_with_table_options(
    bytes: &[u8],
    table: &bytecode::OpcodeTable,
    options: DecompOptions,
) -> Result<String, LuaError> {
    decompile_with_table_options_and_module_stem(bytes, table, options, None)
}

/// Parse and decompile a Lua binary chunk with explicit options, opcode table,
/// and an optional file-stem fallback.
///
/// # Errors
///
/// Returns [`LuaError`] when parsing fails, the table version does not match the
/// chunk version, SSA/decompilation fails, or source emission fails.
pub fn decompile_with_table_options_and_module_stem(
    bytes: &[u8],
    table: &bytecode::OpcodeTable,
    options: DecompOptions,
    fallback_module_stem: Option<&str>,
) -> Result<String, LuaError> {
    let chunk = parse_chunk(bytes)?;
    ensure_compatible_table(&chunk, table)?;
    decompile_chunk_with_table_options_and_module_stem(&chunk, table, options, fallback_module_stem)
}

/// Decompile a chunk and prepend best-effort disassembly annotations as Lua comments.
///
/// # Errors
///
/// Returns [`LuaError`] when parsing, disassembly, decompilation, or source
/// emission fails.
pub fn decompile_annotated(bytes: &[u8]) -> Result<String, LuaError> {
    let (chunk, table) = parse_with_builtin_table(bytes)?;
    Ok(annotate_source(
        &disassemble_chunk_with_table(&chunk, &table),
        &decompile_chunk_with_table(&chunk, &table)?,
    ))
}

/// Decompile with a caller-supplied opcode table and prepend disassembly comments.
///
/// # Errors
///
/// Returns [`LuaError`] when parsing fails, the table version does not match the
/// chunk version, SSA/decompilation fails, or source emission fails.
pub fn decompile_annotated_with(
    bytes: &[u8],
    table: &bytecode::OpcodeTable,
) -> Result<String, LuaError> {
    let chunk = parse_chunk(bytes)?;
    ensure_compatible_table(&chunk, table)?;
    Ok(annotate_source(
        &disassemble_chunk_with_table(&chunk, table),
        &decompile_chunk_with_table(&chunk, table)?,
    ))
}

fn parse_with_builtin_table(
    bytes: &[u8],
) -> Result<(chunk::Chunk, bytecode::OpcodeTable), LuaError> {
    let chunk = parse_chunk(bytes)?;
    let table = bytecode::OpcodeTable::builtin(chunk.header.version)?;
    Ok((chunk, table))
}

fn disassemble_chunk_with_table(chunk: &chunk::Chunk, table: &bytecode::OpcodeTable) -> String {
    let mut out = format!(
        "-- Lua {} Disassembly --\n\n",
        version_label(chunk.header.version)
    );
    out.push_str(&disasm::disassemble_proto(&chunk.root, table));
    out
}

fn ssa_dump_chunk_with_table(chunk: &chunk::Chunk, table: &bytecode::OpcodeTable) -> String {
    ir::dump::dump_proto_tree(&chunk.root, table)
}

fn decompile_chunk_with_table(
    chunk: &chunk::Chunk,
    table: &bytecode::OpcodeTable,
) -> Result<String, LuaError> {
    decompile_chunk_with_table_options(chunk, table, DecompOptions::default())
}

fn decompile_chunk_with_table_options(
    chunk: &chunk::Chunk,
    table: &bytecode::OpcodeTable,
    options: DecompOptions,
) -> Result<String, LuaError> {
    decompile_chunk_with_table_options_and_module_stem(chunk, table, options, None)
}

fn decompile_chunk_with_table_options_and_module_stem(
    chunk: &chunk::Chunk,
    table: &bytecode::OpcodeTable,
    options: DecompOptions,
    fallback_module_stem: Option<&str>,
) -> Result<String, LuaError> {
    let ssa = ir::build_ssa(&chunk.root, table);
    let block = decompile::decompile_proto_with_options_and_module_stem(
        &chunk.root,
        &ssa,
        table,
        options,
        fallback_module_stem,
    )?;
    emit::to_source(&block)
}

fn ensure_compatible_table(
    chunk: &chunk::Chunk,
    table: &bytecode::OpcodeTable,
) -> Result<(), LuaError> {
    if table.version != chunk.header.version {
        return Err(LuaError::Malformed(format!(
            "opcode table version {} does not match chunk version {}",
            version_label(table.version),
            version_label(chunk.header.version)
        )));
    }
    Ok(())
}

fn annotate_source(disassembly: &str, source: &str) -> String {
    let mut out = String::from("-- disassembly annotations\n");
    for line in disassembly.lines() {
        if line.is_empty() {
            out.push_str("--\n");
        } else {
            out.push_str("-- ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push('\n');
    out.push_str(source);
    out
}

fn version_label(version: version::LuaVersion) -> &'static str {
    match version {
        version::LuaVersion::V51 => "5.1",
        version::LuaVersion::V52 => "5.2",
        version::LuaVersion::V53 => "5.3",
        version::LuaVersion::V54 => "5.4",
        version::LuaVersion::V55 => "5.5",
    }
}
