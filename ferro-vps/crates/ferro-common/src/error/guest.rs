//! Guest faults: failures caused by the guest program, not the host.

/// A fault caused by the guest program (a game or server running inside the
/// virtual machine), as opposed to a bug in the host.
///
/// Guest faults are part of normal operation and must never crash the host.
/// Components return a `GuestFault` to the kernel/VM, which decides how to react
/// (terminate the guest, log, ...). A `GuestFault` does not convert into a
/// [`FerroError`](crate::error::FerroError) automatically; it can only be
/// escalated explicitly through
/// [`VmError::GuestFaulted`](crate::error::VmError::GuestFaulted).
#[derive(Debug)]
#[non_exhaustive]
pub enum GuestFault {
    /// The guest executed an opcode that is not valid.
    IllegalInstruction,
    /// The guest performed an integer division by zero.
    DivideByZero,
    /// The guest accessed memory it is not allowed to touch.
    MemoryAccessViolation,
    /// The guest exhausted its stack.
    StackOverflow,
    /// The guest issued an unknown or disallowed system call.
    InvalidSyscall,
}

impl core::fmt::Display for GuestFault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let message = match self {
            Self::IllegalInstruction => "illegal instruction",
            Self::DivideByZero => "divide by zero",
            Self::MemoryAccessViolation => "memory access violation",
            Self::StackOverflow => "stack overflow",
            Self::InvalidSyscall => "invalid system call",
        };
        f.write_str(message)
    }
}

impl std::error::Error for GuestFault {}
