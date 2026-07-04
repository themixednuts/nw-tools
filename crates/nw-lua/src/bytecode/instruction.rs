//! Decoded instruction fields.

use super::SemanticOp;

/// A raw instruction word decoded through an [`OpcodeTable`](super::OpcodeTable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    /// Original 32-bit instruction word.
    pub raw: u32,
    /// Version-independent opcode semantic.
    pub op: SemanticOp,
    /// A field.
    pub a: i32,
    /// B field.
    pub b: i32,
    /// C field.
    pub c: i32,
    /// Unsigned Bx field.
    pub bx: i32,
    /// Signed Bx field.
    pub sbx: i32,
}
