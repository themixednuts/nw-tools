//! Phase 7 multi-value reconstruction.

use crate::{
    LuaError,
    decompile::{analysis::NodeId, ast::Stmt, stmt_build::StatementBuilder},
    ir::SsaNode,
};

pub(crate) mod assign;
pub(crate) mod call_results;
pub(crate) mod table_list;
pub(crate) mod vararg;

pub(crate) struct MultiEmit {
    pub stmt: Stmt,
    pub consumed: Vec<NodeId>,
}

pub(crate) fn try_emit(
    builder: &mut StatementBuilder<'_>,
    node_ids: &[NodeId],
    index: usize,
    node_id: NodeId,
    node: &SsaNode,
    skip: &dyn Fn(&SsaNode) -> bool,
) -> Result<Option<MultiEmit>, LuaError> {
    if let Some(emitted) = table_list::try_emit(builder, node_ids, index, node_id, node, skip)? {
        return Ok(Some(emitted));
    }
    if let Some(emitted) = assign::try_emit_swap(builder, node_ids, index, node, skip)? {
        return Ok(Some(emitted));
    }
    assign::try_emit_multi_local(builder, node_ids, index, node, skip)
}
