//! The architectural state of one virtual core.
//!
//! [`CpuState`] holds the register bank, program counter, status flags,
//! privilege mode, the reserved system registers and the retirement counters.
//! Register `R0` is hardwired to zero: it is enforced exclusively through
//! [`CpuState::read_reg`] / [`CpuState::write_reg`], so the rest of the crate
//! never indexes the bank directly.

#![allow(clippy::module_name_repetitions)]

use core::fmt;

use ferro_isa::{Flags, PrivilegeMode, Register, SysReg};

/// Number of general-purpose registers (`R0..=R31`).
const REG_COUNT: usize = 32;

/// Number of reserved system registers (see [`SysReg`]).
const SYSREG_COUNT: usize = 5;

/// The complete architectural state of a single virtual core.
///
/// Cloneable so the debugger and tests can snapshot it cheaply.
#[derive(Debug, Clone)]
pub struct CpuState {
    regs: [u32; REG_COUNT],
    pc: u32,
    flags: Flags,
    privilege: PrivilegeMode,
    sysregs: [u32; SYSREG_COUNT],
    cycle_count: u64,
    instret: u64,
    halted: bool,
}

impl CpuState {
    /// Creates a fresh state already reset to `reset_vector`.
    #[must_use]
    pub fn new(reset_vector: u32) -> Self {
        let mut state = Self {
            regs: [0; REG_COUNT],
            pc: 0,
            flags: Flags::empty(),
            privilege: PrivilegeMode::Kernel,
            sysregs: [0; SYSREG_COUNT],
            cycle_count: 0,
            instret: 0,
            halted: false,
        };
        state.reset(reset_vector);
        state
    }

    /// Resets the core: zeroes the registers and counters, points the program
    /// counter at `reset_vector`, clears the flags, enters kernel mode and
    /// clears the halted flag.
    pub fn reset(&mut self, reset_vector: u32) {
        self.regs = [0; REG_COUNT];
        self.pc = reset_vector;
        self.flags = Flags::empty();
        self.privilege = PrivilegeMode::Kernel;
        self.sysregs = [0; SYSREG_COUNT];
        self.cycle_count = 0;
        self.instret = 0;
        self.halted = false;
    }

    /// Reads a general-purpose register. `R0` always reads as zero.
    #[inline]
    #[must_use]
    pub fn read_reg(&self, r: Register) -> u32 {
        self.regs[usize::from(r.index())]
    }

    /// Writes a general-purpose register. Writes to `R0` are discarded.
    #[inline]
    pub fn write_reg(&mut self, r: Register, value: u32) {
        if r.is_zero() {
            return;
        }
        self.regs[usize::from(r.index())] = value;
    }

    /// Reads a reserved system register.
    #[must_use]
    pub fn read_sysreg(&self, reg: SysReg) -> u32 {
        self.sysregs[usize::from(reg.index())]
    }

    /// Writes a reserved system register.
    pub fn write_sysreg(&mut self, reg: SysReg, value: u32) {
        self.sysregs[usize::from(reg.index())] = value;
    }

    /// Returns the program counter.
    #[inline]
    #[must_use]
    pub fn pc(&self) -> u32 {
        self.pc
    }

    /// Sets the program counter.
    #[inline]
    pub fn set_pc(&mut self, pc: u32) {
        self.pc = pc;
    }

    /// Returns the status flags.
    #[must_use]
    pub fn flags(&self) -> Flags {
        self.flags
    }

    /// Returns a mutable handle to the status flags.
    pub fn flags_mut(&mut self) -> &mut Flags {
        &mut self.flags
    }

    /// Returns the current privilege mode.
    #[must_use]
    pub fn privilege(&self) -> PrivilegeMode {
        self.privilege
    }

    /// Sets the privilege mode.
    pub fn set_privilege(&mut self, privilege: PrivilegeMode) {
        self.privilege = privilege;
    }

    /// Returns the number of cycles consumed so far.
    #[must_use]
    pub fn cycle_count(&self) -> u64 {
        self.cycle_count
    }

    /// Returns the number of instructions retired so far.
    #[must_use]
    pub fn instret(&self) -> u64 {
        self.instret
    }

    /// Returns `true` once the core has halted.
    #[inline]
    #[must_use]
    pub fn is_halted(&self) -> bool {
        self.halted
    }

    /// Marks the core as halted.
    pub fn set_halted(&mut self, halted: bool) {
        self.halted = halted;
    }

    /// Advances the program counter to the next sequential instruction
    /// (`pc += 4`, wrapping).
    #[inline]
    pub fn advance_pc(&mut self) {
        self.pc = self.pc.wrapping_add(4);
    }

    /// Adds one to the cycle counter (saturating, so it never wraps or panics).
    #[inline]
    pub fn tick_cycle(&mut self) {
        self.cycle_count = self.cycle_count.saturating_add(1);
    }

    /// Adds one to the retired-instruction counter (saturating).
    #[inline]
    pub fn retire(&mut self) {
        self.instret = self.instret.saturating_add(1);
    }

    /// Produces a side-effect-free snapshot for the debugger / CLI.
    #[must_use]
    pub fn dump(&self) -> CpuDump {
        CpuDump {
            regs: self.regs,
            pc: self.pc,
            flags: self.flags.bits(),
            privilege: self.privilege,
            cycle_count: self.cycle_count,
            instret: self.instret,
            halted: self.halted,
        }
    }
}

impl Default for CpuState {
    fn default() -> Self {
        Self::new(0)
    }
}

/// A read-only, owned snapshot of [`CpuState`] for inspection and logging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuDump {
    /// The 32 general-purpose registers, indexed by register number.
    pub regs: [u32; REG_COUNT],
    /// The program counter.
    pub pc: u32,
    /// The raw status-flag bits.
    pub flags: u32,
    /// The privilege mode.
    pub privilege: PrivilegeMode,
    /// The cycle counter.
    pub cycle_count: u64,
    /// The retired-instruction counter.
    pub instret: u64,
    /// Whether the core has halted.
    pub halted: bool,
}

impl fmt::Display for CpuDump {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "pc={:#010x} flags={:#06x} mode={:?} cycles={} instret={} halted={}",
            self.pc, self.flags, self.privilege, self.cycle_count, self.instret, self.halted
        )?;
        for (index, value) in self.regs.iter().enumerate() {
            let reg = Register::from_bits(index_as_u32(index));
            write!(f, "  r{index:<2} ({:>3}) = {value:#010x}", reg.abi_name())?;
            if index % 2 == 1 {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

/// Converts a small register index (`0..32`) to `u32` without a numeric cast.
fn index_as_u32(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(0)
}
