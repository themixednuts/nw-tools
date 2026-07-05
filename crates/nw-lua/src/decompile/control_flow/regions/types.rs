use std::collections::BTreeSet;

use crate::decompile::{analysis::NodeId, region::LinearRegion};

use super::super::{
    conditionals::PhiSource,
    loops::{GenericForLoop, NumericForLoop},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionTree {
    pub root: Region,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Region {
    Sequence(Vec<Region>),
    Linear(LinearRegion),
    If(Box<IfRegion>),
    While(Box<WhileRegion>),
    Repeat(Box<RepeatRegion>),
    NumericFor(Box<NumericForRegion>),
    GenericFor(Box<GenericForRegion>),
    Break,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfRegion {
    pub prefix: LinearRegion,
    pub arms: Vec<IfArm>,
    pub else_: Option<Region>,
    pub else_blocks: Vec<usize>,
    pub merge: Option<usize>,
    pub phis: Vec<PhiSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfArm {
    pub condition: Condition,
    pub body: Region,
    pub blocks: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Condition {
    pub branch: NodeId,
    pub inverted: bool,
    pub compound: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhileRegion {
    pub prefix: LinearRegion,
    pub condition: Condition,
    pub body: Region,
    pub exit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepeatRegion {
    pub body: Region,
    pub condition: Condition,
    pub exit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericForRegion {
    pub prefix: LinearRegion,
    pub info: NumericForLoop,
    pub body: Region,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericForRegion {
    pub prefix: LinearRegion,
    pub info: GenericForLoop,
    pub body: Region,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSet {
    blocks: Vec<bool>,
}

impl BlockSet {
    #[must_use]
    pub fn new(len: usize) -> Self {
        Self {
            blocks: vec![false; len],
        }
    }

    pub fn insert(&mut self, block: usize) {
        if let Some(slot) = self.blocks.get_mut(block) {
            *slot = true;
        }
    }

    #[must_use]
    pub fn contains(&self, block: usize) -> bool {
        self.blocks.get(block).copied().unwrap_or(false)
    }
}

impl Region {
    #[must_use]
    pub fn blocks(&self) -> Vec<usize> {
        let mut set = BTreeSet::new();
        self.collect_blocks(&mut set);
        set.into_iter().collect()
    }

    fn collect_blocks(&self, out: &mut BTreeSet<usize>) {
        match self {
            Region::Sequence(regions) => {
                for region in regions {
                    region.collect_blocks(out);
                }
            }
            Region::Linear(linear) => {
                for block in &linear.covered_blocks {
                    out.insert(*block);
                }
                for node in &linear.nodes {
                    out.insert(node.block);
                }
            }
            Region::If(region) => {
                region.prefix.nodes.iter().for_each(|node| {
                    out.insert(node.block);
                });
                for arm in &region.arms {
                    arm.body.collect_blocks(out);
                }
                if let Some(else_) = &region.else_ {
                    else_.collect_blocks(out);
                }
            }
            Region::While(region) => {
                region.prefix.nodes.iter().for_each(|node| {
                    out.insert(node.block);
                });
                region.body.collect_blocks(out);
            }
            Region::Repeat(region) => region.body.collect_blocks(out),
            Region::NumericFor(region) => {
                out.insert(region.info.prep);
                out.insert(region.info.loop_block);
                region.body.collect_blocks(out);
            }
            Region::GenericFor(region) => {
                out.insert(region.info.entry);
                out.insert(region.info.tfor_block);
                out.insert(region.info.latch_block);
                region.body.collect_blocks(out);
            }
            Region::Break => {}
        }
    }
}
