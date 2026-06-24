//! `ferro-mem`: the guest's physical memory.
//!
//! This crate models the Ferro VM guest's **physical** memory: a contiguous,
//! safe `Vec<u8>` of RAM, a read-only boot ROM, region classification
//! (RAM / ROM / MMIO / unmapped) and byte/half/word little-endian accesses with
//! explicit bounds and alignment checks. It is the physical substrate beneath
//! the future MMU (address translation) and device bus.
//!
//! # Scope of this part
//!
//! - There is **no** virtual-address translation here. Guest accesses arrive
//!   already as physical addresses ([`PhysAddr`]). [`VirtAddr`] is reserved for
//!   the MMU part.
//! - There are **no** concrete MMIO devices. The MMIO window is recognized and
//!   forwarded to an [`MmioBus`]; only the [`NullMmioBus`] stub ships here.
//!
//! # Layout
//!
//! [`MemoryMap`] materializes the ROM/RAM/MMIO ranges from a
//! [`VpsConfig`](ferro_common::VpsConfig) using the ISA memory-map constants,
//! and validates that they are disjoint and fit the 32-bit space.
//!
//! # Access model and the host/guest barrier
//!
//! [`Memory`] is the **guest** access surface: every access classifies the
//! region, checks that the whole interval stays inside it, checks bounds, then
//! reads/writes via the ISA little-endian helpers or forwards to the bus.
//! Invalid accesses return [`GuestFault`](ferro_common::GuestFault) and never
//! crash the host. Writing ROM from the guest faults.
//!
//! [`PhysMemory`]'s inherent `load_rom`/`load_into_ram`/`fill`/`zero`/
//! `dump_region` methods are **host** operations: they initialize and inspect
//! memory, may write ROM (the host loads firmware), and return
//! [`FerroError`](ferro_common::FerroError). The guest never uses them.
//!
//! # Alignment policy
//!
//! Consistent with the ISA part, unaligned **data** accesses to RAM are allowed
//! (handled by the byte-accurate little-endian helpers) and traced. An access
//! whose interval crosses a region boundary faults. MMIO alignment is the
//! device's concern; the stub bus rejects everything.

#![forbid(unsafe_code)]
#![deny(warnings)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]

mod addr;
mod bus;
mod physmem;
mod region;

pub use addr::{PhysAddr, VirtAddr};
pub use bus::{MmioBus, NullMmioBus};
pub use physmem::{Memory, PhysMemory};
pub use region::{MemRegion, MemoryMap};
