use crate::{
    LuaError,
    decompile::{analysis::NodeId, ast::Expr, stmt_build::StatementBuilder},
    ir::{SsaNode, SsaOp},
};

use super::call_results;

pub(crate) fn fixed_vararg_assignment(
    builder: &mut StatementBuilder<'_>,
    node_id: NodeId,
    node: &SsaNode,
    count: i32,
) -> Result<crate::decompile::ast::Stmt, LuaError> {
    let SsaOp::VarArg { base, .. } = &node.op else {
        return Err(LuaError::Malformed(
            "fixed vararg assignment requested for non-vararg node".to_string(),
        ));
    };
    call_results::fixed_result_assignment(builder, node_id, node, *base, count - 1, Expr::VarArg)
}
