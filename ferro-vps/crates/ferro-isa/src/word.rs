//! Fundamental data model for the Ferro VM ISA: word size, signedness,
//! endianness, memory access sizes, and byte/word reinterpretation helpers.
//!
//! The Ferro VM is a 32-bit, little-endian machine. Every multi-byte value in
//! guest memory is stored least-significant byte first, and every helper here
//! is defined in terms of [`u32::from_le_bytes`] and friends so the behaviour
//! is identical and deterministic on any host architecture, relying only on
//! safe standard-library byte conversions.

/// The machine word: an unsigned 32-bit integer.
pub type Word = u32;

/// The signed interpretation of a machine word.
pub type SWord = i32;

/// Byte order used for multi-byte memory accesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Endianness {
    /// Least-significant byte first. This is the only mode the Ferro VM uses.
    Little,
    /// Most-significant byte first. Reserved; never used by the Ferro VM.
    Big,
}

/// The fixed byte order of the Ferro VM.
pub const ENDIANNESS: Endianness = Endianness::Little;

/// The width of a single memory access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessSize {
    /// 8-bit access (1 byte).
    Byte,
    /// 16-bit access (2 bytes).
    Half,
    /// 32-bit access (4 bytes).
    Word,
}

impl AccessSize {
    /// Returns the number of bytes touched by an access of this size.
    #[must_use]
    pub const fn bytes(self) -> u32 {
        match self {
            Self::Byte => 1,
            Self::Half => 2,
            Self::Word => 4,
        }
    }

    /// Returns the alignment mask for this size (`bytes - 1`).
    #[must_use]
    pub const fn alignment_mask(self) -> u32 {
        self.bytes() - 1
    }

    /// Returns `true` when `address` is naturally aligned for this size.
    #[must_use]
    pub const fn is_aligned(self, address: u32) -> bool {
        address & self.alignment_mask() == 0
    }
}

/// Reinterprets a `u32` bit pattern as `i32` without changing any bits.
#[must_use]
pub const fn as_signed(value: u32) -> i32 {
    i32::from_ne_bytes(value.to_ne_bytes())
}

/// Reinterprets an `i32` bit pattern as `u32` without changing any bits.
#[must_use]
pub const fn as_unsigned(value: i32) -> u32 {
    u32::from_ne_bytes(value.to_ne_bytes())
}

/// Reads a single byte at `offset`, or `None` when out of bounds.
#[inline]
#[must_use]
pub fn read_u8(bytes: &[u8], offset: usize) -> Option<u8> {
    bytes.get(offset).copied()
}

/// Reads a little-endian `u16` starting at `offset`, or `None` when the slice
/// does not hold two bytes there.
#[inline]
#[must_use]
pub fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    let array: [u8; 2] = bytes.get(offset..end)?.try_into().ok()?;
    Some(u16::from_le_bytes(array))
}

/// Reads a little-endian `u32` starting at `offset`, or `None` when the slice
/// does not hold four bytes there.
#[inline]
#[must_use]
pub fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let array: [u8; 4] = bytes.get(offset..end)?.try_into().ok()?;
    Some(u32::from_le_bytes(array))
}

/// Writes a single byte at `offset`, returning `None` when out of bounds.
#[inline]
#[must_use = "check whether the write landed in bounds"]
pub fn write_u8(bytes: &mut [u8], offset: usize, value: u8) -> Option<()> {
    *bytes.get_mut(offset)? = value;
    Some(())
}

/// Writes a little-endian `u16` at `offset`, returning `None` when the slice
/// does not hold two bytes there.
#[inline]
#[must_use = "check whether the write landed in bounds"]
pub fn write_u16(bytes: &mut [u8], offset: usize, value: u16) -> Option<()> {
    let end = offset.checked_add(2)?;
    bytes.get_mut(offset..end)?.copy_from_slice(&value.to_le_bytes());
    Some(())
}

/// Writes a little-endian `u32` at `offset`, returning `None` when the slice
/// does not hold four bytes there.
#[inline]
#[must_use = "check whether the write landed in bounds"]
pub fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Option<()> {
    let end = offset.checked_add(4)?;
    bytes.get_mut(offset..end)?.copy_from_slice(&value.to_le_bytes());
    Some(())
}

#[cfg(test)]
mod tests {
    use super::{
        as_signed, as_unsigned, read_u16, read_u32, read_u8, write_u16, write_u32, write_u8,
        AccessSize, Endianness, ENDIANNESS,
    };

    #[test]
    fn endianness_is_little() {
        assert_eq!(ENDIANNESS, Endianness::Little);
    }

    #[test]
    fn access_size_bytes_and_alignment() {
        assert_eq!(AccessSize::Byte.bytes(), 1);
        assert_eq!(AccessSize::Half.bytes(), 2);
        assert_eq!(AccessSize::Word.bytes(), 4);
        assert!(AccessSize::Word.is_aligned(0x1000));
        assert!(!AccessSize::Word.is_aligned(0x1002));
        assert!(AccessSize::Half.is_aligned(0x1002));
        assert!(!AccessSize::Half.is_aligned(0x1001));
        assert!(AccessSize::Byte.is_aligned(0x1001));
    }

    #[test]
    fn signed_unsigned_round_trip() {
        for value in [0u32, 1, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF] {
            assert_eq!(as_unsigned(as_signed(value)), value);
        }
        assert_eq!(as_signed(0xFFFF_FFFF), -1);
    }

    #[test]
    fn little_endian_reads_are_correct() {
        let bytes = [0x78, 0x56, 0x34, 0x12];
        assert_eq!(read_u8(&bytes, 0), Some(0x78));
        assert_eq!(read_u16(&bytes, 0), Some(0x5678));
        assert_eq!(read_u32(&bytes, 0), Some(0x1234_5678));
        assert_eq!(read_u32(&bytes, 1), None);
        assert_eq!(read_u16(&bytes, 4), None);
    }

    #[test]
    fn writes_are_symmetric_with_reads() {
        let mut bytes = [0u8; 8];
        assert_eq!(write_u8(&mut bytes, 0, 0xAB), Some(()));
        assert_eq!(read_u8(&bytes, 0), Some(0xAB));
        assert_eq!(write_u16(&mut bytes, 2, 0xBEEF), Some(()));
        assert_eq!(read_u16(&bytes, 2), Some(0xBEEF));
        assert_eq!(write_u32(&mut bytes, 4, 0xDEAD_C0DE), Some(()));
        assert_eq!(read_u32(&bytes, 4), Some(0xDEAD_C0DE));
        assert_eq!(write_u32(&mut bytes, 6, 0), None);
    }
}
