use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolConnector {
    And,
    Or,
}

impl BoolConnector {
    pub(crate) const fn ast_op(self) -> ast::BinOp {
        match self {
            Self::And => ast::BinOp::And,
            Self::Or => ast::BinOp::Or,
        }
    }
}

/// One condition segment in a compound branch chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionSegment {
    pub node: NodeId,
    pub inverted: bool,
    pub connector: Option<BoolConnector>,
}

/// A collapsed `and`/`or` condition chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionChain {
    pub start: usize,
    pub blocks: Vec<usize>,
    pub body: usize,
    pub false_target: usize,
    pub merge: usize,
    pub segments: Vec<ConditionSegment>,
}

/// A value-producing short-circuit select that materializes at a PHI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuePlan {
    pub start: usize,
    pub merge: usize,
    pub dest: SsaRef,
    pub pc: i32,
    pub kind: ValuePlanKind,
}

impl ValuePlan {
    #[must_use]
    pub fn consumed_blocks(&self) -> std::ops::Range<usize> {
        self.start..self.merge
    }
}

/// Expression payload for a value select.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValuePlanKind {
    Binary {
        left: ValueTerm,
        op: BoolConnector,
        right: ValueTerm,
    },
    Ternary {
        first: ValueTerm,
        second: ValueTerm,
        fallback: ValueTerm,
    },
    Chain {
        terms: Vec<ValueTerm>,
        fallback: ValueTerm,
    },
    AndOrChain {
        groups: Vec<Vec<ValueTerm>>,
    },
    ConditionChain {
        segments: Vec<ConditionSegment>,
        true_block: usize,
        false_block: usize,
    },
    GuardedOrValue {
        prefix: Vec<ConditionSegment>,
        or_condition: ConditionSegment,
        or_value: ValueTerm,
    },
    Condition {
        branch: NodeId,
        inverted: bool,
    },
}

/// One expression segment inside a short-circuit value plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueTerm {
    Ref(SsaRef),
    Node(NodeId),
    Condition { branch: NodeId, inverted: bool },
}

impl From<SsaRef> for ValueTerm {
    fn from(reference: SsaRef) -> Self {
        Self::Ref(reference)
    }
}
