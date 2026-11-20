# Roadmap

## Feito

* Biblioteca: `reforma`, `nfse` (DPS, eventos, leitura, Lista Nacional, correlacao) e `assinatura`, com os contratos
  em `tests/casos/`.
* Cliente da Sefin Nacional (feature `sefin`), testado com transporte falso.
* DANFSe em PDF (feature `danfse`, NT 008): conteudo, leiaute e pagina conferidos por contrato em `tests/casos/danfse.json`.
* Previa do IBS/CBS do servico (`reforma::previa`): base de calculo e tributos, com os casos de teste da calculadora
  oficial como contrato. Fora dela, com recusa explicita: compra governamental, tributacao regular e cClassTrib de
  aliquota fixa ou uniforme setorial sem a aliquota informada.
* Teste de higiene (`tests/higiene.rs`), CI e `cargo deny`.

* Binario `rustnf` (`crates/rustnf-cli`): linha de comando, API HTTP e servidor MCP (Streamable HTTP, JSON-RPC 2.0,
  especificacao 2026-07-28 com compatibilidade para `initialize` da 2025-11-25).

## Proximo

1. MCP: respostas em fluxo (SSE) e `subscriptions/listen`, se algum cliente precisar; autorizacao OAuth no lugar do
   Bearer fixo; ferramenta de consulta da NFS-e.
2. Trocar o `chrono` (sem mantenedor, RUSTSEC-2026-0543) por uma biblioteca de datas mantida e tirar o `ignore` do `deny.toml`.
3. Teste de ponta a ponta com TLS mutuo contra um servidor local de teste.
4. A cada versao nova do Typst, rever os avisos de seguranca das dependencias dele registrados no `deny.toml` (hoje
   dois do `quick-xml`, num caminho que o DANFSe nao executa, e seis de crates sem mantenedor).

## Depois da v1

* Interface visual de desktop, em Rust (GPUI): montar e conferir a DPS, ver a previa do IBS/CBS, emitir, consultar,
  cancelar e abrir o DANFSe, usando a mesma biblioteca do binario.
* Motor de PDF do DANFSe com menos dependencias. Hoje o PDF sai pelo Typst (cerca de 280 crates). A pagina ja e'
  calculada pela biblioteca como uma lista de operacoes de desenho (texto medido, linhas, imagem), entao da para
  escreve-la direto com uma biblioteca de PDF (`krilla`, a que o proprio Typst usa por baixo, cerca de 90 crates) ou,
  no limite, com `pdf-writer` (meia duzia de crates, com a fonte e a imagem embutidas a mao). O contrato de
  `tests/casos/danfse.json` nao muda; muda so' quem transforma a pagina em PDF.

## Fora da v1

* Rotas do ADN (distribuicao de DF-e, parametros municipais): o cliente cobre so a Sefin Nacional;
* NFS-e municipal (ABRASF), NF-e, NFC-e e CT-e, inclusive o calculo do IBS/CBS de mercadoria;
* assinatura ICP de PDF;
* apuracao por periodo e DARF de CBS/IBS;
* cofre de certificados, contas, planos e login (o binario e' de uso proprio, no servidor do usuario).
