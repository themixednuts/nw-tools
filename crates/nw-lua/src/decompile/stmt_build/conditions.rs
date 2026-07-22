use super::*;

impl<'a> StatementBuilder<'a> {
    pub(crate) fn emit_value_region(&mut self, plan: &ValuePlan) -> Result<Vec<Stmt>, LuaError> {
        if !self.should_materialize_ref(plan.dest) {
            return Ok(Vec::new());
        }
        if !self.will_declare(plan.dest) {
            self.activate(plan.dest);
        }
        let value = self.value_plan_expr(plan)?;
        Ok(vec![self.materialize_value(plan.dest, plan.pc, value)])
    }
    pub(crate) fn condition_for_branch(
        &mut self,
        node: &SsaNode,
        invert: bool,
    ) -> Result<Expr, LuaError> {
        self.condition_for_branch_with_inline_blocks(node, invert, &[])
    }

    pub(super) fn condition_for_branch_with_inline_blocks(
        &mut self,
        node: &SsaNode,
        invert: bool,
        inline_blocks: &[usize],
    ) -> Result<Expr, LuaError> {
        self.exprs.with_chain_inline_blocks(inline_blocks, |exprs| {
            let cond = normalize::normalize(exprs.node_expr(node)?);
            Ok(if invert {
                normalize::invert(cond)
            } else {
                cond
            })
        })
    }

    pub(crate) fn compound_condition(
        &mut self,
        chain: &ConditionChain,
        invert: bool,
    ) -> Result<Expr, LuaError> {
        let inline_blocks = chain.blocks.clone();
        self.compound_condition_with_inline_blocks(chain, invert, &inline_blocks)
    }

    pub(super) fn compound_condition_with_inline_blocks(
        &mut self,
        chain: &ConditionChain,
        invert: bool,
        inline_blocks: &[usize],
    ) -> Result<Expr, LuaError> {
        let expr = self
            .exprs
            .with_chain_inline_blocks(inline_blocks, |exprs| {
                exprs.expr_for_condition_segments(&chain.segments)
            })?;
        Ok(if invert {
            normalize::invert(expr)
        } else {
            expr
        })
    }

    pub(crate) fn declare_phi_local(&mut self, reference: SsaRef, pc: i32) -> Option<Stmt> {
        if !self.claim_declaration(reference) {
            return None;
        }
        let name = self
            .plan
            .name(reference)
            .unwrap_or_else(|| self.names.collapsed_name_for_ref(reference, pc));
        self.exprs.activate(reference);
        Some(Stmt::Local {
            names: vec![name],
            attribs: Vec::new(),
            values: Vec::new(),
        })
    }

    pub(crate) fn phi_assignment(
        &mut self,
        dest: SsaRef,
        operand: SsaRef,
        pc: i32,
    ) -> Result<Option<Stmt>, LuaError> {
        let name = self.names.collapsed_name_for_ref(dest, pc);
        self.exprs.activate(dest);
        let value = self.exprs.expr_for_ref(operand, pc)?;
        if value == Expr::Name(name.clone()) {
            return Ok(None);
        }
        Ok(Some(assign_one(Expr::Name(name), value)))
    }

    pub(crate) fn phi_value_plan_assignment_if_covered(
        &mut self,
        dest: SsaRef,
        pc: i32,
        blocks: &[usize],
    ) -> Result<Option<Stmt>, LuaError> {
        let Some(plan) = self.booleans.value_for_phi(dest) else {
            return Ok(None);
        };
        if !blocks.contains(&plan.start) {
            return Ok(None);
        }
        let name = self.names.collapsed_name_for_ref(dest, pc);
        self.exprs.activate(dest);
        let value = self.value_plan_expr(plan)?;
        if value == Expr::Name(name.clone()) {
            return Ok(None);
        }
        Ok(Some(assign_one(Expr::Name(name), value)))
    }
}
