//! Phase 7 multi-value reconstruction.

use crate::{
    LuaError,
    decompile::{ast::Stmt, stmt_build::StatementBuilder},
};

pub(crate) mod assign;
pub(crate) mod call_results;
pub(crate) mod plan;
pub(crate) mod table_constructor;
pub(crate) mod table_list;
pub(crate) mod vararg;

pub(crate) fn emit(
    builder: &mut StatementBuilder<'_>,
    plan: &plan::MultiNodePlan,
) -> Result<Vec<Stmt>, LuaError> {
    match plan {
        plan::MultiNodePlan::TableConstructor(plan) => {
            table_list::emit(builder, plan).map(|stmt| vec![stmt])
        }
        plan::MultiNodePlan::CallTransfer(plan) => assign::emit_call_transfer(builder, plan),
        plan::MultiNodePlan::Swap(plan) => assign::emit_swap(builder, plan).map(|stmt| vec![stmt]),
        plan::MultiNodePlan::MultiLocal(plan) => {
            assign::emit_multi_local(builder, plan).map(|stmt| vec![stmt])
        }
    }
}
