//! The execution core: fetch, decode, dispatch and the bounded run loop.
//!
//! [`Cpu`] owns one [`CpuState`] plus a handful of static configuration values.
//! It deliberately does **not** own memory: every method that touches the guest
//! address space borrows `&mut impl Memory`, so the same core can drive a plain
//! physical memory today and a fuller machine later without changing the hot
//! path. The execution model is the classic three-stage sequence:
//!
//! 1. **fetch** the four-byte word at `pc` (little-endian), faulting if `pc` is
//!    misaligned or points at non-fetchable memory;
//! 2. **decode** the word into an [`Instruction`], turning a decode error into
//!    [`GuestFault::IllegalInstruction`];
//! 3. **execute** the instruction, producing a [`StepOutcome`].
//!
//! The program counter is advanced by the *step*, not by `execute`: a normal
//! instruction leaves `pc + 4`, while a trap leaves `pc` on the trapping
//! instruction so a future handler can inspect it.
//!
//! This part implements the integer ALU plus the logic, shift, compare and
//! upper-immediate instructions, and the memory-access instructions: the
//! `LB`/`LBU`/`LH`/`LHU`/`LW` loads and the `SB`/`SH`/`SW` stores. Branches,
//! jumps, multiply/divide and the system instructions still decode
//! successfully but raise a controlled [`TrapKind::Unimplemented`] instead of
//! panicking, so guest code can never crash the host.
//!
//! Memory accesses are delegated to the [`Memory`] layer: the core computes the
//! effective address with wrapping arithmetic and forwards the access, leaving
//! bounds, region and *data* alignment policy to memory. A [`GuestFault`] from
//! an access propagates unchanged, leaving the destination register and memory
//! untouched and the program counter on the faulting instruction. Note the
//! division of responsibility: *instruction* alignment (`pc % 4`) is enforced
//! by the fetch stage, while *data* alignment is the memory layer's policy.

#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]

use ferro_common::log::LogTarget;
use ferro_common::{log_debug, log_trace, GuestFault, VpsConfig};
use ferro_isa::word::{as_signed, as_unsigned};
use ferro_isa::{Flags, Instruction, Register, RESET_VECTOR};
use ferro_mem::{Memory, PhysAddr};

use crate::outcome::{RunResult, StepOutcome, StopReason, TrapKind};
use crate::state::{CpuDump, CpuState};

/// Mask selecting the low five bits of a shift amount (`0..=31`).
const SHIFT_MASK: u32 = 0x1F;

/// Number of bytes occupied by a single instruction word.
const INSTRUCTION_BYTES: u32 = 4;

/// Bit-shift applied to the upper immediate of `LUI` / `AUIPC`.
const UPPER_IMMEDIATE_SHIFT: u32 = 12;

/// A single virtual processor core.
///
/// The core bundles its architectural [`CpuState`] with a few immutable
/// execution parameters captured from the [`VpsConfig`]. It is cheap to
/// construct and holds no references, so a host can own one directly.
#[derive(Debug, Clone)]
pub struct Cpu {
    state: CpuState,
    instruction_budget_per_frame: u64,
    enable_throttle: bool,
    target_clock_hz: u64,
    trace: bool,
}

impl Cpu {
    /// Builds a core from `config`, reset and ready to run from
    /// [`RESET_VECTOR`].
    #[must_use]
    pub fn new(config: &VpsConfig) -> Self {
        Self {
            state: CpuState::new(RESET_VECTOR),
            instruction_budget_per_frame: config.instruction_budget_per_frame(),
            enable_throttle: config.cpu.enable_throttle,
            target_clock_hz: config.cpu.target_clock_hz.as_hz(),
            trace: false,
        }
    }

    /// Resets the core back to its power-on state at [`RESET_VECTOR`].
    pub fn reset(&mut self) {
        self.state.reset(RESET_VECTOR);
    }

    /// Returns the per-frame instruction budget captured from the config.
    #[must_use]
    pub fn instruction_budget_per_frame(&self) -> u64 {
        self.instruction_budget_per_frame
    }

    /// Returns whether the host intends to throttle execution to the target
    /// clock. The core itself never sleeps; this value is advisory.
    #[must_use]
    pub fn throttle_enabled(&self) -> bool {
        self.enable_throttle
    }

    /// Returns the configured target clock, in hertz.
    #[must_use]
    pub fn target_clock_hz(&self) -> u64 {
        self.target_clock_hz
    }

    /// Enables or disables the per-step trace hook. Tracing is off by default
    /// and costs nothing while disabled.
    pub fn set_trace(&mut self, enabled: bool) {
        self.trace = enabled;
    }

    /// Reads a general-purpose register (`R0` always reads as zero).
    #[must_use]
    pub fn get_reg(&self, register: Register) -> u32 {
        self.state.read_reg(register)
    }

    /// Returns the current program counter.
    #[must_use]
    pub fn get_pc(&self) -> u32 {
        self.state.pc()
    }

    /// Returns the current status flags.
    #[must_use]
    pub fn get_flags(&self) -> Flags {
        self.state.flags()
    }

    /// Returns `true` once the core has halted.
    #[must_use]
    pub fn is_halted(&self) -> bool {
        self.state.is_halted()
    }

    /// Returns the number of cycles consumed so far.
    #[must_use]
    pub fn cycle_count(&self) -> u64 {
        self.state.cycle_count()
    }

    /// Returns the number of instructions retired so far.
    #[must_use]
    pub fn instret(&self) -> u64 {
        self.state.instret()
    }

    /// Produces a side-effect-free snapshot of the architectural state.
    #[must_use]
    pub fn dump_state(&self) -> CpuDump {
        self.state.dump()
    }

    /// Borrows the architectural state immutably.
    #[must_use]
    pub fn state(&self) -> &CpuState {
        &self.state
    }

    /// Borrows the architectural state mutably so the host, debugger and tests
    /// can seed registers or the program counter.
    #[must_use]
    pub fn state_mut(&mut self) -> &mut CpuState {
        &mut self.state
    }

    /// Reads a general-purpose register on the hot path.
    #[inline]
    fn r(&self, register: Register) -> u32 {
        self.state.read_reg(register)
    }

    /// Writes a general-purpose register on the hot path (`R0` is discarded).
    #[inline]
    fn set(&mut self, register: Register, value: u32) {
        self.state.write_reg(register, value);
    }

    /// Computes the effective guest address of a load or store.
    ///
    /// The address is `base + sext(imm)` evaluated with wrapping 32-bit
    /// arithmetic: an address overflow simply wraps around the space and is not
    /// an error here. Whether the resulting address is valid — and whether a
    /// *data* access may be unaligned — is decided entirely by the [`Memory`]
    /// layer, never duplicated by the CPU.
    #[inline]
    fn effective_address(&self, base: Register, imm: i32) -> PhysAddr {
        PhysAddr::new(self.r(base).wrapping_add(as_unsigned(imm)))
    }

    /// Fetches the instruction word at the current program counter.
    fn fetch<M: Memory>(&self, mem: &M) -> Result<u32, GuestFault> {
        let pc = self.state.pc();
        if pc % INSTRUCTION_BYTES != 0 {
            return Err(GuestFault::MemoryAccessViolation);
        }
        mem.read_u32(PhysAddr::new(pc))
    }

    /// Decodes a fetched word, mapping any decode failure onto a guest fault.
    fn decode(word: u32) -> Result<Instruction, GuestFault> {
        Instruction::decode(word).map_err(|_| GuestFault::IllegalInstruction)
    }

    /// Executes one decoded instruction.
    ///
    /// The integer, logic, shift, compare, upper-immediate and memory-access
    /// instructions are implemented here; every other valid instruction returns
    /// a controlled [`TrapKind::Unimplemented`] rather than panicking. `execute`
    /// never touches the program counter; the caller advances it.
    ///
    /// # Errors
    ///
    /// Returns the [`GuestFault`] raised by a load or store whose effective
    /// address is unmapped, out of bounds, crosses a region boundary, or targets
    /// read-only or rejecting MMIO memory. On a fault no register is written and
    /// memory is left unchanged.
    #[allow(clippy::too_many_lines)]
    fn execute<M: Memory>(
        &mut self,
        instruction: Instruction,
        mem: &mut M,
    ) -> Result<StepOutcome, GuestFault> {
        match instruction {
            Instruction::Add { rd, rs1, rs2 } => {
                self.set(rd, self.r(rs1).wrapping_add(self.r(rs2)));
                Ok(StepOutcome::Continue)
            }
            Instruction::Sub { rd, rs1, rs2 } => {
                self.set(rd, self.r(rs1).wrapping_sub(self.r(rs2)));
                Ok(StepOutcome::Continue)
            }
            Instruction::And { rd, rs1, rs2 } => {
                self.set(rd, self.r(rs1) & self.r(rs2));
                Ok(StepOutcome::Continue)
            }
            Instruction::Or { rd, rs1, rs2 } => {
                self.set(rd, self.r(rs1) | self.r(rs2));
                Ok(StepOutcome::Continue)
            }
            Instruction::Xor { rd, rs1, rs2 } => {
                self.set(rd, self.r(rs1) ^ self.r(rs2));
                Ok(StepOutcome::Continue)
            }
            Instruction::Not { rd, rs1 } => {
                self.set(rd, !self.r(rs1));
                Ok(StepOutcome::Continue)
            }
            Instruction::Sll { rd, rs1, rs2 } => {
                let shift = self.r(rs2) & SHIFT_MASK;
                self.set(rd, self.r(rs1).wrapping_shl(shift));
                Ok(StepOutcome::Continue)
            }
            Instruction::Srl { rd, rs1, rs2 } => {
                let shift = self.r(rs2) & SHIFT_MASK;
                self.set(rd, self.r(rs1).wrapping_shr(shift));
                Ok(StepOutcome::Continue)
            }
            Instruction::Sra { rd, rs1, rs2 } => {
                let shift = self.r(rs2) & SHIFT_MASK;
                self.set(rd, as_unsigned(as_signed(self.r(rs1)).wrapping_shr(shift)));
                Ok(StepOutcome::Continue)
            }
            Instruction::Slt { rd, rs1, rs2 } => {
                self.set(rd, u32::from(as_signed(self.r(rs1)) < as_signed(self.r(rs2))));
                Ok(StepOutcome::Continue)
            }
            Instruction::Sltu { rd, rs1, rs2 } => {
                self.set(rd, u32::from(self.r(rs1) < self.r(rs2)));
                Ok(StepOutcome::Continue)
            }
            Instruction::Addi { rd, rs1, imm } => {
                self.set(rd, self.r(rs1).wrapping_add(as_unsigned(imm)));
                Ok(StepOutcome::Continue)
            }
            Instruction::Andi { rd, rs1, imm } => {
                self.set(rd, self.r(rs1) & as_unsigned(imm));
                Ok(StepOutcome::Continue)
            }
            Instruction::Ori { rd, rs1, imm } => {
                self.set(rd, self.r(rs1) | as_unsigned(imm));
                Ok(StepOutcome::Continue)
            }
            Instruction::Xori { rd, rs1, imm } => {
                self.set(rd, self.r(rs1) ^ as_unsigned(imm));
                Ok(StepOutcome::Continue)
            }
            Instruction::Slti { rd, rs1, imm } => {
                self.set(rd, u32::from(as_signed(self.r(rs1)) < imm));
                Ok(StepOutcome::Continue)
            }
            Instruction::Sltiu { rd, rs1, imm } => {
                self.set(rd, u32::from(self.r(rs1) < as_unsigned(imm)));
                Ok(StepOutcome::Continue)
            }
            Instruction::Slli { rd, rs1, imm } => {
                let shift = as_unsigned(imm) & SHIFT_MASK;
                self.set(rd, self.r(rs1).wrapping_shl(shift));
                Ok(StepOutcome::Continue)
            }
            Instruction::Srli { rd, rs1, imm } => {
                let shift = as_unsigned(imm) & SHIFT_MASK;
                self.set(rd, self.r(rs1).wrapping_shr(shift));
                Ok(StepOutcome::Continue)
            }
            Instruction::Srai { rd, rs1, imm } => {
                let shift = as_unsigned(imm) & SHIFT_MASK;
                self.set(rd, as_unsigned(as_signed(self.r(rs1)).wrapping_shr(shift)));
                Ok(StepOutcome::Continue)
            }
            Instruction::Lui { rd, imm } => {
                self.set(rd, as_unsigned(imm).wrapping_shl(UPPER_IMMEDIATE_SHIFT));
                Ok(StepOutcome::Continue)
            }
            Instruction::Auipc { rd, imm } => {
                let offset = as_unsigned(imm).wrapping_shl(UPPER_IMMEDIATE_SHIFT);
                let value = self.state.pc().wrapping_add(offset);
                self.set(rd, value);
                Ok(StepOutcome::Continue)
            }
            Instruction::Lw { rd, rs1, imm } => {
                let value = mem.read_u32(self.effective_address(rs1, imm))?;
                self.set(rd, value);
                Ok(StepOutcome::Continue)
            }
            Instruction::Lh { rd, rs1, imm } => {
                let value = mem.read_u16(self.effective_address(rs1, imm))?;
                self.set(rd, sign_extend_16(value));
                Ok(StepOutcome::Continue)
            }
            Instruction::Lhu { rd, rs1, imm } => {
                let value = mem.read_u16(self.effective_address(rs1, imm))?;
                self.set(rd, u32::from(value));
                Ok(StepOutcome::Continue)
            }
            Instruction::Lb { rd, rs1, imm } => {
                let value = mem.read_u8(self.effective_address(rs1, imm))?;
                self.set(rd, sign_extend_8(value));
                Ok(StepOutcome::Continue)
            }
            Instruction::Lbu { rd, rs1, imm } => {
                let value = mem.read_u8(self.effective_address(rs1, imm))?;
                self.set(rd, u32::from(value));
                Ok(StepOutcome::Continue)
            }
            Instruction::Sw { rs2, rs1, imm } => {
                let address = self.effective_address(rs1, imm);
                mem.write_u32(address, self.r(rs2))?;
                Ok(StepOutcome::Continue)
            }
            Instruction::Sh { rs2, rs1, imm } => {
                let address = self.effective_address(rs1, imm);
                mem.write_u16(address, truncate_u16(self.r(rs2)))?;
                Ok(StepOutcome::Continue)
            }
            Instruction::Sb { rs2, rs1, imm } => {
                let address = self.effective_address(rs1, imm);
                mem.write_u8(address, truncate_u8(self.r(rs2)))?;
                Ok(StepOutcome::Continue)
            }
            other => {
                let word = other.encode();
                log_debug!(
                    LogTarget::Cpu,
                    "instruction not implemented in this part: {word:#010x} ({other:?})"
                );
                Ok(StepOutcome::Trap(TrapKind::Unimplemented { word }))
            }
        }
    }

    /// Runs a single fetch / decode / execute step.
    ///
    /// # Errors
    ///
    /// Returns the [`GuestFault`] raised while fetching (a misaligned or
    /// non-fetchable `pc`) or decoding (an illegal instruction). A halted core
    /// returns [`StepOutcome::Halted`] without touching memory.
    pub fn step<M: Memory>(&mut self, mem: &mut M) -> Result<StepOutcome, GuestFault> {
        if self.state.is_halted() {
            return Ok(StepOutcome::Halted);
        }
        let word = self.fetch(mem)?;
        let instruction = Self::decode(word)?;
        if self.trace {
            log_trace!(LogTarget::Cpu, "pc={:#010x} {instruction:?}", self.state.pc());
        }
        let outcome = self.execute(instruction, mem)?;
        self.state.tick_cycle();
        match outcome {
            StepOutcome::Continue => {
                self.state.retire();
                self.state.advance_pc();
            }
            StepOutcome::Halted => {
                self.state.retire();
                self.state.set_halted(true);
            }
            StepOutcome::Trap(_) => {}
        }
        Ok(outcome)
    }

    /// Runs instructions until the core halts, traps, faults or
    /// `max_instructions` have retired, whichever comes first.
    ///
    /// This is the denial-of-service ceiling for guest code: the loop always
    /// terminates after at most `max_instructions` successful steps.
    #[must_use]
    pub fn run_budget<M: Memory>(&mut self, mem: &mut M, max_instructions: u64) -> RunResult {
        let mut retired: u64 = 0;
        loop {
            if self.state.is_halted() {
                return RunResult {
                    instructions_retired: retired,
                    stop: StopReason::Halted,
                };
            }
            if retired >= max_instructions {
                return RunResult {
                    instructions_retired: retired,
                    stop: StopReason::BudgetExhausted,
                };
            }
            match self.step(mem) {
                Ok(StepOutcome::Continue) => retired = retired.saturating_add(1),
                Ok(StepOutcome::Halted) => {
                    return RunResult {
                        instructions_retired: retired,
                        stop: StopReason::Halted,
                    };
                }
                Ok(StepOutcome::Trap(kind)) => {
                    return RunResult {
                        instructions_retired: retired,
                        stop: StopReason::Trap(kind),
                    };
                }
                Err(fault) => {
                    return RunResult {
                        instructions_retired: retired,
                        stop: StopReason::Faulted(fault),
                    };
                }
            }
        }
    }
}

/// Sign-extends the low 8 bits of a byte load to a full 32-bit word.
///
/// The byte is reinterpreted as `i8`, widened to `i32` (the native sign
/// extension), then reinterpreted as `u32`. This compiles to a single
/// sign-extension instruction and never uses `unsafe`.
#[inline]
fn sign_extend_8(value: u8) -> u32 {
    as_unsigned(i32::from(i8::from_ne_bytes([value])))
}

/// Sign-extends the low 16 bits of a half-word load to a full 32-bit word.
#[inline]
fn sign_extend_16(value: u16) -> u32 {
    as_unsigned(i32::from(i16::from_ne_bytes(value.to_ne_bytes())))
}

/// Truncates a register value to its low 8 bits for a byte store.
#[inline]
fn truncate_u8(value: u32) -> u8 {
    value.to_le_bytes()[0]
}

/// Truncates a register value to its low 16 bits for a half-word store.
#[inline]
fn truncate_u16(value: u32) -> u16 {
    let bytes = value.to_le_bytes();
    u16::from_le_bytes([bytes[0], bytes[1]])
}

#[cfg(test)]
mod tests {
    use super::{Cpu, sign_extend_16, sign_extend_8, truncate_u16, truncate_u8};
    use crate::outcome::{StepOutcome, StopReason, TrapKind};
    use ferro_common::{GuestFault, VpsConfig};
    use ferro_isa::{Instruction, RAM_BASE, Register, RESET_VECTOR, ROM_BASE};
    use ferro_mem::{Memory, NullMmioBus, PhysAddr, PhysMemory};

    fn ram_addr(offset: u32) -> PhysAddr {
        PhysAddr::new(RAM_BASE + offset)
    }

    fn imm15(seed: u32) -> i32 {
        let raw = seed & 0x7FFF;
        if raw & 0x4000 == 0 {
            i32::try_from(raw).unwrap_or(0)
        } else {
            i32::try_from(raw).unwrap_or(0) - 0x8000
        }
    }

    fn lb(rd: u8, rs1: u8, imm: i32) -> Instruction {
        Instruction::Lb {
            rd: reg(rd),
            rs1: reg(rs1),
            imm,
        }
    }

    fn lbu(rd: u8, rs1: u8, imm: i32) -> Instruction {
        Instruction::Lbu {
            rd: reg(rd),
            rs1: reg(rs1),
            imm,
        }
    }

    fn lh(rd: u8, rs1: u8, imm: i32) -> Instruction {
        Instruction::Lh {
            rd: reg(rd),
            rs1: reg(rs1),
            imm,
        }
    }

    fn lhu(rd: u8, rs1: u8, imm: i32) -> Instruction {
        Instruction::Lhu {
            rd: reg(rd),
            rs1: reg(rs1),
            imm,
        }
    }

    fn lw(rd: u8, rs1: u8, imm: i32) -> Instruction {
        Instruction::Lw {
            rd: reg(rd),
            rs1: reg(rs1),
            imm,
        }
    }

    fn sb(rs2: u8, rs1: u8, imm: i32) -> Instruction {
        Instruction::Sb {
            rs2: reg(rs2),
            rs1: reg(rs1),
            imm,
        }
    }

    fn sh(rs2: u8, rs1: u8, imm: i32) -> Instruction {
        Instruction::Sh {
            rs2: reg(rs2),
            rs1: reg(rs1),
            imm,
        }
    }

    fn sw(rs2: u8, rs1: u8, imm: i32) -> Instruction {
        Instruction::Sw {
            rs2: reg(rs2),
            rs1: reg(rs1),
            imm,
        }
    }

    fn reg(index: u8) -> Register {
        Register::new(index).expect("valid register index")
    }

    fn machine() -> (Cpu, PhysMemory<NullMmioBus>) {
        let config = VpsConfig::default();
        let mem = PhysMemory::new(&config, NullMmioBus).expect("physical memory");
        let cpu = Cpu::new(&config);
        (cpu, mem)
    }

    fn load_rom(mem: &mut PhysMemory<NullMmioBus>, program: &[Instruction]) {
        let mut bytes = Vec::new();
        for instruction in program {
            bytes.extend_from_slice(&instruction.encode().to_le_bytes());
        }
        mem.load_rom(&bytes).expect("load rom");
    }

    #[test]
    fn reset_initializes_state() {
        let (cpu, _mem) = machine();
        assert_eq!(cpu.get_pc(), RESET_VECTOR);
        assert!(!cpu.is_halted());
        assert_eq!(cpu.cycle_count(), 0);
        assert_eq!(cpu.instret(), 0);
        for index in 0..32u8 {
            assert_eq!(cpu.get_reg(reg(index)), 0);
        }
    }

    #[test]
    fn r0_is_immutable() {
        let (mut cpu, mut mem) = machine();
        load_rom(
            &mut mem,
            &[Instruction::Addi {
                rd: reg(0),
                rs1: reg(0),
                imm: 42,
            }],
        );
        cpu.state_mut().write_reg(reg(0), 99);
        assert_eq!(cpu.get_reg(reg(0)), 0);
        let outcome = cpu.step(&mut mem).expect("step");
        assert_eq!(outcome, StepOutcome::Continue);
        assert_eq!(cpu.get_reg(reg(0)), 0);
    }

    #[test]
    fn add_sub_and_addi_wrap() {
        let (mut cpu, mut mem) = machine();
        load_rom(
            &mut mem,
            &[
                Instruction::Add {
                    rd: reg(3),
                    rs1: reg(1),
                    rs2: reg(2),
                },
                Instruction::Sub {
                    rd: reg(4),
                    rs1: reg(1),
                    rs2: reg(2),
                },
                Instruction::Addi {
                    rd: reg(5),
                    rs1: reg(1),
                    imm: -1,
                },
            ],
        );
        cpu.state_mut().write_reg(reg(1), 10);
        cpu.state_mut().write_reg(reg(2), 25);
        cpu.step(&mut mem).expect("add");
        assert_eq!(cpu.get_reg(reg(3)), 35);
        cpu.step(&mut mem).expect("sub");
        assert_eq!(cpu.get_reg(reg(4)), 10u32.wrapping_sub(25));
        cpu.step(&mut mem).expect("addi");
        assert_eq!(cpu.get_reg(reg(5)), 9);
    }

    #[test]
    fn add_wraps_on_overflow() {
        let (mut cpu, mut mem) = machine();
        load_rom(
            &mut mem,
            &[Instruction::Add {
                rd: reg(3),
                rs1: reg(1),
                rs2: reg(2),
            }],
        );
        cpu.state_mut().write_reg(reg(1), u32::MAX);
        cpu.state_mut().write_reg(reg(2), 1);
        cpu.step(&mut mem).expect("add");
        assert_eq!(cpu.get_reg(reg(3)), 0);
    }

    #[test]
    fn bitwise_register_and_immediate() {
        let (mut cpu, mut mem) = machine();
        load_rom(
            &mut mem,
            &[
                Instruction::And {
                    rd: reg(3),
                    rs1: reg(1),
                    rs2: reg(2),
                },
                Instruction::Or {
                    rd: reg(4),
                    rs1: reg(1),
                    rs2: reg(2),
                },
                Instruction::Xor {
                    rd: reg(5),
                    rs1: reg(1),
                    rs2: reg(2),
                },
                Instruction::Andi {
                    rd: reg(6),
                    rs1: reg(1),
                    imm: 0x0F,
                },
                Instruction::Ori {
                    rd: reg(7),
                    rs1: reg(1),
                    imm: 0x0F,
                },
                Instruction::Xori {
                    rd: reg(8),
                    rs1: reg(1),
                    imm: -1,
                },
            ],
        );
        cpu.state_mut().write_reg(reg(1), 0xF0F0_F0F0);
        cpu.state_mut().write_reg(reg(2), 0x0FF0_0FF0);
        cpu.step(&mut mem).expect("and");
        assert_eq!(cpu.get_reg(reg(3)), 0xF0F0_F0F0 & 0x0FF0_0FF0);
        cpu.step(&mut mem).expect("or");
        assert_eq!(cpu.get_reg(reg(4)), 0xF0F0_F0F0 | 0x0FF0_0FF0);
        cpu.step(&mut mem).expect("xor");
        assert_eq!(cpu.get_reg(reg(5)), 0xF0F0_F0F0 ^ 0x0FF0_0FF0);
        cpu.step(&mut mem).expect("andi");
        assert_eq!(cpu.get_reg(reg(6)), 0xF0F0_F0F0 & 0x0F);
        cpu.step(&mut mem).expect("ori");
        assert_eq!(cpu.get_reg(reg(7)), 0xF0F0_F0F0 | 0x0F);
        cpu.step(&mut mem).expect("xori");
        assert_eq!(cpu.get_reg(reg(8)), 0xF0F0_F0F0 ^ 0xFFFF_FFFF);
    }

    #[test]
    fn shifts_mask_amount_and_preserve_sign() {
        let (mut cpu, mut mem) = machine();
        load_rom(
            &mut mem,
            &[
                Instruction::Sll {
                    rd: reg(3),
                    rs1: reg(1),
                    rs2: reg(2),
                },
                Instruction::Srl {
                    rd: reg(4),
                    rs1: reg(5),
                    rs2: reg(2),
                },
                Instruction::Sra {
                    rd: reg(6),
                    rs1: reg(5),
                    rs2: reg(2),
                },
                Instruction::Slli {
                    rd: reg(7),
                    rs1: reg(1),
                    imm: 33,
                },
                Instruction::Srli {
                    rd: reg(8),
                    rs1: reg(5),
                    imm: 33,
                },
                Instruction::Srai {
                    rd: reg(9),
                    rs1: reg(5),
                    imm: 33,
                },
            ],
        );
        cpu.state_mut().write_reg(reg(1), 1);
        cpu.state_mut().write_reg(reg(2), 33);
        cpu.state_mut().write_reg(reg(5), 0x8000_0000);
        cpu.step(&mut mem).expect("sll");
        assert_eq!(cpu.get_reg(reg(3)), 2);
        cpu.step(&mut mem).expect("srl");
        assert_eq!(cpu.get_reg(reg(4)), 0x4000_0000);
        cpu.step(&mut mem).expect("sra");
        assert_eq!(cpu.get_reg(reg(6)), 0xC000_0000);
        cpu.step(&mut mem).expect("slli");
        assert_eq!(cpu.get_reg(reg(7)), 2);
        cpu.step(&mut mem).expect("srli");
        assert_eq!(cpu.get_reg(reg(8)), 0x4000_0000);
        cpu.step(&mut mem).expect("srai");
        assert_eq!(cpu.get_reg(reg(9)), 0xC000_0000);
    }

    #[test]
    fn set_less_than_respects_signedness() {
        let (mut cpu, mut mem) = machine();
        load_rom(
            &mut mem,
            &[
                Instruction::Slt {
                    rd: reg(3),
                    rs1: reg(1),
                    rs2: reg(2),
                },
                Instruction::Sltu {
                    rd: reg(4),
                    rs1: reg(1),
                    rs2: reg(2),
                },
                Instruction::Slti {
                    rd: reg(5),
                    rs1: reg(1),
                    imm: 1,
                },
                Instruction::Sltiu {
                    rd: reg(6),
                    rs1: reg(1),
                    imm: 1,
                },
            ],
        );
        cpu.state_mut().write_reg(reg(1), 0xFFFF_FFFF);
        cpu.state_mut().write_reg(reg(2), 1);
        cpu.step(&mut mem).expect("slt");
        assert_eq!(cpu.get_reg(reg(3)), 1);
        cpu.step(&mut mem).expect("sltu");
        assert_eq!(cpu.get_reg(reg(4)), 0);
        cpu.step(&mut mem).expect("slti");
        assert_eq!(cpu.get_reg(reg(5)), 1);
        cpu.step(&mut mem).expect("sltiu");
        assert_eq!(cpu.get_reg(reg(6)), 0);
    }

    #[test]
    fn lui_and_auipc_build_upper_immediates() {
        let (mut cpu, mut mem) = machine();
        load_rom(
            &mut mem,
            &[
                Instruction::Lui {
                    rd: reg(1),
                    imm: 0x1_2345,
                },
                Instruction::Auipc {
                    rd: reg(2),
                    imm: 1,
                },
            ],
        );
        cpu.step(&mut mem).expect("lui");
        assert_eq!(cpu.get_reg(reg(1)), 0x1234_5000);
        let pc_before = cpu.get_pc();
        cpu.step(&mut mem).expect("auipc");
        assert_eq!(cpu.get_reg(reg(2)), pc_before.wrapping_add(0x1000));
    }

    #[test]
    fn step_advances_pc_and_counters() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[Instruction::nop()]);
        let pc_before = cpu.get_pc();
        let outcome = cpu.step(&mut mem).expect("nop");
        assert_eq!(outcome, StepOutcome::Continue);
        assert_eq!(cpu.get_pc(), pc_before.wrapping_add(4));
        assert_eq!(cpu.cycle_count(), 1);
        assert_eq!(cpu.instret(), 1);
    }

    #[test]
    fn misaligned_fetch_faults() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[Instruction::nop()]);
        cpu.state_mut().set_pc(RESET_VECTOR + 1);
        assert!(matches!(
            cpu.step(&mut mem),
            Err(GuestFault::MemoryAccessViolation)
        ));
    }

    #[test]
    fn non_executable_fetch_faults() {
        let (mut cpu, mut mem) = machine();
        cpu.state_mut().set_pc(0xF000_0000);
        assert!(cpu.step(&mut mem).is_err());
    }

    #[test]
    fn unimplemented_instruction_is_controlled_trap() {
        let (mut cpu, mut mem) = machine();
        load_rom(
            &mut mem,
            &[Instruction::Jal {
                rd: reg(0),
                imm: 0,
            }],
        );
        let pc_before = cpu.get_pc();
        let outcome = cpu.step(&mut mem).expect("trap is not a host fault");
        assert!(matches!(
            outcome,
            StepOutcome::Trap(TrapKind::Unimplemented { .. })
        ));
        assert_eq!(cpu.get_pc(), pc_before);
        assert_eq!(cpu.instret(), 0);
        assert_eq!(cpu.cycle_count(), 1);
    }

    #[test]
    fn run_budget_reports_budget() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[Instruction::nop(); 4]);
        let result = cpu.run_budget(&mut mem, 2);
        assert_eq!(result.instructions_retired, 2);
        assert!(matches!(result.stop, StopReason::BudgetExhausted));
    }

    #[test]
    fn run_budget_reports_fault() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[Instruction::nop(), Instruction::nop()]);
        let result = cpu.run_budget(&mut mem, 100);
        assert_eq!(result.instructions_retired, 2);
        assert!(matches!(
            result.stop,
            StopReason::Faulted(GuestFault::IllegalInstruction)
        ));
    }

    #[test]
    fn run_budget_reports_trap() {
        let (mut cpu, mut mem) = machine();
        load_rom(
            &mut mem,
            &[
                Instruction::nop(),
                Instruction::Jal {
                    rd: reg(0),
                    imm: 0,
                },
            ],
        );
        let result = cpu.run_budget(&mut mem, 100);
        assert_eq!(result.instructions_retired, 1);
        assert!(matches!(
            result.stop,
            StopReason::Trap(TrapKind::Unimplemented { .. })
        ));
    }

    #[test]
    fn invalid_word_is_illegal_instruction() {
        let (mut cpu, mut mem) = machine();
        mem.load_rom(&0xFFFF_FFFFu32.to_le_bytes())
            .expect("load rom");
        assert!(matches!(
            cpu.step(&mut mem),
            Err(GuestFault::IllegalInstruction)
        ));
    }

    #[test]
    fn dump_state_reflects_registers_and_pc() {
        let (mut cpu, _mem) = machine();
        cpu.state_mut().write_reg(reg(1), 0xABCD);
        let dump = cpu.dump_state();
        assert_eq!(dump.pc, RESET_VECTOR);
        assert_eq!(dump.regs[1], 0xABCD);
        assert!(!dump.halted);
    }

    #[test]
    fn captures_configuration() {
        let config = VpsConfig::default();
        let cpu = Cpu::new(&config);
        assert_eq!(cpu.target_clock_hz(), config.cpu.target_clock_hz.as_hz());
        assert_eq!(cpu.throttle_enabled(), config.cpu.enable_throttle);
        assert_eq!(
            cpu.instruction_budget_per_frame(),
            config.instruction_budget_per_frame()
        );
    }

    #[test]
    fn store_then_load_word_round_trips() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[sw(2, 1, 0), lw(3, 1, 0)]);
        cpu.state_mut().write_reg(reg(1), RAM_BASE + 0x40);
        cpu.state_mut().write_reg(reg(2), 0x1122_3344);
        cpu.step(&mut mem).expect("sw");
        assert_eq!(mem.read_u8(ram_addr(0x40)).expect("byte 0"), 0x44);
        assert_eq!(mem.read_u8(ram_addr(0x41)).expect("byte 1"), 0x33);
        assert_eq!(mem.read_u8(ram_addr(0x42)).expect("byte 2"), 0x22);
        assert_eq!(mem.read_u8(ram_addr(0x43)).expect("byte 3"), 0x11);
        cpu.step(&mut mem).expect("lw");
        assert_eq!(cpu.get_reg(reg(3)), 0x1122_3344);
    }

    #[test]
    fn byte_and_half_stores_are_narrow_and_little_endian() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[sb(2, 1, 0), sh(3, 1, 4)]);
        mem.load_into_ram(ram_addr(0), &[0xFF; 8]).expect("seed ram");
        cpu.state_mut().write_reg(reg(1), RAM_BASE);
        cpu.state_mut().write_reg(reg(2), 0xAABB_CCDD);
        cpu.state_mut().write_reg(reg(3), 0x1234_5678);
        cpu.step(&mut mem).expect("sb");
        assert_eq!(mem.read_u8(ram_addr(0)).expect("sb byte"), 0xDD);
        assert_eq!(mem.read_u8(ram_addr(1)).expect("adjacent"), 0xFF);
        cpu.step(&mut mem).expect("sh");
        assert_eq!(mem.read_u8(ram_addr(4)).expect("sh low"), 0x78);
        assert_eq!(mem.read_u8(ram_addr(5)).expect("sh high"), 0x56);
        assert_eq!(mem.read_u8(ram_addr(6)).expect("adjacent"), 0xFF);
    }

    #[test]
    fn loads_sign_and_zero_extend() {
        let (mut cpu, mut mem) = machine();
        load_rom(
            &mut mem,
            &[lb(2, 1, 0), lbu(3, 1, 0), lh(4, 1, 2), lhu(5, 1, 2)],
        );
        mem.load_into_ram(ram_addr(0), &[0x80, 0x00, 0x00, 0x80])
            .expect("seed ram");
        cpu.state_mut().write_reg(reg(1), RAM_BASE);
        cpu.step(&mut mem).expect("lb");
        assert_eq!(cpu.get_reg(reg(2)), 0xFFFF_FF80);
        cpu.step(&mut mem).expect("lbu");
        assert_eq!(cpu.get_reg(reg(3)), 0x0000_0080);
        cpu.step(&mut mem).expect("lh");
        assert_eq!(cpu.get_reg(reg(4)), 0xFFFF_8000);
        cpu.step(&mut mem).expect("lhu");
        assert_eq!(cpu.get_reg(reg(5)), 0x0000_8000);
    }

    #[test]
    fn effective_address_uses_signed_immediate() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[lw(2, 1, 8), lw(3, 1, -8)]);
        mem.load_into_ram(ram_addr(0x100 + 8), &0xDEAD_BEEF_u32.to_le_bytes())
            .expect("seed high");
        mem.load_into_ram(ram_addr(0x100 - 8), &0x0BAD_F00D_u32.to_le_bytes())
            .expect("seed low");
        cpu.state_mut().write_reg(reg(1), RAM_BASE + 0x100);
        cpu.step(&mut mem).expect("lw +8");
        assert_eq!(cpu.get_reg(reg(2)), 0xDEAD_BEEF);
        cpu.step(&mut mem).expect("lw -8");
        assert_eq!(cpu.get_reg(reg(3)), 0x0BAD_F00D);
    }

    #[test]
    fn guest_store_to_rom_faults() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[sw(2, 1, 0)]);
        cpu.state_mut().write_reg(reg(1), ROM_BASE);
        cpu.state_mut().write_reg(reg(2), 0xDEAD_BEEF);
        assert!(matches!(
            cpu.step(&mut mem),
            Err(GuestFault::MemoryAccessViolation)
        ));
    }

    #[test]
    fn store_to_unmapped_faults() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[sw(2, 1, 0)]);
        cpu.state_mut().write_reg(reg(1), 0x8000_0000);
        cpu.state_mut().write_reg(reg(2), 1);
        assert!(matches!(
            cpu.step(&mut mem),
            Err(GuestFault::MemoryAccessViolation)
        ));
    }

    #[test]
    fn load_from_unmapped_faults() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[lw(2, 1, 0)]);
        cpu.state_mut().write_reg(reg(1), 0x8000_0000);
        assert!(matches!(
            cpu.step(&mut mem),
            Err(GuestFault::MemoryAccessViolation)
        ));
    }

    #[test]
    fn unaligned_ram_access_is_allowed() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[sw(2, 1, 1), lw(3, 1, 1)]);
        cpu.state_mut().write_reg(reg(1), RAM_BASE);
        cpu.state_mut().write_reg(reg(2), 0x0A0B_0C0D);
        cpu.step(&mut mem).expect("unaligned sw");
        cpu.step(&mut mem).expect("unaligned lw");
        assert_eq!(cpu.get_reg(reg(3)), 0x0A0B_0C0D);
    }

    #[test]
    fn word_crossing_ram_end_faults() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[lw(3, 1, 0)]);
        let ram_size = u32::try_from(mem.size()).expect("ram size fits u32");
        cpu.state_mut().write_reg(reg(1), RAM_BASE + ram_size - 2);
        assert!(matches!(
            cpu.step(&mut mem),
            Err(GuestFault::MemoryAccessViolation)
        ));
        assert_eq!(cpu.get_reg(reg(3)), 0);
    }

    #[test]
    fn load_into_r0_executes_but_discards_result() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[lw(0, 1, 0)]);
        mem.load_into_ram(ram_addr(0), &0xDEAD_BEEF_u32.to_le_bytes())
            .expect("seed ram");
        cpu.state_mut().write_reg(reg(1), RAM_BASE);
        let outcome = cpu.step(&mut mem).expect("lw into r0");
        assert_eq!(outcome, StepOutcome::Continue);
        assert_eq!(cpu.get_reg(reg(0)), 0);
        assert_eq!(cpu.instret(), 1);
        assert_eq!(cpu.cycle_count(), 1);
    }

    #[test]
    fn stores_truncate_high_bits() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[sw(2, 1, 0), sh(2, 1, 4), sb(2, 1, 8)]);
        cpu.state_mut().write_reg(reg(1), RAM_BASE);
        cpu.state_mut().write_reg(reg(2), 0xCAFE_BABE);
        cpu.step(&mut mem).expect("sw");
        assert_eq!(mem.read_u32(ram_addr(0)).expect("word"), 0xCAFE_BABE);
        cpu.step(&mut mem).expect("sh");
        assert_eq!(mem.read_u16(ram_addr(4)).expect("half"), 0xBABE);
        cpu.step(&mut mem).expect("sb");
        assert_eq!(mem.read_u8(ram_addr(8)).expect("byte"), 0xBE);
    }

    #[test]
    fn mixed_sequence_preserves_memory_consistency() {
        let (mut cpu, mut mem) = machine();
        load_rom(
            &mut mem,
            &[sw(2, 1, 0), sb(3, 1, 0), lw(4, 1, 0), lbu(5, 1, 3)],
        );
        cpu.state_mut().write_reg(reg(1), RAM_BASE + 0x20);
        cpu.state_mut().write_reg(reg(2), 0x1122_3344);
        cpu.state_mut().write_reg(reg(3), 0x0000_0099);
        for label in ["sw", "sb", "lw", "lbu"] {
            cpu.step(&mut mem).expect(label);
        }
        assert_eq!(cpu.get_reg(reg(4)), 0x1122_3399);
        assert_eq!(cpu.get_reg(reg(5)), 0x11);
    }

    #[test]
    fn faulting_load_leaves_destination_and_counters_untouched() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[lw(3, 1, 0)]);
        cpu.state_mut().write_reg(reg(3), 0x1234_5678);
        cpu.state_mut().write_reg(reg(1), 0x8000_0000);
        let pc_before = cpu.get_pc();
        assert!(matches!(
            cpu.step(&mut mem),
            Err(GuestFault::MemoryAccessViolation)
        ));
        assert_eq!(cpu.get_reg(reg(3)), 0x1234_5678);
        assert_eq!(cpu.get_pc(), pc_before);
        assert_eq!(cpu.instret(), 0);
        assert_eq!(cpu.cycle_count(), 0);
    }

    #[test]
    fn faulting_store_does_not_partially_modify_memory() {
        let (mut cpu, mut mem) = machine();
        load_rom(&mut mem, &[sw(2, 1, 0)]);
        let ram_size = u32::try_from(mem.size()).expect("ram size fits u32");
        mem.load_into_ram(ram_addr(ram_size - 1), &[0x5A])
            .expect("seed last byte");
        cpu.state_mut().write_reg(reg(1), RAM_BASE + ram_size - 2);
        cpu.state_mut().write_reg(reg(2), 0xFFFF_FFFF);
        assert!(matches!(
            cpu.step(&mut mem),
            Err(GuestFault::MemoryAccessViolation)
        ));
        assert_eq!(mem.read_u8(ram_addr(ram_size - 1)).expect("last"), 0x5A);
        assert_eq!(mem.read_u8(ram_addr(ram_size - 2)).expect("second last"), 0);
    }

    #[test]
    fn extension_and_truncation_helpers() {
        assert_eq!(sign_extend_8(0x80), 0xFFFF_FF80);
        assert_eq!(sign_extend_8(0x7F), 0x0000_007F);
        assert_eq!(sign_extend_16(0x8000), 0xFFFF_8000);
        assert_eq!(sign_extend_16(0x7FFF), 0x0000_7FFF);
        assert_eq!(truncate_u8(0xAABB_CCDD), 0xDD);
        assert_eq!(truncate_u16(0xAABB_CCDD), 0xCCDD);
    }

    #[test]
    fn fuzz_memory_instructions_never_panic() {
        let (mut cpu, mut mem) = machine();
        let mut seed: u32 = 0x1357_9BDF;
        let mut roll = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed
        };
        for _ in 0..2_000 {
            let base = roll();
            let imm = imm15(roll());
            let target = u8::try_from(roll() & 0x1F).unwrap_or(0);
            let programs = [
                lb(target, 1, imm),
                lbu(target, 1, imm),
                lh(target, 1, imm),
                lhu(target, 1, imm),
                lw(target, 1, imm),
                sb(target, 1, imm),
                sh(target, 1, imm),
                sw(target, 1, imm),
            ];
            for program in programs {
                cpu.reset();
                cpu.state_mut().write_reg(reg(1), base);
                cpu.state_mut().write_reg(reg(target), base);
                load_rom(&mut mem, &[program]);
                let outcome = cpu.step(&mut mem);
                assert!(
                    outcome.is_ok() || matches!(outcome, Err(GuestFault::MemoryAccessViolation))
                );
            }
        }
    }
}
