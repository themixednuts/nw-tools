use super::*;

impl<'a> StatementBuilder<'a> {
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
        let Some(first) = chain.segments.first() else {
            return Ok(Expr::True);
        };
        let mut and_expr = self.condition_segment_expr(first, inline_blocks)?;
        let mut or_terms = Vec::new();

        for pair in chain.segments.windows(2) {
            let [left, right] = pair else {
                continue;
            };
            let Some(connector) = left.connector else {
                continue;
            };
            let rhs = self.condition_segment_expr(right, inline_blocks)?;
            match connector {
                crate::decompile::boolean::BoolConnector::And => {
                    and_expr = Expr::Binary {
                        op: connector.ast_op(),
                        lhs: Box::new(and_expr),
                        rhs: Box::new(rhs),
                    };
                }
                crate::decompile::boolean::BoolConnector::Or => {
                    or_terms.push(and_expr);
                    and_expr = rhs;
                }
            }
        }
        or_terms.push(and_expr);

        let mut terms = or_terms.into_iter();
        let mut expr = terms.next().unwrap_or(Expr::True);
        for rhs in terms {
            expr = Expr::Binary {
                op: crate::decompile::ast::BinOp::Or,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
        }

        expr = normalize::normalize(expr);
        Ok(if invert {
            normalize::invert(expr)
        } else {
            expr
        })
    }

    pub(super) fn condition_segment_expr(
        &mut self,
        segment: &crate::decompile::boolean::ConditionSegment,
        inline_blocks: &[usize],
    ) -> Result<Expr, LuaError> {
        let Some(node) = self.analysis.node(self.function, segment.node) else {
            return Ok(Expr::True);
        };
        self.condition_for_branch_with_inline_blocks(node, segment.inverted, inline_blocks)
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
        self.exprs.mark_materialized(dest, name.clone());
        let value = self.value_plan_expr(plan)?;
        if value == Expr::Name(name.clone()) {
            return Ok(None);
        }
        Ok(Some(assign_one(Expr::Name(name), value)))
    }
}
