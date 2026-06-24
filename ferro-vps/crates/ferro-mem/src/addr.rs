//! Guest address types.
//!
//! [`PhysAddr`] is a 32-bit *physical* guest address: an offset into the flat
//! physical address space described by the ISA memory map. In this part every
//! guest access arrives already as a [`PhysAddr`]; there is no address
//! translation. [`VirtAddr`] is reserved for the future MMU part and is never
//! translated here.

#![allow(clippy::module_name_repetitions)]

use core::fmt;

use ferro_isa::AccessSize;

/// A guest *physical* address.
///
/// All arithmetic is checked: an address never wraps silently past the end of
/// the 32-bit space. Construct from a raw value with [`PhysAddr::new`] and read
/// it back with [`PhysAddr::as_u32`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysAddr(u32);

impl PhysAddr {
    /// The all-zero physical address.
    pub const ZERO: Self = Self(0);

    /// Wraps a raw 32-bit value as a physical address.
    #[inline]
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the raw 32-bit value of this address.
    #[inline]
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Returns this address widened to a `u64`, which can hold any 32-bit
    /// address plus an access size without overflowing.
    #[inline]
    #[must_use]
    pub fn as_u64(self) -> u64 {
        u64::from(self.0)
    }

    /// Offsets this address upward by `delta` bytes.
    ///
    /// Returns `None` when the result would leave the 32-bit address space;
    /// the address never wraps around.
    #[inline]
    #[must_use]
    pub const fn offset(self, delta: u32) -> Option<Self> {
        match self.0.checked_add(delta) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns `true` when this address is naturally aligned for an access of
    /// `size`.
    #[inline]
    #[must_use]
    pub const fn is_aligned(self, size: AccessSize) -> bool {
        size.is_aligned(self.0)
    }

    /// Rounds this address *down* to the nearest multiple of `align`.
    ///
    /// `align` must be a power of two. When `align` is zero the address is
    /// returned unchanged.
    #[inline]
    #[must_use]
    pub const fn align_down(self, align: u32) -> Self {
        if align == 0 {
            return self;
        }
        Self(self.0 & !(align - 1))
    }

    /// Rounds this address *up* to the nearest multiple of `align`.
    ///
    /// `align` must be a power of two. When `align` is zero the address is
    /// returned unchanged. Returns `None` when rounding up would overflow the
    /// 32-bit address space.
    #[inline]
    #[must_use]
    pub const fn align_up(self, align: u32) -> Option<Self> {
        if align == 0 {
            return Some(self);
        }
        let mask = align - 1;
        match self.0.checked_add(mask) {
            Some(sum) => Some(Self(sum & !mask)),
            None => None,
        }
    }
}

impl fmt::Display for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#010x}", self.0)
    }
}

impl From<u32> for PhysAddr {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<PhysAddr> for u32 {
    fn from(value: PhysAddr) -> Self {
        value.0
    }
}

/// A guest *virtual* address, reserved for the future MMU part.
///
/// This crate performs no address translation, so it never converts a
/// [`VirtAddr`] into a [`PhysAddr`]. The type exists so that later parts can
/// distinguish translated from untranslated addresses at the type level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtAddr(u32);

impl VirtAddr {
    /// Wraps a raw 32-bit value as a virtual address.
    #[inline]
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the raw 32-bit value of this address.
    #[inline]
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Display for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#010x}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{PhysAddr, VirtAddr};
    use ferro_isa::AccessSize;

    #[test]
    fn raw_round_trips() {
        let addr = PhysAddr::new(0x1234_5678);
        assert_eq!(addr.as_u32(), 0x1234_5678);
        assert_eq!(addr.as_u64(), 0x1234_5678_u64);
        assert_eq!(u32::from(addr), 0x1234_5678);
        assert_eq!(PhysAddr::from(0xABCD).as_u32(), 0xABCD);
        assert_eq!(PhysAddr::ZERO.as_u32(), 0);
    }

    #[test]
    fn offset_is_checked() {
        assert_eq!(PhysAddr::new(0x10).offset(0x10), Some(PhysAddr::new(0x20)));
        assert_eq!(PhysAddr::new(u32::MAX).offset(1), None);
        assert_eq!(PhysAddr::new(u32::MAX - 1).offset(1), Some(PhysAddr::new(u32::MAX)));
    }

    #[test]
    fn alignment_helpers_round_to_boundaries() {
        let addr = PhysAddr::new(0x1003);
        assert_eq!(addr.align_down(4), PhysAddr::new(0x1000));
        assert_eq!(addr.align_up(4), Some(PhysAddr::new(0x1004)));
        assert_eq!(PhysAddr::new(0x1000).align_up(4), Some(PhysAddr::new(0x1000)));
        assert_eq!(PhysAddr::new(0x100).align_down(0), PhysAddr::new(0x100));
        assert_eq!(PhysAddr::new(u32::MAX).align_up(4), None);
    }

    #[test]
    fn is_aligned_matches_access_size() {
        assert!(PhysAddr::new(0x1000).is_aligned(AccessSize::Word));
        assert!(!PhysAddr::new(0x1002).is_aligned(AccessSize::Word));
        assert!(PhysAddr::new(0x1002).is_aligned(AccessSize::Half));
        assert!(PhysAddr::new(0x1001).is_aligned(AccessSize::Byte));
    }

    #[test]
    fn virt_addr_is_independent() {
        let virt = VirtAddr::new(0xDEAD_BEEF);
        assert_eq!(virt.as_u32(), 0xDEAD_BEEF);
        assert_eq!(virt.to_string(), "0xdeadbeef");
    }
}
