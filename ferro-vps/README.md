# Ferro-VPS

Máquina virtual / VPS 100% em software, escrita em Rust, capaz de rodar jogos
2D e servidores DENTRO de seus próprios componentes virtuais (CPU, memória,
GPU, armazenamento, rede). O computador hospedeiro apenas RENDERIZA o
framebuffer da VPS e repassa o input.

> **Projeto construído em partes.** Este repositório é desenvolvido
> incrementalmente em mais de 300 partes. Esta é a **Parte 1: a fundação**.
> Nenhuma lógica de máquina virtual está implementada ainda — apenas a
> estrutura, as convenções e o sistema de build sobre os quais todas as
> próximas partes serão construídas.

## Pré-requisitos

- **Rust stable** (toolchain mais recente estável). O canal é fixado em
  `rust-toolchain.toml`, com os componentes `rustfmt` e `clippy`.
- **Linux x86_64 (Ubuntu 22.04+)**, incluindo Ubuntu rodando via **WSL2** no
  Windows. O código evita APIs específicas de Windows.

Instale o Rust (caso ainda não tenha) com o rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Como compilar e validar

Todo o fluxo de qualidade é orquestrado pelo `xtask` (padrão cargo-xtask):

```bash
cargo run -p xtask -- ci
```

O comando `ci` executa, em sequência: `fmt-check`, `lint`, `build` e `test`.
Se qualquer etapa falhar, o processo para e retorna um código de saída
diferente de zero.

### Comandos individuais do xtask

| Comando      | O que faz                                                |
| ------------ | -------------------------------------------------------- |
| `build`      | `cargo build --workspace --all-targets`                  |
| `check`      | `cargo check --workspace --all-targets`                  |
| `fmt`        | `cargo fmt --all`                                        |
| `fmt-check`  | `cargo fmt --all -- --check`                             |
| `lint`       | `cargo clippy --workspace --all-targets -- -D warnings`  |
| `test`       | `cargo test --workspace`                                 |
| `ci`         | roda `fmt-check`, `lint`, `build` e `test` em sequência  |

Exemplo:

```bash
cargo run -p xtask -- build
cargo run -p xtask -- lint
```

## Estrutura de pastas (resumo)

```
ferro-vps/
├── Cargo.toml          # manifesto do workspace
├── rust-toolchain.toml # fixa o canal stable
├── rustfmt.toml        # formatação
├── clippy.toml         # lints
├── docs/               # arquitetura, convenções, dependências, roadmap
├── crates/             # os 15 crates do projeto (ferro-*)
├── xtask/              # automação de build
├── examples/           # jogos e servidores demo (preenchido depois)
└── tests/              # testes de integração de alto nível (depois)
```

Veja [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) para a visão geral das três
camadas (Host / VM / Toolchain+SDK), [docs/CONVENTIONS.md](docs/CONVENTIONS.md)
para as convenções de código, [docs/DEPENDENCIES.md](docs/DEPENDENCIES.md) para
a política de dependências e [docs/ROADMAP.md](docs/ROADMAP.md) para o
planejamento das fases.

## Licença

Distribuído sob a licença MIT. Veja o arquivo [LICENSE](LICENSE).
