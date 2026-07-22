//! Compact decompiler working IR.
//!
//! This is intentionally smaller than Lua's full grammar. Later decompiler
//! phases build and rewrite this tree, then `emit` materializes `full_moon`.

use bstr::BString;
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

mod bindings;
mod expr;
mod stmt;

pub(crate) use bindings::{
    BindingUsage, binding_references_in_func_body, binding_spelling_available_in_block,
    binding_spelling_available_in_func_body, binding_usage_in_block, binding_usages_in_block,
    rename_binding_in_block, rename_binding_in_func_body,
};
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
#[derive(Debug, Clone)]
pub struct Name(pub BString, pub NameOrigin, Option<BindingId>);

/// Stable identity of a local-like Lua binding, independent of its spelling.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BindingId {
    function: FunctionId,
    slot: BindingSlot,
}

/// Lexical prototype path owning a binding.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FunctionId(Arc<[u32]>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum BindingSlot {
    DebugLocal(usize),
    Synthetic(usize),
    Upvalue(usize),
}

impl FunctionId {
    /// Root chunk function.
    #[must_use]
    pub fn root() -> Self {
        Self::default()
    }

    /// Lexically nested prototype at `index`.
    #[must_use]
    pub fn child(&self, index: usize) -> Self {
        let mut path = self.0.to_vec();
        path.push(u32::try_from(index).unwrap_or(u32::MAX));
        Self(path.into())
    }
}

impl BindingId {
    #[must_use]
    pub fn debug_local(function: &FunctionId, index: usize) -> Self {
        Self {
            function: function.clone(),
            slot: BindingSlot::DebugLocal(index),
        }
    }

    #[must_use]
    pub fn synthetic(function: &FunctionId, index: usize) -> Self {
        Self {
            function: function.clone(),
            slot: BindingSlot::Synthetic(index),
        }
    }

    #[must_use]
    pub fn upvalue(function: &FunctionId, index: usize) -> Self {
        Self {
            function: function.clone(),
            slot: BindingSlot::Upvalue(index),
        }
    }

    #[must_use]
    pub const fn is_external_upvalue(&self) -> bool {
        matches!(self.slot, BindingSlot::Upvalue(_))
    }

    #[must_use]
    pub const fn is_debug_local(&self) -> bool {
        matches!(self.slot, BindingSlot::DebugLocal(_))
    }
}

/// Whether an emitted identifier came from source/debug data or from nw-lua.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameOrigin {
    Recovered,
    Synthetic,
}

impl Name {
    /// Creates a recovered source/debug name from owned bytes.
    pub fn new(bytes: impl Into<BString>) -> Self {
        Self(bytes.into(), NameOrigin::Recovered, None)
    }

    /// Creates a synthetic name introduced by nw-lua.
    pub fn synthetic(bytes: impl Into<BString>) -> Self {
        Self(bytes.into(), NameOrigin::Synthetic, None)
    }

    /// Returns the raw identifier bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// Returns whether the identifier was synthesized by nw-lua.
    pub fn is_synthetic(&self) -> bool {
        self.1 == NameOrigin::Synthetic
    }

    /// Return the local-like binding represented by this identifier.
    pub const fn binding(&self) -> Option<&BindingId> {
        self.2.as_ref()
    }

    /// Attach compiler binding identity without changing emitted spelling.
    pub fn with_binding(mut self, binding: BindingId) -> Self {
        self.2 = Some(binding);
        self
    }

    /// Return whether two names denote the same known binding.
    pub fn same_binding(&self, other: &Self) -> bool {
        self.2.is_some() && self.2 == other.2
    }

    /// Returns a renamed copy preserving the origin.
    pub fn renamed(&self, bytes: impl Into<BString>) -> Self {
        Self(bytes.into(), self.1, self.2.clone())
    }
}

impl PartialEq for Name {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl Eq for Name {}

impl Hash for Name {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.1.hash(state);
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
    /// Receiver binding introduced implicitly by `function object:method(...)`.
    pub implicit_receiver: Option<Name>,
    pub params: Vec<Name>,
    pub is_vararg: bool,
    pub body: Block,
}

impl FuncBody {
    /// Creates a function body.
    pub fn new(params: Vec<Name>, is_vararg: bool, body: Block) -> Self {
        Self {
            implicit_receiver: None,
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
