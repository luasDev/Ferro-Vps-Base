# Ferro VM — Especificação da ISA

Este documento é a **fonte canônica** da arquitetura de conjunto de instruções
(ISA) da Ferro VM, implementada (como contrato) no crate `ferro-isa`. Ele cobre
o modelo de dados, registradores, privilégio, formatos de instrução, flags, a
tabela de opcodes e a semântica formal de cada instrução.

> Convenções: `rd` = registrador destino, `rs1`/`rs2` = registradores fonte,
> `imm` = imediato (já com extensão de sinal salvo indicação), `sext(x)` =
> extensão de sinal, `zext(x)` = extensão com zero, `pc` = contador de programa.
> Toda aritmética é em 32 bits com *wrapping* (módulo 2^32) salvo indicação.

## 1. Modelo de dados

- Palavra: `Word = u32`; interpretação assinada: `SWord = i32`.
- Endianness: **little-endian** (`ENDIANNESS = Endianness::Little`).
- Tamanhos de acesso: `AccessSize::{Byte=1, Half=2, Word=4}`.
- Instruções têm **largura fixa de 32 bits** e devem estar alinhadas a 4 bytes.
  Um `pc` desalinhado (não múltiplo de 4) é uma falha do convidado.
- Acessos de dados desalinhados são permitidos pela ISA (a política fica a
  cargo da unidade de memória).

## 2. Registradores

32 registradores de propósito geral `R0..R31`, todos de 32 bits. `R0` é
*hardwired* a zero: leituras retornam `0`, escritas são descartadas.

| Reg | ABI | Papel |
| --- | --- | --- |
| R0 | zr | Constante zero (hardwired) |
| R1 | ra | Endereço de retorno |
| R2 | sp | Ponteiro de pilha |
| R3 | fp | Ponteiro de quadro |
| R4–R11 | a0–a7 | Argumentos / retorno |
| R12–R27 | t0–t15 | Temporários / salvos |
| R28–R31 | k0–k3 | Reservados para o kernel |

Além deles: `PC` (sempre múltiplo de 4) e `FLAGS` (ver §5).

Registradores de sistema (`SysReg`, privilegiados, índice estável): `cause` (0),
`epc` (1), `ptbase` (2), `status` (3), `scratch` (4). O conjunto é
`#[non_exhaustive]`.

## 3. Privilégio

Dois modos: `User` e `Kernel`. Instruções privilegiadas (marcadas na tabela)
só executam em `Kernel`; em `User` causam *trap*. A aplicação do *trap* é da
fase do kernel — aqui o privilégio é apenas metadado.

## 4. Formatos de instrução (32 bits, largura fixa)

Bits `[6:0]` são sempre o opcode primário (≤ 128 valores).

| Formato | Layout dos bits |
| --- | --- |
| R | `[6:0] op` `[11:7] rd` `[16:12] rs1` `[21:17] rs2` `[31:22] funct(10)` |
| I | `[6:0] op` `[11:7] rd` `[16:12] rs1` `[31:17] imm15 (sinal)` |
| S | `[6:0] op` `[11:7] rs2` `[16:12] rs1` `[31:17] imm15 (sinal)` |
| B | `[6:0] op` `[11:7] rs1` `[16:12] rs2` `[31:17] imm15 (sinal, ×4)` |
| U | `[6:0] op` `[11:7] rd` `[31:12] imm20` |
| J | `[6:0] op` `[11:7] rd` `[31:12] imm20 (×4)` |

O imediato de 15 bits cobre `-16384..=16383`; o de 20 bits, `-524288..=524287`.
Em B e J o imediato é contado em **palavras de instrução**: o deslocamento em
bytes é `imm * 4`, relativo ao `pc`.

## 5. Registrador FLAGS

Bitmask manual sobre `u32` (sem dependência externa):

| Bit | Nome | Significado |
| --- | --- | --- |
| 0 | Z | Resultado zero |
| 1 | N | Resultado negativo (bit 31) |
| 2 | C | Carry / borrow |
| 3 | V | Overflow assinado |
| 4–31 | — | Reservados (sempre preservados) |

Ao estilo RISC, as flags **não** são atualizadas implicitamente por toda
operação da ALU; só instruções específicas (na fase da CPU) as escrevem. Bits
reservados nunca são alterados pelos setters.

## 6. Tabela canônica de opcodes

`op` é o opcode primário; `funct` aplica-se a `R` e ao grupo de sistema.
Div/Rem por zero é `GuestFault::DivideByZero`; `INT_MIN / -1` é definido como
*wrapping* (`INT_MIN`, resto `0`). `NOP` é o pseudo-código de `ADDI r0, r0, 0`.

| Mnem. | op | Fmt | funct | Priv | Semântica |
| --- | --- | --- | --- | --- | --- |
| add | 0x01 | R | 0x000 | não | rd = rs1 + rs2 |
| sub | 0x01 | R | 0x001 | não | rd = rs1 - rs2 |
| and | 0x01 | R | 0x002 | não | rd = rs1 & rs2 |
| or | 0x01 | R | 0x003 | não | rd = rs1 \| rs2 |
| xor | 0x01 | R | 0x004 | não | rd = rs1 ^ rs2 |
| sll | 0x01 | R | 0x005 | não | rd = rs1 << (rs2 & 31) |
| srl | 0x01 | R | 0x006 | não | rd = rs1 >>u (rs2 & 31) |
| sra | 0x01 | R | 0x007 | não | rd = rs1 >>s (rs2 & 31) |
| slt | 0x01 | R | 0x008 | não | rd = (rs1 <s rs2) ? 1 : 0 |
| sltu | 0x01 | R | 0x009 | não | rd = (rs1 <u rs2) ? 1 : 0 |
| mul | 0x01 | R | 0x00A | não | rd = (rs1 * rs2)[31:0] |
| mulh | 0x01 | R | 0x00B | não | rd = (rs1 *s rs2)[63:32] |
| div | 0x01 | R | 0x00C | não | rd = rs1 /s rs2 |
| divu | 0x01 | R | 0x00D | não | rd = rs1 /u rs2 |
| rem | 0x01 | R | 0x00E | não | rd = rs1 %s rs2 |
| remu | 0x01 | R | 0x00F | não | rd = rs1 %u rs2 |
| not | 0x01 | R | 0x010 | não | rd = !rs1 (rs2 = 0) |
| addi | 0x02 | I | — | não | rd = rs1 + sext(imm) |
| andi | 0x03 | I | — | não | rd = rs1 & sext(imm) |
| ori | 0x04 | I | — | não | rd = rs1 \| sext(imm) |
| xori | 0x05 | I | — | não | rd = rs1 ^ sext(imm) |
| slti | 0x06 | I | — | não | rd = (rs1 <s sext(imm)) ? 1 : 0 |
| sltiu | 0x07 | I | — | não | rd = (rs1 <u sext(imm)) ? 1 : 0 |
| slli | 0x08 | I | — | não | rd = rs1 << (imm & 31) |
| srli | 0x09 | I | — | não | rd = rs1 >>u (imm & 31) |
| srai | 0x0A | I | — | não | rd = rs1 >>s (imm & 31) |
| jalr | 0x0B | I | — | não | rd = pc+4; pc = (rs1 + sext(imm)) & ~1 |
| lb | 0x10 | I | — | não | rd = sext8(mem8(rs1 + sext(imm))) |
| lbu | 0x11 | I | — | não | rd = zext8(mem8(rs1 + sext(imm))) |
| lh | 0x12 | I | — | não | rd = sext16(mem16(rs1 + sext(imm))) |
| lhu | 0x13 | I | — | não | rd = zext16(mem16(rs1 + sext(imm))) |
| lw | 0x14 | I | — | não | rd = mem32(rs1 + sext(imm)) |
| sb | 0x18 | S | — | não | mem8(rs1 + sext(imm)) = rs2[7:0] |
| sh | 0x19 | S | — | não | mem16(rs1 + sext(imm)) = rs2[15:0] |
| sw | 0x1A | S | — | não | mem32(rs1 + sext(imm)) = rs2 |
| beq | 0x20 | B | — | não | if rs1 == rs2: pc += sext(imm)*4 |
| bne | 0x21 | B | — | não | if rs1 != rs2: pc += sext(imm)*4 |
| blt | 0x22 | B | — | não | if rs1 <s rs2: pc += sext(imm)*4 |
| bge | 0x23 | B | — | não | if rs1 >=s rs2: pc += sext(imm)*4 |
| bltu | 0x24 | B | — | não | if rs1 <u rs2: pc += sext(imm)*4 |
| bgeu | 0x25 | B | — | não | if rs1 >=u rs2: pc += sext(imm)*4 |
| lui | 0x28 | U | — | não | rd = imm << 12 |
| auipc | 0x29 | U | — | não | rd = pc + (imm << 12) |
| jal | 0x2C | J | — | não | rd = pc+4; pc += sext(imm)*4 |
| ecall | 0x30 | R | 0x000 | não | trap para o kernel (syscall) |
| ebreak | 0x30 | R | 0x001 | não | trap para o depurador |
| halt | 0x30 | R | 0x002 | **sim** | para o processador |
| sret | 0x30 | R | 0x003 | **sim** | retorna de um trap |
| csrr | 0x31 | I | — | **sim** | rd = sysreg[imm15]; rs1 = 0 |
| csrw | 0x32 | I | — | **sim** | sysreg[imm15] = rs1; rd = 0 |

Opcodes `0x40` em diante são **reservados** para expansão futura e decodificam
como `IllegalOpcode`. Não há colisão de par `(opcode, funct)` na tabela
(garantido por teste).

## 7. Semântica formal (pseudocódigo)

Regras gerais:

- Aritmética em 32 bits com *wrapping*.
- Deslocamentos usam apenas os 5 bits inferiores da contagem (`& 31`).
- `>>u` é deslocamento lógico; `>>s` é aritmético.
- Cargas estendem o valor lido conforme o sufixo (`b`/`h` com sinal, `bu`/`hu`
  com zero); armazenamentos truncam para o tamanho.
- Divisão trunca em direção a zero; resto tem o sinal do dividendo.

```
ADD   rd, rs1, rs2 : rd = wrapping(rs1 + rs2)
SUB   rd, rs1, rs2 : rd = wrapping(rs1 - rs2)
AND   rd, rs1, rs2 : rd = rs1 & rs2
OR    rd, rs1, rs2 : rd = rs1 | rs2
XOR   rd, rs1, rs2 : rd = rs1 ^ rs2
SLL   rd, rs1, rs2 : rd = rs1 << (rs2 & 31)
SRL   rd, rs1, rs2 : rd = (u32)rs1 >> (rs2 & 31)
SRA   rd, rs1, rs2 : rd = (i32)rs1 >> (rs2 & 31)
SLT   rd, rs1, rs2 : rd = ((i32)rs1 < (i32)rs2) ? 1 : 0
SLTU  rd, rs1, rs2 : rd = ((u32)rs1 < (u32)rs2) ? 1 : 0
MUL   rd, rs1, rs2 : rd = (u32)((i64)rs1 * (i64)rs2)        ; 32 bits baixos
MULH  rd, rs1, rs2 : rd = (u32)(((i64)rs1 * (i64)rs2) >> 32) ; 32 bits altos
DIV   rd, rs1, rs2 : if rs2 == 0 -> GuestFault::DivideByZero
                     elif rs1 == INT_MIN && rs2 == -1 -> rd = INT_MIN
                     else rd = (i32)rs1 / (i32)rs2
DIVU  rd, rs1, rs2 : if rs2 == 0 -> GuestFault::DivideByZero
                     else rd = (u32)rs1 / (u32)rs2
REM   rd, rs1, rs2 : if rs2 == 0 -> GuestFault::DivideByZero
                     elif rs1 == INT_MIN && rs2 == -1 -> rd = 0
                     else rd = (i32)rs1 % (i32)rs2
REMU  rd, rs1, rs2 : if rs2 == 0 -> GuestFault::DivideByZero
                     else rd = (u32)rs1 % (u32)rs2
NOT   rd, rs1      : rd = !rs1
ADDI  rd, rs1, imm : rd = wrapping(rs1 + sext(imm))
ANDI/ORI/XORI      : rd = rs1 <op> sext(imm)
SLTI/SLTIU         : rd = (rs1 <op> sext(imm)) ? 1 : 0
SLLI/SRLI/SRAI     : rd = rs1 <shift> (imm & 31)
JALR  rd, rs1, imm : t = (rs1 + sext(imm)) & ~1 ; rd = pc + 4 ; pc = t
LB/LBU/LH/LHU/LW   : ea = rs1 + sext(imm) ; rd = ext(mem[ea])
SB/SH/SW           : ea = rs1 + sext(imm) ; mem[ea] = trunc(rs2)
BEQ..BGEU          : if cond(rs1, rs2): pc = pc + sext(imm) * 4 else pc += 4
LUI   rd, imm      : rd = (u32)imm << 12
AUIPC rd, imm      : rd = pc + ((u32)imm << 12)
JAL   rd, imm      : rd = pc + 4 ; pc = pc + sext(imm) * 4
ECALL / EBREAK     : levanta o trap correspondente
HALT               : interrompe a execução (privilegiada)
SRET               : restaura o estado salvo e retorna do trap (privilegiada)
CSRR  rd, sysreg   : rd = sysreg[idx]            (privilegiada)
CSRW  rs1, sysreg  : sysreg[idx] = rs1           (privilegiada)
```

Qualquer acesso fora de toda região mapeada (ver §8) é
`GuestFault::MemoryAccessViolation`.

## 8. Mapa de memória

Espaço físico plano de 32 bits (little-endian). Limites canônicos:

| Região | Início | Tamanho | Notas |
| --- | --- | --- | --- |
| Reservada / vetores de trap | 0x0000_0000 | 0x0000_1000 | `TRAP_VECTOR_BASE` |
| ROM de boot (somente leitura) | 0x0000_1000 | 0x0000_F000 | `RESET_VECTOR` = 0x1000 |
| RAM principal | 0x1000_0000 | `ram_size` (config) | base `RAM_BASE` |
| Janela MMIO | 0xF000_0000 | 0x0100_0000 | `MMIO_BASE` / `MMIO_SIZE` |

No reset, `pc = RESET_VECTOR`. As regiões não se sobrepõem
(`RAM_BASE < MMIO_BASE`). A paginação/MMU é tema da fase de memória; aqui
descreve-se o espaço físico/efetivo visto pelo convidado.

## 9. Notas anti-exploração

- O decodificador nunca entra em pânico, nunca aloca e nunca chama código
  inseguro (`from_le_bytes`/`to_le_bytes`, sem `transmute`).
- Campos de registrador de 5 bits estão sempre em `0..=31`.
- Imediatos têm largura fixa e extensão de sinal determinística.
- Instruções privilegiadas estão marcadas; sua aplicação ocorre na CPU/kernel.
- O acesso à memória é limitado pelos limites rígidos do mapa acima.
