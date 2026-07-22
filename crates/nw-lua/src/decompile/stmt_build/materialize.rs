use super::*;
use crate::decompile::reconstruction::ValueDisposition;

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
        self.plan.disposition(node.dest) == Some(ValueDisposition::Materialize)
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
        let pc = self.plan.materialization_pc(node.dest).unwrap_or(node.pc);
        Ok(Some(self.materialize_value(node.dest, pc, value)))
    }

    pub(super) fn emit_closure_value_def(
        &mut self,
        node: &SsaNode,
    ) -> Result<Option<Stmt>, LuaError> {
        if !self.should_materialize(node) {
            return Ok(None);
        }
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
            let reference = node.def_at_reg(reg).unwrap_or_else(|| SsaRef::reg(reg));
            if self.claim_declaration(reference) {
                self.exprs.activate(reference);
                names.push(self.names.name_for_binding_def(&binding, reference));
            }
        }
        if names.is_empty() {
            return Ok(None);
        }
        Ok(Some(Stmt::Local {
            names,
            attribs: Vec::new(),
            values: Vec::new(),
        }))
    }

    pub(crate) fn should_materialize(&self, node: &SsaNode) -> bool {
        self.should_materialize_ref(node.dest)
    }

    pub(crate) fn should_materialize_ref(&self, reference: SsaRef) -> bool {
        self.plan.disposition(reference) == Some(ValueDisposition::Materialize)
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
            && (multi::table_constructor::is_matching_settable(node, def.dest, table_reg)
                || multi::table_constructor::is_matching_setlist(node, def.dest, table_reg))
            && self.exprs.can_inline_ref(def.dest, node.pc)
    }

    pub(crate) fn materialize_value(&mut self, reference: SsaRef, pc: i32, value: Expr) -> Stmt {
        let binding = reference
            .reg_index()
            .and_then(|reg| self.names.binding_for_def(reg, pc));
        let name = self.plan.name(reference).unwrap_or_else(|| {
            binding.as_ref().map_or_else(
                || self.names.name_for_ref(reference, pc),
                |binding| self.names.name_for_binding_def(binding, reference),
            )
        });
        self.exprs.activate(reference);

        let target = Expr::Name(name.clone());
        if self.claim_declaration(reference) {
            return local_one(name, value);
        }
        assign_one(target, value)
    }
}
