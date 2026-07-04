//! Region tree assembly and lowering to the compact decompiler AST.

mod assembly;
mod lowering;
mod types;

pub use types::{
    BlockSet, Condition, GenericForRegion, IfArm, IfRegion, NumericForRegion, Region, RegionTree,
    RepeatRegion, WhileRegion,
};
