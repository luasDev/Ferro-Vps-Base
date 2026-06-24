//! `ferro-net`: the virtual network stack.
//!
//! Part of the `Ferro-VPS` project. This crate currently exposes only a
//! placeholder symbol; real functionality arrives in a later part.

#![forbid(unsafe_code)]
#![deny(warnings)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]

/// Placeholder item that keeps the crate compilable and exposes a stable
/// public symbol until real functionality is implemented.
pub fn placeholder() {}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_runs() {
        super::placeholder();
    }
}
