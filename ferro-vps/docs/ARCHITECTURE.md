# Arquitetura do Ferro-VPS

> Resumo (português): o Ferro-VPS é uma máquina virtual completa em software.
> Os jogos e servidores são compilados para o bytecode da Ferro VM e executados
> pela CPU virtual. O hospedeiro (host) apenas desenha na tela o framebuffer
> produzido pela GPU virtual e repassa o input. Os recursos consumidos pela
> carga de trabalho são os recursos VIRTUAIS — é isso que torna o Ferro-VPS
> análogo a uma VPS real (além do custo de emulação em si).

## As três camadas

O sistema é composto por três grandes camadas.

### 1. Host (hospedeiro)

Processo nativo que roda no Ubuntu/WSL2. Sua ÚNICA função é:

1. inicializar / "bootar" a máquina virtual;
2. repassar input de teclado e mouse para a máquina virtual;
3. ler o framebuffer produzido pela GPU virtual e exibi-lo numa janela.

O host **não** executa a lógica dos jogos. Crate: `ferro-host`.

### 2. Máquina virtual (a "VPS" / convidado)

Um computador completo em software, com hardware virtual e um pequeno kernel:

- CPU virtual com ISA própria — crates `ferro-isa`, `ferro-cpu`
- Memória + MMU virtual — `ferro-mem`
- Barramento de dispositivos + interrupções — `ferro-bus`
- GPU virtual + rasterizador 2D + framebuffer — `ferro-gpu`
- Armazenamento virtual + sistema de arquivos — `ferro-storage`
- Áudio virtual — `ferro-audio`
- Rede virtual / sockets / pilha TCP-IP simplificada — `ferro-net`
- Kernel do convidado: scheduler, syscalls, ABI — `ferro-kernel`
- Montagem da máquina (liga tudo) — `ferro-vm`

### 3. Toolchain + SDK do convidado

Ferramentas para escrever, montar e carregar programas que rodam DENTRO da VM:

- Assembler + formato de bytecode + loader — `ferro-asm`
- SDK de jogos 2D e de servidores — `ferro-sdk`
- CLI de gerência da VPS — `ferro-cli`

## Diagrama (ASCII)

```
        +-------------------------------------------------------------+
        |                       HOST (nativo)                         |
        |   ferro-host: janela + input + exibe o framebuffer          |
        +----------------------------+--------------------------------+
                     input |         ^ framebuffer
                           v         |
        +-------------------------------------------------------------+
        |                MAQUINA VIRTUAL (convidado)                  |
        |                        ferro-vm                             |
        |                                                             |
        |   ferro-kernel  (scheduler / syscalls / ABI)                |
        |   ferro-cpu  <- ferro-isa                                   |
        |   ferro-mem  ferro-bus  ferro-gpu                           |
        |   ferro-storage  ferro-audio  ferro-net                     |
        +----------------------------+--------------------------------+
                                     ^ bytecode
                                     |
        +-------------------------------------------------------------+
        |              TOOLCHAIN + SDK DO CONVIDADO                    |
        |   ferro-asm (assembler + bytecode + loader)                 |
        |   ferro-sdk (jogos 2D e servidores)                         |
        |   ferro-cli (gerencia a VPS)                                |
        +-------------------------------------------------------------+

  Base compartilhada por TODOS: ferro-common (tipos, erros, log, config)
```

## Princípio-chave

O jogo é compilado para o bytecode da Ferro VM e executado pela CPU virtual. O
host só "desenha na tela" o que a GPU virtual produziu. Os recursos consumidos
são os recursos VIRTUAIS, não os do PC hospedeiro (além do custo de emulação).

## Decisões técnicas fixas

- Rust, edição 2021, toolchain stable.
- Ferro VM de 32 bits, little-endian.
- UTF-8 em todo o projeto.
- Sem `unsafe` nesta fundação (`#![forbid(unsafe_code)]` em todos os crates).
- Modelo de erros baseado em `Result<T, E>` com tipos de erro próprios.

## ISA da Ferro VM (crate `ferro-isa`)

A Parte 5 fixa o contrato da arquitetura de conjunto de instruções (ISA) da
máquina virtual. As decisões abaixo são **definitivas** e justificadas:

- **Tipo RISC, registrador-a-registrador.** Só `load`/`store` acessam memória; o
  resto opera sobre registradores. Isso simplifica o executor e o verificador,
  e torna o caminho de decodificação previsível.
- **Palavra de 32 bits.** É o melhor equilíbrio entre alcance de endereçamento
  (4 GiB) e custo de implementação em software no host. Todos os endereços,
  registradores e imediatos derivam de `Word = u32`.
- **Little-endian.** Combina com as arquiteturas host mais comuns (x86-64,
  ARM64) e é implementado via `from_le_bytes`/`to_le_bytes`, sem `unsafe`.
- **Largura de instrução FIXA de 32 bits (4 bytes).** Toda instrução ocupa
  exatamente uma palavra alinhada a 4. A busca de instrução é trivialmente
  limitada e auto-sincronizante: não existe instrução de tamanho variável que
  um convidado malicioso possa usar para confundir o decodificador. O PC sempre
  avança de 4 em 4; um PC desalinhado é uma falha do convidado.

### Garantias de segurança da decodificação

- A decodificação é `O(1)`: o opcode primário (7 bits) é lido primeiro e
  despachado por um `match` denso.
- `Instruction::decode` **nunca entra em pânico** para nenhuma das 2^32
  entradas possíveis (verificado por teste de fuzzing determinístico).
- Todo campo de registrador tem 5 bits, então sempre cai em `0..=31`.
- Imediatos têm largura fixa e extensão de sinal determinística.
- Para toda palavra válida `w`, vale `decode(w).encode() == w` (round-trip
  total), garantindo que campos reservados não-zero sejam rejeitados.

A tabela canônica de opcodes, os formatos de instrução e a semântica formal de
cada instrução estão documentados em [`docs/ISA.md`](./ISA.md).

## Parte 6 — Memória física do convidado (`ferro-mem`)

A Parte 6 implementa o **substrato físico** de memória sobre o qual a MMU
(tradução de endereços, parte futura) e o barramento de I/O (parte futura)
serão construídos. Aqui o espaço é **físico/efetivo**: os acessos chegam já como
`PhysAddr` e não há tradução.

### Tipos de endereço

- `PhysAddr(u32)` — endereço físico do convidado, com aritmética **checked**
  (`checked_add`); nunca dá wrap silencioso. Helpers: `offset`, `is_aligned`,
  `align_down`, `align_up`.
- `VirtAddr(u32)` — reservado para a fase de MMU; **não** é traduzido nesta
  parte.

### Mapa de memória e regiões

`MemoryMap` materializa, a partir de `&VpsConfig` e das constantes da ISA
(`ROM_BASE`, `RAM_BASE`, `MMIO_BASE`, …), as faixas efetivas e **valida na
construção** que elas são disjuntas e cabem em 32 bits (uma RAM grande demais
que invadiria a janela MMIO é rejeitada). `classify()` mapeia um endereço para
`MemRegion { Rom, Ram, Mmio, Unmapped }`, testando primeiro a região mais quente
(RAM). A área reservada/trap-vector baixa não tem armazenamento nesta parte e
classifica como `Unmapped`.

### Regras de acesso

- **RAM** — leitura e escrita.
- **ROM** — leitura pelo convidado; **escrita do convidado = fault**. Só o host
  grava ROM (via `load_rom`).
- **MMIO** — delegado ao trait `MmioBus`; nunca toca o `Vec` de RAM. O stub
  `NullMmioBus` falha todo acesso com `MemoryAccessViolation` e loga em Debug.
- **Unmapped** — qualquer acesso = `GuestFault::MemoryAccessViolation`.

### API de acesso (`Memory`)

`read_u8/u16/u32`, `write_u8/u16/u32`, `read_bytes`/`write_bytes` e o acesso
genérico por `AccessSize` (`read`/`write`, zero-estendido para `u32`). Todo
acesso: (1) classifica a região, (2) verifica que **o intervalo inteiro** cabe
na **mesma** região mapeada, (3) confere limites **antes** de indexar o `Vec`,
(4) lê/escreve via os helpers little-endian da ISA (`from_le_bytes`/
`to_le_bytes`) ou encaminha ao barramento. Falhas de acesso são `GuestFault` —
**nunca derrubam o host**. A extensão de sinal de `LB`/`LH` é responsabilidade
da CPU.

### Política de alinhamento

Consistente com a Parte 5: acessos de **dados** desalinhados em RAM são
**permitidos** (os helpers LE tratam byte a byte) e registrados em Trace. Um
acesso cujo intervalo cruza a borda de uma região faz fault. O alinhamento de
MMIO é decidido pelo dispositivo; o stub rejeita tudo.

### Barreira host/convidado

O trait `Memory` modela acessos do **convidado** (respeitam permissões e nunca
alcançam a memória do host). Os métodos inerentes `load_rom`, `load_into_ram`,
`fill`, `zero` e `dump_region` são operações do **host**: inicializam/inspecionam
a memória, podem gravar ROM e retornam `FerroError`. O convidado nunca usa essas
APIs. Como `#![forbid(unsafe_code)]` está em vigor, a RAM é um `Vec<u8>` isolado:
não há ponteiros crus nem `transmute`, então o convidado não tem como alcançar a
memória do host.

## Parte 7 — Núcleo da CPU virtual (`ferro-cpu`)

A Parte 7 implementa o **motor de execução** que consome o bytecode decodificado
pela `ferro-isa` e o executa sobre a memória física da `ferro-mem`. Continua sob
`#![forbid(unsafe_code)]` e `#![deny(warnings)]`. Ainda **não** há MMU nem
dispositivos MMIO: a CPU fala diretamente com o trait `Memory`.

### Estado arquitetural (`CpuState`)

Guarda os 32 registradores de uso geral (com `R0` fixo em zero via
`read_reg`/`write_reg`), o `pc`, os `flags`, o modo de privilégio (Kernel no
reset), os registradores de sistema e os contadores `cycle_count`/`instret`,
além do bit `halted`. `reset()` zera tudo, posiciona o `pc` no `RESET_VECTOR` e
volta para Kernel. `R0` é imutável: escritas são descartadas.

### O núcleo (`Cpu`)

Agrega um `CpuState` mais a configuração estática extraída do `VpsConfig`
(orçamento de instruções por frame, intenção de throttle e clock-alvo). A CPU
**não é dona da memória**: todo método que toca o espaço de endereços do
convidado recebe `&mut impl Memory`, e o caminho quente é genérico sobre `M:
Memory` para permitir monomorfização.

### Modelo fetch / decode / execute

1. **fetch** — exige `pc` alinhado a 4 (senão fault), lê 4 bytes little-endian
   via `read_u32`. ROM e RAM são executáveis; MMIO e Unmapped resultam em
   fault, sem nunca derrubar o host.
2. **decode** — `Instruction::decode`; qualquer `DecodeError` vira
   `GuestFault::IllegalInstruction`.
3. **execute** — despacho denso, 100% sem `panic`. Instruções ainda não
   suportadas
   decodificam normalmente mas devolvem um trap controlado
   (`TrapKind::Unimplemented`), logado em Debug.

### Modelo de atualização do `pc`

Quem avança o `pc` é o **step**, não o `execute`. Instrução sequencial deixa
`pc + 4`; um halt deixa `pc + 4` e marca `halted`; um trap deixa o `pc` **sobre**
a instrução que falhou, para um handler futuro inspecioná-la. A ULA **não** mexe
nos flags nesta parte.

### Semântica implementada

Aritmética de 32 bits sempre **wrapping**; imediatos do tipo I são estendidos em
sinal; o valor de deslocamento usa apenas os 5 bits baixos (`& 0x1F`). `SRA`/
`SRAI` são aritméticos (sobre `i32`), `SRL`/`SRLI` lógicos, `SLL`/`SLLI` lógicos.
`SLT`/`SLTI` comparam com sinal, `SLTU`/`SLTIU` sem sinal. `LUI` calcula
`imm << 12`; `AUIPC` calcula `pc + (imm << 12)`, ambos com wrapping. Implementadas
nesta parte: `ADD`, `SUB`, `ADDI`, `AND`, `OR`, `XOR`, `ANDI`, `ORI`, `XORI`,
`SLL`, `SRL`, `SRA`, `SLLI`, `SRLI`, `SRAI`, `SLT`, `SLTU`, `SLTI`, `SLTIU`,
`LUI`, `AUIPC`, `NOP` e o pseudo `NOT`.

### Step e laço com orçamento

`step` executa um fetch/decode/execute, incrementa `cycle_count` e (quando a
instrução se aposenta) `instret`; uma CPU já parada é no-op. `run_budget(max)`
laça até halt, trap, fault ou esgotar o orçamento, devolvendo um `RunResult`
com o motivo da parada (`StopReason`) e quantas instruções se aposentaram — é o
teto anti-DoS para código convidado. Não há throttle de tempo real nesta parte.

### Introspecção e tracing

`get_reg`, `get_pc`, `get_flags`, `cycle_count`, `instret` e `dump_state`
(snapshot `CpuDump` sem efeitos colaterais) expõem o estado para host, debugger
e testes. O gancho de trace fica **desligado por padrão** e, em release, é
compilado para nada a menos que a feature `trace-logs` seja ativada — custo zero
no caminho quente.

### Itens adiados

Loads/stores (`LW`, `SW`, …), saltos e branches (`JAL`, `JALR`, `BEQ`, …) e as
instruções de sistema (`ECALL`, `CSRR`, …) decodificam mas geram
`TrapKind::Unimplemented`; sua execução real chega em partes posteriores, junto
com a integração com o barramento e o kernel do convidado.
