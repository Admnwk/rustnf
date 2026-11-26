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
  certificado (Gerenciador de Credenciais do Windows) e pasta de notas com a numeração da DPS. A linha de comando, a
  API HTTP, o MCP e a interface de desktop usam a mesma camada e o mesmo arquivo: emitente, numeração, pasta de notas
  e alíquotas valem igual nas quatro.

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

* Consulta de DPS, consulta de evento, registro de eventos do contribuinte (como o pedido de análise fiscal, 101103) e
  conferência em lote da situação das notas contra a Sefin (cancelada, substituída, em análise fiscal), na camada de
  aplicação, na linha de comando, na API HTTP e no MCP, testadas com transporte falso: **nenhuma consulta nem evento real
  foi feito**.

* Interface de desktop, eventos na tela: conferir a situação das notas em lote (com andamento e Parar), histórico de
  eventos de uma nota, pedido de análise fiscal do cancelamento, consulta pela série e número da DPS e, para a nota
  recebida, confirmar e recusar. Testada com a demonstração e a janela de teste: **nenhuma consulta nem evento real foi
  feito pela interface**. O pedido de análise fiscal é oferecido no desfecho do cancelamento recusado por prazo (E0822) e
  como ação do detalhe.

* Entrega: interface compilada e testada no Windows pelo CI; fluxo de release que monta os executáveis (Windows sem
  dependência do Visual C++ Redistributable, e Linux só com a linha de comando) e abre a release como rascunho.

## Próximo

**Em andamento: NF-e (modelo 55)**, em fases, uma por vez. A NF-e já é enviada pela biblioteca (feature `nfe_sefaz`) e tem aplicação e linha de comando; sem nenhuma chamada a SEFAZ real conferida; sem data prometida.

* Base (feature `nfe`): chave de acesso de 44 posições, calendário das Notas Técnicas, leitura do XML autorizado e
  tabela de `cStat`. Feita nesta frente.
* Montagem e assinatura do modelo 55 (feature `nfe`): leiaute 4.00, regras do MOC e das NT conferidas antes do envio,
  IBS/CBS de mercadoria, Simples Nacional. Feita nesta frente.
* Grupos da montagem: combustível, medicamento e rastreabilidade do item, cobrança, transporte completo (transportadora,
  retenção do ICMS, veículo, reboques, vagão, balsa, volumes e lacres), exportação e compra, o conserto do DIFAL (UF da
  entrega e combustível nas regras 694 e 695) e o canal de avisos da montagem (PIS/COFINS por regime e Simples sem regime
  regular). Feita nesta frente. **Fica para a próxima:** imposto de importação (`II`, `DI`, `adi`), imposto seletivo,
  `detExport`, armamento, `NVE`, `ISSQN` e `ISSQNtot`, `retTrib`, `obsCont` e `obsFisco`, `procRef`, `cana`,
  `agropecuario`, `impostoDevol` e o QR Code do DANFE simplificado. Sem tabela publicada à parte não há conferência do
  código do produto da ANP (761), do combustível da tabela monofásica (908 e 909 pela tabela), do CFOP de combustível
  (660), do CFOP e do município da retenção do transporte (722 e 288) nem da inscrição estadual da transportadora (544).
* ICMS monofásico sobre combustíveis (CST 02, 15, 53 e 61, NT 2023.001 v1.60) na montagem: grupos do item, totais do
  `ICMSTot` e as regras 959, 508, 767, 768, 769, 962 e 964. Feita nesta frente. **Fica de fora:** a conferência do `cProdANP`
  com a Tabela de Combustíveis Monofásicos (960, e 959 pela tabela), que é publicada à parte.
* Webservices da SEFAZ, parte 1 (feature `nfe_sefaz`): tabela oficial de endereços, SOAP 1.2 com certificado A1, status do
  serviço, autorização síncrona (e assíncrona até o corte da NT 2023.002 v1.02), retorno do recibo, consulta pela chave e
  recuperação da nota enviada sem resposta. Feita nesta frente.
* Webservices da SEFAZ, parte 2 (feature `nfe_sefaz`): cancelamento, carta de correção, EPEC, eventos da apuração do
  IBS/CBS da NT 2025.002 v1.53 (13 tipos mais o cancelamento de evento), inutilização (com o fim do serviço em AC, BA e
  SP) e contingência SVC (`tpEmis` 6 e 7). Feita nesta frente. **Fica para depois:** o evento de vinculação de
  pagamento (110300, NT 2026.006), a distribuição de DF-e e a manifestação do destinatário, a consulta de cadastro e o
  cancelamento por substituição (110112, só da NFC-e).
* DANFE (feature `danfe`): A4 retrato em PDF, várias folhas, situações e contingências, leiaute da Reforma Tributária.
  Feito nesta frente. **Fica para depois da próxima versão:** o DANFE paisagem, o DANFE Simplificado, as colunas de ICMS
  ST por item e o quadro do ISSQN com nota de verdade (hoje só testado com o grupo inserido à mão no XML: entra junto
  com o `ISSQN` e o `ISSQNtot` na montagem).
* Faces, parte 1: aplicação (`rustnf-app`: configuração `[nfe]`, pasta de notas `nfe/`, numeração, emissão com recuperação, eventos, inutilização, EPEC, retomada dos pendentes e DANFE) e linha de comando (`rustnf nfe`). Feita nesta frente, com transporte falso e um simulador local: **nenhuma NF-e real foi emitida e nenhuma SEFAZ real foi chamada**.
* Faces, parte 2: API HTTP (`/nfe/`) e MCP da NF-e (11 ferramentas), sobre as mesmas funções da aplicação e no mesmo servidor. Feita nesta frente, com transporte falso e um simulador local: **nenhuma NF-e real foi emitida, consultada nem cancelada pela API ou pelo MCP, nenhum certificado A1 real foi usado e nenhuma SEFAZ real foi chamada.** Falta um cliente MCP de terceiros ler as ferramentas.
* Documentos recebidos e manifestação do destinatário.
* NFC-e.
* Interface de desktop.

Itens gerais:

1. Interface de desktop: telas para a consulta da DPS, a consulta e o registro de eventos e a atualização das situações
   (hoje só pela linha de comando, pela API e pelo MCP); e os casos de emissão fora do caso comum (tomador no exterior, intermediário, comércio exterior,
   obra, evento, locação, deduções, reembolsos, substituição, benefício municipal, suspensão, imunidade e destinatário
   diferente do tomador) seguem pela linha de comando. Faltam: assinatura de código (sem ela o Windows avisa no
   primeiro uso) e a versão de `rustnf-gui` para outros sistemas quando o GPUI tiver janela neles.
2. Cofre do sistema também no Linux e no macOS (hoje só o Windows); trava que reserve o número da DPS entre processos
   (hoje a trava protege a consistência dos arquivos, não a reserva do número; dentro do servidor, as emissões já
   entram uma por vez).
3. MCP: respostas em fluxo (SSE) e `subscriptions/listen`, se algum cliente precisar; autorização OAuth no lugar do
   Bearer fixo.
4. Trocar o `chrono` (sem mantenedor, RUSTSEC-2026-0543) por uma biblioteca de datas mantida e tirar o `ignore` do `deny.toml`.
5. Teste de ponta a ponta com TLS mútuo contra um servidor local de teste.
6. A cada versão nova do `gpui-kit`, rever os oito crates sem mantenedor que ele traz e que estão registrados no
   `deny.toml` (três deles só no lado Linux do GPUI, que a interface não compila).

## Fora da v1

* Rotas do ADN (distribuição de DF-e, parâmetros municipais): o cliente cobre só a Sefin Nacional;
* NFS-e municipal (ABRASF) e CT-e; a NFC-e fica fora até a sua fase da frente da NF-e; o cálculo do IBS/CBS de
  mercadoria entra com a montagem do modelo 55;
* assinatura ICP de PDF;
* apuração por período e DARF de CBS/IBS;
* cofre de certificados de vários clientes, contas, planos e login (o binário é de uso próprio, no servidor do usuário;
  o cofre do sistema guarda só a senha do certificado de quem usa).
