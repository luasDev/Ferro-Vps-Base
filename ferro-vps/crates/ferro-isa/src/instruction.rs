//! The high-level, semantic instruction enum and its encode/decode.
//!
//! [`Instruction`] is the disassembled view the CPU and assembler consume: one
//! variant per instruction with its operands already decoded. Bit twiddling is
//! delegated to the format structs in [`crate::format`]; this module only maps
//! opcodes/funct codes to variants and validates that reserved fields are
//! zero.
//!
//! Two decoding APIs are available so the CPU can choose its strategy: the
//! lightweight [`crate::format`] structs for a field-dispatch fast path, and
//! this semantic [`Instruction::decode`] for tooling. Decoding is `O(1)`:
//! the primary opcode is extracted first and dispatched through a dense match
//! that the compiler lowers to a jump table.
//!
//! `decode` never panics on any 32-bit input: unknown encodings and non-zero
//! reserved fields return [`DecodeError::IllegalOpcode`]. For every word that
//! decodes successfully, `inst.encode()` reproduces the original word exactly.
//!
//! `NOP` is a pseudo-instruction equal to `ADDI r0, r0, 0`; see
//! [`Instruction::nop`].

use crate::error::DecodeError;
use crate::format::{imm15_field, opcode_of, BType, IType, JType, RType, SType, UType};
use crate::opcode::{
    FUNCT_ADD, FUNCT_AND, FUNCT_DIV, FUNCT_DIVU, FUNCT_MUL, FUNCT_MULH, FUNCT_NOT, FUNCT_OR,
    FUNCT_REM, FUNCT_REMU, FUNCT_SLL, FUNCT_SLT, FUNCT_SLTU, FUNCT_SRA, FUNCT_SRL, FUNCT_SUB,
    FUNCT_XOR, OP_ADDI, OP_ANDI, OP_AUIPC, OP_ALU_R, OP_BEQ, OP_BGE, OP_BGEU, OP_BLT, OP_BLTU,
    OP_BNE, OP_CSRR, OP_CSRW, OP_JAL, OP_JALR, OP_LB, OP_LBU, OP_LH, OP_LHU, OP_LUI, OP_LW,
    OP_ORI, OP_SB, OP_SH, OP_SLLI, OP_SLTI, OP_SLTIU, OP_SRAI, OP_SRLI, OP_SW, OP_SYSTEM, OP_XORI,
    SYS_EBREAK, SYS_ECALL, SYS_HALT, SYS_SRET,
};
use crate::register::Register;

const fn illegal(word: u32) -> DecodeError {
    DecodeError::IllegalOpcode { word }
}

/// A fully decoded Ferro VM instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Instruction {
    /// `rd = rs1 + rs2` (wrapping).
    Add {
        /// Destination register.
        rd: Register,
        /// First source register.
        rs1: Register,
        /// Second source register.
        rs2: Register,
    },
    /// `rd = rs1 - rs2` (wrapping).
    Sub {
        /// Destination register.
        rd: Register,
        /// First source register.
        rs1: Register,
        /// Second source register.
        rs2: Register,
    },
    /// `rd = rs1 & rs2`.
    And {
        /// Destination register.
        rd: Register,
        /// First source register.
        rs1: Register,
        /// Second source register.
        rs2: Register,
    },
    /// `rd = rs1 | rs2`.
    Or {
        /// Destination register.
        rd: Register,
        /// First source register.
        rs1: Register,
        /// Second source register.
        rs2: Register,
    },
    /// `rd = rs1 ^ rs2`.
    Xor {
        /// Destination register.
        rd: Register,
        /// First source register.
        rs1: Register,
        /// Second source register.
        rs2: Register,
    },
    /// `rd = rs1 << (rs2 & 31)`.
    Sll {
        /// Destination register.
        rd: Register,
        /// First source register.
        rs1: Register,
        /// Second source register (shift amount).
        rs2: Register,
    },
    /// `rd = rs1 >> (rs2 & 31)` (logical).
    Srl {
        /// Destination register.
        rd: Register,
        /// First source register.
        rs1: Register,
        /// Second source register (shift amount).
        rs2: Register,
    },
    /// `rd = rs1 >> (rs2 & 31)` (arithmetic).
    Sra {
        /// Destination register.
        rd: Register,
        /// First source register.
        rs1: Register,
        /// Second source register (shift amount).
        rs2: Register,
    },
    /// `rd = (rs1 < rs2) as 1/0` (signed).
    Slt {
        /// Destination register.
        rd: Register,
        /// First source register.
        rs1: Register,
        /// Second source register.
        rs2: Register,
    },
    /// `rd = (rs1 < rs2) as 1/0` (unsigned).
    Sltu {
        /// Destination register.
        rd: Register,
        /// First source register.
        rs1: Register,
        /// Second source register.
        rs2: Register,
    },
    /// `rd = low 32 bits of rs1 * rs2`.
    Mul {
        /// Destination register.
        rd: Register,
        /// First source register.
        rs1: Register,
        /// Second source register.
        rs2: Register,
    },
    /// `rd = high 32 bits of rs1 * rs2` (signed).
    Mulh {
        /// Destination register.
        rd: Register,
        /// First source register.
        rs1: Register,
        /// Second source register.
        rs2: Register,
    },
    /// `rd = rs1 / rs2` (signed).
    Div {
        /// Destination register.
        rd: Register,
        /// First source register (dividend).
        rs1: Register,
        /// Second source register (divisor).
        rs2: Register,
    },
    /// `rd = rs1 / rs2` (unsigned).
    Divu {
        /// Destination register.
        rd: Register,
        /// First source register (dividend).
        rs1: Register,
        /// Second source register (divisor).
        rs2: Register,
    },
    /// `rd = rs1 % rs2` (signed).
    Rem {
        /// Destination register.
        rd: Register,
        /// First source register (dividend).
        rs1: Register,
        /// Second source register (divisor).
        rs2: Register,
    },
    /// `rd = rs1 % rs2` (unsigned).
    Remu {
        /// Destination register.
        rd: Register,
        /// First source register (dividend).
        rs1: Register,
        /// Second source register (divisor).
        rs2: Register,
    },
    /// `rd = !rs1`.
    Not {
        /// Destination register.
        rd: Register,
        /// Source register.
        rs1: Register,
    },
    /// `rd = rs1 + sext(imm)`.
    Addi {
        /// Destination register.
        rd: Register,
        /// Source register.
        rs1: Register,
        /// Signed immediate operand.
        imm: i32,
    },
    /// `rd = rs1 & sext(imm)`.
    Andi {
        /// Destination register.
        rd: Register,
        /// Source register.
        rs1: Register,
        /// Signed immediate operand.
        imm: i32,
    },
    /// `rd = rs1 | sext(imm)`.
    Ori {
        /// Destination register.
        rd: Register,
        /// Source register.
        rs1: Register,
        /// Signed immediate operand.
        imm: i32,
    },
    /// `rd = rs1 ^ sext(imm)`.
    Xori {
        /// Destination register.
        rd: Register,
        /// Source register.
        rs1: Register,
        /// Signed immediate operand.
        imm: i32,
    },
    /// `rd = (rs1 < sext(imm)) as 1/0` (signed).
    Slti {
        /// Destination register.
        rd: Register,
        /// Source register.
        rs1: Register,
        /// Signed immediate operand.
        imm: i32,
    },
    /// `rd = (rs1 < sext(imm)) as 1/0` (unsigned).
    Sltiu {
        /// Destination register.
        rd: Register,
        /// Source register.
        rs1: Register,
        /// Signed immediate operand.
        imm: i32,
    },
    /// `rd = rs1 << (imm & 31)`.
    Slli {
        /// Destination register.
        rd: Register,
        /// Source register.
        rs1: Register,
        /// Shift amount immediate.
        imm: i32,
    },
    /// `rd = rs1 >> (imm & 31)` (logical).
    Srli {
        /// Destination register.
        rd: Register,
        /// Source register.
        rs1: Register,
        /// Shift amount immediate.
        imm: i32,
    },
    /// `rd = rs1 >> (imm & 31)` (arithmetic).
    Srai {
        /// Destination register.
        rd: Register,
        /// Source register.
        rs1: Register,
        /// Shift amount immediate.
        imm: i32,
    },
    /// `rd = pc + 4; pc = (rs1 + sext(imm)) & !1`.
    Jalr {
        /// Destination (link) register.
        rd: Register,
        /// Base address register.
        rs1: Register,
        /// Signed immediate offset.
        imm: i32,
    },
    /// `rd = sign-extend 8 bits of mem[rs1 + sext(imm)]`.
    Lb {
        /// Destination register.
        rd: Register,
        /// Base address register.
        rs1: Register,
        /// Signed immediate offset.
        imm: i32,
    },
    /// `rd = zero-extend 8 bits of mem[rs1 + sext(imm)]`.
    Lbu {
        /// Destination register.
        rd: Register,
        /// Base address register.
        rs1: Register,
        /// Signed immediate offset.
        imm: i32,
    },
    /// `rd = sign-extend 16 bits of mem[rs1 + sext(imm)]`.
    Lh {
        /// Destination register.
        rd: Register,
        /// Base address register.
        rs1: Register,
        /// Signed immediate offset.
        imm: i32,
    },
    /// `rd = zero-extend 16 bits of mem[rs1 + sext(imm)]`.
    Lhu {
        /// Destination register.
        rd: Register,
        /// Base address register.
        rs1: Register,
        /// Signed immediate offset.
        imm: i32,
    },
    /// `rd = mem32(rs1 + sext(imm))`.
    Lw {
        /// Destination register.
        rd: Register,
        /// Base address register.
        rs1: Register,
        /// Signed immediate offset.
        imm: i32,
    },
    /// `mem8(rs1 + sext(imm)) = rs2`.
    Sb {
        /// Source register holding the value to store.
        rs2: Register,
        /// Base address register.
        rs1: Register,
        /// Signed immediate offset.
        imm: i32,
    },
    /// `mem16(rs1 + sext(imm)) = rs2`.
    Sh {
        /// Source register holding the value to store.
        rs2: Register,
        /// Base address register.
        rs1: Register,
        /// Signed immediate offset.
        imm: i32,
    },
    /// `mem32(rs1 + sext(imm)) = rs2`.
    Sw {
        /// Source register holding the value to store.
        rs2: Register,
        /// Base address register.
        rs1: Register,
        /// Signed immediate offset.
        imm: i32,
    },
    /// `if rs1 == rs2 then pc += sext(imm) * 4`.
    Beq {
        /// First comparison register.
        rs1: Register,
        /// Second comparison register.
        rs2: Register,
        /// Signed branch offset in words.
        imm: i32,
    },
    /// `if rs1 != rs2 then pc += sext(imm) * 4`.
    Bne {
        /// First comparison register.
        rs1: Register,
        /// Second comparison register.
        rs2: Register,
        /// Signed branch offset in words.
        imm: i32,
    },
    /// `if rs1 < rs2 (signed) then pc += sext(imm) * 4`.
    Blt {
        /// First comparison register.
        rs1: Register,
        /// Second comparison register.
        rs2: Register,
        /// Signed branch offset in words.
        imm: i32,
    },
    /// `if rs1 >= rs2 (signed) then pc += sext(imm) * 4`.
    Bge {
        /// First comparison register.
        rs1: Register,
        /// Second comparison register.
        rs2: Register,
        /// Signed branch offset in words.
        imm: i32,
    },
    /// `if rs1 < rs2 (unsigned) then pc += sext(imm) * 4`.
    Bltu {
        /// First comparison register.
        rs1: Register,
        /// Second comparison register.
        rs2: Register,
        /// Signed branch offset in words.
        imm: i32,
    },
    /// `if rs1 >= rs2 (unsigned) then pc += sext(imm) * 4`.
    Bgeu {
        /// First comparison register.
        rs1: Register,
        /// Second comparison register.
        rs2: Register,
        /// Signed branch offset in words.
        imm: i32,
    },
    /// `rd = imm << 12`.
    Lui {
        /// Destination register.
        rd: Register,
        /// Upper immediate operand.
        imm: i32,
    },
    /// `rd = pc + (imm << 12)`.
    Auipc {
        /// Destination register.
        rd: Register,
        /// Upper immediate operand.
        imm: i32,
    },
    /// `rd = pc + 4; pc += sext(imm) * 4`.
    Jal {
        /// Destination (link) register.
        rd: Register,
        /// Signed jump offset in words.
        imm: i32,
    },
    /// Trap to the kernel (system call).
    Ecall,
    /// Trap to the debugger.
    Ebreak,
    /// Stop the processor (privileged).
    Halt,
    /// Return from a trap (privileged).
    Sret,
    /// `rd = sysreg[index]` (privileged).
    Csrr {
        /// Destination register.
        rd: Register,
        /// System register index.
        sysreg: u16,
    },
    /// `sysreg[index] = rs1` (privileged).
    Csrw {
        /// Source register.
        rs1: Register,
        /// System register index.
        sysreg: u16,
    },
}

impl Instruction {
    /// Returns the canonical `NOP`, encoded as `ADDI r0, r0, 0`.
    #[must_use]
    pub const fn nop() -> Self {
        Self::Addi {
            rd: Register::ZERO,
            rs1: Register::ZERO,
            imm: 0,
        }
    }

    /// Returns `true` when this instruction may only run in kernel mode.
    #[must_use]
    pub const fn is_privileged(self) -> bool {
        matches!(
            self,
            Self::Halt | Self::Sret | Self::Csrr { .. } | Self::Csrw { .. }
        )
    }

    /// Decodes a 32-bit instruction word.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::IllegalOpcode`] when the word does not match any
    /// known encoding or when a reserved field is non-zero. Never panics for
    /// any input.
    #[allow(clippy::too_many_lines)]
    pub fn decode(word: u32) -> Result<Self, DecodeError> {
        let opcode = opcode_of(word);
        match opcode {
            OP_ALU_R => Self::decode_alu_r(word),
            OP_SYSTEM => Self::decode_system(word),
            OP_ADDI => {
                let i = IType::decode(word);
                Ok(Self::Addi { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_ANDI => {
                let i = IType::decode(word);
                Ok(Self::Andi { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_ORI => {
                let i = IType::decode(word);
                Ok(Self::Ori { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_XORI => {
                let i = IType::decode(word);
                Ok(Self::Xori { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_SLTI => {
                let i = IType::decode(word);
                Ok(Self::Slti { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_SLTIU => {
                let i = IType::decode(word);
                Ok(Self::Sltiu { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_SLLI => {
                let i = IType::decode(word);
                Ok(Self::Slli { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_SRLI => {
                let i = IType::decode(word);
                Ok(Self::Srli { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_SRAI => {
                let i = IType::decode(word);
                Ok(Self::Srai { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_JALR => {
                let i = IType::decode(word);
                Ok(Self::Jalr { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_LB => {
                let i = IType::decode(word);
                Ok(Self::Lb { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_LBU => {
                let i = IType::decode(word);
                Ok(Self::Lbu { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_LH => {
                let i = IType::decode(word);
                Ok(Self::Lh { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_LHU => {
                let i = IType::decode(word);
                Ok(Self::Lhu { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_LW => {
                let i = IType::decode(word);
                Ok(Self::Lw { rd: i.rd, rs1: i.rs1, imm: i.imm })
            }
            OP_SB => {
                let s = SType::decode(word);
                Ok(Self::Sb { rs2: s.rs2, rs1: s.rs1, imm: s.imm })
            }
            OP_SH => {
                let s = SType::decode(word);
                Ok(Self::Sh { rs2: s.rs2, rs1: s.rs1, imm: s.imm })
            }
            OP_SW => {
                let s = SType::decode(word);
                Ok(Self::Sw { rs2: s.rs2, rs1: s.rs1, imm: s.imm })
            }
            OP_BEQ => {
                let b = BType::decode(word);
                Ok(Self::Beq { rs1: b.rs1, rs2: b.rs2, imm: b.imm })
            }
            OP_BNE => {
                let b = BType::decode(word);
                Ok(Self::Bne { rs1: b.rs1, rs2: b.rs2, imm: b.imm })
            }
            OP_BLT => {
                let b = BType::decode(word);
                Ok(Self::Blt { rs1: b.rs1, rs2: b.rs2, imm: b.imm })
            }
            OP_BGE => {
                let b = BType::decode(word);
                Ok(Self::Bge { rs1: b.rs1, rs2: b.rs2, imm: b.imm })
            }
            OP_BLTU => {
                let b = BType::decode(word);
                Ok(Self::Bltu { rs1: b.rs1, rs2: b.rs2, imm: b.imm })
            }
            OP_BGEU => {
                let b = BType::decode(word);
                Ok(Self::Bgeu { rs1: b.rs1, rs2: b.rs2, imm: b.imm })
            }
            OP_LUI => {
                let u = UType::decode(word);
                Ok(Self::Lui { rd: u.rd, imm: u.imm })
            }
            OP_AUIPC => {
                let u = UType::decode(word);
                Ok(Self::Auipc { rd: u.rd, imm: u.imm })
            }
            OP_JAL => {
                let j = JType::decode(word);
                Ok(Self::Jal { rd: j.rd, imm: j.imm })
            }
            OP_CSRR => {
                let i = IType::decode(word);
                if i.rs1.is_zero() {
                    Ok(Self::Csrr { rd: i.rd, sysreg: imm15_field(word) })
                } else {
                    Err(illegal(word))
                }
            }
            OP_CSRW => {
                let i = IType::decode(word);
                if i.rd.is_zero() {
                    Ok(Self::Csrw { rs1: i.rs1, sysreg: imm15_field(word) })
                } else {
                    Err(illegal(word))
                }
            }
            _ => Err(illegal(word)),
        }
    }

    fn decode_alu_r(word: u32) -> Result<Self, DecodeError> {
        let r = RType::decode(word);
        let instruction = match r.funct {
            FUNCT_ADD => Self::Add { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_SUB => Self::Sub { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_AND => Self::And { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_OR => Self::Or { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_XOR => Self::Xor { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_SLL => Self::Sll { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_SRL => Self::Srl { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_SRA => Self::Sra { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_SLT => Self::Slt { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_SLTU => Self::Sltu { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_MUL => Self::Mul { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_MULH => Self::Mulh { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_DIV => Self::Div { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_DIVU => Self::Divu { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_REM => Self::Rem { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_REMU => Self::Remu { rd: r.rd, rs1: r.rs1, rs2: r.rs2 },
            FUNCT_NOT if r.rs2.is_zero() => Self::Not { rd: r.rd, rs1: r.rs1 },
            _ => return Err(illegal(word)),
        };
        Ok(instruction)
    }

    fn decode_system(word: u32) -> Result<Self, DecodeError> {
        let r = RType::decode(word);
        if !(r.rd.is_zero() && r.rs1.is_zero() && r.rs2.is_zero()) {
            return Err(illegal(word));
        }
        match r.funct {
            SYS_ECALL => Ok(Self::Ecall),
            SYS_EBREAK => Ok(Self::Ebreak),
            SYS_HALT => Ok(Self::Halt),
            SYS_SRET => Ok(Self::Sret),
            _ => Err(illegal(word)),
        }
    }

    /// Encodes this instruction into its 32-bit word.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn encode(self) -> u32 {
        let zero = Register::ZERO;
        match self {
            Self::Add { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_ADD),
            Self::Sub { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_SUB),
            Self::And { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_AND),
            Self::Or { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_OR),
            Self::Xor { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_XOR),
            Self::Sll { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_SLL),
            Self::Srl { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_SRL),
            Self::Sra { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_SRA),
            Self::Slt { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_SLT),
            Self::Sltu { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_SLTU),
            Self::Mul { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_MUL),
            Self::Mulh { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_MULH),
            Self::Div { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_DIV),
            Self::Divu { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_DIVU),
            Self::Rem { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_REM),
            Self::Remu { rd, rs1, rs2 } => alu_r(rd, rs1, rs2, FUNCT_REMU),
            Self::Not { rd, rs1 } => alu_r(rd, rs1, zero, FUNCT_NOT),
            Self::Addi { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_ADDI),
            Self::Andi { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_ANDI),
            Self::Ori { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_ORI),
            Self::Xori { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_XORI),
            Self::Slti { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_SLTI),
            Self::Sltiu { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_SLTIU),
            Self::Slli { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_SLLI),
            Self::Srli { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_SRLI),
            Self::Srai { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_SRAI),
            Self::Jalr { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_JALR),
            Self::Lb { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_LB),
            Self::Lbu { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_LBU),
            Self::Lh { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_LH),
            Self::Lhu { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_LHU),
            Self::Lw { rd, rs1, imm } => IType { rd, rs1, imm }.encode(OP_LW),
            Self::Sb { rs2, rs1, imm } => SType { rs2, rs1, imm }.encode(OP_SB),
            Self::Sh { rs2, rs1, imm } => SType { rs2, rs1, imm }.encode(OP_SH),
            Self::Sw { rs2, rs1, imm } => SType { rs2, rs1, imm }.encode(OP_SW),
            Self::Beq { rs1, rs2, imm } => BType { rs1, rs2, imm }.encode(OP_BEQ),
            Self::Bne { rs1, rs2, imm } => BType { rs1, rs2, imm }.encode(OP_BNE),
            Self::Blt { rs1, rs2, imm } => BType { rs1, rs2, imm }.encode(OP_BLT),
            Self::Bge { rs1, rs2, imm } => BType { rs1, rs2, imm }.encode(OP_BGE),
            Self::Bltu { rs1, rs2, imm } => BType { rs1, rs2, imm }.encode(OP_BLTU),
            Self::Bgeu { rs1, rs2, imm } => BType { rs1, rs2, imm }.encode(OP_BGEU),
            Self::Lui { rd, imm } => UType { rd, imm }.encode(OP_LUI),
            Self::Auipc { rd, imm } => UType { rd, imm }.encode(OP_AUIPC),
            Self::Jal { rd, imm } => JType { rd, imm }.encode(OP_JAL),
            Self::Ecall => system(SYS_ECALL),
            Self::Ebreak => system(SYS_EBREAK),
            Self::Halt => system(SYS_HALT),
            Self::Sret => system(SYS_SRET),
            Self::Csrr { rd, sysreg } => {
                IType { rd, rs1: zero, imm: i32::from(sysreg) }.encode(OP_CSRR)
            }
            Self::Csrw { rs1, sysreg } => {
                IType { rd: zero, rs1, imm: i32::from(sysreg) }.encode(OP_CSRW)
            }
        }
    }
}

fn alu_r(rd: Register, rs1: Register, rs2: Register, funct: u16) -> u32 {
    RType { rd, rs1, rs2, funct }.encode(OP_ALU_R)
}

fn system(funct: u16) -> u32 {
    RType {
        rd: Register::ZERO,
        rs1: Register::ZERO,
        rs2: Register::ZERO,
        funct,
    }
    .encode(OP_SYSTEM)
}

#[cfg(test)]
mod tests {
    use super::Instruction;
    use crate::error::DecodeError;
    use crate::register::Register;

    fn r(index: u8) -> Register {
        Register::new(index).unwrap()
    }

    #[allow(clippy::too_many_lines)]
    fn samples() -> Vec<Instruction> {
        vec![
            Instruction::Add { rd: r(1), rs1: r(2), rs2: r(3) },
            Instruction::Sub { rd: r(4), rs1: r(5), rs2: r(6) },
            Instruction::And { rd: r(7), rs1: r(8), rs2: r(9) },
            Instruction::Or { rd: r(10), rs1: r(11), rs2: r(12) },
            Instruction::Xor { rd: r(13), rs1: r(14), rs2: r(15) },
            Instruction::Sll { rd: r(16), rs1: r(17), rs2: r(18) },
            Instruction::Srl { rd: r(19), rs1: r(20), rs2: r(21) },
            Instruction::Sra { rd: r(22), rs1: r(23), rs2: r(24) },
            Instruction::Slt { rd: r(25), rs1: r(26), rs2: r(27) },
            Instruction::Sltu { rd: r(28), rs1: r(29), rs2: r(30) },
            Instruction::Mul { rd: r(31), rs1: r(0), rs2: r(1) },
            Instruction::Mulh { rd: r(2), rs1: r(3), rs2: r(4) },
            Instruction::Div { rd: r(5), rs1: r(6), rs2: r(7) },
            Instruction::Divu { rd: r(8), rs1: r(9), rs2: r(10) },
            Instruction::Rem { rd: r(11), rs1: r(12), rs2: r(13) },
            Instruction::Remu { rd: r(14), rs1: r(15), rs2: r(16) },
            Instruction::Not { rd: r(17), rs1: r(18) },
            Instruction::Addi { rd: r(1), rs1: r(2), imm: -5 },
            Instruction::Andi { rd: r(1), rs1: r(2), imm: 7 },
            Instruction::Ori { rd: r(1), rs1: r(2), imm: 16383 },
            Instruction::Xori { rd: r(1), rs1: r(2), imm: -16384 },
            Instruction::Slti { rd: r(1), rs1: r(2), imm: -1 },
            Instruction::Sltiu { rd: r(1), rs1: r(2), imm: 42 },
            Instruction::Slli { rd: r(1), rs1: r(2), imm: 3 },
            Instruction::Srli { rd: r(1), rs1: r(2), imm: 4 },
            Instruction::Srai { rd: r(1), rs1: r(2), imm: 5 },
            Instruction::Jalr { rd: r(1), rs1: r(2), imm: -8 },
            Instruction::Lb { rd: r(1), rs1: r(2), imm: 1 },
            Instruction::Lbu { rd: r(1), rs1: r(2), imm: 2 },
            Instruction::Lh { rd: r(1), rs1: r(2), imm: 3 },
            Instruction::Lhu { rd: r(1), rs1: r(2), imm: 4 },
            Instruction::Lw { rd: r(1), rs1: r(2), imm: -100 },
            Instruction::Sb { rs2: r(3), rs1: r(2), imm: 6 },
            Instruction::Sh { rs2: r(3), rs1: r(2), imm: -7 },
            Instruction::Sw { rs2: r(3), rs1: r(2), imm: 8 },
            Instruction::Beq { rs1: r(1), rs2: r(2), imm: 9 },
            Instruction::Bne { rs1: r(1), rs2: r(2), imm: -9 },
            Instruction::Blt { rs1: r(1), rs2: r(2), imm: 10 },
            Instruction::Bge { rs1: r(1), rs2: r(2), imm: -10 },
            Instruction::Bltu { rs1: r(1), rs2: r(2), imm: 11 },
            Instruction::Bgeu { rs1: r(1), rs2: r(2), imm: -11 },
            Instruction::Lui { rd: r(5), imm: 0x1_2345 },
            Instruction::Auipc { rd: r(6), imm: -524_288 },
            Instruction::Jal { rd: r(1), imm: 524_287 },
            Instruction::Ecall,
            Instruction::Ebreak,
            Instruction::Halt,
            Instruction::Sret,
            Instruction::Csrr { rd: r(7), sysreg: 3 },
            Instruction::Csrw { rs1: r(8), sysreg: 4 },
            Instruction::nop(),
        ]
    }

    #[test]
    fn every_variant_round_trips() {
        for inst in samples() {
            let word = inst.encode();
            let decoded = Instruction::decode(word).unwrap();
            assert_eq!(decoded, inst, "decode(encode) mismatch");
            assert_eq!(decoded.encode(), word, "encode(decode) mismatch");
        }
    }

    #[test]
    fn nop_is_addi_zero() {
        assert_eq!(
            Instruction::nop(),
            Instruction::Addi { rd: r(0), rs1: r(0), imm: 0 }
        );
    }

    #[test]
    fn privileged_instructions_are_flagged() {
        assert!(Instruction::Halt.is_privileged());
        assert!(Instruction::Sret.is_privileged());
        assert!(Instruction::Csrr { rd: r(1), sysreg: 0 }.is_privileged());
        assert!(!Instruction::Ecall.is_privileged());
        assert!(!Instruction::Add { rd: r(1), rs1: r(2), rs2: r(3) }.is_privileged());
    }

    #[test]
    fn unknown_opcodes_are_illegal() {
        for word in [0x0000_007F, 0x0000_003F, 0x0000_0000] {
            assert!(matches!(
                Instruction::decode(word),
                Err(DecodeError::IllegalOpcode { .. })
            ));
        }
    }

    #[test]
    fn not_with_nonzero_rs2_is_illegal() {
        let valid = Instruction::Not { rd: r(1), rs1: r(2) }.encode();
        let tampered = valid | (1 << 17);
        assert!(matches!(
            Instruction::decode(tampered),
            Err(DecodeError::IllegalOpcode { .. })
        ));
    }

    #[test]
    fn decode_never_panics_and_round_trips_valid_words() {
        let mut state: u32 = 0x1234_5678;
        for _ in 0..200_000 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            if let Ok(inst) = Instruction::decode(state) {
                assert_eq!(inst.encode(), state, "valid word must round-trip");
            }
        }
        for word in 0u32..70_000 {
            let _ = Instruction::decode(word);
        }
    }
}
