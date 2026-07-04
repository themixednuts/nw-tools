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
    boolean::{BooleanAnalysis, ConditionChain, ValuePlan, ValuePlanKind, normalize},
    closure,
    expr_build::{ExprBuilder, global_expr_from_name, index_expr},
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
            exprs: ExprBuilder::new(proto, function, table, analysis, names),
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

    pub(crate) fn condition_for_branch(
        &mut self,
        node: &SsaNode,
        invert: bool,
    ) -> Result<Expr, LuaError> {
        let cond = normalize::normalize(self.exprs.node_expr(node)?);
        Ok(if invert {
            normalize::invert(cond)
        } else {
            cond
        })
    }

    pub(crate) fn compound_condition(
        &mut self,
        chain: &ConditionChain,
        invert: bool,
    ) -> Result<Expr, LuaError> {
        let Some(last) = chain.segments.last() else {
            return Ok(Expr::True);
        };
        let mut expr = self.condition_segment_expr(last)?;

        for segment in chain.segments.iter().rev().skip(1) {
            let Some(connector) = segment.connector else {
                continue;
            };
            let lhs = self.condition_segment_expr(segment)?;
            expr = Expr::Binary {
                op: connector.ast_op(),
                lhs: Box::new(lhs),
                rhs: Box::new(expr),
            };
        }

        expr = normalize::normalize(expr);
        Ok(if invert {
            normalize::invert(expr)
        } else {
            expr
        })
    }

    fn condition_segment_expr(
        &mut self,
        segment: &crate::decompile::boolean::ConditionSegment,
    ) -> Result<Expr, LuaError> {
        let Some(node) = self.analysis.node(self.function, segment.node) else {
            return Ok(Expr::True);
        };
        self.condition_for_branch(node, segment.inverted)
    }

    pub(crate) fn declare_phi_local(&mut self, reference: SsaRef, pc: i32) -> Option<Stmt> {
        let name = self.names.collapsed_name_for_ref(reference, pc);
        self.exprs.mark_materialized(reference, name.clone());

        let reg = reference.reg_index()?;
        if let Some(binding) = self.names.binding_for_def(reg, pc) {
            if self.declared_locals.insert(binding.index) {
                let name = self.names.name_for_binding_def(&binding, reference);
                return Some(Stmt::Local {
                    names: vec![name],
                    attribs: Vec::new(),
                    values: Vec::new(),
                });
            }
            return None;
        }

        if self.declared_phi_regs.insert(reg) {
            self.declared_synthetic_names.insert(name.clone());
            return Some(Stmt::Local {
                names: vec![name],
                attribs: Vec::new(),
                values: Vec::new(),
            });
        }
        None
    }

    pub(crate) fn phi_assignment(
        &mut self,
        dest: SsaRef,
        operand: SsaRef,
        pc: i32,
    ) -> Result<Option<Stmt>, LuaError> {
        let name = self.names.collapsed_name_for_ref(dest, pc);
        self.exprs.mark_materialized(dest, name.clone());
        let value = self.exprs.expr_for_ref(operand, pc)?;
        if value == Expr::Name(name.clone()) {
            return Ok(None);
        }
        Ok(Some(assign_one(Expr::Name(name), value)))
    }

    fn emit_node(&mut self, node_id: NodeId, node: &SsaNode) -> Result<Option<Stmt>, LuaError> {
        match &node.op {
            SsaOp::Nop | SsaOp::Jump { .. } | SsaOp::Close { .. } => Ok(None),
            SsaOp::Phi { .. } => self.emit_boolean_phi(node),
            SsaOp::Move { .. }
            | SsaOp::LoadK { .. }
            | SsaOp::LoadBool { .. }
            | SsaOp::LoadNil { .. }
            | SsaOp::GetUpval { .. }
            | SsaOp::GetGlobal { .. }
            | SsaOp::GetTable { .. }
            | SsaOp::NewTable { .. }
            | SsaOp::SelfOp { .. }
            | SsaOp::BinOp { .. }
            | SsaOp::UnOp { .. }
            | SsaOp::Concat { .. } => self.emit_value_def(node),
            SsaOp::Closure { .. } => self.emit_closure_value_def(node),
            SsaOp::SetGlobal { src, idx } => {
                let target = self.global_target(*idx)?;
                let value = self.exprs.expr_for_ref(*src, node.pc)?;
                Ok(Some(assign_one(target, value)))
            }
            SsaOp::SetUpval { src, upval } => {
                let target = Expr::Name(self.names.upvalue_name(*upval));
                let value = self.exprs.expr_for_ref(*src, node.pc)?;
                Ok(Some(assign_one(target, value)))
            }
            SsaOp::SetTable { table, key, value } => {
                if self.is_inline_constructor_mutation(node) {
                    return Ok(None);
                }
                let table = self.exprs.expr_for_ref(*table, node.pc)?;
                let key = self.exprs.expr_for_ref(*key, node.pc)?;
                let target = index_expr(table, key);
                let value = self.exprs.expr_for_ref(*value, node.pc)?;
                Ok(Some(assign_one(target, value)))
            }
            SsaOp::Call {
                func,
                args,
                return_count,
                arg_count,
                ..
            } => self.emit_call(node_id, node, *func, args, *arg_count, *return_count),
            SsaOp::TailCall {
                func,
                args,
                arg_count,
                ..
            } => {
                let call = self.call_expr(*func, args, *arg_count, node.pc)?;
                Ok(Some(Stmt::Return(vec![call])))
            }
            SsaOp::Return { values, count, .. } => self.emit_return(node, values, *count),
            SsaOp::VarArg { .. } => self.emit_vararg(node_id, node),
            SsaOp::Branch { .. }
            | SsaOp::ForPrep { .. }
            | SsaOp::ForLoop { .. }
            | SsaOp::TForLoop { .. } => Ok(None),
            SsaOp::SetList {
                table,
                values,
                base,
                count,
                batch,
            } => {
                if self.is_inline_constructor_mutation(node) {
                    return Ok(None);
                }
                self.emit_setlist_fallback(node, *table, values, *base, *count, *batch)
            }
        }
    }

    fn emit_setlist_fallback(
        &mut self,
        node: &SsaNode,
        table: SsaRef,
        values: &[SsaRef],
        base: u16,
        count: i32,
        batch: i32,
    ) -> Result<Option<Stmt>, LuaError> {
        if count == 0 {
            return self.emit_open_setlist_fallback(node, table, values, base, batch);
        }
        if values.is_empty() {
            return Ok(None);
        }
        if batch <= 0 {
            return Err(LuaError::Unsupported(format!(
                "invalid SETLIST batch {batch} at pc={}",
                node.pc
            )));
        }

        let table_expr = self.exprs.expr_for_ref(table, node.pc)?;
        if !is_stable_assignment_target(&table_expr) {
            return Err(LuaError::Unsupported(format!(
                "SETLIST fallback needs a materialized table target (pc={} base=R{base})",
                node.pc
            )));
        }

        let first_index = (i64::from(batch) - 1) * LFIELDS_PER_FLUSH + 1;
        let mut targets = Vec::with_capacity(values.len());
        let mut rhs = Vec::with_capacity(values.len());
        let last_index = values.len().saturating_sub(1);
        for (offset, value) in values.iter().copied().enumerate() {
            let offset = i64::try_from(offset).map_err(|_| {
                LuaError::Malformed("SETLIST offset does not fit in i64".to_string())
            })?;
            targets.push(index_expr(
                table_expr.clone(),
                Expr::Integer(first_index + offset),
            ));
            let value_expr = if usize::try_from(offset).ok() == Some(last_index) {
                self.exprs.expr_for_fixed_last_ref(value, node.pc)?
            } else {
                self.exprs.expr_for_ref(value, node.pc)?
            };
            rhs.push(value_expr);
        }

        Ok(Some(Stmt::Assign {
            targets,
            values: rhs,
        }))
    }

    fn emit_open_setlist_fallback(
        &mut self,
        node: &SsaNode,
        table: SsaRef,
        values: &[SsaRef],
        base: u16,
        batch: i32,
    ) -> Result<Option<Stmt>, LuaError> {
        if values.is_empty() {
            return Ok(None);
        }
        if batch <= 0 {
            return Err(LuaError::Unsupported(format!(
                "invalid SETLIST batch {batch} at pc={}",
                node.pc
            )));
        }

        let table_expr = self.exprs.expr_for_ref(table, node.pc)?;
        if !is_stable_assignment_target(&table_expr) {
            return Err(LuaError::Unsupported(format!(
                "SETLIST fallback needs a materialized table target (pc={} base=R{base})",
                node.pc
            )));
        }

        let first_index = (i64::from(batch) - 1) * LFIELDS_PER_FLUSH + 1;
        let pack_name = Name::from(format!("__nw_lua_pack_{}", node.pc));
        let values_name = Name::from(format!("__nw_lua_values_{}", node.pc));
        let index_name = Name::from(format!("__nw_lua_index_{}", node.pc));

        let pack_function = Stmt::Function {
            name: pack_name.clone(),
            local: true,
            body: FuncBody::new(
                Vec::new(),
                true,
                ast::Block::new(vec![Stmt::Return(vec![Expr::Table(vec![
                    TableField::Named {
                        name: Name::from("n"),
                        value: Expr::Call {
                            func: Box::new(Expr::Global(BString::from("select"))),
                            args: vec![Expr::Str(BString::from("#")), Expr::VarArg],
                            method: None,
                        },
                    },
                    TableField::List(Expr::VarArg),
                ])])]),
            ),
        };

        let last_index = values.len().saturating_sub(1);
        let args = values
            .iter()
            .copied()
            .enumerate()
            .map(|(index, value)| {
                if index == last_index {
                    self.exprs.expr_for_ref(value, node.pc)
                } else {
                    self.exprs.expr_for_fixed_last_ref(value, node.pc)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let capture_values = Stmt::Local {
            names: vec![values_name.clone()],
            attribs: Vec::new(),
            values: vec![Expr::Call {
                func: Box::new(Expr::Name(pack_name)),
                args,
                method: None,
            }],
        };

        let target_index = if first_index == 1 {
            Expr::Name(index_name.clone())
        } else {
            Expr::Binary {
                op: ast::BinOp::Add,
                lhs: Box::new(Expr::Integer(first_index - 1)),
                rhs: Box::new(Expr::Name(index_name.clone())),
            }
        };
        let copy_loop = Stmt::NumericFor {
            var: index_name.clone(),
            start: Expr::Integer(1),
            stop: Expr::Field {
                obj: Box::new(Expr::Name(values_name.clone())),
                name: Name::from("n"),
            },
            step: None,
            body: ast::Block::new(vec![Stmt::Assign {
                targets: vec![index_expr(table_expr, target_index)],
                values: vec![index_expr(Expr::Name(values_name), Expr::Name(index_name))],
            }]),
        };

        Ok(Some(Stmt::Do(ast::Block::new(vec![
            pack_function,
            capture_values,
            copy_loop,
        ]))))
    }

    fn emit_boolean_phi(&mut self, node: &SsaNode) -> Result<Option<Stmt>, LuaError> {
        let Some(plan) = self.booleans.value_for_phi(node.dest) else {
            return Ok(None);
        };
        let value = self.value_plan_expr(plan)?;
        Ok(Some(self.materialize_value(plan.dest, plan.pc, value)))
    }

    fn value_plan_expr(&mut self, plan: &ValuePlan) -> Result<Expr, LuaError> {
        let expr = match &plan.kind {
            ValuePlanKind::Binary { left, op, right } => Expr::Binary {
                op: op.ast_op(),
                lhs: Box::new(self.exprs.expr_for_ref(*left, plan.pc)?),
                rhs: Box::new(self.exprs.expr_for_ref(*right, plan.pc)?),
            },
            ValuePlanKind::Ternary {
                first,
                second,
                fallback,
            } => {
                let selected = Expr::Binary {
                    op: ast::BinOp::And,
                    lhs: Box::new(self.exprs.expr_for_ref(*first, plan.pc)?),
                    rhs: Box::new(self.exprs.expr_for_ref(*second, plan.pc)?),
                };
                Expr::Binary {
                    op: ast::BinOp::Or,
                    lhs: Box::new(selected),
                    rhs: Box::new(self.exprs.expr_for_ref(*fallback, plan.pc)?),
                }
            }
            ValuePlanKind::Condition { branch, inverted } => {
                let Some(node) = self.analysis.node(self.function, *branch) else {
                    return Ok(Expr::True);
                };
                self.condition_for_branch(node, *inverted)?
            }
        };
        Ok(normalize::normalize(expr))
    }

    fn emit_value_def(&mut self, node: &SsaNode) -> Result<Option<Stmt>, LuaError> {
        if matches!(&node.op, SsaOp::LoadNil { start, end } if start < end)
            && let Some(stmt) = self.emit_loadnil_range(node)?
        {
            return Ok(Some(stmt));
        }

        if !self.should_materialize(node) {
            return Ok(None);
        }

        let value = self.exprs.node_expr(node)?;
        Ok(Some(self.materialize_value(node.dest, node.pc, value)))
    }

    fn emit_closure_value_def(&mut self, node: &SsaNode) -> Result<Option<Stmt>, LuaError> {
        if let Some(stmt) = closure::try_local_function(self, node)? {
            return Ok(Some(stmt));
        }
        self.emit_value_def(node)
    }

    fn emit_loadnil_range(&mut self, node: &SsaNode) -> Result<Option<Stmt>, LuaError> {
        let SsaOp::LoadNil { start, end } = &node.op else {
            return Ok(None);
        };

        let mut names = Vec::new();
        for reg in *start..=*end {
            let Some(binding) = self.names.binding_for_def(reg, node.pc) else {
                continue;
            };
            if self.declared_locals.insert(binding.index) {
                let reference = if reg == *start {
                    node.dest
                } else {
                    SsaRef::Reg {
                        reg,
                        ver: node.dest.version().unwrap_or(0),
                    }
                };
                names.push(self.names.name_for_binding_def(&binding, reference));
            }
        }

        if names.is_empty() {
            return Ok(None);
        }

        if let Some(first) = names.first().cloned() {
            self.exprs.mark_materialized(node.dest, first);
        }

        Ok(Some(Stmt::Local {
            names,
            attribs: Vec::new(),
            values: Vec::new(),
        }))
    }

    fn should_materialize(&self, node: &SsaNode) -> bool {
        if !matches!(node.dest, SsaRef::Reg { .. }) {
            return false;
        }
        if self.self_op_consumed_by_call(node) {
            return false;
        }
        if self.forced_materialized.contains(&node.dest) {
            return true;
        }
        if self.analysis.facts(node.dest).upvalue_captures > 0 {
            return true;
        }
        if let Some(reg) = node.dest.reg_index()
            && let Some(binding) = self.names.binding_for_def(reg, node.pc)
        {
            if self
                .analysis
                .has_later_def_before(reg, node.pc, binding.start_pc)
            {
                return false;
            }
            return true;
        }
        if self.analysis.has_mutating_table_use(node.dest)
            && !self.exprs.can_inline_ref(node.dest, node.pc)
        {
            return true;
        }
        let uses = self.analysis.real_use_count(node.dest);
        uses > 0 && !self.exprs.can_inline_ref(node.dest, node.pc)
    }

    fn self_op_consumed_by_call(&self, node: &SsaNode) -> bool {
        if !matches!(&node.op, SsaOp::SelfOp { .. }) {
            return false;
        }
        let uses = self.analysis.real_uses(node.dest);
        let [use_id] = uses else {
            return false;
        };
        self.analysis
            .node(self.function, *use_id)
            .is_some_and(|use_node| {
                matches!(
                    &use_node.op,
                    SsaOp::Call { func, .. } | SsaOp::TailCall { func, .. } if *func == node.dest
                )
            })
    }

    fn is_inline_constructor_mutation(&self, node: &SsaNode) -> bool {
        let table = match &node.op {
            SsaOp::SetTable { table, .. } | SsaOp::SetList { table, .. } => *table,
            _ => return false,
        };
        let Some(def_id) = self.analysis.def_site(table) else {
            return false;
        };
        let Some(def) = self.analysis.node(self.function, def_id) else {
            return false;
        };
        let Some(table_reg) = def.dest.reg_index() else {
            return false;
        };
        matches!(&def.op, SsaOp::NewTable { .. })
            && (multi::table_list::is_matching_settable(node, def.dest, table_reg)
                || multi::table_list::is_matching_setlist(node, def.dest, table_reg))
            && self.exprs.can_inline_ref(def.dest, node.pc)
    }

    fn emit_call(
        &mut self,
        node_id: NodeId,
        node: &SsaNode,
        func: SsaRef,
        args: &[SsaRef],
        arg_count: i32,
        return_count: i32,
    ) -> Result<Option<Stmt>, LuaError> {
        let call = self.call_expr(func, args, arg_count, node.pc)?;

        match return_count {
            0 => {
                if self.analysis.real_use_count(node.dest) > 0 {
                    return Ok(None);
                }
                Ok(Some(Stmt::Call(call)))
            }
            1 => Ok(Some(Stmt::Call(call))),
            2 => {
                if self.should_materialize(node) {
                    Ok(Some(self.materialize_value(node.dest, node.pc, call)))
                } else if self.analysis.real_use_count(node.dest) == 0 {
                    Ok(Some(Stmt::Call(call)))
                } else {
                    Ok(None)
                }
            }
            _ => multi::call_results::fixed_call_assignment(
                self,
                node_id,
                node,
                func,
                args,
                arg_count,
                return_count,
            )
            .map(Some),
        }
    }

    fn emit_return(
        &mut self,
        node: &SsaNode,
        values: &[SsaRef],
        count: i32,
    ) -> Result<Option<Stmt>, LuaError> {
        if count == 1 && usize::try_from(node.pc).ok() == self.proto.code.len().checked_sub(1) {
            return Ok(None);
        }
        let fixed_last = count != 0;
        let last_index = values.len().saturating_sub(1);
        let values = values
            .iter()
            .copied()
            .enumerate()
            .map(|(index, value)| {
                if fixed_last && index == last_index {
                    self.expr_for_fixed_last_ref(value, node.pc)
                } else {
                    self.exprs.expr_for_ref(value, node.pc)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Stmt::Return(values)))
    }

    fn emit_vararg(&mut self, node_id: NodeId, node: &SsaNode) -> Result<Option<Stmt>, LuaError> {
        if let SsaOp::VarArg { count, .. } = &node.op
            && *count >= 3
        {
            return multi::vararg::fixed_vararg_assignment(self, node_id, node, *count).map(Some);
        }
        if let SsaOp::VarArg { count: 0, .. } = &node.op
            && self.analysis.real_use_count(node.dest) > 0
        {
            return Ok(None);
        }
        if self.should_materialize(node) {
            return Ok(Some(self.materialize_value(
                node.dest,
                node.pc,
                Expr::VarArg,
            )));
        }
        Ok(None)
    }

    pub(crate) fn materialize_value(&mut self, reference: SsaRef, pc: i32, value: Expr) -> Stmt {
        let binding = reference
            .reg_index()
            .and_then(|reg| self.names.binding_for_def(reg, pc));
        let declares_named_local = binding
            .as_ref()
            .is_some_and(|binding| !self.declared_locals.contains(&binding.index));
        let name = binding.as_ref().map_or_else(
            || self.names.name_for_ref(reference, pc),
            |binding| self.names.name_for_binding_def(binding, reference),
        );

        self.exprs.mark_materialized(reference, name.clone());

        let target = Expr::Name(name.clone());
        if let Some(binding) = binding.as_ref()
            && declares_named_local
        {
            self.declared_locals.insert(binding.index);
            return local_one(name, value);
        }

        if binding.is_some() {
            return assign_one(target, value);
        }

        if matches!(reference, SsaRef::Reg { .. }) && !matches!(target, Expr::Global(_)) {
            if self.declared_synthetic_names.insert(name.clone()) {
                local_one(name, value)
            } else {
                assign_one(target, value)
            }
        } else {
            assign_one(target, value)
        }
    }

    fn global_target(&self, idx: u32) -> Result<Expr, LuaError> {
        Ok(global_expr_from_name(self.string_constant(idx)?))
    }

    fn string_constant(&self, idx: u32) -> Result<BString, LuaError> {
        let idx = usize::try_from(idx)
            .map_err(|_| LuaError::Malformed("constant index does not fit in usize".to_string()))?;
        let Some(Constant::Str(value)) = self.proto.constants.get(idx) else {
            return Err(LuaError::Malformed(format!(
                "constant index {idx} is not a string"
            )));
        };
        Ok(value.clone())
    }
}

pub(crate) fn local_one(name: Name, value: Expr) -> Stmt {
    let values = if matches!(value, Expr::Nil) {
        Vec::new()
    } else {
        vec![value]
    };
    Stmt::Local {
        names: vec![name],
        attribs: Vec::new(),
        values,
    }
}

pub(crate) fn assign_one(target: Expr, value: Expr) -> Stmt {
    Stmt::Assign {
        targets: vec![target],
        values: vec![value],
    }
}

fn is_stable_assignment_target(expr: &Expr) -> bool {
    match expr {
        Expr::Name(_) | Expr::Global(_) => true,
        Expr::Field { obj, .. } | Expr::Index { obj, .. } => is_stable_assignment_target(obj),
        _ => false,
    }
}
