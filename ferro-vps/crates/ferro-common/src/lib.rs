//! `ferro-common`: shared types, error handling, logging, and configuration
//! for the `Ferro-VPS` project.
//!
//! This crate is the common foundation that every other crate depends on. It
//! provides the error subsystem (see [`error`]), the logging facility (see
//! [`log`]), and the configuration system (see [`config`]).
//!
//! Everything is built without any external dependencies. See
//! `docs/CONVENTIONS.md` for the full error philosophy, exit-code mapping,
//! panic strategy, and the configuration format reference.

#![forbid(unsafe_code)]
#![deny(warnings)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]

pub mod config;
pub mod error;
pub mod log;

mod macros;

pub use config::VpsConfig;
pub use error::{FerroError, FerroResult, GuestFault};

#[cfg(test)]
mod tests {
    use crate::error::FerroResult;

    #[test]
    fn result_alias_is_usable() {
        let value: FerroResult<u32> = Ok(7);
        assert!(value.is_ok());
    }
}
