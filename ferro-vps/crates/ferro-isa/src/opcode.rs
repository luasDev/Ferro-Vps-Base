//! Canonical opcode and function-code constants plus the master instruction
//! table shared by the decoder and the assembler.
//!
//! Each distinct instruction that uses an immediate format (I/S/B/U/J) owns a
//! unique primary opcode, because those formats leave no room for a funct
//! field. Register-register instructions share [`OP_ALU_R`] and are
//! distinguished by their 10-bit funct code, and the no-operand system
//! instructions share [`OP_SYSTEM`]. Opcodes not listed here are reserved for
//! future expansion and decode to an error.

use crate::format::Format;

/// Register-register ALU opcode; the funct field selects the operation.
pub const OP_ALU_R: u8 = 0x01;
/// Add immediate.
pub const OP_ADDI: u8 = 0x02;
/// Bitwise-and immediate.
pub const OP_ANDI: u8 = 0x03;
/// Bitwise-or immediate.
pub const OP_ORI: u8 = 0x04;
/// Bitwise-xor immediate.
pub const OP_XORI: u8 = 0x05;
/// Set-less-than immediate (signed).
pub const OP_SLTI: u8 = 0x06;
/// Set-less-than immediate (unsigned).
pub const OP_SLTIU: u8 = 0x07;
/// Shift-left logical immediate.
pub const OP_SLLI: u8 = 0x08;
/// Shift-right logical immediate.
pub const OP_SRLI: u8 = 0x09;
/// Shift-right arithmetic immediate.
pub const OP_SRAI: u8 = 0x0A;
/// Jump-and-link register.
pub const OP_JALR: u8 = 0x0B;
/// Load byte (sign-extended).
pub const OP_LB: u8 = 0x10;
/// Load byte (zero-extended).
pub const OP_LBU: u8 = 0x11;
/// Load half-word (sign-extended).
pub const OP_LH: u8 = 0x12;
/// Load half-word (zero-extended).
pub const OP_LHU: u8 = 0x13;
/// Load word.
pub const OP_LW: u8 = 0x14;
/// Store byte.
pub const OP_SB: u8 = 0x18;
/// Store half-word.
pub const OP_SH: u8 = 0x19;
/// Store word.
pub const OP_SW: u8 = 0x1A;
/// Branch if equal.
pub const OP_BEQ: u8 = 0x20;
/// Branch if not equal.
pub const OP_BNE: u8 = 0x21;
/// Branch if less-than (signed).
pub const OP_BLT: u8 = 0x22;
/// Branch if greater-or-equal (signed).
pub const OP_BGE: u8 = 0x23;
/// Branch if less-than (unsigned).
pub const OP_BLTU: u8 = 0x24;
/// Branch if greater-or-equal (unsigned).
pub const OP_BGEU: u8 = 0x25;
/// Load upper immediate.
pub const OP_LUI: u8 = 0x28;
/// Add upper immediate to PC.
pub const OP_AUIPC: u8 = 0x29;
/// Jump and link.
pub const OP_JAL: u8 = 0x2C;
/// System instructions; the funct field selects the operation.
pub const OP_SYSTEM: u8 = 0x30;
/// Read a system register (privileged).
pub const OP_CSRR: u8 = 0x31;
/// Write a system register (privileged).
pub const OP_CSRW: u8 = 0x32;

/// First opcode in the range reserved for future instructions.
pub const OP_RESERVED_START: u8 = 0x40;

/// Funct code for `ADD` under [`OP_ALU_R`].
pub const FUNCT_ADD: u16 = 0x000;
/// Funct code for `SUB`.
pub const FUNCT_SUB: u16 = 0x001;
/// Funct code for `AND`.
pub const FUNCT_AND: u16 = 0x002;
/// Funct code for `OR`.
pub const FUNCT_OR: u16 = 0x003;
/// Funct code for `XOR`.
pub const FUNCT_XOR: u16 = 0x004;
/// Funct code for `SLL`.
pub const FUNCT_SLL: u16 = 0x005;
/// Funct code for `SRL`.
pub const FUNCT_SRL: u16 = 0x006;
/// Funct code for `SRA`.
pub const FUNCT_SRA: u16 = 0x007;
/// Funct code for `SLT`.
pub const FUNCT_SLT: u16 = 0x008;
/// Funct code for `SLTU`.
pub const FUNCT_SLTU: u16 = 0x009;
/// Funct code for `MUL`.
pub const FUNCT_MUL: u16 = 0x00A;
/// Funct code for `MULH`.
pub const FUNCT_MULH: u16 = 0x00B;
/// Funct code for `DIV`.
pub const FUNCT_DIV: u16 = 0x00C;
/// Funct code for `DIVU`.
pub const FUNCT_DIVU: u16 = 0x00D;
/// Funct code for `REM`.
pub const FUNCT_REM: u16 = 0x00E;
/// Funct code for `REMU`.
pub const FUNCT_REMU: u16 = 0x00F;
/// Funct code for `NOT` (`rd = !rs1`; `rs2` must be zero).
pub const FUNCT_NOT: u16 = 0x010;

/// Funct code for `ECALL` under [`OP_SYSTEM`].
pub const SYS_ECALL: u16 = 0x000;
/// Funct code for `EBREAK`.
pub const SYS_EBREAK: u16 = 0x001;
/// Funct code for `HALT` (privileged).
pub const SYS_HALT: u16 = 0x002;
/// Funct code for `SRET`, return-from-trap (privileged).
pub const SYS_SRET: u16 = 0x003;

/// A row in the canonical instruction table: the static description of one
/// instruction encoding. This is the single source of truth that the assembler
/// reuses to map mnemonics to encodings.
#[derive(Debug, Clone, Copy)]
pub struct OpSpec {
    /// Assembler mnemonic (lowercase).
    pub mnemonic: &'static str,
    /// Primary 7-bit opcode.
    pub opcode: u8,
    /// Encoding format.
    pub format: Format,
    /// Function selector for `R`/system formats, or `None` otherwise.
    pub funct: Option<u16>,
    /// `true` when the instruction may only execute in kernel mode.
    pub privileged: bool,
    /// One-line human description.
    pub summary: &'static str,
}

/// The canonical instruction table.
pub const TABLE: &[OpSpec] = &[
    spec("add", OP_ALU_R, Format::R, Some(FUNCT_ADD), false, "rd = rs1 + rs2 (wrapping)"),
    spec("sub", OP_ALU_R, Format::R, Some(FUNCT_SUB), false, "rd = rs1 - rs2 (wrapping)"),
    spec("and", OP_ALU_R, Format::R, Some(FUNCT_AND), false, "rd = rs1 & rs2"),
    spec("or", OP_ALU_R, Format::R, Some(FUNCT_OR), false, "rd = rs1 | rs2"),
    spec("xor", OP_ALU_R, Format::R, Some(FUNCT_XOR), false, "rd = rs1 ^ rs2"),
    spec("sll", OP_ALU_R, Format::R, Some(FUNCT_SLL), false, "rd = rs1 << (rs2 & 31)"),
    spec("srl", OP_ALU_R, Format::R, Some(FUNCT_SRL), false, "rd = rs1 >> (rs2 & 31) logical"),
    spec("sra", OP_ALU_R, Format::R, Some(FUNCT_SRA), false, "rd = rs1 >> (rs2 & 31) arithmetic"),
    spec("slt", OP_ALU_R, Format::R, Some(FUNCT_SLT), false, "rd = (rs1 < rs2) signed"),
    spec("sltu", OP_ALU_R, Format::R, Some(FUNCT_SLTU), false, "rd = (rs1 < rs2) unsigned"),
    spec("mul", OP_ALU_R, Format::R, Some(FUNCT_MUL), false, "rd = low32(rs1 * rs2)"),
    spec("mulh", OP_ALU_R, Format::R, Some(FUNCT_MULH), false, "rd = high32(rs1 * rs2) signed"),
    spec("div", OP_ALU_R, Format::R, Some(FUNCT_DIV), false, "rd = rs1 / rs2 signed"),
    spec("divu", OP_ALU_R, Format::R, Some(FUNCT_DIVU), false, "rd = rs1 / rs2 unsigned"),
    spec("rem", OP_ALU_R, Format::R, Some(FUNCT_REM), false, "rd = rs1 % rs2 signed"),
    spec("remu", OP_ALU_R, Format::R, Some(FUNCT_REMU), false, "rd = rs1 % rs2 unsigned"),
    spec("not", OP_ALU_R, Format::R, Some(FUNCT_NOT), false, "rd = !rs1"),
    spec("addi", OP_ADDI, Format::I, None, false, "rd = rs1 + sext(imm)"),
    spec("andi", OP_ANDI, Format::I, None, false, "rd = rs1 & sext(imm)"),
    spec("ori", OP_ORI, Format::I, None, false, "rd = rs1 | sext(imm)"),
    spec("xori", OP_XORI, Format::I, None, false, "rd = rs1 ^ sext(imm)"),
    spec("slti", OP_SLTI, Format::I, None, false, "rd = (rs1 < sext(imm)) signed"),
    spec("sltiu", OP_SLTIU, Format::I, None, false, "rd = (rs1 < sext(imm)) unsigned"),
    spec("slli", OP_SLLI, Format::I, None, false, "rd = rs1 << (imm & 31)"),
    spec("srli", OP_SRLI, Format::I, None, false, "rd = rs1 >> (imm & 31) logical"),
    spec("srai", OP_SRAI, Format::I, None, false, "rd = rs1 >> (imm & 31) arithmetic"),
    spec("jalr", OP_JALR, Format::I, None, false, "rd = pc+4; pc = (rs1 + sext(imm)) & !1"),
    spec("lb", OP_LB, Format::I, None, false, "rd = sext8(mem[rs1 + sext(imm)])"),
    spec("lbu", OP_LBU, Format::I, None, false, "rd = zext8(mem[rs1 + sext(imm)])"),
    spec("lh", OP_LH, Format::I, None, false, "rd = sext16(mem[rs1 + sext(imm)])"),
    spec("lhu", OP_LHU, Format::I, None, false, "rd = zext16(mem[rs1 + sext(imm)])"),
    spec("lw", OP_LW, Format::I, None, false, "rd = mem32(rs1 + sext(imm))"),
    spec("sb", OP_SB, Format::S, None, false, "mem8(rs1 + sext(imm)) = rs2"),
    spec("sh", OP_SH, Format::S, None, false, "mem16(rs1 + sext(imm)) = rs2"),
    spec("sw", OP_SW, Format::S, None, false, "mem32(rs1 + sext(imm)) = rs2"),
    spec("beq", OP_BEQ, Format::B, None, false, "if rs1 == rs2 pc += sext(imm)*4"),
    spec("bne", OP_BNE, Format::B, None, false, "if rs1 != rs2 pc += sext(imm)*4"),
    spec("blt", OP_BLT, Format::B, None, false, "if rs1 < rs2 (signed) pc += sext(imm)*4"),
    spec("bge", OP_BGE, Format::B, None, false, "if rs1 >= rs2 (signed) pc += sext(imm)*4"),
    spec("bltu", OP_BLTU, Format::B, None, false, "if rs1 < rs2 (unsigned) pc += sext(imm)*4"),
    spec("bgeu", OP_BGEU, Format::B, None, false, "if rs1 >= rs2 (unsigned) pc += sext(imm)*4"),
    spec("lui", OP_LUI, Format::U, None, false, "rd = imm << 12"),
    spec("auipc", OP_AUIPC, Format::U, None, false, "rd = pc + (imm << 12)"),
    spec("jal", OP_JAL, Format::J, None, false, "rd = pc+4; pc += sext(imm)*4"),
    spec("ecall", OP_SYSTEM, Format::R, Some(SYS_ECALL), false, "trap to kernel (syscall)"),
    spec("ebreak", OP_SYSTEM, Format::R, Some(SYS_EBREAK), false, "trap to debugger"),
    spec("halt", OP_SYSTEM, Format::R, Some(SYS_HALT), true, "stop the processor"),
    spec("sret", OP_SYSTEM, Format::R, Some(SYS_SRET), true, "return from trap"),
    spec("csrr", OP_CSRR, Format::I, None, true, "rd = sysreg[imm]"),
    spec("csrw", OP_CSRW, Format::I, None, true, "sysreg[imm] = rs1"),
];

const fn spec(
    mnemonic: &'static str,
    opcode: u8,
    format: Format,
    funct: Option<u16>,
    privileged: bool,
    summary: &'static str,
) -> OpSpec {
    OpSpec {
        mnemonic,
        opcode,
        format,
        funct,
        privileged,
        summary,
    }
}

/// Looks up the table entry for a mnemonic.
#[must_use]
pub fn spec_for_mnemonic(mnemonic: &str) -> Option<&'static OpSpec> {
    TABLE.iter().find(|entry| entry.mnemonic == mnemonic)
}

/// Looks up the table entry for an `(opcode, funct)` encoding.
#[must_use]
pub fn spec_for_encoding(opcode: u8, funct: Option<u16>) -> Option<&'static OpSpec> {
    TABLE
        .iter()
        .find(|entry| entry.opcode == opcode && entry.funct == funct)
}

#[cfg(test)]
mod tests {
    use super::{spec_for_encoding, spec_for_mnemonic, OP_RESERVED_START, TABLE};
    use std::collections::HashSet;

    #[test]
    fn encodings_are_unique() {
        let mut seen = HashSet::new();
        for entry in TABLE {
            assert!(
                seen.insert((entry.opcode, entry.funct)),
                "duplicate encoding for {}",
                entry.mnemonic
            );
        }
    }

    #[test]
    fn mnemonics_are_unique() {
        let mut seen = HashSet::new();
        for entry in TABLE {
            assert!(seen.insert(entry.mnemonic), "duplicate mnemonic");
        }
    }

    #[test]
    fn opcodes_stay_below_reserved_range() {
        for entry in TABLE {
            assert!(entry.opcode < OP_RESERVED_START);
            assert!(entry.opcode & 0x80 == 0);
        }
    }

    #[test]
    fn lookups_agree_with_table() {
        let add = spec_for_mnemonic("add").unwrap();
        assert_eq!(spec_for_encoding(add.opcode, add.funct).unwrap().mnemonic, "add");
        assert!(spec_for_mnemonic("nope").is_none());
    }
}
