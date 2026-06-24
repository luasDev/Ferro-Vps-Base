//! `ferro-host`: the host process that runs a `Ferro-VPS` virtual machine.
//!
//! Part of the `Ferro-VPS` project. This binary currently performs no VM work;
//! it wires up the shared error-reporting entry point
//! ([`ferro_common::error::run`]) and initializes the global logger from the
//! `FERRO_LOG` environment variable. Any
//! [`FerroError`](ferro_common::error::FerroError) returned by the entry point
//! is printed as a `caused by:` chain and mapped to a stable exit code.

#![forbid(unsafe_code)]
#![deny(warnings)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]

use std::process::ExitCode;

use ferro_common::error::run;
use ferro_common::log::{self, LogConfig, LogTarget};
use ferro_common::log_info;

fn main() -> ExitCode {
    run(|| {
        log::init(&LogConfig::from_env()?)?;
        log_info!(LogTarget::Host, "ferro-host started");
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use ferro_common::log::{self, LogConfig};

    #[test]
    fn logging_initializes() {
        assert!(log::init(&LogConfig::default()).is_ok());
    }
}
