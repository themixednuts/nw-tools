//! Linear SSA node stream to statement reconstruction for Phase 4.

use std::collections::HashSet;

use bstr::BString;

use crate::{
    LuaError,
    bytecode::OpcodeTable,
    chunk::{Constant, Proto},
    decompile::ast::{self, Expr, FuncBody, Name, Stmt, TableField},
    ir::{SsaFunction, SsaNode, SsaOp, SsaRef},
};

use super::{
    analysis::{DecompileAnalysis, NodeId},
    boolean::{BooleanAnalysis, ConditionChain, ValuePlan, normalize},
    closure,
    expr_build::{ExprBuilder, global_expr_from_name, index_expr, is_inlineable_def},
    multi,
    naming::LocalBinding,
    naming::NameResolver,
    region::LinearRegion,
};

const LFIELDS_PER_FLUSH: i64 = 50;

/// Build a compact AST block from a linear SSA region.
pub fn build_block(
    proto: &Proto,
    function: &SsaFunction,
    table: &OpcodeTable,
    region: &LinearRegion,
    analysis: &DecompileAnalysis,
    names: &NameResolver<'_>,
) -> Result<ast::Block, LuaError> {
    let booleans = BooleanAnalysis::empty();
    StatementBuilder::new(proto, function, table, analysis, names, &booleans).build(region)
}

mod conditions;
mod emit;
mod helpers;
mod materialize;

use helpers::is_stable_assignment_target;
pub(crate) use helpers::{assign_one, local_one};

pub(crate) struct StatementBuilder<'a> {
    proto: &'a Proto,
    function: &'a SsaFunction,
    table: &'a OpcodeTable,
    analysis: &'a DecompileAnalysis,
    names: &'a NameResolver<'a>,
    booleans: &'a BooleanAnalysis,
    exprs: ExprBuilder<'a>,
    declared_locals: HashSet<usize>,
    declared_synthetic_names: HashSet<Name>,
    declared_phi_regs: HashSet<u16>,
    forced_materialized: HashSet<SsaRef>,
    consumed_nodes: HashSet<NodeId>,
}

impl<'a> StatementBuilder<'a> {
    pub(crate) fn new(
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
            exprs: ExprBuilder::new(proto, function, table, analysis, names, booleans),
            declared_locals: names.initially_declared_locals().into_iter().collect(),
            declared_synthetic_names: HashSet::new(),
            declared_phi_regs: HashSet::new(),
            forced_materialized: HashSet::new(),
            consumed_nodes: HashSet::new(),
        }
    }

    fn build(mut self, region: &LinearRegion) -> Result<ast::Block, LuaError> {
        Ok(ast::Block::new(self.emit_linear_region(region)?))
    }

    pub(crate) fn emit_linear_region(
        &mut self,
        region: &LinearRegion,
    ) -> Result<Vec<Stmt>, LuaError> {
        self.emit_node_ids(region.nodes.iter().copied(), |_| false)
    }

    pub(crate) fn emit_node_ids(
        &mut self,
        nodes: impl IntoIterator<Item = NodeId>,
        skip: impl Fn(&SsaNode) -> bool,
    ) -> Result<Vec<Stmt>, LuaError> {
        let node_ids = nodes.into_iter().collect::<Vec<_>>();
        let mut stmts = Vec::new();

        for (index, node_id) in node_ids.iter().copied().enumerate() {
            if self.consumed_nodes.contains(&node_id) {
                continue;
            }
            let Some(node) = self.analysis.node(self.function, node_id) else {
                continue;
            };
            if node.is_meta_only || skip(node) {
                continue;
            }
            if let Some(emitted) = multi::try_emit(self, &node_ids, index, node_id, node, &skip)? {
                for consumed in emitted.consumed {
                    self.consumed_nodes.insert(consumed);
                }
                let is_return = matches!(emitted.stmt, Stmt::Return(_));
                stmts.push(emitted.stmt);
                if is_return {
                    break;
                }
                continue;
            }
            if let Some(stmt) = self.emit_node(node_id, node)? {
                let is_return = matches!(stmt, Stmt::Return(_));
                stmts.push(stmt);
                if is_return {
                    break;
                }
            }
        }

        Ok(stmts)
    }

    pub(crate) fn expr_for_node(&mut self, node: &SsaNode) -> Result<Expr, LuaError> {
        self.exprs.node_expr(node)
    }

    pub(crate) fn expr_for_ref(&mut self, reference: SsaRef, pc: i32) -> Result<Expr, LuaError> {
        self.exprs.expr_for_ref(reference, pc)
    }

    pub(crate) fn expr_for_fixed_last_ref(
        &mut self,
        reference: SsaRef,
        pc: i32,
    ) -> Result<Expr, LuaError> {
        self.exprs.expr_for_fixed_last_ref(reference, pc)
    }

    pub(crate) fn call_expr(
        &mut self,
        func: SsaRef,
        args: &[SsaRef],
        arg_count: i32,
        pc: i32,
    ) -> Result<Expr, LuaError> {
        self.exprs
            .call_expr_with_arg_count(func, args, arg_count, pc)
    }

    pub(crate) fn node(&self, id: NodeId) -> Option<&SsaNode> {
        self.analysis.node(self.function, id)
    }

    pub(crate) fn proto(&self) -> &'a Proto {
        self.proto
    }

    pub(crate) fn function(&self) -> &'a SsaFunction {
        self.function
    }

    pub(crate) fn table(&self) -> &'a OpcodeTable {
        self.table
    }

    pub(crate) fn names(&self) -> &'a NameResolver<'a> {
        self.names
    }

    pub(crate) fn def_at_reg(&self, id: NodeId, reg: u16) -> Option<SsaRef> {
        self.analysis.def_at_reg(id, reg)
    }

    pub(crate) fn binding_for_def(&self, reg: u16, pc: i32) -> Option<LocalBinding> {
        self.names.binding_for_def(reg, pc)
    }

    pub(crate) fn binding_for_use(&self, reg: u16, pc: i32) -> Option<LocalBinding> {
        self.names.binding_for_use(reg, pc)
    }

    pub(crate) fn name_for_ref(&self, reference: SsaRef, pc: i32) -> Name {
        self.names.name_for_ref(reference, pc)
    }

    pub(crate) fn name_for_binding_def(&self, binding: &LocalBinding, reference: SsaRef) -> Name {
        self.names.name_for_binding_def(binding, reference)
    }

    pub(crate) fn is_local_declared(&self, index: usize) -> bool {
        self.declared_locals.contains(&index)
    }

    pub(crate) fn mark_local_declared(&mut self, index: usize) {
        self.declared_locals.insert(index);
    }

    pub(crate) fn mark_synthetic_declared(&mut self, name: Name) {
        self.declared_synthetic_names.insert(name);
    }

    pub(crate) fn mark_materialized(&mut self, reference: SsaRef, name: Name) {
        self.exprs.mark_materialized(reference, name);
    }

    pub(crate) fn force_materialized(&mut self, reference: SsaRef) {
        if matches!(reference, SsaRef::Reg { .. }) {
            self.forced_materialized.insert(reference);
        }
    }
}
