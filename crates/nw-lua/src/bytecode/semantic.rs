//! Version-independent opcode names.

/// Abstract opcode used by the rest of the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticOp {
    /// `MOVE`
    Move,
    /// `LOADK`
    LoadK,
    /// `LOADBOOL`
    LoadBool,
    /// `LOADNIL`
    LoadNil,
    /// `GETUPVAL`
    GetUpval,
    /// `GETGLOBAL`
    GetGlobal,
    /// `GETTABLE`
    GetTable,
    /// `SETGLOBAL`
    SetGlobal,
    /// `SETUPVAL`
    SetUpval,
    /// `SETTABLE`
    SetTable,
    /// `NEWTABLE`
    NewTable,
    /// `SELF`
    SelfOp,
    /// `ADD`
    Add,
    /// `SUB`
    Sub,
    /// `MUL`
    Mul,
    /// `DIV`
    Div,
    /// `MOD`
    Mod,
    /// `POW`
    Pow,
    /// `UNM`
    Unm,
    /// `NOT`
    Not,
    /// `LEN`
    Len,
    /// `CONCAT`
    Concat,
    /// `JMP`
    Jmp,
    /// `EQ`
    Eq,
    /// `LT`
    Lt,
    /// `LE`
    Le,
    /// `TEST`
    Test,
    /// `TESTSET`
    TestSet,
    /// `CALL`
    Call,
    /// `TAILCALL`
    TailCall,
    /// `RETURN`
    Return,
    /// `FORLOOP`
    ForLoop,
    /// `FORPREP`
    ForPrep,
    /// `TFORLOOP`
    TForLoop,
    /// `SETLIST`
    SetList,
    /// `CLOSE`
    Close,
    /// `CLOSURE`
    Closure,
    /// `VARARG`
    VarArg,
    /// `LOADKX`
    LoadKx,
    /// `GETTABUP`
    GetTabUp,
    /// `SETTABUP`
    SetTabUp,
    /// `TFORCALL`
    TForCall,
    /// `TFORLOOP54`
    TForLoop54,
    /// `EXTRAARG`
    ExtraArg,
    /// `IDIV`
    Idiv,
    /// `BAND`
    Band,
    /// `BOR`
    Bor,
    /// `BXOR`
    Bxor,
    /// `SHL`
    Shl,
    /// `SHR`
    Shr,
    /// `BNOT`
    Bnot,
    /// `LOADI`
    LoadI,
    /// `LOADF`
    LoadF,
    /// `LOADFALSE`
    LoadFalse,
    /// `LOADTRUE`
    LoadTrue,
    /// `LFALSESKIP`
    LFalseSkip,
    /// `ADDI`
    AddI,
    /// `ADDK`
    AddK,
    /// `SUBK`
    SubK,
    /// `MULK`
    MulK,
    /// `MODK`
    ModK,
    /// `POWK`
    PowK,
    /// `DIVK`
    DivK,
    /// `IDIVK`
    IdivK,
    /// `BANDK`
    BandK,
    /// `BORK`
    BorK,
    /// `BXORK`
    BxorK,
    /// `SHRI`
    ShrI,
    /// `SHLI`
    ShlI,
    /// `GETI`
    GetI,
    /// `GETFIELD`
    GetField,
    /// `SETI`
    SetI,
    /// `SETFIELD`
    SetField,
    /// `EQK`
    EqK,
    /// `EQI`
    EqI,
    /// `LTI`
    LtI,
    /// `LEI`
    LeI,
    /// `GTI`
    GtI,
    /// `GEI`
    GeI,
    /// `MMBIN`
    MmBin,
    /// `MMBINI`
    MmBinI,
    /// `MMBINK`
    MmBinK,
    /// `TBC`
    Tbc,
    /// `RETURN0`
    Return0,
    /// `RETURN1`
    Return1,
    /// `TFORPREP`
    TForPrep,
    /// `VARARGPREP`
    VarArgPrep,
    /// `GETVARG`
    GetVarG,
    /// `ERRNNIL`
    ErrNnil,
    /// Raw ordinal has no semantic mapping.
    Unknown,
}

impl SemanticOp {
    /// Return the canonical opcode spelling.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Move => "MOVE",
            Self::LoadK => "LOADK",
            Self::LoadBool => "LOADBOOL",
            Self::LoadNil => "LOADNIL",
            Self::GetUpval => "GETUPVAL",
            Self::GetGlobal => "GETGLOBAL",
            Self::GetTable => "GETTABLE",
            Self::SetGlobal => "SETGLOBAL",
            Self::SetUpval => "SETUPVAL",
            Self::SetTable => "SETTABLE",
            Self::NewTable => "NEWTABLE",
            Self::SelfOp => "SELF",
            Self::Add => "ADD",
            Self::Sub => "SUB",
            Self::Mul => "MUL",
            Self::Div => "DIV",
            Self::Mod => "MOD",
            Self::Pow => "POW",
            Self::Unm => "UNM",
            Self::Not => "NOT",
            Self::Len => "LEN",
            Self::Concat => "CONCAT",
            Self::Jmp => "JMP",
            Self::Eq => "EQ",
            Self::Lt => "LT",
            Self::Le => "LE",
            Self::Test => "TEST",
            Self::TestSet => "TESTSET",
            Self::Call => "CALL",
            Self::TailCall => "TAILCALL",
            Self::Return => "RETURN",
            Self::ForLoop => "FORLOOP",
            Self::ForPrep => "FORPREP",
            Self::TForLoop => "TFORLOOP",
            Self::SetList => "SETLIST",
            Self::Close => "CLOSE",
            Self::Closure => "CLOSURE",
            Self::VarArg => "VARARG",
            Self::LoadKx => "LOADKX",
            Self::GetTabUp => "GETTABUP",
            Self::SetTabUp => "SETTABUP",
            Self::TForCall => "TFORCALL",
            Self::TForLoop54 => "TFORLOOP54",
            Self::ExtraArg => "EXTRAARG",
            Self::Idiv => "IDIV",
            Self::Band => "BAND",
            Self::Bor => "BOR",
            Self::Bxor => "BXOR",
            Self::Shl => "SHL",
            Self::Shr => "SHR",
            Self::Bnot => "BNOT",
            Self::LoadI => "LOADI",
            Self::LoadF => "LOADF",
            Self::LoadFalse => "LOADFALSE",
            Self::LoadTrue => "LOADTRUE",
            Self::LFalseSkip => "LFALSESKIP",
            Self::AddI => "ADDI",
            Self::AddK => "ADDK",
            Self::SubK => "SUBK",
            Self::MulK => "MULK",
            Self::ModK => "MODK",
            Self::PowK => "POWK",
            Self::DivK => "DIVK",
            Self::IdivK => "IDIVK",
            Self::BandK => "BANDK",
            Self::BorK => "BORK",
            Self::BxorK => "BXORK",
            Self::ShrI => "SHRI",
            Self::ShlI => "SHLI",
            Self::GetI => "GETI",
            Self::GetField => "GETFIELD",
            Self::SetI => "SETI",
            Self::SetField => "SETFIELD",
            Self::EqK => "EQK",
            Self::EqI => "EQI",
            Self::LtI => "LTI",
            Self::LeI => "LEI",
            Self::GtI => "GTI",
            Self::GeI => "GEI",
            Self::MmBin => "MMBIN",
            Self::MmBinI => "MMBINI",
            Self::MmBinK => "MMBINK",
            Self::Tbc => "TBC",
            Self::Return0 => "RETURN0",
            Self::Return1 => "RETURN1",
            Self::TForPrep => "TFORPREP",
            Self::VarArgPrep => "VARARGPREP",
            Self::GetVarG => "GETVARG",
            Self::ErrNnil => "ERRNNIL",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// Parse a canonical opcode spelling.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let upper = name.trim().to_ascii_uppercase();
        let upper = upper.strip_prefix("SEM").unwrap_or(&upper);
        match upper {
            "MOVE" => Some(Self::Move),
            "LOADK" => Some(Self::LoadK),
            "LOADBOOL" => Some(Self::LoadBool),
            "LOADNIL" => Some(Self::LoadNil),
            "GETUPVAL" => Some(Self::GetUpval),
            "GETGLOBAL" => Some(Self::GetGlobal),
            "GETTABLE" => Some(Self::GetTable),
            "SETGLOBAL" => Some(Self::SetGlobal),
            "SETUPVAL" => Some(Self::SetUpval),
            "SETTABLE" => Some(Self::SetTable),
            "NEWTABLE" => Some(Self::NewTable),
            "SELF" => Some(Self::SelfOp),
            "ADD" => Some(Self::Add),
            "SUB" => Some(Self::Sub),
            "MUL" => Some(Self::Mul),
            "DIV" => Some(Self::Div),
            "MOD" => Some(Self::Mod),
            "POW" => Some(Self::Pow),
            "UNM" => Some(Self::Unm),
            "NOT" => Some(Self::Not),
            "LEN" => Some(Self::Len),
            "CONCAT" => Some(Self::Concat),
            "JMP" => Some(Self::Jmp),
            "EQ" => Some(Self::Eq),
            "LT" => Some(Self::Lt),
            "LE" => Some(Self::Le),
            "TEST" => Some(Self::Test),
            "TESTSET" => Some(Self::TestSet),
            "CALL" => Some(Self::Call),
            "TAILCALL" => Some(Self::TailCall),
            "RETURN" => Some(Self::Return),
            "FORLOOP" => Some(Self::ForLoop),
            "FORPREP" => Some(Self::ForPrep),
            "TFORLOOP" => Some(Self::TForLoop),
            "SETLIST" => Some(Self::SetList),
            "CLOSE" => Some(Self::Close),
            "CLOSURE" => Some(Self::Closure),
            "VARARG" => Some(Self::VarArg),
            "LOADKX" => Some(Self::LoadKx),
            "GETTABUP" => Some(Self::GetTabUp),
            "SETTABUP" => Some(Self::SetTabUp),
            "TFORCALL" => Some(Self::TForCall),
            "TFORLOOP54" => Some(Self::TForLoop54),
            "EXTRAARG" => Some(Self::ExtraArg),
            "IDIV" => Some(Self::Idiv),
            "BAND" => Some(Self::Band),
            "BOR" => Some(Self::Bor),
            "BXOR" => Some(Self::Bxor),
            "SHL" => Some(Self::Shl),
            "SHR" => Some(Self::Shr),
            "BNOT" => Some(Self::Bnot),
            "LOADI" => Some(Self::LoadI),
            "LOADF" => Some(Self::LoadF),
            "LOADFALSE" => Some(Self::LoadFalse),
            "LOADTRUE" => Some(Self::LoadTrue),
            "LFALSESKIP" => Some(Self::LFalseSkip),
            "ADDI" => Some(Self::AddI),
            "ADDK" => Some(Self::AddK),
            "SUBK" => Some(Self::SubK),
            "MULK" => Some(Self::MulK),
            "MODK" => Some(Self::ModK),
            "POWK" => Some(Self::PowK),
            "DIVK" => Some(Self::DivK),
            "IDIVK" => Some(Self::IdivK),
            "BANDK" => Some(Self::BandK),
            "BORK" => Some(Self::BorK),
            "BXORK" => Some(Self::BxorK),
            "SHRI" => Some(Self::ShrI),
            "SHLI" => Some(Self::ShlI),
            "GETI" => Some(Self::GetI),
            "GETFIELD" => Some(Self::GetField),
            "SETI" => Some(Self::SetI),
            "SETFIELD" => Some(Self::SetField),
            "EQK" => Some(Self::EqK),
            "EQI" => Some(Self::EqI),
            "LTI" => Some(Self::LtI),
            "LEI" => Some(Self::LeI),
            "GTI" => Some(Self::GtI),
            "GEI" => Some(Self::GeI),
            "MMBIN" => Some(Self::MmBin),
            "MMBINI" => Some(Self::MmBinI),
            "MMBINK" => Some(Self::MmBinK),
            "TBC" => Some(Self::Tbc),
            "RETURN0" => Some(Self::Return0),
            "RETURN1" => Some(Self::Return1),
            "TFORPREP" => Some(Self::TForPrep),
            "VARARGPREP" => Some(Self::VarArgPrep),
            "GETVARG" => Some(Self::GetVarG),
            "ERRNNIL" => Some(Self::ErrNnil),
            "UNKNOWN" => Some(Self::Unknown),
            _ => None,
        }
    }
}
