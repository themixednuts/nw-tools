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

/// Controls optional AST cleanup after the correctness-oriented decompile pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecompOptions {
    /// Apply the P9b idiomatic AST cleanup pass.
    pub idiomatic: bool,
}

impl DecompOptions {
    /// Options for the default user-facing decompiler output.
    #[must_use]
    pub const fn idiomatic() -> Self {
        Self { idiomatic: true }
    }

    /// Options for bytecode-structure-preserving core output.
    #[must_use]
    pub const fn core() -> Self {
        Self { idiomatic: false }
    }
}

impl Default for DecompOptions {
    fn default() -> Self {
        Self::idiomatic()
    }
}

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
    decompile_proto_with_options(proto, ssa, table, DecompOptions::default())
}

/// Reconstruct a Lua source block with explicit decompile options.
///
/// # Errors
///
/// Returns [`LuaError::Unsupported`] for later-phase constructs, or an
/// emit/decompile error from a sub-pass.
pub fn decompile_proto_with_options(
    proto: &Proto,
    ssa: &SsaFunction,
    table: &OpcodeTable,
    options: DecompOptions,
) -> Result<ast::Block, LuaError> {
    decompile_proto_with_options_and_module_stem(proto, ssa, table, options, None)
}

/// Reconstruct a Lua source block with an optional file-stem fallback.
///
/// # Errors
///
/// Returns [`LuaError::Unsupported`] for later-phase constructs, or an
/// emit/decompile error from a sub-pass.
pub fn decompile_proto_with_options_and_module_stem(
    proto: &Proto,
    ssa: &SsaFunction,
    table: &OpcodeTable,
    options: DecompOptions,
    fallback_module_stem: Option<&str>,
) -> Result<ast::Block, LuaError> {
    let names = naming::NameResolver::new(proto, ssa);
    let block = decompile_proto_with_names(proto, ssa, table, &names)?;
    if options.idiomatic {
        Ok(idiomatic::clean(
            block,
            idiomatic::context_for_proto_with_fallback(proto, fallback_module_stem),
        ))
    } else {
        Ok(block)
    }
}

pub(crate) fn decompile_proto_with_names(
    proto: &Proto,
    ssa: &SsaFunction,
    table: &OpcodeTable,
    names: &naming::NameResolver<'_>,
) -> Result<ast::Block, LuaError> {
    control_flow::lower_with_names(proto, ssa, table, names)
}
