use super::*;

impl<'a> StatementBuilder<'a> {
    pub(super) fn emit_boolean_phi(&mut self, node: &SsaNode) -> Result<Option<Stmt>, LuaError> {
        let Some(plan) = self.booleans.value_for_phi(node.dest) else {
            return Ok(None);
        };
        if !self.should_materialize_boolean_phi(node) {
            return Ok(None);
        }
        let value = self.value_plan_expr(plan)?;
        Ok(Some(self.materialize_value(plan.dest, plan.pc, value)))
    }

    pub(super) fn value_plan_expr(&mut self, plan: &ValuePlan) -> Result<Expr, LuaError> {
        self.exprs.expr_for_value_plan(plan)
    }

    pub(super) fn should_materialize_boolean_phi(&self, node: &SsaNode) -> bool {
        if self.exprs.is_materialized(node.dest) {
            return false;
        }
        let Some(reg) = node.dest.reg_index() else {
            return false;
        };
        if self.forced_materialized.contains(&node.dest) {
            return true;
        }
        if self.analysis.facts(node.dest).upvalue_captures > 0 {
            return true;
        }
        if self.names.binding_for_def(reg, node.pc).is_some() {
            return true;
        }
        self.analysis.real_use_count(node.dest) > 1
    }

    pub(super) fn emit_value_def(&mut self, node: &SsaNode) -> Result<Option<Stmt>, LuaError> {
        if matches!(&node.op, SsaOp::LoadNil { start, end } if start < end)
            && let Some(stmt) = self.emit_loadnil_range(node)?
        {
            return Ok(Some(stmt));
        }

        if !self.should_materialize(node) {
            return Ok(None);
        }

        let value = self.exprs.node_expr(node)?;
        Ok(Some(self.materialize_value(
            node.dest,
            self.materialization_pc(node),
            value,
        )))
    }

    pub(super) fn emit_closure_value_def(
        &mut self,
        node: &SsaNode,
    ) -> Result<Option<Stmt>, LuaError> {
        if let Some(stmt) = closure::try_local_function(self, node)? {
            return Ok(Some(stmt));
        }
        self.emit_value_def(node)
    }

    pub(super) fn emit_loadnil_range(&mut self, node: &SsaNode) -> Result<Option<Stmt>, LuaError> {
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

    pub(super) fn should_materialize(&self, node: &SsaNode) -> bool {
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
        let materialization_pc = self.materialization_pc(node);
        if let Some(reg) = node.dest.reg_index()
            && let Some(binding) = self.names.binding_for_def(reg, materialization_pc)
        {
            if self
                .analysis
                .has_later_def_before(reg, node.pc, binding.start_pc)
            {
                return false;
            }
            return true;
        }
        if self.value_plan_consumes_constructor_def(node) {
            return false;
        }
        if self.value_plan_consumes_def(node) {
            return false;
        }
        if self.analysis.has_mutating_table_use(node.dest)
            && !self.exprs.can_inline_ref(node.dest, node.pc)
        {
            return true;
        }
        let uses = self.analysis.real_use_count(node.dest);
        uses > 0 && !self.exprs.can_inline_ref(node.dest, node.pc)
    }

    pub(super) fn value_plan_consumes_def(&self, node: &SsaNode) -> bool {
        if !matches!(node.dest, SsaRef::Reg { .. }) || !is_inlineable_def(&node.op) {
            return false;
        }
        let Some((block, _)) = self.node_position(node) else {
            return false;
        };
        let Some(plan) = self
            .booleans
            .value_select_start(block)
            .or_else(|| self.booleans.value_select_covering(block))
        else {
            return false;
        };
        if node.dest == plan.dest || !plan.consumed_blocks().contains(&block) {
            return false;
        }
        let facts = self.analysis.facts(node.dest);
        if facts.uses == 0 || facts.upvalue_captures > 0 || facts.mutating_table_uses > 0 {
            return false;
        }
        let Some(start_branch_node) = self.function.blocks.get(plan.start).and_then(|block| {
            block
                .nodes
                .iter()
                .position(|candidate| matches!(candidate.op, SsaOp::Branch { .. }))
        }) else {
            return false;
        };
        self.analysis.real_uses(node.dest).iter().all(|use_id| {
            plan.consumed_blocks().contains(&use_id.block)
                && (use_id.block != plan.start || use_id.node >= start_branch_node)
        })
    }

    pub(super) fn value_plan_consumes_constructor_def(&self, node: &SsaNode) -> bool {
        if !matches!(node.op, SsaOp::NewTable { .. }) {
            return false;
        }
        let Some((block, _)) = self.node_position(node) else {
            return false;
        };
        let Some(plan) = self
            .booleans
            .value_select_start(block)
            .or_else(|| self.booleans.value_select_covering(block))
        else {
            return false;
        };
        if node.dest == plan.dest || !plan.consumed_blocks().contains(&block) {
            return false;
        }
        let Some(table_reg) = node.dest.reg_index() else {
            return false;
        };
        let uses = self.analysis.real_uses(node.dest);
        !uses.is_empty()
            && uses.iter().all(|use_id| {
                let Some(use_node) = self.analysis.node(self.function, *use_id) else {
                    return false;
                };
                if plan.consumed_blocks().contains(&use_id.block) {
                    return multi::table_list::is_matching_settable(use_node, node.dest, table_reg)
                        || multi::table_list::is_matching_setlist(use_node, node.dest, table_reg);
                }
                use_id.block == plan.merge
                    && matches!(use_node.op, SsaOp::Phi { .. })
                    && use_node.dest == plan.dest
            })
    }

    pub(super) fn node_position(&self, needle: &SsaNode) -> Option<(usize, usize)> {
        self.function
            .blocks
            .iter()
            .enumerate()
            .find_map(|(block_index, block)| {
                block
                    .nodes
                    .iter()
                    .position(|node| node.pc == needle.pc && node.dest == needle.dest)
                    .map(|node_index| (block_index, node_index))
            })
    }

    pub(super) fn materialization_pc(&self, node: &SsaNode) -> i32 {
        if !matches!(node.op, SsaOp::NewTable { .. }) {
            return node.pc;
        }
        let Some(table_reg) = node.dest.reg_index() else {
            return node.pc;
        };
        let pc = self
            .analysis
            .real_uses(node.dest)
            .iter()
            .filter_map(|id| self.analysis.node(self.function, *id))
            .filter(|use_node| {
                multi::table_list::is_matching_settable(use_node, node.dest, table_reg)
                    || multi::table_list::is_matching_setlist(use_node, node.dest, table_reg)
            })
            .map(|use_node| use_node.pc)
            .max()
            .unwrap_or(node.pc);
        if let Some(binding) = self.names.binding_for_def(table_reg, pc)
            && self
                .analysis
                .has_later_def_before(table_reg, node.pc, binding.start_pc)
        {
            return node.pc;
        }
        pc
    }

    pub(super) fn self_op_consumed_by_call(&self, node: &SsaNode) -> bool {
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

    pub(super) fn is_inline_constructor_mutation(&self, node: &SsaNode) -> bool {
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
            && (self.exprs.can_inline_ref(def.dest, node.pc)
                || self.value_plan_consumes_constructor_def(def))
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
}
