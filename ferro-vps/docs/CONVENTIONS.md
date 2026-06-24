# Convenções de código

Estas convenções valem para TODO o projeto e são aplicadas via `rustfmt`,
`clippy` e atributos no topo de cada crate.

## Nomenclatura

- Tipos e traits: `UpperCamelCase`.
- Funções, variáveis e módulos: `snake_case`.
- Constantes e estáticos: `SCREAMING_SNAKE_CASE`.
- Prefixo de crates: sempre `ferro-`. O nome do módulo de biblioteca não tem o
  prefixo com hífen (ex.: o crate `ferro-cpu` expõe a biblioteca `ferro_cpu`).

## Organização de arquivos e módulos

- Um conceito por arquivo.
- Estilo arquivo-módulo (ex.: `foo.rs` + pasta `foo/`); evita-se `mod.rs`.
- Cada arquivo começa com um doc-comment de módulo (`//!`).
- Cada item público tem um doc-comment (`///`). O lint `missing_docs` exige isso.

## Formatação (rustfmt)

Configuração em `rustfmt.toml`:

```toml
edition = "2021"
max_width = 100
use_small_heuristics = "Default"
```

A formatação é obrigatória. O `xtask` valida com `fmt-check`.

## Lints (clippy)

No topo de cada `lib.rs` / `main.rs`:

```rust
#![forbid(unsafe_code)]
#![deny(warnings)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
```

Os lints `clippy::unwrap_used` e `clippy::expect_used` sinalizam qualquer
`unwrap()`/`expect()`. Como o workspace usa `#![deny(warnings)]`, na prática eles
proíbem `unwrap`/`expect` em código de produção. Testes ficam liberados via
`clippy.toml` (`allow-unwrap-in-tests` e `allow-expect-in-tests`).

- Exceções pontuais a lints pedânticos devem ser feitas **por item** com
  `#[allow(clippy::xxx)]` e um comentário justificando — NUNCA silenciando
  categorias inteiras globalmente.
- Código morto (dead code) só é permitido com `#[allow(dead_code)]` justificado.
  Stubs de partes futuras podem usar esse allow com um comentário como
  `// usado na Parte X`.

### Limiares configurados (`clippy.toml`)

| Chave                            | Valor | Motivo                                  |
| -------------------------------- | ----- | --------------------------------------- |
| `too-many-arguments-threshold`   | 8     | permite construtores de hardware ricos  |
| `cognitive-complexity-threshold` | 30    | tolera laços de emulação não triviais   |
| `type-complexity-threshold`      | 250   | tolera tipos de barramento/registradores|

`doc-valid-idents` adiciona termos do domínio (CPU, GPU, MMU, VPS, etc.) à
lista de identificadores permitidos em documentação, mantendo o padrão do
Clippy via a entrada `".."`.

## Erros

### Filosofia

- **Erros recuperáveis** (esperados em operação normal: arquivo não encontrado,
  endereço inválido do convidado, pacote de rede malformado, ...) usam
  `Result<T, FerroError>` / `FerroResult<T>`. NUNCA causam panic do host.
- **Erros irrecuperáveis do host** (bug de programação: invariante interna
  violada, índice impossível) podem usar panic, mas isso deve ser raro e
  justificado. Prefira retornar `InternalError`/`internal!` quando viável.
- **Erro do convidado ≠ panic do host.** Um programa convidado (jogo/servidor)
  que faz algo ilegal (divisão por zero na CPU virtual, acesso a memória
  protegida, ...) NUNCA pode derrubar o processo host. Isso vira um *fault* do
  convidado, representado por `GuestFault` (erro recuperável) e tratado pelo
  kernel/VM.
- **Regra de ouro:** a fronteira host/convidado é uma barreira de contenção.
  Tudo que vem do convidado é tratado como potencialmente hostil/inválido.

### Hierarquia de tipos

- `FerroError` (em `ferro-common`) é o tipo de erro raiz do host. Tem uma
  variante por domínio (`Config`, `Cpu`, `Memory`, `Bus`, `Gpu`, `Storage`,
  `Audio`, `Network`, `Kernel`, `Vm`, `Asm`, `Host`, `Io`, `Internal`) e a
  variante `Contextualized` para anexar contexto.
- Cada sub-erro deriva `Debug`, implementa `Display` (mensagens acionáveis em
  inglês) e `std::error::Error` (com `source()` quando encapsula outra causa).
- Todos os enums de erro são `#[non_exhaustive]`, permitindo adicionar variantes
  no futuro sem quebrar quem faz `match`.
- `From<SubError> for FerroError` existe para todos os sub-erros, então `?`
  converte automaticamente. `From<std::io::Error>` preserva o `ErrorKind`.
- Sub-erros "pesados" (que carregam `String`) são guardados em `Box<...>` para
  manter `FerroError` pequeno (`size_of::<FerroError>() <= 32` bytes, verificado
  por teste). O caminho de sucesso (`Ok`) nunca aloca.

### Contexto

- O trait `ResultContextExt` adiciona `.context(msg)` e `.with_context(|| msg)`
  a qualquer `Result` cujo erro converta para `FerroError`. A causa original
  NUNCA é descartada: aparece via `source()`.
- Mensagens de contexto são truncadas em 4 KiB para evitar inflar memória/logs
  com entrada maliciosa.
- Macros (exportadas no crate root de `ferro-common`):
  - `internal!(...)` — cria `FerroError::Internal(Invariant { .. })` formatado,
    capturando a localização via `#[track_caller]`.
  - `bail!(...)` — `return Err(internal!(...))`.
  - `ensure!(cond, ...)` — se `cond` for falsa, faz `bail!`.
  - Segurança: nunca passe string vinda do convidado como *format string*; passe
    como argumento de valor (`internal!("token inválido: {token}")`).

### Guest faults

- `GuestFault` é um tipo SEPARADO de `FerroError`, representando falha causada
  pelo programa convidado. NÃO há `From<GuestFault> for FerroError` automático.
- Para escalar um fault como erro de host em contextos específicos, use o
  construtor explícito `VmError::GuestFaulted(GuestFault)`.

### Exit codes (binários)

`ferro_common::error::run(|| { ... })` recebe um `FerroResult<()>`, imprime a
cadeia completa do erro (`error:` + linhas `caused by:` indentadas) em `stderr`
e retorna um `ExitCode` estável por domínio. Não usa panic.

| Domínio                | Código |
| ---------------------- | ------ |
| Sucesso                | 0      |
| Genérico (demais)      | 1      |
| `Config`               | 2      |
| `Io`                   | 3      |
| `Vm`                   | 4      |
| `Internal`             | 70     |

Erros `Contextualized` herdam o código do domínio da causa subjacente.

### Estratégia de panic

- Panic só é permitido para violações de invariantes internas impossíveis em
  execução correta — e, mesmo assim, prefira `internal!`/`InternalError`.
- Proibido `unwrap()`/`expect()` fora de testes (e de inicialização claramente
  fatal e documentada). Aplicado por `clippy::unwrap_used`/`expect_used`.
- Em release o perfil usa `panic = "abort"`: panics são fatais e não desenrolam
  a pilha — mais um motivo para evitar panic em caminhos do convidado.
- Mensagens de panic, quando usadas, devem descrever a invariante violada.

## Logging

O subsistema de logging vive em `ferro-common`, módulo `log`, e é escrito à mão
(sem `log`/`tracing`/`env_logger`).

### Níveis

| Nível   | Quando usar                                                      |
| ------- | --------------------------------------------------------------- |
| `Trace` | passo-a-passo extremamente detalhado (ex.: cada instrução)      |
| `Debug` | diagnóstico útil em desenvolvimento                             |
| `Info`  | eventos normais relevantes (boot da VM, carga de programa)      |
| `Warn`  | algo inesperado, mas recuperável                                |
| `Error` | falha que impede uma operação                                   |

A ordem é significativa: `Trace < Debug < Info < Warn < Error`. O nível global
mínimo padrão é `Info`.

### Macros e convenção de target

- Use `log_trace!`, `log_debug!`, `log_info!`, `log_warn!`, `log_error!`
  (exportadas no crate root de `ferro-common`). O primeiro argumento é sempre um
  `LogTarget`, seguido de um *format string* literal + argumentos.
- Cada crate loga SEMPRE com o seu próprio `LogTarget` (ex.: `ferro-cpu` usa
  `LogTarget::Cpu`). Isso permite filtragem fina por subsistema.
- As macros checam o filtro ANTES de formatar: um `(nível, target)` desabilitado
  custa apenas uma comparação de inteiros e NÃO avalia os argumentos.
- `log_trace!` vira no-op em release, a menos que a feature `trace-logs` de
  `ferro-common` esteja ativa (em debug está sempre ativo). Assim o tracing mais
  verboso tem custo zero em produção por padrão.
- Segurança: nunca passe texto vindo do convidado como *format string*; passe-o
  como argumento de valor. Mensagens são sanitizadas (caracteres de controle e
  sequências `ANSI` são neutralizados) e truncadas (16 KiB) antes de escrever.

### `FERRO_LOG`

Os binários (`ferro-host`, `ferro-cli`) inicializam o logger no início do `main`
lendo a variável de ambiente `FERRO_LOG` sobre os defaults. Sintaxe: lista
separada por vírgulas; um token simples define o nível global, e `target=nível`
define um override por subsistema.

```
FERRO_LOG="info,cpu=warn,gpu=debug"
```

Entradas inválidas (nível ou target desconhecido) produzem `ConfigError`.

### Formato da linha

```
<timestamp> <LEVEL> [<target>] (<file>:<line>) <thread>: <message>
```

- O timestamp é `ISO-8601`-like em UTC (`YYYY-MM-DDTHH:MM:SS.mmmZ`) ou relativo
  (segundos.milis desde o início), conforme a config. Gerado manualmente, sem
  `chrono`/`time`.
- O campo de nível tem largura fixa (5) para leitura em coluna.
- `(file:line)` e `<thread>` são opcionais (controlados pela config;
  `file:line` é ligado por padrão só em builds debug).
- Cores `ANSI` são aplicadas APENAS pelo `StderrSink` quando habilitadas
  (auto-detecção de TTY via `std::io::IsTerminal`). `FileSink`/`MemorySink`
  gravam sempre sem `ANSI`.

### Diretriz anti-spam

- Logue um erro UMA vez, no ponto onde ele é tratado/decidido — não relogue o
  mesmo `FerroError` em cada camada da pilha. Camadas intermediárias preferem
  anexar contexto (`.context(...)`) e propagar com `?`.
- Falhas do convidado em laços quentes devem ser logadas com parcimônia (nível
  apropriado + override por target), para não permitir *log flooding*.

## Segurança de memória

- `#![forbid(unsafe_code)]` em todos os crates nesta fundação. Quando `unsafe`
  for inevitável no futuro, será introduzido em parte específica, isolado e
  documentado com comentários `// SAFETY:`.

## Idioma

- Identificadores e comentários técnicos: inglês.
- Documentos de arquitetura: podem ter resumo em português.

## Sistema de configuração (`ferro_common::config`)

A configuração da VPS vive em `ferro-common` (acessível a todos os crates) e é
descrita por `VpsConfig`, que agrega oito seções: `cpu`, `memory`, `display`,
`storage`, `audio`, `network`, `limits` e `meta`. Toda seção implementa
`Default`, então `VpsConfig::default()` já é uma máquina válida e inicializável.

### Formato de arquivo (`INI`/`TOML`-lite)

- Linhas em branco e linhas iniciadas por `#` são ignoradas.
- `[secao]` seleciona a seção ativa; chaves antes de qualquer seção pertencem a
  `meta` (`config_version`, `vps_name`, `description`).
- `chave = valor`, com espaços tolerados em volta de `=`.
- Valores: inteiros, `true`/`false`, strings entre aspas duplas, ou unidades sem
  aspas (`64MiB`, `8MHz`).
- Unidades de tamanho aceitam binário (`KiB`/`MiB`/`GiB`/`TiB` = 1024ⁿ) e decimal
  (`KB`/`MB`/`GB`/`TB` = 1000ⁿ); frequência aceita `Hz`/`kHz`/`MHz`/`GHz`. O
  sufixo é *case-insensitive* e frações exatas (`3.5MHz`) usam aritmética inteira
  (sem ponto flutuante).

### Carregamento e validação

- `VpsConfig::from_str(text, strict)`, `from_file(path)` e
  `load_or_default(Option<&Path>)` carregam a config; a ausência de arquivo cai
  para o `Default` (com log `Info`). Todo carregamento valida ao final.
- Em modo `strict`, seções e chaves desconhecidas são erro; fora dele, são
  logadas em `Warn` (via `LogTarget::Config`) e ignoradas.
- `validate()` acumula TODOS os problemas numa única `ConfigError::Invalid`, em
  vez de falhar no primeiro. Verifica faixas, `page_size`/`block_size` como
  potência de dois divisora de `ram_size`/`disk_size`, área de tela ≤ teto,
  `channels ∈ {1,2}` e `max_total_memory ≥ ram_size`.
- `to_config_string()` serializa de volta e faz *round-trip* exato
  (`from_str(&x.to_config_string(), true) == x`).

### Anti-exploit

- Documentos têm teto de tamanho (1 `MiB`) e de número de linhas (100_000).
- Strings têm comprimento máximo e rejeitam caracteres de controle; caminhos não
  podem ser vazios.
- Todo cálculo numérico usa `checked_*`/`saturating_*` — overflow é rejeitado,
  nunca silencioso.
- O parser é puramente declarativo: não inclui outros arquivos, não expande
  variáveis de ambiente e não executa comandos. `network.loopback_only = true` é
  o padrão (acesso externo é *opt-in*).
