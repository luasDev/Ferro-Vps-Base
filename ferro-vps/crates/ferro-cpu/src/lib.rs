//! `ferro-cpu`: the virtual processor core of Ferro-VPS.
//!
//! This crate implements the execution engine that drives guest programs. It
//! owns the architectural register file and program counter ([`CpuState`]) and
//! wraps them in a [`Cpu`] that fetches, decodes and executes instructions
//! against any [`ferro_mem::Memory`] implementation.
//!
//! # Execution model
//!
//! [`Cpu`] does not own memory. Every method that touches the guest address
//! space borrows `&mut impl Memory`, and the hot path is generic over that
//! type so the optimiser can specialise it. A single step is the classic
//! three stages:
//!
//! 1. **fetch** four little-endian bytes at `pc`, faulting on a misaligned or
//!    non-fetchable address;
//! 2. **decode** the word into an [`Instruction`](ferro_isa::Instruction),
//!    mapping a decode failure onto an illegal-instruction fault;
//! 3. **execute** the instruction, yielding a [`StepOutcome`].
//!
//! # Program-counter model
//!
//! The *step* owns the program counter, not `execute`. A sequential
//! instruction leaves `pc + 4`; a halt leaves `pc + 4` and marks the core
//! halted; a trap leaves `pc` on the trapping instruction so a future handler
//! can inspect it. The arithmetic and logic unit never sets the status flags
//! in this part.
//!
//! # Implemented vs. deferred
//!
//! This part implements the integer ALU (`ADD`, `SUB`, `ADDI`), the bitwise
//! logic (`AND`, `OR`, `XOR`, their immediates and the `NOT` pseudo-op), the
//! shifts (`SLL`, `SRL`, `SRA` and immediates, honouring the five-bit shift
//! mask), the comparisons (`SLT`, `SLTU`, `SLTI`, `SLTIU`) and the
//! upper-immediate instructions (`LUI`, `AUIPC`) and the memory-access
//! instructions (`LB`, `LBU`, `LH`, `LHU`, `LW`, `SB`, `SH`, `SW`). Branches,
//! jumps, multiply/divide and the system instructions decode correctly but
//! raise a controlled [`TrapKind::Unimplemented`] instead of running, so guest
//! input can never panic the host. They arrive in later parts.

#![forbid(unsafe_code)]
#![deny(warnings)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]

mod cpu;
mod outcome;
mod state;

pub use cpu::Cpu;
pub use outcome::{RunResult, StepOutcome, StopReason, TrapKind};
pub use state::{CpuDump, CpuState};
