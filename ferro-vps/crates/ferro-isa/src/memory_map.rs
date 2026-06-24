//! The guest physical address-space map.
//!
//! The Ferro VM has a flat 32-bit physical address space (4 GiB theoretical).
//! This module fixes the canonical layout and its boundary constants. Address
//! translation (paging) belongs to the MMU part; this map describes the
//! *physical/effective* space the guest sees. Accesses outside every mapped
//! region become [`GuestFault::MemoryAccessViolation`](ferro_common::error::GuestFault)
//! in the memory part; here we only define the contract and the limits.
//!
//! Layout (low to high):
//!
//! - `0x0000_0000 .. 0x0000_1000` reserved / trap vector table.
//! - `0x0000_1000 .. 0x0001_0000` read-only boot ROM (reset lands here).
//! - `0x1000_0000 .. 0x1000_0000 + ram_size` guest main RAM.
//! - `0xF000_0000 .. 0xF100_0000` memory-mapped I/O window.

use ferro_common::config::VpsConfig;

/// Base of the low reserved region that holds the trap vector table.
pub const TRAP_VECTOR_BASE: u32 = 0x0000_0000;
/// Size of the low reserved / trap region, in bytes.
pub const RESERVED_SIZE: u32 = 0x0000_1000;
/// Base of the read-only boot ROM.
pub const ROM_BASE: u32 = 0x0000_1000;
/// Size of the boot ROM region, in bytes.
pub const ROM_SIZE: u32 = 0x0000_F000;
/// Program-counter value at reset (the start of ROM).
pub const RESET_VECTOR: u32 = ROM_BASE;
/// Base of guest main RAM.
pub const RAM_BASE: u32 = 0x1000_0000;
/// Base of the memory-mapped I/O window.
pub const MMIO_BASE: u32 = 0xF000_0000;
/// Size of the memory-mapped I/O window, in bytes.
pub const MMIO_SIZE: u32 = 0x0100_0000;

/// A region of the guest physical address space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Region {
    /// Low reserved region, including the trap vector table.
    Reserved,
    /// Read-only boot ROM.
    Rom,
    /// Guest main RAM.
    Ram,
    /// Memory-mapped I/O window.
    Mmio,
    /// Not part of any mapped region.
    Unmapped,
}

/// Returns the number of RAM bytes configured for the guest.
#[must_use]
pub fn ram_size_bytes(config: &VpsConfig) -> u64 {
    config.memory.ram_size.as_bytes()
}

/// Returns the half-open RAM address range from start to end for `config`, as
/// 64-bit values so the end never overflows the 32-bit address type. The range
/// includes start and excludes end.
#[must_use]
pub fn ram_range(config: &VpsConfig) -> (u64, u64) {
    let start = u64::from(RAM_BASE);
    (start, start + ram_size_bytes(config))
}

/// Returns `true` when `address` is correctly aligned to fetch an instruction
/// (a multiple of four). A misaligned program counter is a guest fault.
#[must_use]
pub const fn is_instruction_aligned(address: u32) -> bool {
    address.trailing_zeros() >= 2
}

/// Classifies a guest physical address into the region that contains it.
#[must_use]
pub fn classify(address: u32, config: &VpsConfig) -> Region {
    if address < RESERVED_SIZE {
        return Region::Reserved;
    }
    if (ROM_BASE..ROM_BASE + ROM_SIZE).contains(&address) {
        return Region::Rom;
    }
    let (ram_start, ram_end) = ram_range(config);
    let wide = u64::from(address);
    if wide >= ram_start && wide < ram_end {
        return Region::Ram;
    }
    if (MMIO_BASE..MMIO_BASE + MMIO_SIZE).contains(&address) {
        return Region::Mmio;
    }
    Region::Unmapped
}

#[cfg(test)]
mod tests {
    use super::{
        classify, is_instruction_aligned, ram_range, Region, MMIO_BASE, MMIO_SIZE, RAM_BASE,
        RESERVED_SIZE, RESET_VECTOR, ROM_BASE, ROM_SIZE,
    };
    use ferro_common::config::VpsConfig;

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn regions_are_ordered_and_disjoint() {
        assert!(RESERVED_SIZE <= ROM_BASE);
        assert!(ROM_BASE + ROM_SIZE <= RAM_BASE);
        assert!(RAM_BASE < MMIO_BASE);
        assert!(MMIO_SIZE > 0);
        assert!(u64::from(MMIO_BASE) + u64::from(MMIO_SIZE) <= u64::from(u32::MAX) + 1);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn reset_vector_is_inside_rom() {
        assert!(RESET_VECTOR >= ROM_BASE && RESET_VECTOR < ROM_BASE + ROM_SIZE);
    }

    #[test]
    fn classify_matches_layout() {
        let config = VpsConfig::default();
        assert_eq!(classify(0, &config), Region::Reserved);
        assert_eq!(classify(ROM_BASE, &config), Region::Rom);
        assert_eq!(classify(RAM_BASE, &config), Region::Ram);
        assert_eq!(classify(MMIO_BASE, &config), Region::Mmio);
        assert_eq!(classify(0x8000_0000, &config), Region::Unmapped);
        let (start, end) = ram_range(&config);
        assert_eq!(start, u64::from(RAM_BASE));
        assert!(end > start);
    }

    #[test]
    fn instruction_alignment_detects_misalignment() {
        assert!(is_instruction_aligned(RESET_VECTOR));
        assert!(is_instruction_aligned(0x1000_0000));
        assert!(!is_instruction_aligned(0x1000_0002));
        assert!(!is_instruction_aligned(0x1000_0001));
    }
}
