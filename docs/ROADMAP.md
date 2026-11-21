# Roadmap

## Feito

* Biblioteca: `reforma`, `nfse` (DPS, eventos, leitura, Lista Nacional, correlação) e `assinatura`, com os contratos
  em `tests/casos/`.
* Cliente da Sefin Nacional (feature `sefin`), testado com transporte falso.
* DANFSe em PDF (feature `danfse`, NT 008): conteúdo, leiaute e página conferidos por contrato em `tests/casos/danfse.json`.
* Motor de PDF do DANFSe com menos dependências: a página, que a biblioteca já calcula como lista de operações de
  desenho, passa a ser escrita direto em PDF com o `krilla` (sem o Typst, sem fonte do sistema, mesma entrada gera os
  mesmos bytes). A árvore de dependências da biblioteca com `sefin` e `danfse` caiu de 387 para 207 crates.
* Prévia do IBS/CBS do serviço (`reforma::previa`): base de cálculo e tributos, com os casos de teste da calculadora
  oficial como contrato. Fora dela, com recusa explícita: compra governamental, tributação regular e cClassTrib de
  alíquota fixa ou uniforme setorial sem a alíquota informada.
* Teste de higiene (`tests/higiene.rs`), CI e `cargo deny`.

* Binário `rustnf` (`crates/rustnf-cli`): linha de comando, API HTTP e servidor MCP (Streamable HTTP, JSON-RPC 2.0,
  especificação 2026-07-28 com compatibilidade para `initialize` da 2025-11-25).

* Camada de aplicação (`crates/rustnf-app`) compartilhada: operações, arquivo de configuração TOML, cofre da senha do
  certificado (Gerenciador de Credenciais do Windows) e pasta de notas com a numeração da DPS. A linha de comando já usa
  tudo isso; a interface de desktop vai usar a mesma camada e o mesmo arquivo.

* Interface de desktop (`crates/rustnf-gui`, GPUI, só Windows): primeiro uso em três passos, Início, Configurações
  completas (emitente, Sefin Nacional, certificado e senha no cofre, notas, DANFSe, alíquotas, tema), teste do certificado
  e da conexão com a Sefin, selo do ambiente e estado do certificado sempre visíveis. Usa a mesma camada de aplicação e o
  mesmo arquivo de configuração do binário.

* Interface de desktop, **Nova nota**: formulário do caso comum com sugestão pela correlação, conferência ao vivo, prévia do
  IBS/CBS, rascunho, confirmação (explícita em produção) e emissão com pedido congelado e reenvio seguro. Testada com um
  emissor de demonstração: **nenhuma nota real foi emitida por ela**.

* Interface de desktop, **Notas**, **Consultar** e **Prévia**: lista da pasta de notas com busca e filtro, detalhe, DANFSe,
  cancelamento, "usar como modelo", consulta pela chave e calculadora avulsa do IBS/CBS. Testadas com o emissor de
  demonstração: **nenhuma nota real foi emitida, consultada nem cancelada por elas**.

* Entrega: interface compilada e testada no Windows pelo CI; fluxo de release que monta os executáveis (Windows sem
  dependência do Visual C++ Redistributable, e Linux só com a linha de comando) e abre a release como rascunho.

## Próximo

1. Interface de desktop: os casos de emissão fora do caso comum (tomador no exterior, intermediário, comércio exterior,
   obra, evento, locação, deduções, reembolsos, substituição, benefício municipal, suspensão, imunidade e destinatário
   diferente do tomador) seguem pela linha de comando. Faltam: ícone do executável, assinatura de código (sem ela o
   Windows avisa no primeiro uso) e a versão de `rustnf-gui` para outros sistemas quando o GPUI tiver janela neles.
2. Cofre do sistema também no Linux e no macOS (hoje só o Windows); alíquotas do arquivo de
   configuração também na API HTTP e no MCP (hoje só no `calcular` da linha de comando); trava que reserve o número da
   DPS entre processos (hoje a trava protege a consistência dos arquivos, não a reserva do número).
3. MCP: respostas em fluxo (SSE) e `subscriptions/listen`, se algum cliente precisar; autorização OAuth no lugar do
   Bearer fixo; ferramenta de consulta da NFS-e.
4. Trocar o `chrono` (sem mantenedor, RUSTSEC-2026-0543) por uma biblioteca de datas mantida e tirar o `ignore` do `deny.toml`.
5. Teste de ponta a ponta com TLS mútuo contra um servidor local de teste.
6. A cada versão nova do `gpui-kit`, rever os oito crates sem mantenedor que ele traz e que estão registrados no
   `deny.toml` (três deles só no lado Linux do GPUI, que a interface não compila).

## Fora da v1

* Rotas do ADN (distribuição de DF-e, parâmetros municipais): o cliente cobre só a Sefin Nacional;
* NFS-e municipal (ABRASF), NF-e, NFC-e e CT-e, inclusive o cálculo do IBS/CBS de mercadoria;
* assinatura ICP de PDF;
* apuração por período e DARF de CBS/IBS;
* cofre de certificados de vários clientes, contas, planos e login (o binário é de uso próprio, no servidor do usuário;
  o cofre do sistema guarda só a senha do certificado de quem usa).
