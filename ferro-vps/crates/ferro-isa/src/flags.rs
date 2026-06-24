//! The processor status register (`FLAGS`).
//!
//! The Ferro VM follows a RISC-style model: ALU instructions do **not** update
//! the flags implicitly. The `Z`/`N`/`C`/`V` bits exist only for the specific
//! instructions that explicitly consume or produce them (carry-aware
//! arithmetic and compare-and-set style operations). Conditional branches
//! compare registers directly (`BEQ`, `BLT`, ...) instead of reading these
//! flags. Bits outside [`DEFINED_MASK`] are reserved and are preserved across
//! every mutating operation.

/// Bit mask for the zero flag (`Z`).
pub const FLAG_Z: u32 = 1 << 0;
/// Bit mask for the negative / sign flag (`N`).
pub const FLAG_N: u32 = 1 << 1;
/// Bit mask for the carry / unsigned-overflow flag (`C`).
pub const FLAG_C: u32 = 1 << 2;
/// Bit mask for the signed-overflow flag (`V`).
pub const FLAG_V: u32 = 1 << 3;

/// Mask covering every defined flag bit. Bits outside this mask are reserved
/// for future use and must never be disturbed.
pub const DEFINED_MASK: u32 = FLAG_Z | FLAG_N | FLAG_C | FLAG_V;

/// The processor status register: a set of status bits packed into a `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Flags(u32);

impl Flags {
    /// Creates a flags register with every bit clear.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Creates a flags register from a raw bit pattern.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns the raw bit pattern, including any reserved bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns `true` when every bit in `mask` is set.
    #[must_use]
    pub const fn contains(self, mask: u32) -> bool {
        self.0 & mask == mask
    }

    /// Sets every bit in `mask`.
    pub fn insert(&mut self, mask: u32) {
        self.0 |= mask;
    }

    /// Clears every bit in `mask`.
    pub fn remove(&mut self, mask: u32) {
        self.0 &= !mask;
    }

    /// Sets or clears the bits in `mask` according to `value`.
    pub fn set(&mut self, mask: u32, value: bool) {
        if value {
            self.insert(mask);
        } else {
            self.remove(mask);
        }
    }

    /// Returns the zero flag.
    #[must_use]
    pub const fn zero(self) -> bool {
        self.contains(FLAG_Z)
    }

    /// Returns the negative / sign flag.
    #[must_use]
    pub const fn negative(self) -> bool {
        self.contains(FLAG_N)
    }

    /// Returns the carry / unsigned-overflow flag.
    #[must_use]
    pub const fn carry(self) -> bool {
        self.contains(FLAG_C)
    }

    /// Returns the signed-overflow flag.
    #[must_use]
    pub const fn overflow(self) -> bool {
        self.contains(FLAG_V)
    }

    /// Sets or clears the zero flag.
    pub fn set_zero(&mut self, value: bool) {
        self.set(FLAG_Z, value);
    }

    /// Sets or clears the negative / sign flag.
    pub fn set_negative(&mut self, value: bool) {
        self.set(FLAG_N, value);
    }

    /// Sets or clears the carry / unsigned-overflow flag.
    pub fn set_carry(&mut self, value: bool) {
        self.set(FLAG_C, value);
    }

    /// Sets or clears the signed-overflow flag.
    pub fn set_overflow(&mut self, value: bool) {
        self.set(FLAG_V, value);
    }
}

#[cfg(test)]
mod tests {
    use super::{Flags, DEFINED_MASK, FLAG_C, FLAG_N, FLAG_V, FLAG_Z};

    #[test]
    fn masks_are_distinct_and_within_defined() {
        let masks = [FLAG_Z, FLAG_N, FLAG_C, FLAG_V];
        let combined = masks.iter().fold(0u32, |acc, m| acc | m);
        assert_eq!(combined, DEFINED_MASK);
        assert_eq!(combined.count_ones(), 4);
    }

    #[test]
    fn individual_flags_are_independent() {
        let mut flags = Flags::empty();
        flags.set_zero(true);
        assert!(flags.zero());
        assert!(!flags.negative() && !flags.carry() && !flags.overflow());
        flags.set_overflow(true);
        assert!(flags.zero() && flags.overflow());
        flags.set_zero(false);
        assert!(!flags.zero() && flags.overflow());
    }

    #[test]
    fn reserved_bits_are_preserved() {
        let reserved = !DEFINED_MASK;
        let mut flags = Flags::from_bits(reserved);
        flags.set_zero(true);
        flags.set_negative(true);
        flags.set_carry(true);
        flags.set_overflow(true);
        flags.set_zero(false);
        assert_eq!(flags.bits() & reserved, reserved);
    }
}
