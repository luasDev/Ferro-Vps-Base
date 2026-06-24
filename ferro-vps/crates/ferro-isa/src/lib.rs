//! # ferro-isa
//!
//! Pure, dependency-light definition of the Ferro VM instruction-set
//! architecture (ISA). This crate is a *contract*: it describes the registers,
//! the data model, the instruction formats, the opcode table, the flag layout
//! and the physical memory map, plus reversible encode/decode routines. It does
//! **not** contain a CPU executor or any physical memory — those live in
//! `ferro-cpu` and the memory part respectively.
//!
//! ## Fixed architectural decisions
//!
//! - A reduced instruction set (RISC), register-to-register design (loads and
//!   stores are the only memory operations).
//! - 32-bit word size; addresses, registers and immediates all build on
//!   [`Word`](word::Word).
//! - Little-endian byte order ([`ENDIANNESS`](word::ENDIANNESS)).
//! - Fixed 32-bit (4-byte) instruction width, so instruction fetch is trivially
//!   bounded and self-synchronising; see `docs/ARCHITECTURE.md` in the repo.
//!
//! These choices keep decoding `O(1)` and panic-free, which is a security
//! property: a malicious guest image can never make the host decoder loop,
//! allocate, or trap. See the module docs for the precise guarantees.
//!
//! ## Module map
//!
//! - [`word`] data model, endianness, access sizes, little-endian helpers.
//! - [`register`] register file, ABI names, system registers, privilege modes.
//! - [`flags`] the condition-flags register layout.
//! - [`format`] the six fixed-width instruction encodings.
//! - [`opcode`] opcode/funct constants and the canonical instruction table.
//! - [`instruction`] the semantic instruction enum with encode/decode.
//! - [`memory_map`] the guest physical address-space layout.
//! - [`error`] the [`error::DecodeError`] type.

#![forbid(unsafe_code)]
#![deny(warnings)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]

pub mod error;
pub mod flags;
pub mod format;
pub mod instruction;
pub mod memory_map;
pub mod opcode;
pub mod register;
pub mod word;

pub use error::DecodeError;
pub use flags::{Flags, FLAG_C, FLAG_N, FLAG_V, FLAG_Z};
pub use format::{BType, Format, IType, JType, RType, SType, UType};
pub use instruction::Instruction;
pub use memory_map::{
    classify, ram_range, ram_size_bytes, Region, MMIO_BASE, MMIO_SIZE, RAM_BASE, RESERVED_SIZE,
    RESET_VECTOR, ROM_BASE, ROM_SIZE, TRAP_VECTOR_BASE,
};
pub use opcode::{spec_for_encoding, spec_for_mnemonic, OpSpec, TABLE};
pub use register::{PrivilegeMode, Register, SysReg, REGISTER_COUNT};
pub use word::{AccessSize, Endianness, SWord, Word, ENDIANNESS};
