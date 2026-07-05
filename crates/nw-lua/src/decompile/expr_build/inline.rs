use super::*;

impl<'a> ExprBuilder<'a> {
    /// Return whether a value should be inlined at its use site.
    #[must_use]
    pub fn can_inline_ref(&self, reference: SsaRef, use_pc: i32) -> bool {
        let Some(node_id) = self.analysis.def_site(reference) else {
            return false;
        };
        let Some(node) = self.analysis.node(self.function, node_id) else {
            return false;
        };

        if self.can_inline_table_constructor(reference, node) {
            return true;
        }
        if self.can_inline_new_table(reference, node) {
            return true;
        }
        self.analysis.use_count(reference) == 1
            && self.analysis.real_use_count(reference) == 1
            && is_inlineable_def(&node.op)
            && !self.is_stable_named_def(node)
            && !matches!(&node.op, SsaOp::NewTable { .. } | SsaOp::Phi { .. })
            && self.inline_preserves_order(reference, node_id, node, use_pc)
    }

    pub(super) fn reg_expr(&mut self, reference: SsaRef, use_pc: i32) -> Result<Expr, LuaError> {
        if let Some(name) = self.materialized_name(reference) {
            return Ok(Expr::Name(name));
        }

        if let Some(plan) = self.booleans.value_for_phi(reference) {
            if self.is_visiting(reference) {
                return Ok(Expr::Name(self.names.name_for_ref(reference, use_pc)));
            }
            self.set_visiting(reference, true);
            let result = self.expr_for_value_plan(plan);
            self.set_visiting(reference, false);
            return result;
        }

        let Some(node_id) = self.analysis.def_site(reference) else {
            return Ok(Expr::Name(self.names.name_for_ref(reference, use_pc)));
        };
        let Some(node) = self.analysis.node(self.function, node_id) else {
            return Ok(Expr::Name(self.names.name_for_ref(reference, use_pc)));
        };

        if node.dest != reference {
            return self.implicit_def_expr(reference, node, use_pc);
        }

        if self.can_chain_inline_ref(reference, node_id, node) {
            if self.is_visiting(reference) {
                return Ok(Expr::Name(self.names.name_for_ref(reference, use_pc)));
            }
            self.set_visiting(reference, true);
            let result = self.node_expr(node);
            self.set_visiting(reference, false);
            return result;
        }

        if !self.can_inline_ref(reference, use_pc) {
            return Ok(Expr::Name(self.names.name_for_ref(reference, use_pc)));
        }

        if self.is_visiting(reference) {
            return Ok(Expr::Name(self.names.name_for_ref(reference, use_pc)));
        }

        self.set_visiting(reference, true);
        let result = self.node_expr(node);
        self.set_visiting(reference, false);
        result
    }

    pub(super) fn can_chain_inline_ref(
        &self,
        reference: SsaRef,
        node_id: NodeId,
        node: &SsaNode,
    ) -> bool {
        if !matches!(reference, SsaRef::Reg { .. }) {
            return false;
        }
        self.chain_inline_blocks.contains(&node_id.block) && is_inlineable_def(&node.op)
    }

    pub(super) fn inline_preserves_order(
        &self,
        reference: SsaRef,
        def_id: NodeId,
        node: &SsaNode,
        _use_pc: i32,
    ) -> bool {
        let Some(use_id) = self.analysis.single_real_use(reference) else {
            return false;
        };
        let Some(use_node) = self.analysis.node(self.function, use_id) else {
            return false;
        };
        if self.dependencies_redefined_before_use(&node.op, node.pc, use_node.pc) {
            return false;
        }
        if is_pure_def(&node.op) || !self.analysis.has_side_effect_between(def_id, use_id) {
            return true;
        }
        self.use_preserves_intervening_effects(reference, def_id, use_id, use_node)
    }

    pub(super) fn dependencies_redefined_before_use(
        &self,
        op: &SsaOp,
        def_pc: i32,
        use_pc: i32,
    ) -> bool {
        let mut redefined = false;
        for_each_use(op, |reference| {
            if let Some(reg) = reference.reg_index()
                && self.analysis.has_later_def_before(reg, def_pc, use_pc)
            {
                redefined = true;
            }
        });
        redefined
    }

    pub(super) fn use_preserves_intervening_effects(
        &self,
        reference: SsaRef,
        def_id: NodeId,
        use_id: NodeId,
        use_node: &SsaNode,
    ) -> bool {
        if def_id.block != use_id.block || def_id.node >= use_id.node {
            return false;
        }
        let eval_refs = direct_eval_order_refs(&use_node.op);
        let Some(reference_index) = eval_refs.iter().position(|operand| *operand == reference)
        else {
            return false;
        };
        let block = &self.function.blocks[def_id.block];
        for current in &block.nodes[def_id.node + 1..use_id.node] {
            if !node_has_observable_side_effect(&current.op) {
                continue;
            }
            let Some(effect_index) = current.dest.reg_index().and_then(|_| {
                eval_refs
                    .iter()
                    .position(|operand| *operand == current.dest)
            }) else {
                let Some(table) = constructor_mutation_table(&current.op) else {
                    return false;
                };
                let Some(table_index) = eval_refs.iter().position(|operand| *operand == table)
                else {
                    return false;
                };
                if table_index <= reference_index {
                    return false;
                }
                continue;
            };
            if effect_index <= reference_index {
                return false;
            }
        }
        true
    }

    pub(super) fn is_stable_named_def(&self, node: &SsaNode) -> bool {
        let Some(reg) = node.dest.reg_index() else {
            return false;
        };
        let Some(binding) = self.names.binding_for_def(reg, node.pc) else {
            return false;
        };
        if self.value_used_only_before_binding(node.dest, binding.start_pc) {
            return false;
        }
        !self
            .analysis
            .has_later_def_before(reg, node.pc, binding.start_pc)
    }

    pub(super) fn value_used_only_before_binding(
        &self,
        reference: SsaRef,
        binding_start_pc: i32,
    ) -> bool {
        let uses = self.analysis.real_uses(reference);
        !uses.is_empty()
            && uses.iter().all(|use_id| {
                self.analysis
                    .node(self.function, *use_id)
                    .is_some_and(|use_node| use_node.pc < binding_start_pc)
            })
    }

    pub(super) fn last_position_needs_adjustment(&self, reference: SsaRef) -> bool {
        let Some(node_id) = self.analysis.def_site(reference) else {
            return false;
        };
        let Some(node) = self.analysis.node(self.function, node_id) else {
            return false;
        };
        matches!(
            node.op,
            SsaOp::Call {
                return_count: 2,
                ..
            } | SsaOp::VarArg { count: 2, .. }
        )
    }

    pub(super) fn is_visiting(&self, reference: SsaRef) -> bool {
        let Some(value) = ValueId::from_ref(reference) else {
            return false;
        };
        self.visiting
            .get(usize::from(value.reg))
            .and_then(|versions| versions.get(usize::try_from(value.ver).ok()?))
            .copied()
            .unwrap_or(false)
    }

    pub(super) fn set_visiting(&mut self, reference: SsaRef, visiting: bool) {
        let Some(value) = ValueId::from_ref(reference) else {
            return;
        };
        let reg = usize::from(value.reg);
        let Ok(version) = usize::try_from(value.ver) else {
            return;
        };
        if reg >= self.visiting.len() {
            self.visiting.resize_with(reg + 1, Vec::new);
        }
        if version >= self.visiting[reg].len() {
            self.visiting[reg].resize(version + 1, false);
        }
        self.visiting[reg][version] = visiting;
    }
}
