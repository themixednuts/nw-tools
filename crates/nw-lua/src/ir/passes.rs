//! Composable SSA transforms and their analysis cache.

use std::{collections::HashMap, num::NonZeroU8};

use crate::chunk::Constant;

use super::{SsaFunction, SsaNode, SsaRef};

mod simplify;

pub use simplify::{ConstantFolding, CopyPropagation, DeadCodeElimination, TrivialPhiElimination};

/// Analyses retained after a pass changes the function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreservedAnalyses {
    All,
    ControlFlow,
    None,
}

/// Typed result of one pass invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassChange {
    pub changed: bool,
    pub preserved: PreservedAnalyses,
}

impl PassChange {
    #[must_use]
    pub const fn unchanged() -> Self {
        Self {
            changed: false,
            preserved: PreservedAnalyses::All,
        }
    }

    #[must_use]
    pub const fn changed(preserved: PreservedAnalyses) -> Self {
        Self {
            changed: true,
            preserved,
        }
    }
}

/// One independently testable SSA transformation.
pub trait SsaPass {
    fn name(&self) -> &'static str;

    fn run(&mut self, function: &mut SsaFunction, context: &mut PassContext<'_>) -> PassChange;
}

/// Execution policy for one pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassSchedule {
    Once,
    Fixpoint { max_iterations: NonZeroU8 },
}

impl PassSchedule {
    #[must_use]
    pub const fn fixpoint(max_iterations: NonZeroU8) -> Self {
        Self::Fixpoint { max_iterations }
    }
}

struct ScheduledPass {
    pass: Box<dyn SsaPass>,
    schedule: PassSchedule,
}

/// Ordered heterogeneous pass pipeline.
#[derive(Default)]
pub struct PassPipeline {
    passes: Vec<ScheduledPass>,
}

impl PassPipeline {
    #[must_use]
    pub const fn new() -> Self {
        Self { passes: Vec::new() }
    }

    pub fn add(&mut self, pass: impl SsaPass + 'static, schedule: PassSchedule) -> &mut Self {
        self.passes.push(ScheduledPass {
            pass: Box::new(pass),
            schedule,
        });
        self
    }

    pub fn run(&mut self, function: &mut SsaFunction, constants: &[Constant]) -> PipelineReport {
        let mut context = PassContext::new(constants);
        let mut passes = Vec::with_capacity(self.passes.len());

        for scheduled in &mut self.passes {
            let limit = match scheduled.schedule {
                PassSchedule::Once => 1,
                PassSchedule::Fixpoint { max_iterations } => max_iterations.get(),
            };
            let mut iterations = 0;
            let mut changed = false;
            let mut converged = matches!(scheduled.schedule, PassSchedule::Once);

            for _ in 0..limit {
                iterations += 1;
                let change = scheduled.pass.run(function, &mut context);
                context.invalidate(change);
                changed |= change.changed;
                if !change.changed {
                    converged = true;
                    break;
                }
            }

            passes.push(PassReport {
                name: scheduled.pass.name(),
                iterations,
                changed,
                converged,
            });
        }

        PipelineReport { passes }
    }
}

/// One pass's deterministic execution summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassReport {
    pub name: &'static str,
    pub iterations: u8,
    pub changed: bool,
    pub converged: bool,
}

/// Complete execution summary for a pipeline.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PipelineReport {
    pub passes: Vec<PassReport>,
}

/// Lazily computed value analyses shared by passes.
pub struct PassContext<'a> {
    constants: &'a [Constant],
    use_counts: Option<HashMap<SsaRef, usize>>,
    definitions: Option<HashMap<SsaRef, NodePosition>>,
}

impl<'a> PassContext<'a> {
    fn new(constants: &'a [Constant]) -> Self {
        Self {
            constants,
            use_counts: None,
            definitions: None,
        }
    }

    #[must_use]
    pub fn constants(&self) -> &'a [Constant] {
        self.constants
    }

    pub fn use_count(&mut self, function: &SsaFunction, reference: SsaRef) -> usize {
        self.ensure_use_counts(function);
        self.use_counts
            .as_ref()
            .and_then(|counts| counts.get(&reference))
            .copied()
            .unwrap_or(0)
    }

    pub(crate) fn definition_position(
        &mut self,
        function: &SsaFunction,
        reference: SsaRef,
    ) -> Option<NodePosition> {
        self.ensure_definitions(function);
        self.definitions.as_ref()?.get(&reference).copied()
    }

    fn ensure_use_counts(&mut self, function: &SsaFunction) {
        if self.use_counts.is_some() {
            return;
        }
        let mut counts = HashMap::new();
        for block in &function.blocks {
            for node in &block.nodes {
                node.op.visit_uses(|reference, _| {
                    if matches!(reference, SsaRef::Reg { .. }) {
                        *counts.entry(reference).or_default() += 1;
                    }
                });
            }
        }
        self.use_counts = Some(counts);
    }

    fn ensure_definitions(&mut self, function: &SsaFunction) {
        if self.definitions.is_some() {
            return;
        }
        let mut definitions = HashMap::new();
        for (block, block_ref) in function.blocks.iter().enumerate() {
            for (node, node_ref) in block_ref.nodes.iter().enumerate() {
                node_ref.visit_defs(|reference| {
                    definitions
                        .entry(reference)
                        .or_insert(NodePosition { block, node });
                });
            }
        }
        self.definitions = Some(definitions);
    }

    fn invalidate(&mut self, change: PassChange) {
        if !change.changed || change.preserved == PreservedAnalyses::All {
            return;
        }
        self.use_counts = None;
        self.definitions = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NodePosition {
    pub block: usize,
    pub node: usize,
}

pub(crate) fn node(function: &SsaFunction, position: NodePosition) -> Option<&SsaNode> {
    function
        .blocks
        .get(position.block)?
        .nodes
        .get(position.node)
}

pub(crate) fn node_mut(function: &mut SsaFunction, position: NodePosition) -> Option<&mut SsaNode> {
    function
        .blocks
        .get_mut(position.block)?
        .nodes
        .get_mut(position.node)
}

/// Full cleanup pipeline for differential validation and future reconstruction use.
#[must_use]
pub fn cleanup_pipeline() -> PassPipeline {
    let mut pipeline = PassPipeline::new();
    let fixpoint = PassSchedule::fixpoint(NonZeroU8::new(8).expect("8 is non-zero"));
    pipeline
        .add(TrivialPhiElimination, fixpoint)
        .add(CopyPropagation, fixpoint)
        .add(ConstantFolding, fixpoint)
        .add(DeadCodeElimination, fixpoint);
    pipeline
}

#[cfg(test)]
mod tests {
    use bstr::BString;

    use super::*;
    use crate::{
        ir::{BasicBlock, BinOp, SsaLiteral, SsaOp},
        version::LuaTarget,
    };

    fn reference(reg: u16) -> SsaRef {
        SsaRef::Reg { reg, ver: 1 }
    }

    fn function(nodes: Vec<SsaNode>) -> SsaFunction {
        let mut block = BasicBlock::synthetic(0, Vec::new(), Vec::new());
        block.nodes = nodes;
        let blocks = vec![block];
        let dom = crate::ir::dom::analyze(&blocks);
        let def_sites = crate::ir::ssa::collect_def_sites(&blocks, 8);
        SsaFunction {
            source: BString::default(),
            line_defined: 0,
            last_line_defined: 0,
            version: LuaTarget::V51,
            num_params: 0,
            is_vararg: 0,
            max_stack: 8,
            num_regs: 8,
            instructions: Vec::new(),
            blocks,
            dom,
            def_sites,
        }
    }

    #[test]
    fn constant_folding_resolves_ssa_definitions() {
        let mut function = function(vec![
            SsaNode::with_dest(0, 1, reference(0), SsaOp::LoadK { idx: 0 }),
            SsaNode::with_dest(1, 1, reference(1), SsaOp::LoadK { idx: 1 }),
            SsaNode::with_dest(
                2,
                1,
                reference(2),
                SsaOp::BinOp {
                    op: BinOp::Add,
                    left: reference(0),
                    right: reference(1),
                },
            ),
        ]);
        let constants = [Constant::Number(1.25), Constant::Number(2.75)];
        let mut pipeline = PassPipeline::new();
        pipeline.add(ConstantFolding, PassSchedule::Once);

        let report = pipeline.run(&mut function, &constants);

        assert_eq!(report.passes[0].iterations, 1);
        assert!(report.passes[0].changed);
        assert_eq!(
            function.blocks[0].nodes[2].op,
            SsaOp::LoadLiteral {
                value: SsaLiteral::number(4.0),
            }
        );
    }

    #[test]
    fn constant_folding_leaves_non_finite_results_as_operations() {
        let operation = SsaOp::BinOp {
            op: BinOp::Div,
            left: SsaRef::Const(0),
            right: SsaRef::Const(0),
        };
        let mut function = function(vec![SsaNode::with_dest(
            0,
            1,
            reference(0),
            operation.clone(),
        )]);
        let constants = [Constant::Number(0.0)];
        let mut pipeline = PassPipeline::new();
        pipeline.add(ConstantFolding, PassSchedule::Once);

        let report = pipeline.run(&mut function, &constants);

        assert!(!report.passes[0].changed);
        assert_eq!(function.blocks[0].nodes[0].op, operation);
    }

    #[test]
    fn copy_propagation_rewrites_chains_in_definition_order() {
        let mut function = function(vec![
            SsaNode::with_dest(0, 1, reference(0), SsaOp::LoadK { idx: 0 }),
            SsaNode::with_dest(1, 1, reference(1), SsaOp::Move { src: reference(0) }),
            SsaNode::with_dest(2, 1, reference(2), SsaOp::Move { src: reference(1) }),
            SsaNode::new(
                3,
                1,
                SsaOp::Return {
                    values: vec![reference(2)],
                    base: 2,
                    count: 2,
                },
            ),
        ]);
        let mut pipeline = PassPipeline::new();
        pipeline.add(
            CopyPropagation,
            PassSchedule::fixpoint(NonZeroU8::new(4).unwrap()),
        );

        let report = pipeline.run(&mut function, &[]);

        assert_eq!(report.passes[0].iterations, 2);
        assert!(report.passes[0].converged);
        assert_eq!(function.blocks[0].nodes[1].op, SsaOp::Nop);
        assert_eq!(function.blocks[0].nodes[2].op, SsaOp::Nop);
        assert_eq!(
            function.blocks[0].nodes[3].op,
            SsaOp::Return {
                values: vec![reference(0)],
                base: 2,
                count: 2,
            }
        );
    }

    #[test]
    fn trivial_phi_rewrites_uses_but_ignores_its_self_edge() {
        let mut function = function(vec![
            SsaNode::with_dest(
                0,
                -1,
                reference(2),
                SsaOp::Phi {
                    operands: vec![reference(2), reference(1), reference(1)],
                    blocks: vec![0, 1, 2],
                },
            ),
            SsaNode::new(
                1,
                1,
                SsaOp::Return {
                    values: vec![reference(2)],
                    base: 2,
                    count: 2,
                },
            ),
        ]);
        let mut pipeline = PassPipeline::new();
        pipeline.add(TrivialPhiElimination, PassSchedule::Once);

        pipeline.run(&mut function, &[]);

        assert_eq!(function.blocks[0].nodes[0].op, SsaOp::Nop);
        assert_eq!(
            function.blocks[0].nodes[1].op,
            SsaOp::Return {
                values: vec![reference(1)],
                base: 2,
                count: 2,
            }
        );
    }

    #[test]
    fn dead_code_elimination_preserves_operations_that_may_invoke_lua() {
        let mut function = function(vec![
            SsaNode::with_dest(
                0,
                1,
                reference(0),
                SsaOp::LoadLiteral {
                    value: SsaLiteral::Nil,
                },
            ),
            SsaNode::with_dest(
                1,
                1,
                reference(1),
                SsaOp::GetTable {
                    table: SsaRef::Const(0),
                    key: SsaRef::Const(1),
                },
            ),
        ]);
        let mut pipeline = PassPipeline::new();
        pipeline.add(DeadCodeElimination, PassSchedule::Once);

        pipeline.run(&mut function, &[]);

        assert_eq!(function.blocks[0].nodes[0].op, SsaOp::Nop);
        assert!(matches!(
            function.blocks[0].nodes[1].op,
            SsaOp::GetTable { .. }
        ));
    }

    struct NeverConverges;

    impl SsaPass for NeverConverges {
        fn name(&self) -> &'static str {
            "never-converges"
        }

        fn run(
            &mut self,
            _function: &mut SsaFunction,
            _context: &mut PassContext<'_>,
        ) -> PassChange {
            PassChange::changed(PreservedAnalyses::All)
        }
    }

    #[test]
    fn fixpoint_schedule_reports_iteration_exhaustion() {
        let mut function = function(Vec::new());
        let mut pipeline = PassPipeline::new();
        pipeline.add(
            NeverConverges,
            PassSchedule::fixpoint(NonZeroU8::new(2).unwrap()),
        );

        let report = pipeline.run(&mut function, &[]);

        assert_eq!(report.passes[0].iterations, 2);
        assert!(!report.passes[0].converged);
    }
}
