//! Region classification and the materialized memory map.
//!
//! [`MemoryMap`] turns a [`VpsConfig`] into concrete address ranges for ROM,
//! RAM and the MMIO window, validating on construction that the ranges are
//! disjoint and fit inside the 32-bit physical space. [`MemRegion`] names the
//! region an address falls into.

#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]

use ferro_common::{ensure, FerroResult, VpsConfig};
use ferro_isa::{MMIO_BASE, MMIO_SIZE, RAM_BASE, ROM_BASE, ROM_SIZE};

use crate::addr::PhysAddr;

/// A region of the guest physical address space, as seen by the physical
/// memory layer.
///
/// The low reserved/trap-vector area defined by the ISA has no backing storage
/// in this part, so addresses there classify as [`MemRegion::Unmapped`] and
/// any guest access to them faults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MemRegion {
    /// Read-only boot ROM.
    Rom,
    /// Guest main RAM.
    Ram,
    /// Memory-mapped I/O window.
    Mmio,
    /// Not part of any mapped, accessible region.
    Unmapped,
}

/// The concrete address-space layout for one guest, derived from its
/// [`VpsConfig`].
///
/// Bases and the ROM/MMIO sizes come from the ISA memory map; the RAM size
/// comes from the configuration. All ranges are half-open `[base, base+size)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryMap {
    rom_base: u32,
    rom_size: u32,
    ram_base: u32,
    ram_size: u64,
    mmio_base: u32,
    mmio_size: u32,
}

impl MemoryMap {
    /// Builds the memory map for `config`.
    ///
    /// # Errors
    ///
    /// Returns a [`FerroError`](ferro_common::FerroError) when the configured
    /// RAM size would overflow the 32-bit address space or overlap the MMIO
    /// window, or when the fixed ROM/RAM/MMIO ranges are not ordered and
    /// disjoint (a defensive re-check of the ISA constants).
    pub fn new(config: &VpsConfig) -> FerroResult<Self> {
        let ram_size = config.memory.ram_size.as_bytes();
        let ram_base = u64::from(RAM_BASE);
        let ram_end = ram_base
            .checked_add(ram_size)
            .ok_or_else(|| ferro_common::internal!("RAM range overflows the 64-bit address computation"))?;

        let rom_end = u64::from(ROM_BASE) + u64::from(ROM_SIZE);
        ensure!(
            rom_end <= ram_base,
            "ROM range [{ROM_BASE:#x}, {rom_end:#x}) overlaps RAM base {RAM_BASE:#x}"
        );
        ensure!(
            ram_end <= u64::from(MMIO_BASE),
            "configured RAM of {ram_size} bytes overflows into the MMIO window at {MMIO_BASE:#x}"
        );

        Ok(Self {
            rom_base: ROM_BASE,
            rom_size: ROM_SIZE,
            ram_base: RAM_BASE,
            ram_size,
            mmio_base: MMIO_BASE,
            mmio_size: MMIO_SIZE,
        })
    }

    /// Returns the base address of the boot ROM.
    #[inline]
    #[must_use]
    pub const fn rom_base(&self) -> PhysAddr {
        PhysAddr::new(self.rom_base)
    }

    /// Returns the size of the boot ROM, in bytes.
    #[inline]
    #[must_use]
    pub const fn rom_size(&self) -> u32 {
        self.rom_size
    }

    /// Returns the base address of guest RAM.
    #[inline]
    #[must_use]
    pub const fn ram_base(&self) -> PhysAddr {
        PhysAddr::new(self.ram_base)
    }

    /// Returns the size of guest RAM, in bytes.
    #[inline]
    #[must_use]
    pub const fn ram_size(&self) -> u64 {
        self.ram_size
    }

    /// Returns the base address of the MMIO window.
    #[inline]
    #[must_use]
    pub const fn mmio_base(&self) -> PhysAddr {
        PhysAddr::new(self.mmio_base)
    }

    /// Returns the size of the MMIO window, in bytes.
    #[inline]
    #[must_use]
    pub const fn mmio_size(&self) -> u32 {
        self.mmio_size
    }

    /// Classifies `addr` into the region that contains it.
    ///
    /// The hottest region (RAM) is checked first.
    #[inline]
    #[must_use]
    pub fn classify(&self, addr: PhysAddr) -> MemRegion {
        let value = addr.as_u64();

        let ram_start = u64::from(self.ram_base);
        let ram_end = ram_start + self.ram_size;
        if (ram_start..ram_end).contains(&value) {
            return MemRegion::Ram;
        }

        let rom_start = u64::from(self.rom_base);
        let rom_end = rom_start + u64::from(self.rom_size);
        if (rom_start..rom_end).contains(&value) {
            return MemRegion::Rom;
        }

        let mmio_start = u64::from(self.mmio_base);
        let mmio_end = mmio_start + u64::from(self.mmio_size);
        if (mmio_start..mmio_end).contains(&value) {
            return MemRegion::Mmio;
        }

        MemRegion::Unmapped
    }

    /// Returns the byte offset of `addr` within RAM, or `None` when `addr` is
    /// not in RAM.
    #[inline]
    #[must_use]
    pub fn ram_offset(&self, addr: PhysAddr) -> Option<u64> {
        let value = addr.as_u64();
        let ram_start = u64::from(self.ram_base);
        let ram_end = ram_start + self.ram_size;
        if (ram_start..ram_end).contains(&value) {
            Some(value - ram_start)
        } else {
            None
        }
    }

    /// Returns the byte offset of `addr` within the ROM, or `None` when `addr`
    /// is not in ROM.
    #[inline]
    #[must_use]
    pub fn rom_offset(&self, addr: PhysAddr) -> Option<u64> {
        let value = addr.as_u64();
        let rom_start = u64::from(self.rom_base);
        let rom_end = rom_start + u64::from(self.rom_size);
        if (rom_start..rom_end).contains(&value) {
            Some(value - rom_start)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MemRegion, MemoryMap};
    use crate::addr::PhysAddr;
    use ferro_common::config::ByteSize;
    use ferro_common::VpsConfig;
    use ferro_isa::{MMIO_BASE, RAM_BASE, ROM_BASE};

    #[test]
    fn classify_matches_layout() {
        let map = MemoryMap::new(&VpsConfig::default()).unwrap();
        assert_eq!(map.classify(PhysAddr::new(0)), MemRegion::Unmapped);
        assert_eq!(map.classify(PhysAddr::new(ROM_BASE)), MemRegion::Rom);
        assert_eq!(map.classify(PhysAddr::new(RAM_BASE)), MemRegion::Ram);
        assert_eq!(map.classify(PhysAddr::new(MMIO_BASE)), MemRegion::Mmio);
        assert_eq!(map.classify(PhysAddr::new(0x8000_0000)), MemRegion::Unmapped);
    }

    #[test]
    fn ram_and_rom_offsets_are_relative_to_bases() {
        let map = MemoryMap::new(&VpsConfig::default()).unwrap();
        assert_eq!(map.ram_offset(PhysAddr::new(RAM_BASE)), Some(0));
        assert_eq!(map.ram_offset(PhysAddr::new(RAM_BASE + 0x100)), Some(0x100));
        assert_eq!(map.ram_offset(PhysAddr::new(ROM_BASE)), None);
        assert_eq!(map.rom_offset(PhysAddr::new(ROM_BASE + 4)), Some(4));
        assert_eq!(map.rom_offset(PhysAddr::new(RAM_BASE)), None);
    }

    #[test]
    fn oversized_ram_overlapping_mmio_is_rejected() {
        let mut config = VpsConfig::default();
        config.memory.ram_size = ByteSize::from_gib(4);
        assert!(MemoryMap::new(&config).is_err());
    }

    #[test]
    fn default_map_exposes_expected_geometry() {
        let map = MemoryMap::new(&VpsConfig::default()).unwrap();
        assert_eq!(map.ram_base().as_u32(), RAM_BASE);
        assert_eq!(map.ram_size(), 64 * 1024 * 1024);
        assert_eq!(map.rom_base().as_u32(), ROM_BASE);
        assert_eq!(map.mmio_base().as_u32(), MMIO_BASE);
    }
}
