//! High level integration smoke test for the assembled virtual machine.
//!
//! It verifies that the `ferro-vm` crate can be referenced and linked from an
//! integration test target, exercising `cargo test --workspace` end-to-end.

#[test]
fn ferro_vm_is_referenceable() {
    ferro_vm::placeholder();
}
