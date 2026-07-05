//! SSA value to compact expression reconstruction for Phase 4.

use bstr::BString;

use crate::{
    LuaError,
    bytecode::OpcodeTable,
    chunk::{Constant, Proto},
    decompile::ast::{self, Expr, Name},
    decompile::control_flow::conditionals,
    ir::{self, SsaFunction, SsaNode, SsaOp, SsaRef},
};

use super::{
    analysis::{DecompileAnalysis, NodeId, ValueId, for_each_use, node_has_observable_side_effect},
    boolean::{BooleanAnalysis, ValuePlan, ValuePlanKind, ValueTerm, normalize},
    closure, multi,
    naming::{NameResolver, is_valid_identifier},
};

mod boolean;
mod calls;
mod constructors;
mod core;
mod helpers;
mod inline;

#[cfg(test)]
mod tests;

use helpers::{
    constructor_mutation_table, direct_eval_order_refs, is_parent_constructor_mutation,
    is_pure_def, map_bin_op, map_un_op, op_uses_ref,
};
pub(crate) use helpers::{
    global_expr_from_name, ident_from_string_expr, index_expr, is_inlineable_def,
};

/// Builds expressions while remembering values materialized by statement
/// reconstruction.
#[derive(Debug)]
pub struct ExprBuilder<'a> {
    proto: &'a Proto,
    function: &'a SsaFunction,
    table: &'a OpcodeTable,
    analysis: &'a DecompileAnalysis,
    names: &'a NameResolver<'a>,
    booleans: &'a BooleanAnalysis,
    materialized: Vec<Vec<Option<Name>>>,
    visiting: Vec<Vec<bool>>,
    chain_inline_blocks: Vec<usize>,
}

impl<'a> ExprBuilder<'a> {
    #[must_use]
    pub fn new(
        proto: &'a Proto,
        function: &'a SsaFunction,
        table: &'a OpcodeTable,
        analysis: &'a DecompileAnalysis,
        names: &'a NameResolver<'a>,
        booleans: &'a BooleanAnalysis,
    ) -> Self {
        Self {
            proto,
            function,
            table,
            analysis,
            names,
            booleans,
            materialized: vec![Vec::new(); function.num_regs],
            visiting: vec![Vec::new(); function.num_regs],
            chain_inline_blocks: Vec::new(),
        }
    }

    /// Remember that a value was emitted as a statement and must be referenced
    /// by name afterward.
    pub fn mark_materialized(&mut self, reference: SsaRef, name: Name) {
        let Some(value) = ValueId::from_ref(reference) else {
            return;
        };
        let reg = usize::from(value.reg);
        let Ok(version) = usize::try_from(value.ver) else {
            return;
        };
        if reg >= self.materialized.len() {
            self.materialized.resize_with(reg + 1, Vec::new);
        }
        if version >= self.materialized[reg].len() {
            self.materialized[reg].resize(version + 1, None);
        }
        self.materialized[reg][version] = Some(name);
    }

    pub(crate) fn is_materialized(&self, reference: SsaRef) -> bool {
        self.materialized_name(reference).is_some()
    }

    pub(crate) fn with_chain_inline_blocks<T>(
        &mut self,
        blocks: &[usize],
        f: impl FnOnce(&mut Self) -> Result<T, LuaError>,
    ) -> Result<T, LuaError> {
        let old_len = self.chain_inline_blocks.len();
        self.chain_inline_blocks.extend(blocks.iter().copied());
        let result = f(self);
        self.chain_inline_blocks.truncate(old_len);
        result
    }
}
