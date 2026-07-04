//! Version-aware Lua bytecode decoding.

pub mod instruction;
pub mod opinfo;
pub mod semantic;
pub mod table;

pub use instruction::Instruction;
pub use opinfo::{ControlFlowClass, OpArgMode, OpInfo, OperandSlot};
pub use semantic::SemanticOp;
pub use table::{InstructionFormat, OpcodeTable};
