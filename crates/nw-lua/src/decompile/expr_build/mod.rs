//! SSA value to compact expression reconstruction for Phase 4.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

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
    analysis::{DecompileAnalysis, NodeId, ValueId},
    boolean::{BooleanAnalysis, ValuePlan, ValuePlanKind, ValueTerm, normalize},
    closure, multi,
    naming::{NameResolver, is_valid_identifier},
    reconstruction::ReconstructionPlan,
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
    constructor_mutation_table, direct_eval_order_refs, is_pure_def, map_bin_op, map_un_op,
};
pub(crate) use helpers::{
    global_expr_from_name, ident_from_string_expr, index_expr, is_inlineable_def,
};

/// Builds expressions from the immutable reconstruction plan.
#[derive(Debug)]
pub struct ExprBuilder<'a> {
    proto: &'a Proto,
    function: &'a SsaFunction,
    table: &'a OpcodeTable,
    analysis: &'a DecompileAnalysis,
    names: &'a NameResolver<'a>,
    booleans: &'a BooleanAnalysis,
    plan: &'a ReconstructionPlan,
    activated: Vec<Vec<bool>>,
    visiting: Vec<Vec<bool>>,
    chain_inline_blocks: Vec<usize>,
    inline_cache: RefCell<HashMap<(SsaRef, i32), bool>>,
    inline_visiting: RefCell<HashSet<(SsaRef, i32)>>,
    evaluation_index_cache: RefCell<HashMap<(SsaRef, NodeId), Option<usize>>>,
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
        plan: &'a ReconstructionPlan,
    ) -> Self {
        Self {
            proto,
            function,
            table,
            analysis,
            names,
            booleans,
            plan,
            activated: vec![Vec::new(); function.num_regs],
            visiting: vec![Vec::new(); function.num_regs],
            chain_inline_blocks: Vec::new(),
            inline_cache: RefCell::new(HashMap::new()),
            inline_visiting: RefCell::new(HashSet::new()),
            evaluation_index_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Activate a materialization decision after its declaration is emitted.
    pub fn activate(&mut self, reference: SsaRef) {
        let Some(value) = ValueId::from_ref(reference) else {
            return;
        };
        let reg = usize::from(value.reg);
        let Ok(version) = usize::try_from(value.ver) else {
            return;
        };
        if reg >= self.activated.len() {
            self.activated.resize_with(reg + 1, Vec::new);
        }
        if version >= self.activated[reg].len() {
            self.activated[reg].resize(version + 1, false);
        }
        self.activated[reg][version] = true;
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
