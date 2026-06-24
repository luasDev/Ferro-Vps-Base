//! The guest physical memory: RAM, ROM, and MMIO routing.
//!
//! [`PhysMemory`] owns the guest's RAM (a flat, zeroed `Vec<u8>`), the boot
//! ROM, the [`MemoryMap`] describing the layout, and an [`MmioBus`] for I/O.
//! The [`Memory`] trait is the access surface the CPU uses; the future MMU can
//! implement it too. Guest accesses return [`Result<_, GuestFault>`] so that an
//! invalid access faults the *guest* without crashing the host.
//!
//! # Host vs. guest barrier
//!
//! The [`Memory`] trait methods model *guest* accesses: they honour region
//! permissions (writing to ROM faults) and never touch host memory. The
//! inherent `load_*`/`fill`/`zero`/`dump_region` methods are *host* operations
//! used to initialize and inspect memory; they bypass the read-only ROM rule
//! (the host is the one writing the firmware) and return
//! [`FerroError`](ferro_common::FerroError) instead of a guest fault. The guest
//! never has access to these host APIs.

#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]

use ferro_common::log::LogTarget;
use ferro_common::{ensure, log_info, log_trace, FerroResult, GuestFault, VpsConfig};
use ferro_isa::word::{read_u16, read_u32, read_u8, write_u16, write_u32, write_u8};
use ferro_isa::AccessSize;

use crate::addr::PhysAddr;
use crate::bus::{MmioBus, NullMmioBus};
use crate::region::{MemRegion, MemoryMap};

/// The central read/write interface for guest physical memory.
///
/// Implemented by [`PhysMemory`] now and by the MMU in a later part. All
/// methods take guest [`PhysAddr`]es and surface invalid accesses as
/// [`GuestFault`]s rather than host errors.
pub trait Memory {
    /// Reads one byte.
    ///
    /// # Errors
    /// Faults when the address is unmapped, in ROM/RAM out of bounds, or when
    /// an MMIO device rejects the access.
    fn read_u8(&self, addr: PhysAddr) -> Result<u8, GuestFault>;

    /// Reads a little-endian half word (16 bits).
    ///
    /// # Errors
    /// Faults when any byte of the access is outside a single mapped region or
    /// an MMIO device rejects the access.
    fn read_u16(&self, addr: PhysAddr) -> Result<u16, GuestFault>;

    /// Reads a little-endian word (32 bits).
    ///
    /// # Errors
    /// Faults when any byte of the access is outside a single mapped region or
    /// an MMIO device rejects the access.
    fn read_u32(&self, addr: PhysAddr) -> Result<u32, GuestFault>;

    /// Reads `buf.len()` bytes starting at `addr` into `buf`.
    ///
    /// # Errors
    /// Faults when the whole interval is not contained in a single RAM or ROM
    /// region. Block transfers to MMIO are not supported and fault.
    fn read_bytes(&self, addr: PhysAddr, buf: &mut [u8]) -> Result<(), GuestFault>;

    /// Writes one byte.
    ///
    /// # Errors
    /// Faults when the address is unmapped, in RAM out of bounds, in ROM (the
    /// guest cannot write ROM), or when an MMIO device rejects the access.
    fn write_u8(&mut self, addr: PhysAddr, value: u8) -> Result<(), GuestFault>;

    /// Writes a little-endian half word (16 bits).
    ///
    /// # Errors
    /// Faults like [`Memory::write_u8`], extended to the full interval.
    fn write_u16(&mut self, addr: PhysAddr, value: u16) -> Result<(), GuestFault>;

    /// Writes a little-endian word (32 bits).
    ///
    /// # Errors
    /// Faults like [`Memory::write_u8`], extended to the full interval.
    fn write_u32(&mut self, addr: PhysAddr, value: u32) -> Result<(), GuestFault>;

    /// Writes `src.len()` bytes starting at `addr`.
    ///
    /// # Errors
    /// Faults when the whole interval is not contained in a single writable
    /// RAM region. ROM and MMIO block writes fault.
    fn write_bytes(&mut self, addr: PhysAddr, src: &[u8]) -> Result<(), GuestFault>;

    /// Generic sized read, zero-extended into a `u32`.
    ///
    /// Sign extension for `LB`/`LH` is the CPU's responsibility.
    ///
    /// # Errors
    /// Faults like the matching typed read.
    fn read(&self, addr: PhysAddr, size: AccessSize) -> Result<u32, GuestFault>;

    /// Generic sized write of the low `size` bytes of `value`.
    ///
    /// # Errors
    /// Faults like the matching typed write.
    fn write(&mut self, addr: PhysAddr, size: AccessSize, value: u32) -> Result<(), GuestFault>;
}

/// Where a validated access lands, with the slice indices already bounds
/// checked for RAM and ROM.
enum Resolved {
    /// Zero-length access; nothing to do.
    Empty,
    /// RAM access at byte `index` spanning `len` bytes.
    Ram { index: usize, len: usize },
    /// ROM access at byte `index` spanning `len` bytes.
    Rom { index: usize, len: usize },
    /// MMIO access; delegated to the bus.
    Mmio,
}

#[inline]
fn offset_to_index(offset: u64) -> Result<usize, GuestFault> {
    usize::try_from(offset).map_err(|_| GuestFault::MemoryAccessViolation)
}

/// The guest's physical memory.
///
/// Generic over the [`MmioBus`] so the CPU can access it through a concrete
/// type (monomorphized, inlined) on the hot path; defaults to [`NullMmioBus`].
pub struct PhysMemory<B: MmioBus = NullMmioBus> {
    ram: Vec<u8>,
    rom: Vec<u8>,
    map: MemoryMap,
    bus: B,
}

impl<B: MmioBus> PhysMemory<B> {
    /// Allocates zeroed RAM and ROM sized from `config` and attaches `bus`.
    ///
    /// # Errors
    ///
    /// Returns a [`FerroError`](ferro_common::FerroError) when the configured
    /// RAM layout is invalid (see [`MemoryMap::new`]) or when the RAM/ROM size
    /// does not fit this host's address width.
    pub fn new(config: &VpsConfig, bus: B) -> FerroResult<Self> {
        let map = MemoryMap::new(config)?;
        let ram_size = map.ram_size();
        let ram_len = usize::try_from(ram_size)
            .map_err(|_| ferro_common::internal!("configured RAM of {ram_size} bytes does not fit this host's usize"))?;
        let rom_len = usize::try_from(map.rom_size())
            .map_err(|_| ferro_common::internal!("ROM size does not fit this host's usize"))?;

        log_info!(
            LogTarget::Memory,
            "allocating {ram_size} bytes of guest RAM at {}",
            map.ram_base()
        );

        Ok(Self {
            ram: vec![0_u8; ram_len],
            rom: vec![0_u8; rom_len],
            map,
            bus,
        })
    }

    /// Returns the memory map describing this guest's address space.
    #[inline]
    #[must_use]
    pub fn map(&self) -> &MemoryMap {
        &self.map
    }

    /// Returns a shared reference to the attached MMIO bus.
    #[inline]
    #[must_use]
    pub fn bus(&self) -> &B {
        &self.bus
    }

    /// Returns the length of the RAM backing store, in bytes.
    #[inline]
    #[must_use]
    pub fn ram_len(&self) -> usize {
        self.ram.len()
    }

    /// Returns the length of the ROM backing store, in bytes.
    #[inline]
    #[must_use]
    pub fn rom_len(&self) -> usize {
        self.rom.len()
    }

    /// Returns the configured RAM size, in bytes.
    #[inline]
    #[must_use]
    pub fn size(&self) -> u64 {
        self.map.ram_size()
    }

    /// Validates an access of `len` bytes at `addr` and resolves where it
    /// lands. Guarantees the whole interval lies inside a single mapped region
    /// and, for RAM/ROM, that the slice indices are within the backing store.
    #[inline]
    fn resolve(&self, addr: PhysAddr, len: u64) -> Result<Resolved, GuestFault> {
        if len == 0 {
            return Ok(Resolved::Empty);
        }

        let start = addr.as_u64();
        let last = start
            .checked_add(len - 1)
            .ok_or(GuestFault::MemoryAccessViolation)?;
        if last > u64::from(u32::MAX) {
            return Err(GuestFault::MemoryAccessViolation);
        }
        let last_addr = PhysAddr::new(u32::try_from(last).map_err(|_| GuestFault::MemoryAccessViolation)?);

        let region = self.map.classify(addr);
        if self.map.classify(last_addr) != region {
            return Err(GuestFault::MemoryAccessViolation);
        }

        match region {
            MemRegion::Ram => {
                let offset = self
                    .map
                    .ram_offset(addr)
                    .ok_or(GuestFault::MemoryAccessViolation)?;
                let index = offset_to_index(offset)?;
                let span = offset_to_index(len)?;
                let end = index.checked_add(span).ok_or(GuestFault::MemoryAccessViolation)?;
                if end > self.ram.len() {
                    return Err(GuestFault::MemoryAccessViolation);
                }
                Ok(Resolved::Ram { index, len: span })
            }
            MemRegion::Rom => {
                let offset = self
                    .map
                    .rom_offset(addr)
                    .ok_or(GuestFault::MemoryAccessViolation)?;
                let index = offset_to_index(offset)?;
                let span = offset_to_index(len)?;
                let end = index.checked_add(span).ok_or(GuestFault::MemoryAccessViolation)?;
                if end > self.rom.len() {
                    return Err(GuestFault::MemoryAccessViolation);
                }
                Ok(Resolved::Rom { index, len: span })
            }
            MemRegion::Mmio => Ok(Resolved::Mmio),
            MemRegion::Unmapped => Err(GuestFault::MemoryAccessViolation),
        }
    }

    #[inline]
    fn trace_unaligned(addr: PhysAddr, size: AccessSize) {
        if !addr.is_aligned(size) {
            log_trace!(
                LogTarget::Memory,
                "unaligned {size:?} access in RAM at {addr}"
            );
        }
    }

    // ---- Host-only initialization and inspection (bypass guest barrier) ----

    /// Loads `bytes` into the boot ROM, starting at offset 0 (host operation).
    ///
    /// # Errors
    ///
    /// Returns a [`FerroError`](ferro_common::FerroError) when `bytes` is
    /// larger than the ROM.
    pub fn load_rom(&mut self, bytes: &[u8]) -> FerroResult<()> {
        ensure!(
            bytes.len() <= self.rom.len(),
            "ROM image of {} bytes exceeds ROM capacity of {} bytes",
            bytes.len(),
            self.rom.len()
        );
        self.rom[..bytes.len()].copy_from_slice(bytes);
        log_info!(LogTarget::Memory, "loaded {} bytes into ROM", bytes.len());
        Ok(())
    }

    /// Writes `bytes` into RAM at `addr`, bypassing guest write protection
    /// (host operation, e.g. loading an initial image).
    ///
    /// # Errors
    ///
    /// Returns a [`FerroError`](ferro_common::FerroError) when the destination
    /// interval is not fully contained in RAM.
    pub fn load_into_ram(&mut self, addr: PhysAddr, bytes: &[u8]) -> FerroResult<()> {
        let range = self.host_ram_range(addr, bytes.len())?;
        self.ram[range].copy_from_slice(bytes);
        Ok(())
    }

    /// Fills `len` RAM bytes at `addr` with `value` (host operation).
    ///
    /// # Errors
    ///
    /// Returns a [`FerroError`](ferro_common::FerroError) when the interval is
    /// not fully contained in RAM.
    pub fn fill(&mut self, addr: PhysAddr, len: usize, value: u8) -> FerroResult<()> {
        let range = self.host_ram_range(addr, len)?;
        self.ram[range].fill(value);
        Ok(())
    }

    /// Zeroes `len` RAM bytes at `addr` (host operation).
    ///
    /// # Errors
    ///
    /// Returns a [`FerroError`](ferro_common::FerroError) when the interval is
    /// not fully contained in RAM.
    pub fn zero(&mut self, addr: PhysAddr, len: usize) -> FerroResult<()> {
        self.fill(addr, len, 0)
    }

    /// Copies `len` RAM bytes at `addr` into a fresh `Vec` for inspection
    /// (host/debug operation).
    ///
    /// # Errors
    ///
    /// Returns a [`FerroError`](ferro_common::FerroError) when the interval is
    /// not fully contained in RAM.
    pub fn dump_region(&self, addr: PhysAddr, len: usize) -> FerroResult<Vec<u8>> {
        let range = self.host_ram_range(addr, len)?;
        let mut out = Vec::with_capacity(len);
        out.extend_from_slice(&self.ram[range]);
        Ok(out)
    }

    /// Validates a host RAM interval and returns the backing-store index range.
    fn host_ram_range(&self, addr: PhysAddr, len: usize) -> FerroResult<core::ops::Range<usize>> {
        let len_u64 = u64::try_from(len)
            .map_err(|_| ferro_common::internal!("requested length does not fit u64"))?;
        let offset = self
            .map
            .ram_offset(addr)
            .ok_or_else(|| ferro_common::internal!("address {addr} is not in RAM"))?;
        let end = offset
            .checked_add(len_u64)
            .ok_or_else(|| ferro_common::internal!("RAM interval overflows the address computation"))?;
        ensure!(
            end <= self.map.ram_size(),
            "RAM interval [{addr}, +{len}) exceeds RAM size of {} bytes",
            self.map.ram_size()
        );
        let start = usize::try_from(offset)
            .map_err(|_| ferro_common::internal!("RAM offset does not fit usize"))?;
        let end = usize::try_from(end)
            .map_err(|_| ferro_common::internal!("RAM interval end does not fit usize"))?;
        Ok(start..end)
    }
}

impl<B: MmioBus> Memory for PhysMemory<B> {
    #[inline]
    fn read_u8(&self, addr: PhysAddr) -> Result<u8, GuestFault> {
        match self.resolve(addr, 1)? {
            Resolved::Ram { index, .. } => read_u8(&self.ram, index).ok_or(GuestFault::MemoryAccessViolation),
            Resolved::Rom { index, .. } => read_u8(&self.rom, index).ok_or(GuestFault::MemoryAccessViolation),
            Resolved::Mmio => {
                let value = self.bus.read(addr, AccessSize::Byte)?;
                Ok(value.to_le_bytes()[0])
            }
            Resolved::Empty => Err(GuestFault::MemoryAccessViolation),
        }
    }

    #[inline]
    fn read_u16(&self, addr: PhysAddr) -> Result<u16, GuestFault> {
        Self::trace_unaligned(addr, AccessSize::Half);
        match self.resolve(addr, 2)? {
            Resolved::Ram { index, .. } => read_u16(&self.ram, index).ok_or(GuestFault::MemoryAccessViolation),
            Resolved::Rom { index, .. } => read_u16(&self.rom, index).ok_or(GuestFault::MemoryAccessViolation),
            Resolved::Mmio => {
                let value = self.bus.read(addr, AccessSize::Half)?;
                let bytes = value.to_le_bytes();
                Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
            }
            Resolved::Empty => Err(GuestFault::MemoryAccessViolation),
        }
    }

    #[inline]
    fn read_u32(&self, addr: PhysAddr) -> Result<u32, GuestFault> {
        Self::trace_unaligned(addr, AccessSize::Word);
        match self.resolve(addr, 4)? {
            Resolved::Ram { index, .. } => read_u32(&self.ram, index).ok_or(GuestFault::MemoryAccessViolation),
            Resolved::Rom { index, .. } => read_u32(&self.rom, index).ok_or(GuestFault::MemoryAccessViolation),
            Resolved::Mmio => self.bus.read(addr, AccessSize::Word),
            Resolved::Empty => Err(GuestFault::MemoryAccessViolation),
        }
    }

    fn read_bytes(&self, addr: PhysAddr, buf: &mut [u8]) -> Result<(), GuestFault> {
        let len = u64::try_from(buf.len()).map_err(|_| GuestFault::MemoryAccessViolation)?;
        match self.resolve(addr, len)? {
            Resolved::Empty => Ok(()),
            Resolved::Ram { index, len: span } => {
                let end = index.checked_add(span).ok_or(GuestFault::MemoryAccessViolation)?;
                let src = self.ram.get(index..end).ok_or(GuestFault::MemoryAccessViolation)?;
                buf.copy_from_slice(src);
                Ok(())
            }
            Resolved::Rom { index, len: span } => {
                let end = index.checked_add(span).ok_or(GuestFault::MemoryAccessViolation)?;
                let src = self.rom.get(index..end).ok_or(GuestFault::MemoryAccessViolation)?;
                buf.copy_from_slice(src);
                Ok(())
            }
            Resolved::Mmio => Err(GuestFault::MemoryAccessViolation),
        }
    }

    #[inline]
    fn write_u8(&mut self, addr: PhysAddr, value: u8) -> Result<(), GuestFault> {
        match self.resolve(addr, 1)? {
            Resolved::Ram { index, .. } => write_u8(&mut self.ram, index, value).ok_or(GuestFault::MemoryAccessViolation),
            Resolved::Rom { .. } | Resolved::Empty => Err(GuestFault::MemoryAccessViolation),
            Resolved::Mmio => self.bus.write(addr, AccessSize::Byte, u32::from(value)),
        }
    }

    #[inline]
    fn write_u16(&mut self, addr: PhysAddr, value: u16) -> Result<(), GuestFault> {
        Self::trace_unaligned(addr, AccessSize::Half);
        match self.resolve(addr, 2)? {
            Resolved::Ram { index, .. } => write_u16(&mut self.ram, index, value).ok_or(GuestFault::MemoryAccessViolation),
            Resolved::Rom { .. } | Resolved::Empty => Err(GuestFault::MemoryAccessViolation),
            Resolved::Mmio => self.bus.write(addr, AccessSize::Half, u32::from(value)),
        }
    }

    #[inline]
    fn write_u32(&mut self, addr: PhysAddr, value: u32) -> Result<(), GuestFault> {
        Self::trace_unaligned(addr, AccessSize::Word);
        match self.resolve(addr, 4)? {
            Resolved::Ram { index, .. } => write_u32(&mut self.ram, index, value).ok_or(GuestFault::MemoryAccessViolation),
            Resolved::Rom { .. } | Resolved::Empty => Err(GuestFault::MemoryAccessViolation),
            Resolved::Mmio => self.bus.write(addr, AccessSize::Word, value),
        }
    }

    fn write_bytes(&mut self, addr: PhysAddr, src: &[u8]) -> Result<(), GuestFault> {
        let len = u64::try_from(src.len()).map_err(|_| GuestFault::MemoryAccessViolation)?;
        match self.resolve(addr, len)? {
            Resolved::Empty => Ok(()),
            Resolved::Ram { index, len: span } => {
                let end = index.checked_add(span).ok_or(GuestFault::MemoryAccessViolation)?;
                let dst = self.ram.get_mut(index..end).ok_or(GuestFault::MemoryAccessViolation)?;
                dst.copy_from_slice(src);
                Ok(())
            }
            Resolved::Rom { .. } | Resolved::Mmio => Err(GuestFault::MemoryAccessViolation),
        }
    }

    #[inline]
    fn read(&self, addr: PhysAddr, size: AccessSize) -> Result<u32, GuestFault> {
        match size {
            AccessSize::Byte => self.read_u8(addr).map(u32::from),
            AccessSize::Half => self.read_u16(addr).map(u32::from),
            AccessSize::Word => self.read_u32(addr),
        }
    }

    #[inline]
    fn write(&mut self, addr: PhysAddr, size: AccessSize, value: u32) -> Result<(), GuestFault> {
        let bytes = value.to_le_bytes();
        match size {
            AccessSize::Byte => self.write_u8(addr, bytes[0]),
            AccessSize::Half => self.write_u16(addr, u16::from_le_bytes([bytes[0], bytes[1]])),
            AccessSize::Word => self.write_u32(addr, value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Memory, PhysMemory};
    use crate::addr::PhysAddr;
    use crate::bus::{MmioBus, NullMmioBus};
    use core::cell::RefCell;
    use ferro_common::{GuestFault, VpsConfig};
    use ferro_isa::{AccessSize, MMIO_BASE, RAM_BASE, ROM_BASE};

    fn mem() -> PhysMemory {
        PhysMemory::new(&VpsConfig::default(), NullMmioBus).unwrap()
    }

    fn ram(off: u32) -> PhysAddr {
        PhysAddr::new(RAM_BASE + off)
    }

    #[test]
    fn new_allocates_zeroed_ram() {
        let memory = mem();
        assert_eq!(memory.ram_len(), 64 * 1024 * 1024);
        assert_eq!(memory.size(), 64 * 1024 * 1024);
        for off in [0_u32, 1, 100, 1024, (64 * 1024 * 1024) - 1] {
            assert_eq!(memory.read_u8(ram(off)).unwrap(), 0);
        }
    }

    #[test]
    fn round_trip_little_endian() {
        let mut memory = mem();
        memory.write_u32(ram(0x40), 0x1122_3344).unwrap();
        assert_eq!(memory.read_u32(ram(0x40)).unwrap(), 0x1122_3344);
        assert_eq!(memory.read_u8(ram(0x40)).unwrap(), 0x44);
        assert_eq!(memory.read_u8(ram(0x41)).unwrap(), 0x33);
        assert_eq!(memory.read_u8(ram(0x42)).unwrap(), 0x22);
        assert_eq!(memory.read_u8(ram(0x43)).unwrap(), 0x11);

        memory.write_u16(ram(0x80), 0xBEEF).unwrap();
        assert_eq!(memory.read_u16(ram(0x80)).unwrap(), 0xBEEF);
        assert_eq!(memory.read_u8(ram(0x80)).unwrap(), 0xEF);
        assert_eq!(memory.read_u8(ram(0x81)).unwrap(), 0xBE);
    }

    #[test]
    fn generic_access_matches_typed() {
        let mut memory = mem();
        memory.write(ram(0x10), AccessSize::Word, 0xDEAD_BEEF).unwrap();
        assert_eq!(memory.read(ram(0x10), AccessSize::Word).unwrap(), 0xDEAD_BEEF);
        assert_eq!(memory.read(ram(0x10), AccessSize::Byte).unwrap(), 0xEF);
        assert_eq!(memory.read(ram(0x10), AccessSize::Half).unwrap(), 0xBEEF);

        memory.write(ram(0x20), AccessSize::Byte, 0x0000_00AB).unwrap();
        assert_eq!(memory.read_u8(ram(0x20)).unwrap(), 0xAB);
    }

    #[test]
    fn unmapped_access_faults() {
        let mut memory = mem();
        let addr = PhysAddr::new(0x8000_0000);
        assert!(matches!(memory.read_u32(addr), Err(GuestFault::MemoryAccessViolation)));
        assert!(matches!(memory.write_u32(addr, 1), Err(GuestFault::MemoryAccessViolation)));
        assert!(matches!(memory.read_u8(PhysAddr::ZERO), Err(GuestFault::MemoryAccessViolation)));
    }

    #[test]
    fn guest_write_to_rom_faults_but_host_load_works() {
        let mut memory = mem();
        let rom = PhysAddr::new(ROM_BASE);
        assert!(matches!(memory.write_u8(rom, 0x99), Err(GuestFault::MemoryAccessViolation)));
        memory.load_rom(&[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
        assert_eq!(memory.read_u32(rom).unwrap(), 0xEFBE_ADDE);
        assert_eq!(memory.read_u8(rom).unwrap(), 0xDE);
    }

    #[test]
    fn access_crossing_ram_end_faults() {
        let mut memory = mem();
        let last = u32::try_from((64 * 1024 * 1024_u64) - 1).unwrap();
        let last_byte = PhysAddr::new(RAM_BASE + last);
        assert!(memory.read_u8(last_byte).is_ok());
        assert!(matches!(memory.read_u16(last_byte), Err(GuestFault::MemoryAccessViolation)));
        let word_start = PhysAddr::new(RAM_BASE + last - 2);
        assert!(matches!(memory.read_u32(word_start), Err(GuestFault::MemoryAccessViolation)));
        assert!(matches!(memory.write_u32(word_start, 0), Err(GuestFault::MemoryAccessViolation)));
    }

    #[test]
    fn unaligned_ram_access_is_allowed() {
        let mut memory = mem();
        memory.write_u32(ram(0x101), 0x0A0B_0C0D).unwrap();
        assert_eq!(memory.read_u32(ram(0x101)).unwrap(), 0x0A0B_0C0D);
        memory.write_u16(ram(0x203), 0x1234).unwrap();
        assert_eq!(memory.read_u16(ram(0x203)).unwrap(), 0x1234);
    }

    #[derive(Default)]
    struct RecordingBus {
        reads: RefCell<Vec<(u32, AccessSize)>>,
        writes: RefCell<Vec<(u32, AccessSize, u32)>>,
    }

    impl MmioBus for RecordingBus {
        fn read(&self, addr: PhysAddr, size: AccessSize) -> Result<u32, GuestFault> {
            self.reads.borrow_mut().push((addr.as_u32(), size));
            Ok(0xA5A5_A5A5)
        }

        fn write(&self, addr: PhysAddr, size: AccessSize, value: u32) -> Result<(), GuestFault> {
            self.writes.borrow_mut().push((addr.as_u32(), size, value));
            Ok(())
        }
    }

    #[test]
    fn mmio_is_forwarded_to_bus() {
        let mut memory = PhysMemory::new(&VpsConfig::default(), RecordingBus::default()).unwrap();
        let addr = PhysAddr::new(MMIO_BASE);
        assert_eq!(memory.read_u32(addr).unwrap(), 0xA5A5_A5A5);
        memory.write_u32(addr, 0x1234_5678).unwrap();
        assert_eq!(memory.bus().reads.borrow().len(), 1);
        assert_eq!(memory.bus().writes.borrow()[0], (MMIO_BASE, AccessSize::Word, 0x1234_5678));

        let mut null = mem();
        assert!(matches!(null.read_u32(addr), Err(GuestFault::MemoryAccessViolation)));
        assert!(matches!(null.write_u32(addr, 0), Err(GuestFault::MemoryAccessViolation)));
    }

    #[test]
    fn load_into_ram_writes_and_validates_bounds() {
        let mut memory = mem();
        memory.load_into_ram(ram(0x10), &[1, 2, 3, 4]).unwrap();
        let mut buf = [0_u8; 4];
        memory.read_bytes(ram(0x10), &mut buf).unwrap();
        assert_eq!(buf, [1, 2, 3, 4]);

        let too_big = vec![0_u8; (64 * 1024 * 1024) + 1];
        assert!(memory.load_into_ram(ram(0), &too_big).is_err());
        assert!(memory.load_into_ram(PhysAddr::new(0x8000_0000), &[1]).is_err());
    }

    #[test]
    fn dump_zero_fill_respect_bounds() {
        let mut memory = mem();
        memory.fill(ram(0x40), 8, 0xCC).unwrap();
        assert_eq!(memory.dump_region(ram(0x40), 8).unwrap(), vec![0xCC; 8]);
        memory.zero(ram(0x40), 4).unwrap();
        assert_eq!(memory.dump_region(ram(0x40), 8).unwrap(), vec![0, 0, 0, 0, 0xCC, 0xCC, 0xCC, 0xCC]);
        assert!(memory.fill(ram(0), 64 * 1024 * 1024 + 1, 0).is_err());
        assert!(memory.dump_region(PhysAddr::new(0x8000_0000), 4).is_err());
    }

    #[test]
    fn no_access_panics_for_any_address() {
        let mut memory = mem();
        let mut addr: u32 = 0;
        loop {
            let pa = PhysAddr::new(addr);
            let _ = memory.read_u8(pa);
            let _ = memory.read_u16(pa);
            let _ = memory.read_u32(pa);
            let _ = memory.write_u8(pa, 0xAB);
            let _ = memory.write_u16(pa, 0xABCD);
            let _ = memory.write_u32(pa, 0xABCD_1234);
            for size in [AccessSize::Byte, AccessSize::Half, AccessSize::Word] {
                let _ = memory.read(pa, size);
                let _ = memory.write(pa, size, 0x55);
            }
            match addr.checked_add(0x000F_FFF7) {
                Some(next) => addr = next,
                None => break,
            }
        }
        // Exercise the exact region boundaries too.
        for edge in [
            0_u32,
            ROM_BASE,
            ROM_BASE.wrapping_sub(1),
            RAM_BASE,
            RAM_BASE.wrapping_sub(1),
            MMIO_BASE,
            MMIO_BASE.wrapping_sub(1),
            u32::MAX,
        ] {
            let pa = PhysAddr::new(edge);
            let _ = memory.read_u32(pa);
            let _ = memory.write_u32(pa, 0);
        }
    }
}
