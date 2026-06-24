//! Per-domain error types.
//!
//! Most of these are intentionally minimal stubs in this part: each component's
//! own part replaces the placeholder variant with real, domain-specific
//! variants. [`VmError`] is written out by hand because it already needs to
//! carry an escalated [`GuestFault`].

use super::GuestFault;

macro_rules! stub_domain_error {
    ($(#[$meta:meta])* $name:ident, $message:literal) => {
        $(#[$meta])*
        #[derive(Debug)]
        #[non_exhaustive]
        pub enum $name {
            /// Reserved placeholder variant; real variants are added in the
            /// part that implements this component.
            Unimplemented,
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self {
                    Self::Unimplemented => f.write_str($message),
                }
            }
        }

        impl ::std::error::Error for $name {}
    };
}

/// Errors produced by the virtual processor (`ferro-cpu`).
///
/// This domain is written out by hand (rather than using the stub macro)
/// because the ISA crate (`ferro-isa`) already needs to escalate instruction
/// decoding failures into a host-visible error. Because the enum is
/// `#[non_exhaustive]`, downstream crates cannot build its variants directly;
/// use [`CpuError::decode`] as the public constructor.
#[derive(Debug)]
#[non_exhaustive]
pub enum CpuError {
    /// Reserved placeholder variant; real execution variants are added in the
    /// `ferro-cpu` part.
    Unimplemented,
    /// An instruction word could not be decoded into a valid instruction.
    Decode {
        /// Human-readable explanation of why decoding failed.
        reason: String,
    },
}

impl CpuError {
    /// Builds a [`CpuError::Decode`] from any string-like reason.
    #[must_use]
    pub fn decode(reason: impl Into<String>) -> Self {
        Self::Decode {
            reason: reason.into(),
        }
    }
}

impl ::core::fmt::Display for CpuError {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        match self {
            Self::Unimplemented => f.write_str("cpu subsystem is not yet implemented"),
            Self::Decode { reason } => write!(f, "instruction decode error: {reason}"),
        }
    }
}

impl ::std::error::Error for CpuError {}

stub_domain_error!(
    /// Errors produced by the virtual memory subsystem (`ferro-mem`).
    MemoryError,
    "memory subsystem is not yet implemented"
);

stub_domain_error!(
    /// Errors produced by the device bus (`ferro-bus`).
    BusError,
    "bus subsystem is not yet implemented"
);

stub_domain_error!(
    /// Errors produced by the virtual GPU (`ferro-gpu`).
    GpuError,
    "gpu subsystem is not yet implemented"
);

stub_domain_error!(
    /// Errors produced by the storage subsystem (`ferro-storage`).
    StorageError,
    "storage subsystem is not yet implemented"
);

stub_domain_error!(
    /// Errors produced by the audio subsystem (`ferro-audio`).
    AudioError,
    "audio subsystem is not yet implemented"
);

stub_domain_error!(
    /// Errors produced by the network subsystem (`ferro-net`).
    NetworkError,
    "network subsystem is not yet implemented"
);

stub_domain_error!(
    /// Errors produced by the guest kernel (`ferro-kernel`).
    KernelError,
    "kernel subsystem is not yet implemented"
);

stub_domain_error!(
    /// Errors produced by the assembler (`ferro-asm`).
    AsmError,
    "asm subsystem is not yet implemented"
);

stub_domain_error!(
    /// Errors produced by the host integration layer.
    HostError,
    "host subsystem is not yet implemented"
);

/// Errors produced while assembling or running the virtual machine
/// (`ferro-vm`), including faults escalated from a guest program.
#[derive(Debug)]
#[non_exhaustive]
pub enum VmError {
    /// Reserved placeholder variant; real variants are added in the VM part.
    Unimplemented,
    /// A guest fault that was explicitly escalated into a host-visible error.
    /// See [`GuestFault`] for the containment rules around this.
    GuestFaulted(GuestFault),
}

impl core::fmt::Display for VmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unimplemented => f.write_str("vm subsystem is not yet implemented"),
            Self::GuestFaulted(fault) => write!(f, "guest faulted: {fault}"),
        }
    }
}

impl std::error::Error for VmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::GuestFaulted(fault) => Some(fault),
            Self::Unimplemented => None,
        }
    }
}
