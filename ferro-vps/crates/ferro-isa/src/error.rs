//! Decode errors for the Ferro VM ISA and their conversion into the host
//! [`FerroError`].

use ferro_common::error::{CpuError, FerroError};

/// An error produced while decoding or validating an instruction word.
///
/// When a *guest* program executes a malformed word the CPU raises
/// [`GuestFault::IllegalInstruction`](ferro_common::error::GuestFault), which
/// deliberately never converts into a [`FerroError`]. This type is for
/// *host-side* tooling (the assembler, a debugger, the program loader) that
/// needs to surface a decode failure through the normal host error channel.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodeError {
    /// The instruction word does not match any known opcode/funct combination,
    /// or a reserved field was not zero.
    IllegalOpcode {
        /// The raw 32-bit instruction word that failed to decode.
        word: u32,
    },
    /// A register index outside the valid `0..=31` range was supplied.
    InvalidRegister {
        /// The offending register index.
        index: u8,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::IllegalOpcode { word } => {
                write!(f, "illegal instruction word {word:#010x}")
            }
            Self::InvalidRegister { index } => {
                write!(f, "invalid register index {index}")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

impl From<DecodeError> for FerroError {
    fn from(value: DecodeError) -> Self {
        CpuError::decode(value.to_string()).into()
    }
}

#[cfg(test)]
mod tests {
    use super::DecodeError;
    use ferro_common::error::FerroError;

    #[test]
    fn illegal_opcode_displays_hex_word() {
        let error = DecodeError::IllegalOpcode { word: 0xDEAD_BEEF };
        assert_eq!(error.to_string(), "illegal instruction word 0xdeadbeef");
    }

    #[test]
    fn converts_into_ferro_error() {
        let error: FerroError = DecodeError::IllegalOpcode { word: 0 }.into();
        assert!(matches!(error, FerroError::Cpu(_)));
    }
}
