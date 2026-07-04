//! Compact decompiler working IR.
//!
//! This is intentionally smaller than Lua's full grammar. Later decompiler
//! phases build and rewrite this tree, then `emit` materializes `full_moon`.

use bstr::BString;

mod expr;
mod stmt;

pub use expr::Expr;
pub use stmt::Stmt;

/// A block of decompiled statements.
#[derive(Debug, Clone, PartialEq)]
pub struct Block(pub Vec<Stmt>);

impl Block {
    /// Creates a block from statements.
    pub fn new(stmts: Vec<Stmt>) -> Self {
        Self(stmts)
    }

    /// Creates an empty block.
    pub fn empty() -> Self {
        Self(Vec::new())
    }
}

/// A byte-backed Lua identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name(pub BString, pub NameOrigin);

/// Whether an emitted identifier came from source/debug data or from nw-lua.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameOrigin {
    Recovered,
    Synthetic,
}

impl Name {
    /// Creates a recovered source/debug name from owned bytes.
    pub fn new(bytes: impl Into<BString>) -> Self {
        Self(bytes.into(), NameOrigin::Recovered)
    }

    /// Creates a synthetic name introduced by nw-lua.
    pub fn synthetic(bytes: impl Into<BString>) -> Self {
        Self(bytes.into(), NameOrigin::Synthetic)
    }

    /// Returns the raw identifier bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// Returns whether the identifier was synthesized by nw-lua.
    pub fn is_synthetic(&self) -> bool {
        self.1 == NameOrigin::Synthetic
    }

    /// Returns a renamed copy preserving the origin.
    pub fn renamed(&self, bytes: impl Into<BString>) -> Self {
        Self(bytes.into(), self.1)
    }
}

impl From<&str> for Name {
    fn from(value: &str) -> Self {
        Self::synthetic(BString::from(value))
    }
}

impl From<String> for Name {
    fn from(value: String) -> Self {
        Self::synthetic(BString::from(value))
    }
}

/// A non-local function declaration target, `A.B.f` or `A.B:m`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionName {
    pub path: Vec<Name>,
    pub method: Option<Name>,
}

impl FunctionName {
    /// Creates a dot-form function name.
    pub fn dotted(path: Vec<Name>) -> Self {
        Self { path, method: None }
    }

    /// Creates a colon-method function name.
    pub fn method(path: Vec<Name>, method: Name) -> Self {
        Self {
            path,
            method: Some(method),
        }
    }
}

/// Binary operators represented by the decompiler IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    IDiv,
    BAnd,
    BOr,
    BXor,
    Shl,
    Shr,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// Unary operators represented by the decompiler IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnOp {
    Neg,
    Not,
    Len,
    BNot,
}

/// A table constructor field.
#[derive(Debug, Clone, PartialEq)]
pub enum TableField {
    List(Expr),
    Named { name: Name, value: Expr },
    ExprKey { key: Expr, value: Expr },
}

/// A function body.
#[derive(Debug, Clone, PartialEq)]
pub struct FuncBody {
    pub params: Vec<Name>,
    pub is_vararg: bool,
    pub body: Block,
}

impl FuncBody {
    /// Creates a function body.
    pub fn new(params: Vec<Name>, is_vararg: bool, body: Block) -> Self {
        Self {
            params,
            is_vararg,
            body,
        }
    }
}

/// Lua 5.4 local variable attributes, kept for later bytecode versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Attrib {
    Const,
    Close,
}
