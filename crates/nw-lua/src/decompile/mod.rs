//! SSA/region decompilation into the compact decompiler IR.

use crate::{LuaError, bytecode::OpcodeTable, chunk::Proto, ir::SsaFunction};

pub mod analysis;
pub mod ast;
pub mod boolean;
pub mod closure;
pub mod control_flow;
pub mod expr_build;
pub mod idiomatic;
pub mod naming;
pub mod region;
pub mod stmt_build;

pub mod multi;

/// Reconstruct a Lua source block for one prototype from its SSA form.
///
/// # Errors
///
/// Returns [`LuaError::Unsupported`] for later-phase constructs, or an
/// emit/decompile error from a sub-pass.
pub fn decompile_proto(
    proto: &Proto,
    ssa: &SsaFunction,
    table: &OpcodeTable,
) -> Result<ast::Block, LuaError> {
    let names = naming::NameResolver::new(proto, ssa);
    let block = decompile_proto_with_names(proto, ssa, table, &names)?;
    Ok(idiomatic::clean(block, idiomatic::context_for_proto(proto)))
}

pub(crate) fn decompile_proto_with_names(
    proto: &Proto,
    ssa: &SsaFunction,
    table: &OpcodeTable,
    names: &naming::NameResolver<'_>,
) -> Result<ast::Block, LuaError> {
    control_flow::lower_with_names(proto, ssa, table, names)
}
