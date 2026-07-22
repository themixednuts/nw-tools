//! Phase 5 control-flow reconstruction.

use crate::{
    LuaError,
    bytecode::OpcodeTable,
    chunk::Proto,
    decompile::{
        analysis, ast, boolean, naming::NameResolver, reconstruction::ReconstructionPlan,
        stmt_build::StatementBuilder,
    },
    ir::SsaFunction,
};

pub mod conditionals;
pub mod loops;
pub mod regions;

pub use regions::RegionTree;

/// Build the structured region tree for a function.
///
/// # Errors
///
/// Returns a decompiler error if the current CFG shape cannot be represented
/// by the Phase 5 region tree.
pub fn structure(
    _proto: &Proto,
    function: &SsaFunction,
    _table: &OpcodeTable,
) -> Result<RegionTree, LuaError> {
    let pc_map = conditionals::pc_to_block_map(function);
    let loops = loops::analyze(function, &pc_map);
    let facts = analysis::analyze(function);
    let booleans = boolean::analyze(function, &facts, &loops, &pc_map);
    RegionTree::build(function, &facts, &loops, &pc_map, &booleans)
}

/// Structure and lower a function to the compact Lua AST.
///
/// # Errors
///
/// Returns a decompiler error when expression or statement reconstruction for
/// a structured region fails.
pub fn lower(
    proto: &Proto,
    function: &SsaFunction,
    table: &OpcodeTable,
) -> Result<ast::Block, LuaError> {
    let names = NameResolver::new(proto, function);
    lower_with_names(proto, function, table, &names)
}

pub(crate) fn lower_with_names(
    proto: &Proto,
    function: &SsaFunction,
    table: &OpcodeTable,
    names: &NameResolver<'_>,
) -> Result<ast::Block, LuaError> {
    let pc_map = conditionals::pc_to_block_map(function);
    let loops = loops::analyze(function, &pc_map);
    let facts = analysis::analyze(function);
    let booleans = boolean::analyze(function, &facts, &loops, &pc_map);
    let tree = RegionTree::build(function, &facts, &loops, &pc_map, &booleans)?;
    let plan = ReconstructionPlan::build(
        proto,
        function,
        table,
        &facts,
        names,
        &booleans,
        Some(&tree),
    );
    let mut builder =
        StatementBuilder::new(proto, function, table, &facts, names, &booleans, &plan);
    tree.lower(function, &facts, names, &booleans, &mut builder)
}
