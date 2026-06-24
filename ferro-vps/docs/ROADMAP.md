# Roadmap do Ferro-VPS

O projeto é gigante (300+ partes) e construído incrementalmente. Abaixo estão
as **fases** planejadas (apenas títulos). As partes individuais de cada fase
serão preenchidas conforme avançarmos. Cada parte só implementa o que está
explicitamente descrito nela.

- **Fase 0 — Fundação do projeto** *(Parte 1, ATUAL)*
  Workspace Cargo, estrutura de pastas/módulos, convenções, política de
  dependências, sistema de build (`xtask`) e documentação base.
- **Fase 1 — Erros e logging**
  Implementação completa de `ferro-common::error` e `ferro-common::log`.
- **Fase 2 — ISA**
  Definição da arquitetura de instruções e opcodes (`ferro-isa`).
- **Fase 3 — Memória e MMU** (`ferro-mem`).
- **Fase 4 — Barramento e interrupções** (`ferro-bus`).
- **Fase 5 — CPU virtual** (`ferro-cpu`).
- **Fase 6 — GPU, rasterizador 2D e framebuffer** (`ferro-gpu`).
- **Fase 7 — Armazenamento e filesystem** (`ferro-storage`).
- **Fase 8 — Áudio virtual** (`ferro-audio`).
- **Fase 9 — Rede virtual** (`ferro-net`).
- **Fase 10 — Kernel do convidado** (`ferro-kernel`).
- **Fase 11 — Montagem da máquina** (`ferro-vm`).
- **Fase 12 — Host: janela, input e exibição** (`ferro-host`).
- **Fase 13 — Assembler, bytecode e loader** (`ferro-asm`).
- **Fase 14 — SDK do convidado (jogos 2D e servidores)** (`ferro-sdk`).
- **Fase 15 — CLI de gerência** (`ferro-cli`).
- **Fase 16 — Exemplos: jogos e servidores demo** (`examples/`).
- **Fase 17 — Otimização, profiling e hardening**.

> Observação: a ordem das fases segue o grafo de dependências internas, de modo
> que cada camada é construída sobre fundações já testadas.
