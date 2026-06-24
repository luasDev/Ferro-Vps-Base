//! The memory-mapped I/O bus interface and a null stub implementation.
//!
//! When the physical memory layer classifies an access as [`MemRegion::Mmio`]
//! it delegates to an [`MmioBus`] instead of touching RAM. This part ships only
//! the [`NullMmioBus`] stub, which faults on every access; the real device bus
//! and the concrete devices arrive in later parts.
//!
//! [`MemRegion::Mmio`]: crate::region::MemRegion::Mmio

#![allow(clippy::module_name_repetitions)]

use ferro_common::log::LogTarget;
use ferro_common::{log_debug, GuestFault};
use ferro_isa::AccessSize;

use crate::addr::PhysAddr;

/// A device bus that services memory-mapped I/O accesses.
///
/// Reads and writes take `&self`; concrete devices use interior mutability for
/// their registers. Values are passed and returned zero-extended into a `u32`,
/// matching the physical memory layer's generic access API.
pub trait MmioBus {
    /// Reads `size` bytes from MMIO `addr`, returning a zero-extended `u32`.
    ///
    /// # Errors
    ///
    /// Returns a [`GuestFault`] when no device answers `addr` or the device
    /// rejects the access.
    fn read(&self, addr: PhysAddr, size: AccessSize) -> Result<u32, GuestFault>;

    /// Writes the low `size` bytes of `value` to MMIO `addr`.
    ///
    /// # Errors
    ///
    /// Returns a [`GuestFault`] when no device answers `addr` or the device
    /// rejects the access.
    fn write(&self, addr: PhysAddr, size: AccessSize, value: u32) -> Result<(), GuestFault>;
}

/// An MMIO bus with no devices attached: every access faults with
/// [`GuestFault::MemoryAccessViolation`].
///
/// This is the default bus for [`PhysMemory`](crate::physmem::PhysMemory)
/// until the real device bus exists. Each rejected access is logged at debug
/// level on [`LogTarget::Memory`].
#[derive(Debug, Default, Clone, Copy)]
pub struct NullMmioBus;

impl MmioBus for NullMmioBus {
    fn read(&self, addr: PhysAddr, size: AccessSize) -> Result<u32, GuestFault> {
        log_debug!(
            LogTarget::Memory,
            "MMIO read with no device attached: addr={addr} size={size:?}"
        );
        Err(GuestFault::MemoryAccessViolation)
    }

    fn write(&self, addr: PhysAddr, size: AccessSize, value: u32) -> Result<(), GuestFault> {
        log_debug!(
            LogTarget::Memory,
            "MMIO write with no device attached: addr={addr} size={size:?} value={value:#x}"
        );
        Err(GuestFault::MemoryAccessViolation)
    }
}

#[cfg(test)]
mod tests {
    use super::{MmioBus, NullMmioBus};
    use crate::addr::PhysAddr;
    use ferro_common::GuestFault;
    use ferro_isa::{AccessSize, MMIO_BASE};

    #[test]
    fn null_bus_faults_on_every_access() {
        let bus = NullMmioBus;
        let addr = PhysAddr::new(MMIO_BASE);
        assert!(matches!(
            bus.read(addr, AccessSize::Word),
            Err(GuestFault::MemoryAccessViolation)
        ));
        assert!(matches!(
            bus.write(addr, AccessSize::Word, 0x1234),
            Err(GuestFault::MemoryAccessViolation)
        ));
    }
}
