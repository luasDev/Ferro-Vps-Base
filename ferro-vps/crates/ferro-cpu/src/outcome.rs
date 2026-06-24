//! The result types produced by stepping and running the core.
//!
//! A single fetch/decode/execute step yields a [`StepOutcome`]; a bounded run
//! yields a [`RunResult`] describing how many instructions retired and why the
//! loop stopped ([`StopReason`]).

#![allow(clippy::module_name_repetitions)]

use ferro_common::GuestFault;

/// Why a synchronous trap occurred.
///
/// `Syscall` and `Breakpoint` are reserved for the system-instruction part and
/// are not produced yet. `Unimplemented` is raised for a *valid* instruction
/// whose category is not implemented yet (branches, jumps, multiply/divide and
/// the privileged/system instructions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TrapKind {
    /// An `ECALL` system call (implemented in a later part).
    Syscall,
    /// An `EBREAK` debugger trap (implemented in a later part).
    Breakpoint,
    /// A decoded-but-not-yet-implemented instruction word.
    Unimplemented {
        /// The raw instruction word that is not implemented yet.
        word: u32,
    },
}

/// The outcome of a single successful fetch/decode/execute step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StepOutcome {
    /// Sequential execution continues; the program counter advanced by four.
    Continue,
    /// The core executed `HALT` and stopped (reserved for a later part).
    Halted,
    /// Execution reached a synchronous trap; the program counter is left on the
    /// trapping instruction for a future handler.
    Trap(TrapKind),
}

/// Why [`crate::Cpu::run_budget`] stopped executing.
#[derive(Debug)]
#[non_exhaustive]
pub enum StopReason {
    /// The instruction budget for this call was exhausted.
    BudgetExhausted,
    /// The core halted.
    Halted,
    /// Execution hit a synchronous trap.
    Trap(TrapKind),
    /// Execution hit a guest fault (illegal instruction, bad fetch, ...).
    Faulted(GuestFault),
}

/// The result of a bounded run.
#[derive(Debug)]
pub struct RunResult {
    /// How many instructions retired during this call.
    pub instructions_retired: u64,
    /// Why the run loop stopped.
    pub stop: StopReason,
}
