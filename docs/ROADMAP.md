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

* Camada de aplicacao (`crates/rustnf-app`) compartilhada: operacoes, arquivo de configuracao TOML, cofre da senha do
  certificado (Gerenciador de Credenciais do Windows) e pasta de notas com a numeracao da DPS. A linha de comando ja usa
  tudo isso; a interface de desktop vai usar a mesma camada e o mesmo arquivo.

* Interface de desktop (`crates/rustnf-gui`, GPUI, so' Windows): primeiro uso em tres passos, Inicio, Configuracoes
  completas (emitente, Sefin Nacional, certificado e senha no cofre, notas, DANFSe, aliquotas, tema), teste do certificado
  e da conexao com a Sefin, selo do ambiente e estado do certificado sempre visiveis. Usa a mesma camada de aplicacao e o
  mesmo arquivo de configuracao do binario.

* Interface de desktop, **Nova nota**: formulario do caso comum com sugestao pela correlacao, conferencia ao vivo, previa do
  IBS/CBS, rascunho, confirmacao (explicita em producao) e emissao com pedido congelado e reenvio seguro. Testada com um
  emissor de demonstracao: **nenhuma nota real foi emitida por ela**.

* Interface de desktop, **Notas**, **Consultar** e **Previa**: lista da pasta de notas com busca e filtro, detalhe, DANFSe,
  cancelamento, "usar como modelo", consulta pela chave e calculadora avulsa do IBS/CBS. Testadas com o emissor de
  demonstracao: **nenhuma nota real foi emitida, consultada nem cancelada por elas**.

* Entrega: interface compilada e testada no Windows pelo CI; fluxo de release que monta os executaveis (Windows sem
  dependencia do Visual C++ Redistributable, e Linux so' com a linha de comando) e abre a release como rascunho.

## Proximo

1. Interface de desktop: os casos de emissao fora do caso comum (tomador no exterior, intermediario, comercio exterior,
   obra, evento, locacao, deducoes, reembolsos, substituicao, beneficio municipal, suspensao, imunidade e destinatario
   diferente do tomador) seguem pela linha de comando. Faltam: icone do executavel, assinatura de codigo (sem ela o
   Windows avisa no primeiro uso) e a versao de `rustnf-gui` para outros sistemas quando o GPUI tiver janela neles.
2. Cofre do sistema tambem no Linux e no macOS (hoje so' o Windows); aliquotas do arquivo de
   configuracao tambem na API HTTP e no MCP (hoje so' no `calcular` da linha de comando); trava que reserve o numero da
   DPS entre processos (hoje a trava protege a consistencia dos arquivos, nao a reserva do numero).
3. MCP: respostas em fluxo (SSE) e `subscriptions/listen`, se algum cliente precisar; autorizacao OAuth no lugar do
   Bearer fixo; ferramenta de consulta da NFS-e.
4. Trocar o `chrono` (sem mantenedor, RUSTSEC-2026-0543) por uma biblioteca de datas mantida e tirar o `ignore` do `deny.toml`.
5. Teste de ponta a ponta com TLS mutuo contra um servidor local de teste.
6. A cada versao nova do Typst, rever os avisos de seguranca das dependencias dele registrados no `deny.toml` (hoje
   dois do `quick-xml`, num caminho que o DANFSe nao executa, e seis de crates sem mantenedor). O mesmo a cada versao
   nova do `gpui-kit`: hoje cinco crates sem mantenedor (tres deles so' no lado Linux do GPUI, que a interface nao
   compila).

## Depois da v1

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
* cofre de certificados de varios clientes, contas, planos e login (o binario e' de uso proprio, no servidor do usuario;
  o cofre do sistema guarda so' a senha do certificado de quem usa).
