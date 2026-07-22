//! Short-circuit branch-chain recognition.

use crate::{
    decompile::{
        analysis::{DecompileAnalysis, NodeId},
        ast,
        control_flow::conditionals,
    },
    ir::{RelOp, SsaFunction, SsaOp, SsaRef},
};

use super::{ConditionContext, branch_at, branch_info, is_pure_value_node, phi_sources};

mod condition;
mod guards;
mod helpers;
mod selectors;
mod types;
mod value;

pub use condition::condition_chain;
pub use types::{
    BoolConnector, ConditionChain, ConditionSegment, ValuePlan, ValuePlanKind, ValueTerm,
};
pub use value::value_plan;

pub(in crate::decompile::boolean) use helpers::{branch_rel, pure_select_range, selected_operand};
