//! The six instruction formats (R, I, S, B, U, J) and their bit-level
//! encode/decode.
//!
//! Every instruction is a single little-endian 32-bit word. The low seven bits
//! `[6:0]` are the primary opcode; the remaining fields depend on the format:
//!
//! - **R** `[6:0] op | [11:7] rd | [16:12] rs1 | [21:17] rs2 | [31:22] funct`
//! - **I** `[6:0] op | [11:7] rd | [16:12] rs1 | [31:17] imm15`
//! - **S** `[6:0] op | [11:7] rs2 | [16:12] rs1 | [31:17] imm15`
//! - **B** `[6:0] op | [11:7] rs1 | [16:12] rs2 | [31:17] imm15`
//! - **U** `[6:0] op | [11:7] rd | [31:12] imm20`
//! - **J** `[6:0] op | [11:7] rd | [31:12] imm20`
//!
//! Decoding a format never fails: every five-bit register field is in range and
//! every immediate field is sign-extended deterministically. Validation of the
//! opcode/funct combination happens one layer up, in
//! [`crate::instruction::Instruction::decode`].

use crate::register::Register;
use crate::word::{as_signed, as_unsigned};

/// Mask for the primary opcode field (bits `[6:0]`).
pub const OPCODE_MASK: u32 = 0x7F;

const RD_SHIFT: u32 = 7;
const RS1_SHIFT: u32 = 12;
const RS2_SHIFT: u32 = 17;
const FUNCT_SHIFT: u32 = 22;
const FUNCT_MASK: u32 = 0x3FF;
const IMM15_SHIFT: u32 = 17;
const IMM15_BITS: u32 = 15;
const IMM15_MASK: u32 = 0x7FFF;
const IMM20_SHIFT: u32 = 12;
const IMM20_BITS: u32 = 20;
const IMM20_MASK: u32 = 0x000F_FFFF;

/// The encoding family an instruction belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Register-register format.
    R,
    /// Immediate format.
    I,
    /// Store format.
    S,
    /// Conditional-branch format.
    B,
    /// Upper-immediate format.
    U,
    /// Jump format.
    J,
}

/// Extracts the primary opcode (bits `[6:0]`) from an instruction word.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn opcode_of(word: u32) -> u8 {
    (word & OPCODE_MASK) as u8
}

/// Extracts the 10-bit funct field (bits `[31:22]`) from an instruction word.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn funct_of(word: u32) -> u16 {
    ((word >> FUNCT_SHIFT) & FUNCT_MASK) as u16
}

/// Extracts the raw 15-bit immediate field (bits `[31:17]`) without sign
/// extension. Used for fields that carry an unsigned index rather than a signed
/// offset (for example a system-register number).
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn imm15_field(word: u32) -> u16 {
    ((word >> IMM15_SHIFT) & IMM15_MASK) as u16
}

const fn sign_extend(value: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    as_signed(value << shift) >> shift
}

const fn reg(word: u32, shift: u32) -> Register {
    Register::from_bits(word >> shift)
}

/// Register-register format: `opcode | rd | rs1 | rs2 | funct`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RType {
    /// Destination register.
    pub rd: Register,
    /// First source register.
    pub rs1: Register,
    /// Second source register.
    pub rs2: Register,
    /// 10-bit function selector distinguishing operations under one opcode.
    pub funct: u16,
}

impl RType {
    /// Decodes the register and funct fields of an `R`-format word.
    #[must_use]
    pub const fn decode(word: u32) -> Self {
        Self {
            rd: reg(word, RD_SHIFT),
            rs1: reg(word, RS1_SHIFT),
            rs2: reg(word, RS2_SHIFT),
            funct: funct_of(word),
        }
    }

    /// Encodes this format into a word together with `opcode`.
    #[must_use]
    pub fn encode(self, opcode: u8) -> u32 {
        (u32::from(opcode) & OPCODE_MASK)
            | (u32::from(self.rd.index()) << RD_SHIFT)
            | (u32::from(self.rs1.index()) << RS1_SHIFT)
            | (u32::from(self.rs2.index()) << RS2_SHIFT)
            | ((u32::from(self.funct) & FUNCT_MASK) << FUNCT_SHIFT)
    }
}

/// Immediate format: `opcode | rd | rs1 | imm15` (sign-extended 15-bit imm).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IType {
    /// Destination register.
    pub rd: Register,
    /// Source / base register.
    pub rs1: Register,
    /// Sign-extended 15-bit immediate (`-16384..=16383`).
    pub imm: i32,
}

impl IType {
    /// Decodes an `I`-format word.
    #[must_use]
    pub const fn decode(word: u32) -> Self {
        Self {
            rd: reg(word, RD_SHIFT),
            rs1: reg(word, RS1_SHIFT),
            imm: sign_extend((word >> IMM15_SHIFT) & IMM15_MASK, IMM15_BITS),
        }
    }

    /// Encodes this format into a word together with `opcode`.
    ///
    /// The immediate is masked to 15 bits; values outside the representable
    /// signed range are truncated, which is a no-op for any in-range value.
    #[must_use]
    pub fn encode(self, opcode: u8) -> u32 {
        (u32::from(opcode) & OPCODE_MASK)
            | (u32::from(self.rd.index()) << RD_SHIFT)
            | (u32::from(self.rs1.index()) << RS1_SHIFT)
            | ((as_unsigned(self.imm) & IMM15_MASK) << IMM15_SHIFT)
    }
}

/// Store format: `opcode | rs2 | rs1 | imm15`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SType {
    /// Value register (the data being stored).
    pub rs2: Register,
    /// Base address register.
    pub rs1: Register,
    /// Sign-extended 15-bit address offset.
    pub imm: i32,
}

impl SType {
    /// Decodes an `S`-format word.
    #[must_use]
    pub const fn decode(word: u32) -> Self {
        Self {
            rs2: reg(word, RD_SHIFT),
            rs1: reg(word, RS1_SHIFT),
            imm: sign_extend((word >> IMM15_SHIFT) & IMM15_MASK, IMM15_BITS),
        }
    }

    /// Encodes this format into a word together with `opcode`.
    #[must_use]
    pub fn encode(self, opcode: u8) -> u32 {
        (u32::from(opcode) & OPCODE_MASK)
            | (u32::from(self.rs2.index()) << RD_SHIFT)
            | (u32::from(self.rs1.index()) << RS1_SHIFT)
            | ((as_unsigned(self.imm) & IMM15_MASK) << IMM15_SHIFT)
    }
}

/// Conditional-branch format: `opcode | rs1 | rs2 | imm15`.
///
/// The immediate is a count of instruction words; the effective byte offset is
/// `imm * 4` relative to the current program counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BType {
    /// First comparison register.
    pub rs1: Register,
    /// Second comparison register.
    pub rs2: Register,
    /// Sign-extended 15-bit branch offset, in units of instruction words.
    pub imm: i32,
}

impl BType {
    /// Decodes a `B`-format word.
    #[must_use]
    pub const fn decode(word: u32) -> Self {
        Self {
            rs1: reg(word, RD_SHIFT),
            rs2: reg(word, RS1_SHIFT),
            imm: sign_extend((word >> IMM15_SHIFT) & IMM15_MASK, IMM15_BITS),
        }
    }

    /// Encodes this format into a word together with `opcode`.
    #[must_use]
    pub fn encode(self, opcode: u8) -> u32 {
        (u32::from(opcode) & OPCODE_MASK)
            | (u32::from(self.rs1.index()) << RD_SHIFT)
            | (u32::from(self.rs2.index()) << RS1_SHIFT)
            | ((as_unsigned(self.imm) & IMM15_MASK) << IMM15_SHIFT)
    }
}

/// Upper-immediate format: `opcode | rd | imm20`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UType {
    /// Destination register.
    pub rd: Register,
    /// Sign-extended 20-bit immediate (`-524288..=524287`).
    pub imm: i32,
}

impl UType {
    /// Decodes a `U`-format word.
    #[must_use]
    pub const fn decode(word: u32) -> Self {
        Self {
            rd: reg(word, RD_SHIFT),
            imm: sign_extend((word >> IMM20_SHIFT) & IMM20_MASK, IMM20_BITS),
        }
    }

    /// Encodes this format into a word together with `opcode`.
    #[must_use]
    pub fn encode(self, opcode: u8) -> u32 {
        (u32::from(opcode) & OPCODE_MASK)
            | (u32::from(self.rd.index()) << RD_SHIFT)
            | ((as_unsigned(self.imm) & IMM20_MASK) << IMM20_SHIFT)
    }
}

/// Jump format: `opcode | rd | imm20`.
///
/// The immediate is a count of instruction words; the effective byte offset is
/// `imm * 4` relative to the current program counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JType {
    /// Link register that receives the return address.
    pub rd: Register,
    /// Sign-extended 20-bit jump offset, in units of instruction words.
    pub imm: i32,
}

impl JType {
    /// Decodes a `J`-format word.
    #[must_use]
    pub const fn decode(word: u32) -> Self {
        Self {
            rd: reg(word, RD_SHIFT),
            imm: sign_extend((word >> IMM20_SHIFT) & IMM20_MASK, IMM20_BITS),
        }
    }

    /// Encodes this format into a word together with `opcode`.
    #[must_use]
    pub fn encode(self, opcode: u8) -> u32 {
        (u32::from(opcode) & OPCODE_MASK)
            | (u32::from(self.rd.index()) << RD_SHIFT)
            | ((as_unsigned(self.imm) & IMM20_MASK) << IMM20_SHIFT)
    }
}

#[cfg(test)]
mod tests {
    use super::{opcode_of, BType, IType, JType, RType, SType, UType};
    use crate::register::Register;

    fn r(index: u8) -> Register {
        Register::new(index).unwrap()
    }

    #[test]
    fn r_type_round_trips() {
        let value = RType {
            rd: r(5),
            rs1: r(10),
            rs2: r(31),
            funct: 0x2AA,
        };
        let word = value.encode(0x33);
        assert_eq!(opcode_of(word), 0x33);
        assert_eq!(RType::decode(word), value);
    }

    #[test]
    fn i_type_sign_extends_immediate() {
        let positive = IType { rd: r(1), rs1: r(2), imm: 16383 };
        assert_eq!(IType::decode(positive.encode(0x02)), positive);
        let negative = IType { rd: r(1), rs1: r(2), imm: -16384 };
        assert_eq!(IType::decode(negative.encode(0x02)), negative);
        let minus_one = IType { rd: r(0), rs1: r(0), imm: -1 };
        assert_eq!(IType::decode(minus_one.encode(0x02)).imm, -1);
    }

    #[test]
    fn s_and_b_place_registers_distinctly() {
        let store = SType { rs2: r(7), rs1: r(8), imm: -4 };
        assert_eq!(SType::decode(store.encode(0x18)), store);
        let branch = BType { rs1: r(7), rs2: r(8), imm: 12 };
        assert_eq!(BType::decode(branch.encode(0x20)), branch);
    }

    #[test]
    fn u_and_j_sign_extend_imm20() {
        let upper = UType { rd: r(3), imm: -524_288 };
        assert_eq!(UType::decode(upper.encode(0x28)), upper);
        let jump = JType { rd: r(4), imm: 524_287 };
        assert_eq!(JType::decode(jump.encode(0x2C)), jump);
    }
}
