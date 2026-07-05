use super::*;

#[derive(Debug, Default)]
pub(super) struct ConstructorPlan {
    setlists: Vec<SsaNode>,
    keyed: Vec<SsaNode>,
    mutation_count: usize,
    final_use: Option<NodeId>,
}

impl<'a> ExprBuilder<'a> {
    pub(super) fn table_constructor_expr(&mut self, node: &SsaNode) -> Result<Expr, LuaError> {
        let Some(table_reg) = node.dest.reg_index() else {
            return Ok(Expr::Table(Vec::new()));
        };
        let plan = if self.can_inline_table_constructor(node.dest, node) {
            self.constructor_plan(node, table_reg)
        } else {
            ConstructorPlan::default()
        };
        let mut expr_for_ref = |reference, pc, mode| match mode {
            multi::table_list::ConstructorValueMode::Normal => self.expr_for_ref(reference, pc),
            multi::table_list::ConstructorValueMode::FixedLast => {
                self.expr_for_fixed_last_ref(reference, pc)
            }
        };
        let fields = multi::table_list::fields_from_nodes(
            plan.setlists.iter(),
            plan.keyed.iter(),
            &mut expr_for_ref,
        )?;
        Ok(Expr::Table(fields))
    }

    pub(super) fn table_constructor_value_term_expr(
        &mut self,
        node: &SsaNode,
    ) -> Result<Expr, LuaError> {
        let Some(table_reg) = node.dest.reg_index() else {
            return Ok(Expr::Table(Vec::new()));
        };
        let plan = self.constructor_plan(node, table_reg);
        if plan.mutation_count == 0 {
            return Ok(Expr::Table(Vec::new()));
        }
        let mut expr_for_ref = |reference, pc, mode| match mode {
            multi::table_list::ConstructorValueMode::Normal => self.expr_for_ref(reference, pc),
            multi::table_list::ConstructorValueMode::FixedLast => {
                self.expr_for_fixed_last_ref(reference, pc)
            }
        };
        let fields = multi::table_list::fields_from_nodes(
            plan.setlists.iter(),
            plan.keyed.iter(),
            &mut expr_for_ref,
        )?;
        Ok(Expr::Table(fields))
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

    pub(super) fn can_inline_new_table(&self, reference: SsaRef, node: &SsaNode) -> bool {
        matches!(&node.op, SsaOp::NewTable { .. })
            && self.analysis.use_count(reference) == 1
            && !self.analysis.has_mutating_table_use(reference)
            && !self.is_stable_named_def(node)
    }

    pub(super) fn can_inline_table_constructor(&self, reference: SsaRef, node: &SsaNode) -> bool {
        if !matches!(&node.op, SsaOp::NewTable { .. }) || self.is_stable_named_def(node) {
            return false;
        }
        let Some(table_reg) = node.dest.reg_index() else {
            return false;
        };
        let plan = self.constructor_plan(node, table_reg);
        plan.mutation_count > 0
            && plan.final_use.is_some()
            && self.analysis.facts(reference).mutating_table_uses == plan.mutation_count
            && self.analysis.real_use_count(reference) == plan.mutation_count + 1
    }

    pub(super) fn constructor_plan(&self, node: &SsaNode, table_reg: u16) -> ConstructorPlan {
        let Some((block, node_index)) = self.node_position(node).or_else(|| {
            self.analysis
                .def_site(node.dest)
                .map(|id| (id.block, id.node))
        }) else {
            return ConstructorPlan::default();
        };

        let mut plan = ConstructorPlan::default();
        let mut saw_setup_effect = false;
        for (offset, current) in self.function.blocks[block]
            .nodes
            .iter()
            .skip(node_index + 1)
            .enumerate()
        {
            if current.is_meta_only {
                break;
            }
            let current_id = NodeId {
                block,
                node: node_index + offset + 1,
            };
            if multi::table_list::is_matching_setlist(current, node.dest, table_reg) {
                plan.setlists.push(current.clone());
                plan.mutation_count += 1;
                continue;
            }
            if multi::table_list::is_matching_settable(current, node.dest, table_reg) {
                plan.keyed.push(current.clone());
                plan.mutation_count += 1;
                continue;
            }
            if op_uses_ref(&current.op, node.dest) {
                if saw_setup_effect && !is_parent_constructor_mutation(current, table_reg) {
                    return ConstructorPlan::default();
                }
                plan.final_use = Some(current_id);
                break;
            }
            if multi::table_list::is_constructor_setup(current, table_reg) {
                if node_has_observable_side_effect(&current.op) {
                    saw_setup_effect = true;
                }
                continue;
            }
            break;
        }
        plan
    }
}
