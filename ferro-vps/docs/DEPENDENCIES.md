# Política de dependências

## Regra de ouro: NADA PAGO

- SOMENTE dependências gratuitas e open-source. É PROIBIDO qualquer crate,
  serviço, SDK ou recurso pago, "freemium pago", trial, ou que exija pagamento
  direto ou indireto (chave de API paga, licença comercial, etc.).
- Nenhum acesso de rede é feito durante o build. Nenhum `build.rs` pode baixar
  ou executar código externo.

## Licenças permitidas

MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, MPL-2.0, ISC, Zlib, Unicode-DFS.

Licenças copyleft fortes (GPL/AGPL) devem ser **EVITADAS** para não contaminar
o licenciamento do projeto. Se uma dependência (ou uma transitiva) realmente
exigir GPL/AGPL/comercial, **pare e peça confirmação humana** antes de incluir,
e documente a decisão aqui.

## Processo para adicionar uma dependência

1. Preferir a biblioteca padrão (`std`) sempre que possível.
2. Adicionar a versão em `[workspace.dependencies]` no `Cargo.toml` raiz
   (versões fixas/compatíveis), e referenciar no crate com
   `dep = { workspace = true }`.
3. Versionar o `Cargo.lock` (builds reprodutíveis; evita atualização maliciosa
   silenciosa e "dependency confusion").
4. Registrar a dependência na tabela abaixo.

## Dependências externas registradas

| Nome | Versão | Licença | Motivo de uso | Crate(s) que usam |
| ---- | ------ | ------- | ------------- | ----------------- |
| _(nenhuma)_ | — | — | Até a Parte 3 não há dependências de runtime externas. O subsistema de logging (Parte 3) usa apenas a `std`: a detecção de TTY para cores `ANSI` usa `std::io::IsTerminal` (estável desde o Rust 1.70; o workspace exige 1.74), sem crate externa. | — |

## Dependências internas (crates do workspace)

Estas não são dependências externas, mas o grafo interno é mantido sem ciclos:

```
ferro-common  -> (nenhum crate interno)
ferro-isa     -> ferro-common
ferro-mem     -> ferro-common
ferro-bus     -> ferro-common
ferro-cpu     -> ferro-common, ferro-isa, ferro-mem, ferro-bus
ferro-gpu     -> ferro-common, ferro-bus, ferro-mem
ferro-storage -> ferro-common, ferro-bus
ferro-audio   -> ferro-common, ferro-bus
ferro-net     -> ferro-common, ferro-bus
ferro-kernel  -> ferro-common, ferro-cpu, ferro-mem, ferro-bus, ferro-storage, ferro-net
ferro-vm      -> todos os crates de componentes acima
ferro-asm     -> ferro-common, ferro-isa
ferro-sdk     -> ferro-common, ferro-isa
ferro-host    -> ferro-common, ferro-vm
ferro-cli     -> ferro-common, ferro-vm, ferro-asm
xtask         -> (apenas std)
```
