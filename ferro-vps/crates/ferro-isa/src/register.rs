//! The register model: the 32 general-purpose registers and their ABI names,
//! the reserved system registers, and the execution privilege modes.

use crate::error::DecodeError;

/// Number of general-purpose registers in the Ferro VM.
pub const REGISTER_COUNT: u8 = 32;

/// A general-purpose register index in the range `0..=31`.
///
/// Register `R0` is hardwired to zero: reads always yield `0` and writes are
/// discarded. Because every register field in the instruction encoding is five
/// bits wide, a decoded register is always in range and can never index outside
/// the 32-register bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Register(u8);

impl Register {
    /// Register `R0`, the hardwired-zero register.
    pub const ZERO: Self = Self(0);

    /// Creates a register from an index, validating the `0..=31` range.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::InvalidRegister`] when `index` is greater than 31.
    pub const fn new(index: u8) -> Result<Self, DecodeError> {
        if index < REGISTER_COUNT {
            Ok(Self(index))
        } else {
            Err(DecodeError::InvalidRegister { index })
        }
    }

    /// Creates a register from the low five bits of `bits`.
    ///
    /// This is infallible: only the five least-significant bits are used, so
    /// the result is always a valid `0..=31` index. It is the constructor used
    /// by the instruction decoder.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn from_bits(bits: u32) -> Self {
        Self((bits & 0x1F) as u8)
    }

    /// Returns the numeric index of this register (`0..=31`).
    #[must_use]
    pub const fn index(self) -> u8 {
        self.0
    }

    /// Returns `true` for `R0`, the hardwired-zero register.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Returns the ABI mnemonic for this register (for example `sp` for `R2`).
    ///
    /// The ABI is purely a software convention; the hardware does not
    /// distinguish registers beyond `R0`'s hardwired-zero behaviour.
    #[must_use]
    pub const fn abi_name(self) -> &'static str {
        match self.0 {
            0 => "zr",
            1 => "ra",
            2 => "sp",
            3 => "fp",
            4 => "a0",
            5 => "a1",
            6 => "a2",
            7 => "a3",
            8 => "a4",
            9 => "a5",
            10 => "a6",
            11 => "a7",
            12 => "t0",
            13 => "t1",
            14 => "t2",
            15 => "t3",
            16 => "t4",
            17 => "t5",
            18 => "t6",
            19 => "t7",
            20 => "t8",
            21 => "t9",
            22 => "t10",
            23 => "t11",
            24 => "t12",
            25 => "t13",
            26 => "t14",
            27 => "t15",
            28 => "k0",
            29 => "k1",
            30 => "k2",
            _ => "k3",
        }
    }
}

/// System / privileged registers, kept separate from the general-purpose bank.
///
/// These are reserved by the ISA now so that privileged instructions have a
/// stable target; their effects are implemented in the kernel/MMU part. Each
/// has a stable [`SysReg::index`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SysReg {
    /// Trap cause register.
    Cause,
    /// Exception program counter (the saved return address for a trap).
    Epc,
    /// Page-table base register for the MMU.
    PtBase,
    /// Processor status register (privilege mode, interrupt-enable, ...).
    Status,
    /// Scratch register reserved for the trap handler.
    Scratch,
}

impl SysReg {
    /// Returns the stable numeric index of this system register.
    #[must_use]
    pub const fn index(self) -> u16 {
        match self {
            Self::Cause => 0,
            Self::Epc => 1,
            Self::PtBase => 2,
            Self::Status => 3,
            Self::Scratch => 4,
        }
    }

    /// Returns the system register for `index`, or `None` when unknown.
    #[must_use]
    pub const fn from_index(index: u16) -> Option<Self> {
        match index {
            0 => Some(Self::Cause),
            1 => Some(Self::Epc),
            2 => Some(Self::PtBase),
            3 => Some(Self::Status),
            4 => Some(Self::Scratch),
            _ => None,
        }
    }

    /// Returns the canonical lowercase name of this system register.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cause => "cause",
            Self::Epc => "epc",
            Self::PtBase => "ptbase",
            Self::Status => "status",
            Self::Scratch => "scratch",
        }
    }
}

/// Execution privilege level.
///
/// Reserved now so the ISA can mark which instructions are privileged; the
/// trapping behaviour is implemented in the kernel part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeMode {
    /// Unprivileged user mode. Privileged instructions trap here.
    User,
    /// Privileged kernel mode.
    Kernel,
}

impl PrivilegeMode {
    /// Returns `true` when running in kernel (privileged) mode.
    #[must_use]
    pub const fn is_privileged(self) -> bool {
        matches!(self, Self::Kernel)
    }
}

#[cfg(test)]
mod tests {
    use super::{PrivilegeMode, Register, SysReg, REGISTER_COUNT};
    use crate::error::DecodeError;

    #[test]
    fn new_rejects_out_of_range() {
        assert_eq!(Register::new(0).map(Register::index), Ok(0));
        assert_eq!(Register::new(31).map(Register::index), Ok(31));
        assert_eq!(Register::new(32), Err(DecodeError::InvalidRegister { index: 32 }));
        assert_eq!(Register::new(255), Err(DecodeError::InvalidRegister { index: 255 }));
    }

    #[test]
    fn from_bits_masks_to_five_bits() {
        for raw in 0u32..256 {
            let reg = Register::from_bits(raw);
            assert!(reg.index() < REGISTER_COUNT);
            assert_eq!(u32::from(reg.index()), raw & 0x1F);
        }
    }

    #[test]
    fn zero_register_reports_is_zero() {
        assert!(Register::ZERO.is_zero());
        assert!(Register::new(0).unwrap().is_zero());
        assert!(!Register::new(1).unwrap().is_zero());
    }

    #[test]
    fn abi_names_are_unique_and_present() {
        let mut seen = std::collections::HashSet::new();
        for index in 0..REGISTER_COUNT {
            let reg = Register::new(index).unwrap();
            assert!(seen.insert(reg.abi_name()), "duplicate abi name");
        }
        assert_eq!(seen.len(), 32);
        assert_eq!(Register::new(2).unwrap().abi_name(), "sp");
    }

    #[test]
    fn sys_reg_index_round_trips() {
        for reg in [
            SysReg::Cause,
            SysReg::Epc,
            SysReg::PtBase,
            SysReg::Status,
            SysReg::Scratch,
        ] {
            assert_eq!(SysReg::from_index(reg.index()), Some(reg));
        }
        assert_eq!(SysReg::from_index(999), None);
    }

    #[test]
    fn privilege_mode_reports_privilege() {
        assert!(PrivilegeMode::Kernel.is_privileged());
        assert!(!PrivilegeMode::User.is_privileged());
    }
}
