use super::*;

impl<'a> ExprBuilder<'a> {
    /// Convert a precomputed short-circuit value plan to an expression.
    pub(crate) fn expr_for_value_plan(&mut self, plan: &ValuePlan) -> Result<Expr, LuaError> {
        let inline_blocks = plan.consumed_blocks().collect::<Vec<_>>();
        self.with_chain_inline_blocks(&inline_blocks, |exprs| {
            exprs.expr_for_value_plan_inner(plan)
        })
    }

    pub(super) fn expr_for_value_plan_inner(&mut self, plan: &ValuePlan) -> Result<Expr, LuaError> {
        let expr = match &plan.kind {
            ValuePlanKind::Binary { left, op, right } => Expr::Binary {
                op: op.ast_op(),
                lhs: Box::new(self.expr_for_value_term(*left, plan.pc)?),
                rhs: Box::new(self.expr_for_value_term(*right, plan.pc)?),
            },
            ValuePlanKind::Ternary {
                first,
                second,
                fallback,
            } => {
                let selected = Expr::Binary {
                    op: ast::BinOp::And,
                    lhs: Box::new(self.expr_for_value_term(*first, plan.pc)?),
                    rhs: Box::new(self.expr_for_value_term(*second, plan.pc)?),
                };
                Expr::Binary {
                    op: ast::BinOp::Or,
                    lhs: Box::new(selected),
                    rhs: Box::new(self.expr_for_value_term(*fallback, plan.pc)?),
                }
            }
            ValuePlanKind::Chain { terms, fallback } => {
                let terms = self.dedup_adjacent_terms(terms);
                let mut terms = terms.into_iter();
                let Some(first) = terms.next() else {
                    return self.expr_for_value_term(*fallback, plan.pc);
                };
                let mut selected = self.expr_for_value_term(first, plan.pc)?;
                for term in terms {
                    selected = Expr::Binary {
                        op: ast::BinOp::And,
                        lhs: Box::new(selected),
                        rhs: Box::new(self.expr_for_value_term(term, plan.pc)?),
                    };
                }
                Expr::Binary {
                    op: ast::BinOp::Or,
                    lhs: Box::new(selected),
                    rhs: Box::new(self.expr_for_value_term(*fallback, plan.pc)?),
                }
            }
            ValuePlanKind::AndOrChain { groups } => {
                let mut group_exprs = Vec::new();
                for group in groups {
                    let terms = self.dedup_adjacent_terms(group);
                    let mut terms = terms.into_iter();
                    let Some(first) = terms.next() else {
                        continue;
                    };
                    let mut expr = self.expr_for_value_term(first, plan.pc)?;
                    for term in terms {
                        expr = Expr::Binary {
                            op: ast::BinOp::And,
                            lhs: Box::new(expr),
                            rhs: Box::new(self.expr_for_value_term(term, plan.pc)?),
                        };
                    }
                    group_exprs.push(expr);
                }

                let mut groups = group_exprs.into_iter();
                let mut expr = groups.next().unwrap_or(Expr::Nil);
                for rhs in groups {
                    expr = Expr::Binary {
                        op: ast::BinOp::Or,
                        lhs: Box::new(expr),
                        rhs: Box::new(rhs),
                    };
                }
                expr
            }
            ValuePlanKind::ConditionChain {
                segments,
                true_block,
                false_block,
            } => self.expr_for_condition_chain_plan(
                plan.start,
                plan.merge,
                *true_block,
                *false_block,
                segments,
            )?,
            ValuePlanKind::GuardedOrValue {
                prefix,
                or_condition,
                or_value,
            } => {
                let tail = Expr::Binary {
                    op: ast::BinOp::Or,
                    lhs: Box::new(self.expr_for_condition_segment(or_condition)?),
                    rhs: Box::new(self.expr_for_value_term(*or_value, plan.pc)?),
                };
                let mut expr = tail;
                for segment in prefix.iter().rev() {
                    let lhs = self.expr_for_condition_segment(segment)?;
                    expr = Expr::Binary {
                        op: ast::BinOp::And,
                        lhs: Box::new(lhs),
                        rhs: Box::new(expr),
                    };
                }
                expr
            }
            ValuePlanKind::Condition { branch, inverted } => {
                let Some(node) = self.analysis.node(self.function, *branch) else {
                    return Ok(Expr::True);
                };
                self.branch_expr_from_node(node, *inverted)?
            }
        };
        Ok(normalize::normalize(expr))
    }

    pub(super) fn expr_for_condition_segment(
        &mut self,
        segment: &crate::decompile::boolean::ConditionSegment,
    ) -> Result<Expr, LuaError> {
        let Some(node) = self.analysis.node(self.function, segment.node) else {
            return Ok(Expr::True);
        };
        self.branch_expr_from_node(node, segment.inverted)
    }

    pub(super) fn expr_for_condition_chain_plan(
        &mut self,
        start: usize,
        merge: usize,
        true_block: usize,
        false_block: usize,
        segments: &[crate::decompile::boolean::ConditionSegment],
    ) -> Result<Expr, LuaError> {
        if segments.is_empty() {
            return Ok(Expr::True);
        }
        let allowed = segments
            .iter()
            .map(|segment| segment.node.block)
            .collect::<std::collections::BTreeSet<_>>();
        let pc_map = conditionals::pc_to_block_map(self.function);
        let inline_blocks = allowed.iter().copied().collect::<Vec<_>>();
        let expr = self.with_chain_inline_blocks(&inline_blocks, |exprs| {
            exprs.expr_for_condition_chain_block(
                start,
                true_block,
                false_block,
                merge,
                &allowed,
                &pc_map,
                0,
            )
        })?;
        Ok(match self.load_bool_value(false_block) {
            Some(false) => Expr::Binary {
                op: ast::BinOp::Or,
                lhs: Box::new(expr),
                rhs: Box::new(Expr::False),
            },
            Some(true) => Expr::Binary {
                op: ast::BinOp::Or,
                lhs: Box::new(expr),
                rhs: Box::new(Expr::True),
            },
            None => expr,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn expr_for_condition_chain_block(
        &mut self,
        block: usize,
        true_block: usize,
        false_block: usize,
        merge: usize,
        allowed: &std::collections::BTreeSet<usize>,
        pc_map: &[Option<usize>],
        depth: usize,
    ) -> Result<Expr, LuaError> {
        if depth > 32 || !allowed.contains(&block) {
            return Ok(Expr::True);
        }
        let Some(info) = conditionals::branch_info(self.function, block, pc_map) else {
            return Ok(Expr::True);
        };
        let true_target =
            conditionals::follow_jmp_only(self.function, info.true_block, Some(merge));
        let false_target =
            conditionals::follow_jmp_only(self.function, info.false_block, Some(merge));

        if true_target == true_block && false_target == false_block {
            return self.expr_for_branch(info.node, false);
        }
        if true_target == false_block && false_target == true_block {
            return self.expr_for_branch(info.node, true);
        }

        if allowed.contains(&true_target) && false_target == false_block {
            let lhs = self.expr_for_branch(info.node, false)?;
            let rhs = self.expr_for_condition_chain_block(
                true_target,
                true_block,
                false_block,
                merge,
                allowed,
                pc_map,
                depth + 1,
            )?;
            return Ok(Expr::Binary {
                op: ast::BinOp::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            });
        }
        if allowed.contains(&false_target) && true_target == false_block {
            let lhs = self.expr_for_branch(info.node, true)?;
            let rhs = self.expr_for_condition_chain_block(
                false_target,
                true_block,
                false_block,
                merge,
                allowed,
                pc_map,
                depth + 1,
            )?;
            return Ok(Expr::Binary {
                op: ast::BinOp::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            });
        }
        if allowed.contains(&true_target) && false_target == true_block {
            let lhs = self.expr_for_branch(info.node, true)?;
            let rhs = self.expr_for_condition_chain_block(
                true_target,
                true_block,
                false_block,
                merge,
                allowed,
                pc_map,
                depth + 1,
            )?;
            return Ok(Expr::Binary {
                op: ast::BinOp::Or,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            });
        }
        if allowed.contains(&false_target) && true_target == true_block {
            let lhs = self.expr_for_branch(info.node, false)?;
            let rhs = self.expr_for_condition_chain_block(
                false_target,
                true_block,
                false_block,
                merge,
                allowed,
                pc_map,
                depth + 1,
            )?;
            return Ok(Expr::Binary {
                op: ast::BinOp::Or,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            });
        }

        self.expr_for_branch(info.node, false)
    }

    pub(super) fn expr_for_branch(
        &mut self,
        branch: crate::decompile::analysis::NodeId,
        inverted: bool,
    ) -> Result<Expr, LuaError> {
        let Some(node) = self.analysis.node(self.function, branch) else {
            return Ok(Expr::True);
        };
        self.branch_expr_from_node(node, inverted)
    }

    pub(super) fn load_bool_value(&self, block: usize) -> Option<bool> {
        self.function
            .blocks
            .get(block)?
            .nodes
            .iter()
            .find_map(|node| {
                let SsaOp::LoadBool { value, .. } = node.op else {
                    return None;
                };
                Some(value)
            })
    }

    pub(super) fn expr_for_value_term(
        &mut self,
        term: ValueTerm,
        pc: i32,
    ) -> Result<Expr, LuaError> {
        match term {
            ValueTerm::Ref(reference) => self.expr_for_ref(reference, pc),
            ValueTerm::Node(id) => {
                let Some(node) = self.analysis.node(self.function, id) else {
                    return Ok(Expr::Nil);
                };
                if matches!(node.op, SsaOp::NewTable { .. }) {
                    return self.table_constructor_value_term_expr(node);
                }
                self.node_expr(node)
            }
            ValueTerm::Condition { branch, inverted } => {
                let Some(node) = self.analysis.node(self.function, branch) else {
                    return Ok(Expr::True);
                };
                self.branch_expr_from_node(node, inverted)
            }
        }
    }

    pub(super) fn dedup_adjacent_terms(&self, terms: &[ValueTerm]) -> Vec<ValueTerm> {
        let mut deduped = Vec::with_capacity(terms.len());
        for term in terms {
            if deduped
                .last()
                .is_some_and(|previous| self.same_value_term(*previous, *term))
            {
                continue;
            }
            deduped.push(*term);
        }
        deduped
    }

    pub(super) fn same_value_term(&self, left: ValueTerm, right: ValueTerm) -> bool {
        if let (Some(left_ref), Some(right_ref)) =
            (self.term_value_ref(left), self.term_value_ref(right))
        {
            return left_ref == right_ref;
        }

        match (left, right) {
            (ValueTerm::Ref(left), ValueTerm::Ref(right)) => left == right,
            (ValueTerm::Node(left), ValueTerm::Node(right)) => {
                left == right
                    || self
                        .node_dest(left)
                        .is_some_and(|left_dest| Some(left_dest) == self.node_dest(right))
            }
            (ValueTerm::Ref(reference), ValueTerm::Node(node))
            | (ValueTerm::Node(node), ValueTerm::Ref(reference)) => {
                self.node_dest(node) == Some(reference)
            }
            _ => false,
        }
    }

    pub(super) fn term_value_ref(&self, term: ValueTerm) -> Option<SsaRef> {
        match term {
            ValueTerm::Ref(reference) => Some(reference),
            ValueTerm::Node(node) => self.node_dest(node),
            ValueTerm::Condition { branch, inverted } => {
                self.positive_branch_test_ref(branch, inverted)
            }
        }
    }

    pub(super) fn positive_branch_test_ref(&self, id: NodeId, inverted: bool) -> Option<SsaRef> {
        let node = self.analysis.node(self.function, id)?;
        let SsaOp::Branch {
            rel,
            a,
            invert: node_invert,
            ..
        } = node.op
        else {
            return None;
        };
        (matches!(rel, ir::RelOp::Test | ir::RelOp::TestSet) && !(node_invert ^ inverted))
            .then_some(a)
    }

    pub(super) fn node_dest(&self, id: NodeId) -> Option<SsaRef> {
        let node = self.analysis.node(self.function, id)?;
        (node.dest != SsaRef::None).then_some(node.dest)
    }

    pub(super) fn branch_expr(
        &mut self,
        rel: ir::RelOp,
        a: SsaRef,
        b: SsaRef,
        invert: bool,
        pc: i32,
    ) -> Result<Expr, LuaError> {
        self.branch_expr_parts(rel, a, b, invert, pc)
    }

    pub(super) fn branch_expr_from_node(
        &mut self,
        node: &SsaNode,
        invert: bool,
    ) -> Result<Expr, LuaError> {
        let SsaOp::Branch {
            rel,
            a,
            b,
            invert: node_invert,
            ..
        } = node.op
        else {
            return Ok(Expr::True);
        };
        self.branch_expr_parts(rel, a, b, node_invert ^ invert, node.pc)
    }

    pub(super) fn branch_expr_parts(
        &mut self,
        rel: ir::RelOp,
        a: SsaRef,
        b: SsaRef,
        invert: bool,
        pc: i32,
    ) -> Result<Expr, LuaError> {
        let op = match (rel, invert) {
            (ir::RelOp::Eq, false) => ast::BinOp::Eq,
            (ir::RelOp::Eq, true) => ast::BinOp::Ne,
            (ir::RelOp::Lt, false) => ast::BinOp::Lt,
            (ir::RelOp::Lt, true) => ast::BinOp::Ge,
            (ir::RelOp::Le, false) => ast::BinOp::Le,
            (ir::RelOp::Le, true) => ast::BinOp::Gt,
            (ir::RelOp::Test | ir::RelOp::TestSet, false) => {
                return self.expr_for_ref(a, pc);
            }
            (ir::RelOp::Test | ir::RelOp::TestSet, true) => {
                return Ok(Expr::Unary {
                    op: ast::UnOp::Not,
                    operand: Box::new(self.expr_for_ref(a, pc)?),
                });
            }
        };
        Ok(normalize::normalize(Expr::Binary {
            op,
            lhs: Box::new(self.expr_for_ref(a, pc)?),
            rhs: Box::new(self.expr_for_ref(b, pc)?),
        }))
    }
}
