# Testes de integração de alto nível

Esta pasta é reservada para testes de integração que cruzam vários crates do
workspace e serão adicionados em partes futuras.

> **Nota técnica:** o Cargo só compila testes de integração que pertencem a um
> pacote (`package`). Como a raiz do workspace é um manifesto virtual (sem
> `[package]`), o smoke test inicial que referencia o crate `ferro-vm` vive em
> `crates/ferro-vm/tests/integration.rs`, garantindo que
> `cargo test --workspace` realmente o execute. Esta pasta passa a hospedar
> harnesses de integração quando o workspace ganhar um pacote agregador
> dedicado.
