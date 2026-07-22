//! SSA IR and Phase 2 construction entry points.

use bstr::BString;

use crate::{
    bytecode::{Instruction, OpcodeTable},
    chunk::Proto,
    version::LuaTarget,
};

pub mod cfg;
pub mod dom;
pub mod dump;
pub mod lift;
pub mod operands;
pub mod passes;
pub mod ssa;
pub mod table;

pub use operands::{ControlFlowRole, LoopControl, OpEffects, UseRole};
pub use table::TableSizeHint;

/// SSA value reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SsaRef {
    /// No value.
    None,
    /// Register version.
    Reg {
        /// Lua register index.
        reg: u16,
        /// SSA version.
        ver: u32,
    },
    /// Constant table index.
    Const(u32),
}

/// Constant value introduced by an SSA transform without mutating the chunk's pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SsaLiteral {
    Nil,
    Boolean(bool),
    Number(u64),
    Integer(i64),
    Str(BString),
}

impl SsaLiteral {
    #[must_use]
    pub const fn number(value: f64) -> Self {
        Self::Number(value.to_bits())
    }

    #[must_use]
    pub const fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(bits) => Some(f64::from_bits(*bits)),
            Self::Nil | Self::Boolean(_) | Self::Integer(_) | Self::Str(_) => None,
        }
    }
}

impl SsaRef {
    /// Unversioned register reference.
    #[must_use]
    pub const fn reg(reg: u16) -> Self {
        Self::Reg { reg, ver: 0 }
    }

    /// Constant table reference.
    #[must_use]
    pub const fn constant(idx: u32) -> Self {
        Self::Const(idx)
    }

    /// Register index, if this is a register reference.
    #[must_use]
    pub const fn reg_index(self) -> Option<u16> {
        match self {
            Self::Reg { reg, .. } => Some(reg),
            Self::None | Self::Const(_) => None,
        }
    }

    /// Version, if this is a register reference.
    #[must_use]
    pub const fn version(self) -> Option<u32> {
        match self {
            Self::Reg { ver, .. } => Some(ver),
            Self::None | Self::Const(_) => None,
        }
    }

    pub(crate) fn set_version(&mut self, version: u32) {
        if let Self::Reg { ver, .. } = self {
            *ver = version;
        }
    }
}

/// Binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

/// Unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    Len,
    BNot,
}

/// Relational/test operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelOp {
    Eq,
    Lt,
    Le,
    Test,
    TestSet,
}

/// One Lua closure upvalue binding as read from the parent function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpvalueCapture {
    /// Lua 5.1 `MOVE 0, B`: capture a parent stack register.
    ParentLocal(SsaRef),
    /// Lua 5.1 `GETUPVAL 0, B`: capture a parent upvalue.
    ParentUpvalue(u16),
}

/// SSA operation payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SsaOp {
    Nop,
    Phi {
        operands: Vec<SsaRef>,
        blocks: Vec<usize>,
    },
    Move {
        src: SsaRef,
    },
    LoadK {
        idx: u32,
    },
    LoadLiteral {
        value: SsaLiteral,
    },
    LoadBool {
        value: bool,
        skip_next: bool,
    },
    LoadNil {
        start: u16,
        end: u16,
    },
    GetUpval {
        upval: u16,
    },
    GetGlobal {
        idx: u32,
    },
    GetTable {
        table: SsaRef,
        key: SsaRef,
    },
    SetGlobal {
        src: SsaRef,
        idx: u32,
    },
    SetUpval {
        src: SsaRef,
        upval: u16,
    },
    SetTable {
        table: SsaRef,
        key: SsaRef,
        value: SsaRef,
    },
    NewTable {
        array_hint: TableSizeHint,
        hash_hint: TableSizeHint,
    },
    SelfOp {
        table: SsaRef,
        key: SsaRef,
        self_reg: u16,
    },
    BinOp {
        op: BinOp,
        left: SsaRef,
        right: SsaRef,
    },
    UnOp {
        op: UnOp,
        value: SsaRef,
    },
    Concat {
        operands: Vec<SsaRef>,
    },
    Jump {
        target: i32,
    },
    Branch {
        rel: RelOp,
        a: SsaRef,
        b: SsaRef,
        invert: bool,
        t_true: i32,
        t_false: i32,
    },
    Call {
        func: SsaRef,
        args: Vec<SsaRef>,
        base: u16,
        arg_count: i32,
        return_count: i32,
    },
    TailCall {
        func: SsaRef,
        args: Vec<SsaRef>,
        base: u16,
        arg_count: i32,
        return_count: i32,
    },
    Return {
        values: Vec<SsaRef>,
        base: u16,
        count: i32,
    },
    ForPrep {
        control: LoopControl,
        target: i32,
    },
    ForLoop {
        control: LoopControl,
        target: i32,
    },
    TForLoop {
        control: LoopControl,
        count: i32,
    },
    SetList {
        table: SsaRef,
        values: Vec<SsaRef>,
        base: u16,
        count: i32,
        batch: i32,
    },
    Close {
        base: u16,
    },
    Closure {
        proto: u32,
        upvalues: Vec<UpvalueCapture>,
    },
    VarArg {
        base: u16,
        count: i32,
    },
}

/// Single SSA node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaNode {
    /// Source instruction PC, zero-based.
    pub pc: i32,
    /// Source line, or -1 if unavailable.
    pub line: i32,
    /// Destination value.
    pub dest: SsaRef,
    /// Operation payload.
    pub op: SsaOp,
    /// Versioned definitions produced by the same instruction after `dest`.
    secondary_defs: Vec<SsaRef>,
}

impl SsaNode {
    /// Create a node with no destination.
    #[must_use]
    pub fn new(pc: i32, line: i32, op: SsaOp) -> Self {
        let secondary_defs = operands::secondary_defs(&op, SsaRef::None);
        Self {
            pc,
            line,
            dest: SsaRef::None,
            op,
            secondary_defs,
        }
    }

    /// Create a node with a destination.
    #[must_use]
    pub fn with_dest(pc: i32, line: i32, dest: SsaRef, op: SsaOp) -> Self {
        let secondary_defs = operands::secondary_defs(&op, dest);
        Self {
            pc,
            line,
            dest,
            op,
            secondary_defs,
        }
    }

    /// Create a phi node.
    #[must_use]
    pub fn phi(pc: i32, reg: u16, preds: &[usize]) -> Self {
        Self::with_dest(
            pc,
            -1,
            SsaRef::reg(reg),
            SsaOp::Phi {
                operands: vec![SsaRef::reg(reg); preds.len()],
                blocks: preds.to_vec(),
            },
        )
    }
}

/// Basic block with CFG and stored dominance data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock {
    pub index: usize,
    pub start_pc: usize,
    pub end_pc: usize,
    pub nodes: Vec<SsaNode>,
    pub succs: Vec<usize>,
    pub preds: Vec<usize>,
    pub idom: Option<usize>,
    pub dom_children: Vec<usize>,
    pub dominance_frontier: Vec<usize>,
}

impl BasicBlock {
    /// Create an empty block for a PC range.
    #[must_use]
    pub fn new(index: usize, start_pc: usize, end_pc: usize) -> Self {
        Self {
            index,
            start_pc,
            end_pc,
            nodes: Vec::new(),
            succs: Vec::new(),
            preds: Vec::new(),
            idom: None,
            dom_children: Vec::new(),
            dominance_frontier: Vec::new(),
        }
    }

    /// Create a synthetic block for analysis tests.
    #[must_use]
    pub fn synthetic(index: usize, succs: Vec<usize>, preds: Vec<usize>) -> Self {
        Self {
            index,
            start_pc: index,
            end_pc: index,
            nodes: Vec::new(),
            succs,
            preds,
            idom: None,
            dom_children: Vec::new(),
            dominance_frontier: Vec::new(),
        }
    }
}

/// Complete SSA function for one prototype.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaFunction {
    pub source: BString,
    pub line_defined: i32,
    pub last_line_defined: i32,
    pub version: LuaTarget,
    pub num_params: u8,
    pub is_vararg: u8,
    pub max_stack: u8,
    pub num_regs: usize,
    pub instructions: Vec<Instruction>,
    pub blocks: Vec<BasicBlock>,
    pub dom: dom::DomInfo,
    pub def_sites: ssa::DefSites,
}

/// Build SSA for one prototype.
#[must_use]
pub fn build_ssa(proto: &Proto, table: &OpcodeTable) -> SsaFunction {
    let instructions = proto
        .code
        .iter()
        .map(|raw| table.decode(*raw))
        .collect::<Vec<_>>();
    let mut blocks = cfg::build_cfg(&instructions);
    let dom = dom::analyze(&blocks);
    dom::apply_to_blocks(&mut blocks, &dom);
    lift::lift_all(proto, table, &instructions, &mut blocks);
    let def_sites = ssa::collect_def_sites(&blocks, usize::from(proto.max_stack));
    ssa::insert_phi_functions(&mut blocks, usize::from(proto.max_stack), &dom, &def_sites);
    ssa::rename(
        &mut blocks,
        usize::from(proto.max_stack),
        usize::from(proto.num_params),
        &dom,
    );

    SsaFunction {
        source: proto.source.clone(),
        line_defined: proto.line_defined,
        last_line_defined: proto.last_line_defined,
        version: proto.version,
        num_params: proto.num_params,
        is_vararg: proto.is_vararg,
        max_stack: proto.max_stack,
        num_regs: usize::from(proto.max_stack),
        instructions,
        blocks,
        dom,
        def_sites,
    }
}
