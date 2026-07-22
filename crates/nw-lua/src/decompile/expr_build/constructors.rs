use super::*;

use crate::decompile::multi::table_constructor::TableConstructorPlan;

impl<'a> ExprBuilder<'a> {
    pub(super) fn table_constructor_expr(&mut self, node: &SsaNode) -> Result<Expr, LuaError> {
        let plan = self
            .table_constructor_plan(node)
            .filter(|plan| self.can_inline_table_constructor_plan(node.dest, node, plan));
        self.table_expr_from_plan(plan.as_ref())
    }

    pub(super) fn table_constructor_value_term_expr(
        &mut self,
        node: &SsaNode,
    ) -> Result<Expr, LuaError> {
        let plan = self.table_constructor_plan(node);
        self.table_expr_from_plan(plan.as_ref())
    }

    pub(super) fn can_inline_new_table(&self, reference: SsaRef, node: &SsaNode) -> bool {
        matches!(&node.op, SsaOp::NewTable { .. })
            && self.analysis.use_count(reference) == 1
            && !self.analysis.has_mutating_table_use(reference)
            && !self.is_stable_named_def(node)
    }

    pub(super) fn can_inline_table_constructor(&self, reference: SsaRef, node: &SsaNode) -> bool {
        self.table_constructor_plan(node)
            .is_some_and(|plan| self.can_inline_table_constructor_plan(reference, node, &plan))
    }

    fn can_inline_table_constructor_plan(
        &self,
        reference: SsaRef,
        node: &SsaNode,
        plan: &TableConstructorPlan,
    ) -> bool {
        matches!(&node.op, SsaOp::NewTable { .. })
            && !self.is_stable_named_def(node)
            && plan.mutation_count() > 0
            && plan.final_use().is_some()
            && self.analysis.facts(reference).mutating_table_uses == plan.mutation_count()
            && self.analysis.real_use_count(reference) == plan.mutation_count() + 1
    }

    fn table_constructor_plan(&self, node: &SsaNode) -> Option<TableConstructorPlan> {
        let start = self.analysis.def_site(node.dest)?;
        TableConstructorPlan::recognize(self.function, self.analysis, start)
    }

    fn table_expr_from_plan(
        &mut self,
        plan: Option<&TableConstructorPlan>,
    ) -> Result<Expr, LuaError> {
        let (setlists, keyed) = plan.map_or_else(
            || (Vec::new(), Vec::new()),
            |plan| {
                (
                    self.nodes_for(plan.setlists()),
                    self.nodes_for(plan.keyed()),
                )
            },
        );
        let mut expr_for_ref = |reference, pc, mode| match mode {
            multi::table_list::ConstructorValueMode::Normal => self.expr_for_ref(reference, pc),
            multi::table_list::ConstructorValueMode::FixedLast => {
                self.expr_for_fixed_last_ref(reference, pc)
            }
        };
        let fields =
            multi::table_list::fields_from_nodes(setlists.iter(), keyed.iter(), &mut expr_for_ref)?;
        Ok(Expr::Table(fields))
    }

    fn nodes_for(&self, ids: &[NodeId]) -> Vec<SsaNode> {
        ids.iter()
            .filter_map(|id| self.analysis.node(self.function, *id))
            .cloned()
            .collect()
    }
}
