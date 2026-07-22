//! Opcode tables and instruction field decoding.

use crate::{LuaError, version::LuaTarget};

use super::{Instruction, SemanticOp};

/// Raw instruction layout for an opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionFormat {
    /// A, B, C fields.
    Abc,
    /// A, Bx fields.
    Abx,
    /// A, signed Bx fields.
    AsBx,
    /// Ax field.
    Ax,
    /// Signed jump field.
    IsJ,
    /// Lua 5.5 variant ABC.
    IvAbc,
}

impl InstructionFormat {
    fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_uppercase().as_str() {
            "ABC" => Self::Abc,
            "ABX" => Self::Abx,
            "ASBX" => Self::AsBx,
            "AX" => Self::Ax,
            "ISJ" => Self::IsJ,
            "IVABC" => Self::IvAbc,
            _ => Self::Abc,
        }
    }
}

/// A version-specific raw-opcode to semantic-opcode map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcodeTable {
    /// Lua bytecode version.
    pub version: LuaTarget,
    /// Opcode field width.
    pub op_bits: u8,
    /// A field width.
    pub a_bits: u8,
    /// B field width.
    pub b_bits: u8,
    /// C field width.
    pub c_bits: u8,
    /// Bx field width.
    pub bx_bits: u8,
    /// Signed Bx excess-K bias.
    pub sbx_bias: i32,
    /// RK constant tag bit for 5.1-5.3 layouts.
    pub rk_bit: i32,
    /// Whether this layout uses the later separate k flag.
    pub has_k_flag: bool,
    /// Raw opcode ordinal to semantic opcode.
    pub map: Vec<SemanticOp>,
    formats: Vec<InstructionFormat>,
}

impl OpcodeTable {
    /// Return a built-in opcode table.
    ///
    pub fn builtin(version: LuaTarget) -> Self {
        match version {
            LuaTarget::V51 => Self::builtin_51(),
        }
    }

    /// Parse a custom opcode table text file.
    ///
    /// # Errors
    ///
    /// Returns [`LuaError::Malformed`] when a directive, ordinal, or opcode name is invalid.
    pub fn from_custom_text(text: &str) -> Result<Self, LuaError> {
        let mut table = Self {
            version: LuaTarget::V51,
            op_bits: 6,
            a_bits: 8,
            b_bits: 9,
            c_bits: 9,
            bx_bits: 18,
            sbx_bias: 131_071,
            rk_bit: 256,
            has_k_flag: false,
            map: Vec::new(),
            formats: Vec::new(),
        };

        for (line_index, original_line) in text.lines().enumerate() {
            let line_number = line_index + 1;
            let line = original_line
                .split_once('#')
                .map_or(original_line, |(line, _)| line);
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let mut tokens = trimmed.split_whitespace();
            let first = tokens
                .next()
                .ok_or_else(|| malformed_line(line_number, "empty line"))?;
            let second = tokens.next();

            match first.to_ascii_uppercase().as_str() {
                "VERSION" => {
                    let Some(value) = second else {
                        return Err(malformed_line(line_number, "missing VERSION value"));
                    };
                    table.version = parse_version(value).ok_or_else(|| {
                        malformed_line(line_number, format!("unknown version \"{value}\""))
                    })?;
                    continue;
                }
                "OPBITS" => {
                    table.op_bits = parse_u8_directive(line_number, "OPBITS", second)?;
                    continue;
                }
                "ABITS" => {
                    table.a_bits = parse_u8_directive(line_number, "ABITS", second)?;
                    continue;
                }
                "BBITS" => {
                    table.b_bits = parse_u8_directive(line_number, "BBITS", second)?;
                    continue;
                }
                "CBITS" => {
                    table.c_bits = parse_u8_directive(line_number, "CBITS", second)?;
                    continue;
                }
                "HASKFLAG" => {
                    let Some(value) = second else {
                        return Err(malformed_line(line_number, "missing HASKFLAG value"));
                    };
                    table.has_k_flag =
                        value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("1");
                    continue;
                }
                _ => {}
            }

            let ordinal = first.parse::<usize>().map_err(|_| {
                malformed_line(line_number, format!("invalid ordinal in \"{trimmed}\""))
            })?;
            let Some(op_name) = second else {
                return Err(malformed_line(line_number, "missing opcode name"));
            };
            let op = SemanticOp::from_name(op_name).ok_or_else(|| {
                malformed_line(line_number, format!("unknown opcode name \"{op_name}\""))
            })?;
            if op == SemanticOp::Unknown {
                return Err(malformed_line(
                    line_number,
                    "UNKNOWN is not a valid mapping",
                ));
            }

            let format = tokens
                .next()
                .map_or(InstructionFormat::Abc, InstructionFormat::from_name);
            table.set_entry(ordinal, op, format);
        }

        table.recompute_derived()?;
        Ok(table)
    }

    /// Decode a raw instruction word.
    #[must_use]
    pub fn decode(&self, raw: u32) -> Instruction {
        Instruction {
            raw,
            op: self.decode_op(raw),
            a: self.decode_a(raw),
            b: self.decode_b(raw),
            c: self.decode_c(raw),
            bx: self.decode_bx(raw),
            sbx: self.decode_sbx(raw),
        }
    }

    /// Decode the raw opcode ordinal.
    #[must_use]
    pub fn raw_opcode(&self, raw: u32) -> usize {
        (raw & bit_mask(self.op_bits)) as usize
    }

    /// Decode the semantic opcode.
    #[must_use]
    pub fn decode_op(&self, raw: u32) -> SemanticOp {
        self.map
            .get(self.raw_opcode(raw))
            .copied()
            .unwrap_or(SemanticOp::Unknown)
    }

    /// Return the instruction format for a raw instruction.
    #[must_use]
    pub fn format_for_raw(&self, raw: u32) -> InstructionFormat {
        self.formats
            .get(self.raw_opcode(raw))
            .copied()
            .unwrap_or(InstructionFormat::Abc)
    }

    /// Decode the A field.
    #[must_use]
    pub fn decode_a(&self, raw: u32) -> i32 {
        decode_field(raw, self.op_bits, self.a_bits)
    }

    /// Decode the B field.
    #[must_use]
    pub fn decode_b(&self, raw: u32) -> i32 {
        let shift = if self.has_k_flag {
            self.op_bits + self.a_bits + 1
        } else {
            self.op_bits + self.a_bits + self.c_bits
        };
        decode_field(raw, shift, self.b_bits)
    }

    /// Decode the C field.
    #[must_use]
    pub fn decode_c(&self, raw: u32) -> i32 {
        let shift = if self.has_k_flag {
            self.op_bits + self.a_bits + 1 + self.b_bits
        } else {
            self.op_bits + self.a_bits
        };
        decode_field(raw, shift, self.c_bits)
    }

    /// Decode the unsigned Bx field.
    #[must_use]
    pub fn decode_bx(&self, raw: u32) -> i32 {
        decode_field(raw, self.op_bits + self.a_bits, self.bx_bits)
    }

    /// Decode the signed sBx field.
    #[must_use]
    pub fn decode_sbx(&self, raw: u32) -> i32 {
        self.decode_bx(raw) - self.sbx_bias
    }

    /// Decode the Ax field used by later Lua versions.
    #[must_use]
    pub fn decode_ax(&self, raw: u32) -> i32 {
        decode_field(raw, self.op_bits, 32 - self.op_bits)
    }

    /// Decode the signed jump field used by later Lua versions.
    #[must_use]
    pub fn decode_sj(&self, raw: u32) -> i32 {
        let bits = 32 - self.op_bits;
        let bias = signed_bias(bits);
        decode_field(raw, self.op_bits, bits) - bias
    }

    /// Decode the later-version k flag.
    #[must_use]
    pub fn decode_k(&self, raw: u32) -> bool {
        if !self.has_k_flag {
            return false;
        }
        ((raw >> (self.op_bits + self.a_bits)) & 1) != 0
    }

    /// Decode signed C for later immediate opcodes.
    #[must_use]
    pub fn decode_sc(&self, raw: u32) -> i32 {
        self.decode_c(raw) - signed_bias(self.c_bits)
    }

    /// Decode signed B for later immediate opcodes.
    #[must_use]
    pub fn decode_sb(&self, raw: u32) -> i32 {
        self.decode_b(raw) - signed_bias(self.b_bits)
    }

    /// Decode the vB field from Lua 5.5 ivABC format.
    #[must_use]
    pub fn decode_vb(&self, raw: u32) -> i32 {
        decode_field(raw, self.op_bits + self.a_bits + 1, 6)
    }

    /// Decode the vC field from Lua 5.5 ivABC format.
    #[must_use]
    pub fn decode_vc(&self, raw: u32) -> i32 {
        decode_field(raw, self.op_bits + self.a_bits + 1 + 6, 10)
    }

    /// Return whether the raw opcode uses Lua 5.5 ivABC format.
    #[must_use]
    pub fn is_iv_abc_format(&self, raw: u32) -> bool {
        self.format_for_raw(raw) == InstructionFormat::IvAbc
    }

    /// Return whether a B/C operand is RK-encoded as a constant.
    #[must_use]
    pub fn is_k(&self, field: i32) -> bool {
        !self.has_k_flag && (field & self.rk_bit) != 0
    }

    /// Strip the RK bit to get the constant index.
    #[must_use]
    pub fn rk_index(&self, field: i32) -> i32 {
        if self.has_k_flag {
            field
        } else {
            field & (self.rk_bit - 1)
        }
    }

    fn builtin_51() -> Self {
        let mut table = Self {
            version: LuaTarget::V51,
            op_bits: 6,
            a_bits: 8,
            b_bits: 9,
            c_bits: 9,
            bx_bits: 18,
            sbx_bias: 131_071,
            rk_bit: 256,
            has_k_flag: false,
            map: vec![SemanticOp::Unknown; 38],
            formats: vec![InstructionFormat::Abc; 38],
        };

        table.set_entry(0, SemanticOp::Move, InstructionFormat::Abc);
        table.set_entry(1, SemanticOp::LoadK, InstructionFormat::Abx);
        table.set_entry(2, SemanticOp::LoadBool, InstructionFormat::Abc);
        table.set_entry(3, SemanticOp::LoadNil, InstructionFormat::Abc);
        table.set_entry(4, SemanticOp::GetUpval, InstructionFormat::Abc);
        table.set_entry(5, SemanticOp::GetGlobal, InstructionFormat::Abx);
        table.set_entry(6, SemanticOp::GetTable, InstructionFormat::Abc);
        table.set_entry(7, SemanticOp::SetGlobal, InstructionFormat::Abx);
        table.set_entry(8, SemanticOp::SetUpval, InstructionFormat::Abc);
        table.set_entry(9, SemanticOp::SetTable, InstructionFormat::Abc);
        table.set_entry(10, SemanticOp::NewTable, InstructionFormat::Abc);
        table.set_entry(11, SemanticOp::SelfOp, InstructionFormat::Abc);
        table.set_entry(12, SemanticOp::Add, InstructionFormat::Abc);
        table.set_entry(13, SemanticOp::Sub, InstructionFormat::Abc);
        table.set_entry(14, SemanticOp::Mul, InstructionFormat::Abc);
        table.set_entry(15, SemanticOp::Div, InstructionFormat::Abc);
        table.set_entry(16, SemanticOp::Mod, InstructionFormat::Abc);
        table.set_entry(17, SemanticOp::Pow, InstructionFormat::Abc);
        table.set_entry(18, SemanticOp::Unm, InstructionFormat::Abc);
        table.set_entry(19, SemanticOp::Not, InstructionFormat::Abc);
        table.set_entry(20, SemanticOp::Len, InstructionFormat::Abc);
        table.set_entry(21, SemanticOp::Concat, InstructionFormat::Abc);
        table.set_entry(22, SemanticOp::Jmp, InstructionFormat::AsBx);
        table.set_entry(23, SemanticOp::Eq, InstructionFormat::Abc);
        table.set_entry(24, SemanticOp::Lt, InstructionFormat::Abc);
        table.set_entry(25, SemanticOp::Le, InstructionFormat::Abc);
        table.set_entry(26, SemanticOp::Test, InstructionFormat::Abc);
        table.set_entry(27, SemanticOp::TestSet, InstructionFormat::Abc);
        table.set_entry(28, SemanticOp::Call, InstructionFormat::Abc);
        table.set_entry(29, SemanticOp::TailCall, InstructionFormat::Abc);
        table.set_entry(30, SemanticOp::Return, InstructionFormat::Abc);
        table.set_entry(31, SemanticOp::ForLoop, InstructionFormat::AsBx);
        table.set_entry(32, SemanticOp::ForPrep, InstructionFormat::AsBx);
        table.set_entry(33, SemanticOp::TForLoop, InstructionFormat::Abc);
        table.set_entry(34, SemanticOp::SetList, InstructionFormat::Abc);
        table.set_entry(35, SemanticOp::Close, InstructionFormat::Abc);
        table.set_entry(36, SemanticOp::Closure, InstructionFormat::Abx);
        table.set_entry(37, SemanticOp::VarArg, InstructionFormat::Abc);
        table
    }

    fn set_entry(&mut self, ordinal: usize, op: SemanticOp, format: InstructionFormat) {
        if ordinal >= self.map.len() {
            self.map.resize(ordinal + 1, SemanticOp::Unknown);
            self.formats.resize(ordinal + 1, InstructionFormat::Abc);
        }
        self.map[ordinal] = op;
        self.formats[ordinal] = format;
    }

    fn recompute_derived(&mut self) -> Result<(), LuaError> {
        validate_bits("OPBITS", self.op_bits)?;
        validate_bits("ABITS", self.a_bits)?;
        validate_bits("BBITS", self.b_bits)?;
        validate_bits("CBITS", self.c_bits)?;

        let k_bits = u8::from(self.has_k_flag);
        self.bx_bits = self
            .b_bits
            .checked_add(self.c_bits)
            .and_then(|bits| bits.checked_add(k_bits))
            .ok_or_else(|| LuaError::Malformed("opcode field widths overflow".to_string()))?;

        let total_bits = self
            .op_bits
            .checked_add(self.a_bits)
            .and_then(|bits| bits.checked_add(self.bx_bits))
            .ok_or_else(|| LuaError::Malformed("opcode field widths overflow".to_string()))?;
        if total_bits > 32 {
            return Err(LuaError::Malformed(format!(
                "opcode field widths total {total_bits} bits, expected at most 32"
            )));
        }
        if self.bx_bits == 0 || self.bx_bits > 31 {
            return Err(LuaError::Malformed(format!(
                "unsupported Bx width {}",
                self.bx_bits
            )));
        }
        if self.b_bits > 31 {
            return Err(LuaError::Malformed(format!(
                "unsupported B width {}",
                self.b_bits
            )));
        }

        self.sbx_bias = signed_bias(self.bx_bits);
        self.rk_bit = 1_i32 << (self.b_bits - 1);
        Ok(())
    }
}

fn parse_version(value: &str) -> Option<LuaTarget> {
    match value {
        "51" => Some(LuaTarget::V51),
        _ => None,
    }
}

fn parse_u8_directive(line_number: usize, name: &str, value: Option<&str>) -> Result<u8, LuaError> {
    let Some(value) = value else {
        return Err(malformed_line(line_number, format!("missing {name} value")));
    };
    value
        .parse::<u8>()
        .map_err(|_| malformed_line(line_number, format!("invalid {name} value \"{value}\"")))
}

fn validate_bits(name: &str, bits: u8) -> Result<(), LuaError> {
    if bits == 0 || bits > 31 {
        return Err(LuaError::Malformed(format!(
            "unsupported {name} width {bits}"
        )));
    }
    Ok(())
}

fn malformed_line(line_number: usize, message: impl Into<String>) -> LuaError {
    LuaError::Malformed(format!(
        "opcode table line {line_number}: {}",
        message.into()
    ))
}

fn decode_field(raw: u32, shift: u8, bits: u8) -> i32 {
    ((raw >> shift) & bit_mask(bits)) as i32
}

fn bit_mask(bits: u8) -> u32 {
    if bits >= 32 {
        u32::MAX
    } else {
        (1_u32 << bits) - 1
    }
}

fn signed_bias(bits: u8) -> i32 {
    ((1_i64 << (bits - 1)) - 1) as i32
}
