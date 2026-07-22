use super::*;

impl<'a> ExprBuilder<'a> {
    /// Return whether a value should be inlined at its use site.
    #[must_use]
    pub fn can_inline_ref(&self, reference: SsaRef, use_pc: i32) -> bool {
        if let Some(can_inline) = self.plan.can_inline_at(reference, use_pc) {
            return can_inline;
        }

        let key = (reference, use_pc);
        if let Some(can_inline) = self.inline_cache.borrow().get(&key).copied() {
            return can_inline;
        }
        if !self.inline_visiting.borrow_mut().insert(key) {
            return false;
        }
        let can_inline = self.compute_can_inline_ref(reference, use_pc);
        self.inline_visiting.borrow_mut().remove(&key);
        self.inline_cache.borrow_mut().insert(key, can_inline);
        can_inline
    }

    fn compute_can_inline_ref(&self, reference: SsaRef, use_pc: i32) -> bool {
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
        op.visit_uses(|reference, _| {
            if let Some(reg) = reference.reg_index()
                && self.analysis.has_later_def_before(reg, def_pc, use_pc)
                && !self.can_inline_ref(reference, def_pc)
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
        for (node_index, current) in block.nodes[def_id.node + 1..use_id.node].iter().enumerate() {
            if !current.op.effects().blocks_reordering() {
                continue;
            }
            let current_id = NodeId {
                block: def_id.block,
                node: def_id.node + 1 + node_index,
            };
            if let Some(table) = self.plan.constructor_for_node(current_id) {
                let Some(table_index) = self.evaluation_dependency_index(table, use_id) else {
                    return false;
                };
                if table_index <= reference_index {
                    return false;
                }
                continue;
            }
            let Some(effect_index) = self.evaluation_dependency_index(current.dest, use_id) else {
                let Some(table) = constructor_mutation_table(&current.op) else {
                    return false;
                };
                let Some(table_index) = self.evaluation_dependency_index(table, use_id) else {
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

    fn evaluation_dependency_index(&self, dependency: SsaRef, use_id: NodeId) -> Option<usize> {
        (dependency != SsaRef::None).then_some(())?;
        let key = (dependency, use_id);
        if let Some(index) = self.evaluation_index_cache.borrow().get(&key).copied() {
            return index;
        }
        let index = self
            .analysis
            .node(self.function, use_id)
            .map(|node| direct_eval_order_refs(&node.op))?
            .iter()
            .position(|root| self.value_depends_on(*root, dependency, use_id, &mut Vec::new()));
        self.evaluation_index_cache.borrow_mut().insert(key, index);
        index
    }

    fn value_depends_on(
        &self,
        value: SsaRef,
        dependency: SsaRef,
        before: NodeId,
        visiting: &mut Vec<SsaRef>,
    ) -> bool {
        if value == dependency {
            return true;
        }
        if visiting.contains(&value) {
            return false;
        }
        let Some(def_id) = self.analysis.def_site(value) else {
            return false;
        };
        if def_id.block != before.block || def_id.node >= before.node {
            return false;
        }
        let Some(node) = self.analysis.node(self.function, def_id) else {
            return false;
        };

        visiting.push(value);
        let mut found = false;
        node.op.visit_uses(|reference, _| {
            found |= self.value_depends_on(reference, dependency, def_id, visiting);
        });
        visiting.pop();
        found
    }

    pub(crate) fn is_stable_named_def(&self, node: &SsaNode) -> bool {
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
