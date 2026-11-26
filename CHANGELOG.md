# Changelog

## Próxima versão

Ao atualizar: o índice da pasta de notas ganha duas situações (`substituida` e `analise_fiscal`). **Um índice que tenha
alguma delas não é lido pela versão 0.4.0** (ela o trata como ilegível); o índice antigo, só com `autorizada` e
`cancelada`, continua sendo lido por esta versão.

* NF-e (modelo 55), DANFE em PDF (feature `danfe` = `nfe` + `danfse`, nenhuma crate nova): `rustnf::danfe::{dados, pdf}`
  desenham o DANFE A4 retrato direto no PDF, com o escritor do DANFSe (que segue com os mesmos bytes), em Liberation
  Serif (a métrica da Times New Roman que o item 3.7 do Anexo II manda usar; mesma licença SIL OFL 1.1). Blocos e
  medidas do item 3.8.1 do Anexo II do MOC 7.00, corpos de fonte mínimos do 3.7 (os campos crescem na altura em vez de o
  texto descer abaixo do corpo), itens em várias folhas com o cabeçalho de identificação repetido e "FOLHA nn/total"
  (3.5 e 3.10.2), continuação das informações complementares, canhoto e logotipo opcionais. O código de barras é o
  CODE-128 da chave (conjunto C; B só onde a chave tem letra), com margem clara de 10 módulos e módulo de pelo menos
  0,2 mm; `danfe::barras::decodificar` lê as barras de volta. Situações: homologação, cancelada, denegada, nota sem
  protocolo (só prévia, com marca), contingência com formulário de segurança (campos 1 e 2 do item 3.9.2, com o
  código de barras dos dados da NF-e), SVC e EPEC (protocolo do evento nas `Opcoes`, como sai de
  `nfe::sefaz::protocolo_epec_danfe`), com as expressões e o motivo e a hora de entrada em contingência que o Anexo III
  e a Visão Geral mandam imprimir. O leiaute da Reforma Tributária (NT 2026.010 v1.00: bloco IBS/CBS/IS, CRT, QR Code
  e as bases, alíquotas e valores por item) vale para a emissão a partir de 01/12/2026 (`nfe::vigencias`); antes disso
  vale o do MOC, e `Leiaute::Rtc` o força. Contrato em `tests/casos/danfe.json`. **Fica de fora:** NFC-e (DANFE NFC-e),
  as faces (aplicação, linha de comando, API, MCP e interface), o DANFE em paisagem e as colunas de ICMS ST por item.
  O ponto de partida foi o desenho equivalente da `gaialib`. **Nenhum DANFE foi conferido por fiscal ou SEFAZ e nenhum
  foi impresso em papel.**
* NF-e (modelo 55), ICMS monofásico sobre combustíveis (NT 2023.001 v1.60): os grupos `ICMS02`, `ICMS15`, `ICMS53` e
  `ICMS61` (CST 02, 15, 53 e 61, também no CRT 1) no pedido (13 campos novos em `itens[].icms`), na montagem e nos totais
  do `ICMSTot` (`qBCMono`, `vICMSMono`, `qBCMonoReten`, `vICMSMonoReten`, `qBCMonoRet`, `vICMSMonoRet`, só com valor). Regras
  conferidas antes do envio: 959 (sem o grupo `comb`), 508 (não contribuinte), 767, 768 e 769 (quantidade obrigatória), 962 e
  964 (valor = quantidade x alíquota ad rem, com o fator 0,3333 do biodiesel e do diesel B; só na nota normal) e a soma do
  `vICMSMonoReten` no `vNF` e no `vItem` (W16-10 e VB01-10). Os campos fora do XSD e os de outro CST são recusados. As regras
  revogadas (961, 963, 965 e 966) e a conferência do `cProdANP` com a Tabela de Combustíveis Monofásicos (960, e 959 pela
  tabela) não entram. O ponto de partida foi a correção equivalente da `gaialib`. O percentual de redução da alíquota ad rem
  zero é aceito, como no XSD. **Nenhuma chamada a uma SEFAZ real foi feita.**
* NF-e (modelo 55), cliente dos webservices da SEFAZ, parte 2: eventos, inutilização e contingência SVC, na mesma
  feature `nfe_sefaz` (nenhuma crate nova). **O DANFE, as faces (aplicação, linha de comando, API, MCP, interface), a
  distribuição de DF-e e a manifestação do destinatário não estão nesta parte. Nenhuma chamada a uma SEFAZ real foi
  feita.**
  * `nfe::sefaz::evento` (puro): `cancelamento` (110111, `nProt` de 15 ou 17 dígitos, justificativa de 15 a 255),
    `carta_correcao` (110110, sequência de 1 a 20, texto fixo de `xCondUso` do esquema; a carta nova substitui as
    anteriores), `epec` (110140, dados tirados da NF-e com `tpEmis` 4, `cOrgao` 91 do Ambiente Nacional) e `evento_rtc`
    (os 13 eventos de apuração do IBS/CBS da NT 2025.002 v1.53 mais o 110001; o 211120, eliminado pela NT, e os do
    Fisco ficam de fora). `Montado::assinar` assina o `infEvento` com o A1 e monta o `envEvento`.
  * `nfe::sefaz::inutilizacao`: o `inutNFe` com as regras da tabela 5-12 (faixa de até 10.000 números, ano de 2006 até o
    atual) e `disponivel`/`eliminada_em`: AC, BA e SP perderam o serviço em 11/11/2026 nos dois ambientes (NT 2023.002
    v1.02) e o cliente devolve o erro em palavras sem chamar a rede. O `inutNFe` é só de CNPJ, como o esquema nacional.
  * `nfe::sefaz::svc`: a SVC de cada UF pelo Ato COTEPE/ICMS 39/12 (SVC-AN: `tpEmis` 6; SVC-RS: `tpEmis` 7) e o que a
    SVC atende, pelo manual de contingência (MOC 7.00, Anexo III, 2.1.3.4). A nota com `tpEmis` 6 ou 7 vai ao endereço
    da SVC, e a consulta, o recibo e o cancelamento dela também; a carta de correção de uma nota da SVC vai à SEFAZ de
    origem (a SVC não a registra) e a inutilização nunca vai à SVC. O `tpEmis` que não é o da SVC da UF é recusado antes
    da rede (rejeição 713). `status_svc` consulta o status da SVC (107, 113 e 114). A montagem passa a exigir `dhCont`
    e `xJust` nos `tpEmis` 6 e 7 (Anexo III, 2.1.3.5; o Anexo I não tem rejeição para isso). A página de homologação do
    Portal lista o PI na SVC-RS e o `NfeInutilizacao` na SVC-AN; o Ato e o manual dizem o contrário.
  * `ClienteSefaz`: `cancelar`, `corrigir`, `registrar_epec`, `registrar_evento_rtc` e `inutilizar`, com o resultado
    tipado em palavras (`DesfechoEvento`: registrado, registrado fora do prazo, sem vínculo, prazo vencido, EPEC
    bloqueado, rejeitado, indisponível, não registrado, sem resposta; `DesfechoInutilizacao`). Evento sem resposta
    consulta a nota (que devolve os eventos) antes de qualquer reenvio: achou, devolve o registro; a consulta respondeu
    sem o evento, reenviar o MESMO evento é seguro; sem resposta nem da consulta, manda consultar de novo. A duplicidade
    573 também se resolve pela consulta. `resposta_evento` lê o `retEnvEvento` e o `retInutNFe` e monta o
    `procEventoNFe` e o `ProcInutNFe` pelo texto cru.
  * A resposta com o XML escapado dentro do `nfeResultMsg` (texto em vez de elemento) passa a ser lida, em todos os
    serviços.
  * Correção: a frase da consulta que acha só o EPEC (124) saía com uma fileira de espaços no meio.
  * Os pedidos, a carta de correção, o EPEC, os 14 eventos de apuração, o `inutNFe` e os documentos de distribuição foram
    validados nos esquemas XSD oficiais fora do repositório. Os eventos, a inutilização e a SVC partiram da `gaialib`;
    aqui foram reescritos e cobertos por casos novos.
* NF-e (modelo 55), cliente dos webservices da SEFAZ, parte 1, atrás da nova feature `nfe_sefaz` (= `nfe` + as
  dependências que a `sefin` já usa; nenhuma crate nova). **Nenhuma chamada a uma SEFAZ real foi feita.**
  * `nfe::sefaz::tabela`: endereços dos webservices lidos das páginas oficiais "Relação de Serviços Web" (produção e
    homologação; 15 tabelas, 96 endereços por ambiente), UF -> autorizador e a lista da contingência.
  * `nfe::sefaz::pedidos` e `resposta`: `consStatServ`, `enviNFe` (síncrono e assíncrono), `consReciNFe` e `consSitNFe`;
    envelope SOAP 1.2 sem `nfeCabecMsg`; leitura de `retConsStatServ`, `retEnviNFe`, `retConsReciNFe` e `retConsSitNFe`. A
    NF-e vai no lote como veio assinada, sem reescrever um byte, e o `nfeProc` usa o `protNFe` recortado do texto da
    resposta.
  * `nfe::sefaz::ClienteSefaz`: `status`, `autorizar` (síncrono), `autorizar_assincrono` (só até a data de corte da NT
    2023.002 v1.02 de cada ambiente), `retorno` (consulta do recibo com espera e tentativas configuráveis) e `consultar`.
    Resposta perdida vira consulta pela chave antes de qualquer reenvio; duplicidade 204 devolve a nota já autorizada;
    539 diz qual é a nota que tem o número. O resultado diz em palavras se a nota foi autorizada, denegada, rejeitada ou
    ficou sem resposta, e o que fazer.
  * `nfe::sefaz::limbo`: a varredura das notas enviadas sem resultado (espera crescente, aviso, reenvio da mesma NF-e).
  * Transporte real: `reqwest` com mTLS pelo certificado A1, raízes vigentes da AC Raiz da ICP-Brasil somadas às do
    `webpki-roots` só neste cliente (`dados/icp_brasil_raizes.crt`), tempo limite configurável, resposta limitada a 16
    MB e raiz extra para simulador local. O que o `nfse::sefin` já fazia para montar o cliente TLS agora mora num módulo
    comum (`https`), sem mudar o comportamento dele.
  * A recuperação da nota e a varredura do limbo partiram da `gaialib`; aqui foram reescritas e cobertas por casos novos.
* NF-e (modelo 55), base da biblioteca, atrás da feature `nfe` (sem dependência nova). **A NF-e ainda não emite (o envio vem no item do cliente da SEFAZ, acima)**: só
  chave, leitura e tabelas.
  * `chave_dfe`: a chave de acesso de 44 posições (NF-e e NFC-e). `montar` recusa campo que não cabe, UF ou modelo
    desconhecido, CPF ou CNPJ do emitente com dígito verificador inválido e `cNF` que a regra B09-30 recusa; `validar`,
    `decompor`, `formatar`, `limpar` e `achar` (texto de OCR e URL de QR Code) completam. O emitente CPF da chave é o da
    série 910 a 969 e o da Nota Fiscal Fácil (`tpEmis` 3, 5º dígito do número igual a 2); na série 890 a 899 a chave leva
    o CNPJ da SEFAZ e `nfe::conferir` não a compara com o emitente do XML (NT 2021.002 v1.12, itens 2.1 e 2.2). O que a
    NFS-e usa não mudou.
  * `nfe::ler`: lê o `nfeProc` ou a `NFe` solta (leiautes 3.10 e 4.00) para a struct `Nfe`, com decimal exato e data com
    fuso; recusa DTD, XML de outro documento, modelo diferente de 55 e campo fora do formato, com erro tipado.
    **A leitura não confere a assinatura XML nem o `digVal`.** `nfe::conferir` aponta divergência entre a chave e o `ide`,
    entre os totais e a soma dos itens e entre as duplicatas e a fatura.
  * `nfe::cstat`: tabela dos 953 códigos de retorno (MOC 7.00 Anexo I e Visão Geral, e Notas Técnicas), com a mensagem oficial e a
    classe (autorizada, cancelada, denegada, rejeitada, processamento). O `cStat` 110 (Uso Denegado) é classe denegada.
  * `nfe::vigencias`: calendário das Notas Técnicas por ambiente, com as datas conferidas no cronograma de cada NT.
  * A chave de acesso e a leitura da NF-e partiram da `gaialib`; aqui foram reescritas e cobertas por casos novos, com
    documentos fictícios.
* NF-e (modelo 55), montagem e assinatura, atrás da feature `nfe` (sem dependência nova). **O envio vem no item do cliente da SEFAZ, acima**:
  não há cliente da SEFAZ, DANFE, linha de comando, API nem interface para ela.
  * `nfe::montagem`: o pedido em JSON (`nota_de_json`, formato em `docs/NFE_PEDIDO.md`) vira a `Nota` e o XML `NFe` 4.00
    do modelo 55 (`montar`), na ordem do XSD PL_010h: ide (referenciadas, intermediador, contingência, `cIndOp`), emit,
    dest, retirada e entrega, det/prod (veículo novo, CEST, benefício fiscal), ICMS completo (CST e CSOSN, ST, diferimento,
    desoneração, partilha, repasse, FCP, ICMS efetivo), IPI, PIS, COFINS, ICMSUFDest (DIFAL), IBS/CBS, compra
    governamental, pagamento antecipado, valor líquido do produto, totais (`ICMSTot`, `IBSCBSTot`, `vNFTot`), transporte
    (`modFrete`), pagamento e cartão, informações adicionais e responsável técnico. A mesma entrada dá o mesmo XML: o
    `cNF` e a data de emissão vêm do pedido.
  * `conferir` devolve as rejeições do MOC e das Notas Técnicas (`<cStat>-Rejeicao: ...`) que a nota levaria, antes de
    gastar o número; `montar` recusa, com o campo e o código da rejeição, o que o leiaute não aceita.
  * `assinar` assina o `infNFe` com o A1 pelo módulo `assinatura`: RSA-SHA1 e digest SHA-1, os únicos que o
    `xmldsig-core-schema_v1.01.xsd` da NF-e aceita (a NFS-e usa SHA-256). O DANFE simplificado (`tpImp` 6) monta mas não
    assina: pede o QR Code, que ainda não existe. O modelo 65 (NFC-e) é recusado.
  * Campo desconhecido no pedido: aviso (`campos_desconhecidos`) e, no modo estrito (`nota_de_json_estrito`), recusa.
  * `reforma::calculo` (feature `nfe`): o IBS/CBS de mercadoria por item e o total (`ibscbs_do_item`, `totalizar`,
    `vnftot`); mais `grupo_exigido`, `tributos_somam_no_total`, `mono_modalidade` e `indop::erro_nfe`.
  * Simples Nacional (NT 2026.005, tabela da seção 2.3): o Simples sem regime regular do IBS/CBS (CRT 1, sem
    `tpRegimeIBSCBS`) leva o item SEM o `gIBSCBS`, mesmo quando o CST o exige, e COM o `gTribSN` (`gIBSSN` e
    `gCBSSN`); a montagem exige o `gTribSN` nesse regime. No CRT 2 sem regime regular o `gCBS` vai com a alíquota
    zerada.
  * A montagem, o cálculo e as regras partiram da `gaialib`; aqui foram reescritos e cobertos por casos novos, com
    documentos fictícios e o XML validado no XSD oficial.
* NF-e (modelo 55), grupos da montagem (feature `nfe`, sem dependência nova). **A NF-e ainda não é enviada.**
  * Item: `comb` (combustível, com CIDE, encerrante, origem 0 a 30 e GLP), `med` (medicamento) e `rastro` (lotes, até 500);
    um grupo específico só por item (`veicProd`, `med` ou `comb`). Nota: `cobr` (fatura e duplicatas), `transp` completo
    (transportadora, `retTransp`, veículo, reboques, vagão, balsa, volumes e lacres), `exporta` e `compra`, na ordem do XSD.
    Rejeições conferidas: 461, 855, 856, 817, 379, 380, 909 e 858 (combustível), 873, 877 e 870 (medicamento e lote),
    905, 901, 902, 852, 900, 850 e 851 (cobrança), 542, 543, 559 e 868 (transporte), 355 e 356 (exportação).
  * **DIFAL**: as regras 694 e 695 passam a ler a UF do local de entrega e o `cProdANP` do combustível, que a montagem nunca
    repassava a elas. Antes, a entrega na UF do emitente continuava exigindo o `ICMSUFDest`, a entrega em outra UF continuava
    recusando o grupo e o combustível derivado de petróleo fora da lista da ANP não dispensava nem proibia o grupo.
    Nenhum XML de caso anterior mudou.
  * **Canal de avisos**: `Montada` ganha `avisos` e há `montagem::avisos(&Nota)`; aviso nunca impede a montagem nem muda o
    XML. Campo novo e opcional do pedido, `regime_pis_cofins` (`cumulativo` ou `nao_cumulativo`, não vai ao XML), para o
    aviso de CST e alíquota de PIS/COFINS que não combinam com o regime (Leis 10.637/2002, 10.833/2003, 9.715/1998 e
    9.718/1998). Segundo aviso: Simples sem regime regular com CST que exige o `gIBSCBS` (tabela 2.3 da NT 2026.005 contra a
    regra UB13-30 da NT 2025.002, rejeição 1022).
  * Quem monta `Nota`, `Item` ou `Montada` por literal de struct precisa dos campos novos (`transp`, `cobr`, `exporta`,
    `compra`, `regime_pis_cofins`; `combustivel`, `med`, `rastro`; `avisos`). `montar` não mudou de assinatura.
* Reforma: o aviso ao optante do Simples Nacional passa a explicar a opção pelo regime regular do IBS e da CBS (LC 123/2006,
  art. 13, §§ 9º a 11, incluídos pela LC 214/2025, art. 517, na redação da LC 227/2026, com efeito a partir de 1º/1/2027):
  vale por semestre iniciado em janeiro ou julho, é irretratável no semestre e é feita em setembro ou março, antes dele.
* Consulta da DPS: `rustnf consultar --dps <id>` (ou `--serie` e `--numero`, com o `[emitente]` da configuração),
  `GET /dps/{id}` e a ferramenta MCP `consultar_dps` devolvem a chave da NFS-e que a DPS gerou, e a nota com
  `--com-nota`. O Id fora da forma é recusado antes da rede.
* Consulta de evento: `rustnf evento consultar`, `GET /nfse/{chave}/eventos/{tipo}/{sequencia}` e `consultar_evento`.
  O evento devolvido é conferido contra a chave e o tipo pedidos: a resposta de outra nota ou de outro tipo, ou um
  corpo que não é um evento, é erro, não sucesso. Isso vale também para a conferência do cancelamento.
* Eventos além do cancelamento: `rustnf evento registrar`, `POST /eventos` e `registrar_evento` registram os eventos
  que o contribuinte pede (por exemplo o 101103, pedido de análise fiscal); o 101103 registrado marca a nota da pasta
  como em análise fiscal. O cancelamento (101101) e os eventos do fisco e do sistema são recusados antes de qualquer
  envio, dizendo o que fazer ou quem os registra. `rustnf evento tipos` lista os tipos do contribuinte.
* Atualização das situações em lote: `rustnf notas atualizar`, `POST /notas/atualizar` e `atualizar_situacoes`
  conferem na Sefin as notas da pasta (cancelada, substituída, em análise fiscal), uma nota e uma consulta por vez,
  com intervalo entre as consultas (200 ms por padrão). Nota que a pasta dá como cancelada ou substituída e que a
  Sefin não mostra assim nunca volta para autorizada: vai para as falhas, como divergência.
* A situação da nota na pasta passa a ter `substituida` (105102) e `analise_fiscal` (101103 sem decisão do fisco);
  aparecem no selo, no filtro e no detalhe da lista de notas da interface, que não ganhou tela nova. O `reindexar`
  aproveita todas as situações do índice anterior, não só a cancelada.
* Interface de desktop, **eventos na tela**: "Conferir situação na Sefin" nas Notas (as notas que a lista mostra, com
  andamento, Parar, confirmação acima de 50 notas e resultado em palavras: o que mudou de-para, quantas estavam iguais e,
  de cada falha, o que fazer); "Histórico na Sefin" no detalhe da nota; "Pedir análise fiscal do cancelamento" (mesmo
  diálogo do cancelamento, oferecido também quando a Sefin recusa o cancelamento por prazo, E0822; o pedido sem
  resposta é conferido, não reenviado às cegas); Consultar pela série e número da
  DPS; e, para a nota recebida (de outro emitente, com o emitente da configuração como tomador), "Confirmar que o serviço
  foi prestado" e "Recusar esta nota". `rustnf-app`: `atualizar_lista` (a conferência em lote com acompanhamento e parada)
  e `marcar_analise_fiscal` pública. O emissor de demonstração responde consulta de DPS e de evento e registra eventos por
  roteiro (`--evento`). Testados com a demonstração e com a janela no ambiente de teste do GPUI: **nenhuma consulta nem
  evento real foi feito pela interface**.
* Correção: quando a resposta do registro de um evento se perdia, a biblioteca só olhava a sequência 1 do tipo e dava
  por registrado um pedido novo de um tipo que já tinha evento (o 101103 depois de um 105105, por exemplo). Agora ela
  percorre as sequências do tipo e só aceita o evento que traz o mesmo `dhEvento` do pedido; não achando, devolve
  "pendente" para reenviar o mesmo pedido.
* `rustnf-app`: dependência direta de `tokio` (só o relógio, para o intervalo entre as consultas); a interface de
  desktop e as demais faces usam o executor de quem chama.

## 0.4.0 - 2026-11-25

Ao atualizar: a configuração nova (`aliquota_iss`) é opcional e vazia por padrão, e a prévia do IBS/CBS só muda no
empate exato de arredondamento (veja abaixo).

* Nova nota: "Buscar endereço pelo CEP" preenche logradouro, bairro e município (código IBGE) do tomador. Só no
  clique: o CEP vai ao ViaCEP e, se ele não responder, à BrasilAPI; sem resposta, o endereço é preenchido à mão. Não
  traz dependência nova (o cliente HTTP já era o da Sefin).
* Quem já emitia por outro sistema continua a numeração de lá: `rustnf notas numero --proximo <N>` ou, na interface,
  Configurações > Notas e DANFSe > "Continuar deste número". A série passa a seguir do número informado; a
  numeração só anda para a frente (um número já usado faria a Sefin devolver a nota antiga). Sem `--proximo`, o
  comando mostra a próxima DPS da série.
* ISS: alíquota padrão no `[emitente]` (`aliquota_iss`, vazio = não informar), que completa a DPS só onde ela leva
  a alíquota (ISS tributável, sem regime especial, fora do MEI e, para ME/EPP, com ISS retido). Na interface, o campo
  fica em Configurações > Emitente, aparece como dica na Nova nota, e o resumo de valores mostra o ISS estimado (ou
  "calculado pela Sefin" sem alíquota conhecida); o ISS retido sai do valor líquido.
* Interface de desktop pelo teclado: o Tab leva a tela até o campo que ganha o foco (antes ele podia ficar fora de
  vista), PageUp/PageDown rolam a tela mesmo com o foco num campo, e as setas e Ctrl+Home/Ctrl+End rolam quando o
  foco não está num campo. Na lista de notas as setas continuam andando pela lista.
* Retorno E0008 da Sefin (hora de emissão depois da hora de processamento) ganha a dica de acertar o relógio ou
  aumentar `folga_relogio_s`.
* Prévia do IBS/CBS: a leitura de alíquota ou redução com mais de 4 casas passa a arredondar "meio para o par", como
  os valores da prévia e a calculadora oficial; só muda no empate exato (ex.: `0,00005`).
* `rustnf completar <shell>`: imprime o script que completa comandos e opções com Tab no PowerShell, bash, zsh,
  fish ou elvish (veja "Linha de comando" no README).
* Dependência `url` 2.5.8 -> 2.5.9 (RUSTSEC-2026-0592, estouro de pilha em `Url::origin()`; chega pela interface
  de desktop).
* Dependências: atualização semanal proposta pelo Dependabot, agrupada, sempre passando pelo CI antes de entrar; o
  kit da interface de desktop continua sendo trocado à mão.
* Versão mínima do Rust declarada no `Cargo.toml` (`rust-version = "1.92"`) e conferida no CI, que compila o
  workspace nessa versão além do Rust estável.
* Exemplo da biblioteca: `cargo run -p rustnf --example montar_e_assinar --features cert_a1_teste` monta uma DPS a
  partir do pedido em JSON, assina com um certificado A1 de teste e confere a assinatura, sem rede.
* CI: documentação gerada com aviso virando erro, o exemplo da biblioteca roda, e o histórico inteiro passa pelo
  gitleaks (binário oficial conferido pelo SHA-256); as ações do GitHub foram para as versões em Node.js 24.
* Release: cada arquivo publicado ganha um atestado de procedência do GitHub, conferível com
  `gh attestation verify <arquivo> --repo Admnwk/rustnf`.
* Dependências `roxmltree` 0.21 e `qrcode` 0.15 (os DANFSe do contrato saem byte a byte iguais aos da 0.3.0) e correções
  de `clap`, `regex`, `toml`, `keyring-core` e `x509-cert`

## 0.3.0 - 2026-11-21

Atenção ao atualizar: a emissão passa a recusar o pedido com campo de nome errado (veja "Mudança de comportamento"
abaixo); a API HTTP e o MCP passam a usar a pasta de notas e o `[emitente]` da configuração, como a linha de comando.

* A API HTTP e o MCP passam a emitir e a cancelar pelo mesmo caminho da linha de comando e da interface. Em
  `POST /emitir` e em `emitir_nfse`, o `[emitente]` da configuração completa o que o pedido não traz e, com
  `[notas].pasta`, o número da DPS pode faltar (vale o próximo da série) e a nota autorizada é guardada na pasta; a
  resposta ganha `guardada`, `numero_dps`, `aviso` e `recuperada_difere`, como no `rustnf emitir`. Em `POST /cancelar`
  e em `cancelar_nfse`, o cancelamento registrado marca a nota na pasta, e a recusa de um cancelamento repetido é
  conferida na Sefin antes de virar erro. Antes, uma nota emitida pelo servidor não entrava na pasta nem na
  numeração. Sem `[emitente]` e sem pasta de notas na configuração, nada muda.
* O servidor emite uma nota por vez: duas emissões simultâneas não pegam mais o mesmo número de DPS.
* `POST /calcular` e `calcular_cbs_ibs` usam as `[[aliquotas]]` da configuração quando o pedido não traz
  `publicadas`, como o `rustnf calcular` já fazia.
* MCP: ferramenta nova `consultar_nfse` (consulta pela chave de acesso), só com `--permitir-emissao` e o Bearer, como
  o `GET /nfse/{chave}` da API HTTP.
* Linha de comando, o que a interface de desktop já fazia e faltava nela:
  * `rustnf certificado mostrar`: titular, documento, emissor, validade, dias restantes e avisos do certificado A1,
    sem usar a rede; código de saída 3 quando ele não assina hoje, para servir a um aviso agendado;
  * `rustnf sefin testar`: uma chamada real à Sefin Nacional (a consulta de uma chave que não existe) que diz se a
    conexão e o certificado foram aceitos;
  * `rustnf danfse --chave <chave>`: o DANFSe de uma nota da pasta de notas, a partir do XML guardado, com a marca de
    cancelada quando a nota está cancelada na pasta;
  * `rustnf notas listar` com `--de`, `--ate`, `--situacao`, `--busca` e saída `--csv` para planilha;
  * `rustnf notas exportar --mes AAAA-MM --para <pasta>`: copia os XML do mês e a lista em CSV, para a contabilidade;
  * `rustnf notas reindexar`: refaz o índice da pasta de notas a partir dos XML guardados, guardando o índice anterior
    ao lado.
* **Mudança de comportamento:** campo de nome errado no pedido da DPS deixa de ser ignorado em silêncio. A
  **emissão recusa** o pedido com campo que a DPS não conhece, antes de abrir o certificado e de qualquer envio: em
  `rustnf emitir` (código 3; `--ignorar-desconhecidos` emite mesmo assim, sem o campo, e `--estrito` continua aceito),
  em `POST /emitir` (422; `?estrito=false` emite sem o campo) e em `emitir_nfse` (`isError`). `rustnf dps montar` e
  `dps conferir`, `/dps/conferir` e `/dps/montar` e `conferir_dps` avisam e devolvem a lista em
  `campos_desconhecidos`; com `--estrito` ou `?estrito=true`, também recusam.
* `rustnf dps esquema` imprime o JSON Schema do pedido da DPS (`--emissao` para o do `emitir`).
* Interface de desktop:
  * correção na busca da tela Notas: um nome de tomador comprido com número no meio ("Clínica 24 Horas ...") era
    tomado por chave de acesso e não achava nada. A busca passa a usar a mesma regra da linha de comando: só é chave
    o texto que começa pelos 7 dígitos do município;
  * "Abrir a pasta", na tela Notas, mostra a pasta de notas no gerenciador de arquivos;
  * "Copiar o XML", no detalhe da nota, põe o XML inteiro na área de transferência;
  * Ctrl+Shift+D descarta, nas Configurações, o que foi alterado e não foi salvo (o botão "Descartar" já existia; a
    ação não tinha tecla);
  * período na tela Notas ("Todo o período", "Este mês", "Mês passado", "Este ano"), junto com a busca e o filtro;
  * "Exportar a lista", na tela Notas: grava numa pasta escolhida o XML de cada nota da lista e a lista em CSV;
  * a coluna DPS da lista não quebra mais um número de DPS longo em duas linhas: o meio dá lugar a reticências;
  * tomador já usado: na Nova nota, ao digitar o CPF ou CNPJ completo de quem já recebeu nota, os campos vazios do
    tomador (nome, inscrição municipal, e-mail, telefone) e o endereço vêm da nota mais recente dele na pasta de
    notas, com um aviso de qual nota. O que a pessoa já digitou fica; com parte do endereço digitada, o endereço não
    é mexido.
* `rustnf-gui.exe` ganha ícone (o da janela e o do arquivo) e a versão nas propriedades do arquivo (aba "Detalhes"),
  gerada a partir da versão do pacote. O CI e o fluxo de release conferem os dois no executável, e a release só sai
  se a versão gravada for a da tag.
* Biblioteca: a NFS-e lida (`nfse::leitura::Nfse`) traz também a inscrição municipal, o e-mail, o telefone e o
  endereço nacional do tomador (`tomador_im`, `tomador_email`, `tomador_fone`, `tomador_cep`, `tomador_municipio`,
  `tomador_logradouro`, `tomador_numero`, `tomador_complemento`, `tomador_bairro`). O JSON de `emitir` e de
  `consultar` (linha de comando, API HTTP e MCP) ganha esses campos em `nfse`.
* Biblioteca: `Certificado::resumo` (e `resumo_de_agora`) devolve o que se mostra de um certificado aberto, com o CPF
  mascarado. Camada de aplicação: `notas::Filtro`, `notas::csv`, `Notas::registro`, `Notas::exportar` e
  `Notas::reindexar`.

## 0.2.0 - 2026-11-21

* DANFSe sem o Typst: o PDF passa a ser escrito direto pelo `krilla` (feature `danfse`), a partir da mesma lista de
  operações de desenho que `danfse::dados` já devolvia. O JSON de `dados` não mudou e o leiaute é o mesmo; o texto
  segue selecionável e copiável (Liberation Sans embutida em subconjunto), e a mesma entrada passa a gerar sempre os
  mesmos bytes. A diferença visível é só o contorno das letras: o desenho não aplica mais o kerning do Typst. A
  árvore de dependências da biblioteca com `sefin` e `danfse` caiu de 387 para 207 crates: o DANFSe acrescentava 215
  e passa a acrescentar 35 (o `Cargo.lock` do workspace foi de 1044 para 935). No `deny.toml` saíram cinco exceções
  de segurança que só existiam por causa do Typst (duas do `quick-xml` e três de crates sem mantenedor) e duas
  licenças que nenhuma dependência usa mais. Medido numa compilação de release para Windows: `rustnf.exe` foi de
  48,9 MB para 14,5 MB, `rustnf-gui.exe` de 71,1 MB para 39,7 MB, e gerar um DANFSe pela linha de comando foi de
  cerca de 500 ms para cerca de 180 ms.
* Documentação em português com acentos (README, este arquivo, guia de contribuição, política de segurança, roadmap,
  fontes, NOTICE e modelos de issue). O README ganhou o passo a passo de emissão e passou a dizer a versão mínima do
  Rust (1.92) no lugar de "edição 2021".

## 0.1.0 - 2026-11-21

Primeira versão pública.

* Entrega: o CI passa a compilar e testar a interface de desktop no Windows (job `interface`); novo fluxo `release`
  que, com uma tag de versão, monta `rustnf.exe` e `rustnf-gui.exe` (Windows, com a biblioteca C dentro do executável:
  quem baixa não precisa instalar nada) e `rustnf` (Linux), confere as dependências dos executáveis, calcula os
  SHA-256 e abre a release como rascunho. README com as telas e a instrução de quem baixa pronto.
* Novo crate `rustnf-app` (`crates/rustnf-app`): a camada de aplicação, sem tela, sem servidor e sem `clap`, que a linha
  de comando e qualquer interface de desktop compartilham. Recebeu `nucleo`, `emissao`, `segredo` e o erro (sem o status
  HTTP, que ficou no binário); ganhou:
  * `config`: arquivo de configuração TOML único (`[emitente]`, `[sefin]`, `[notas]`, `[danfse]`, `[servidor]`,
    `[[aliquotas]]`), com padrão em tudo, campo desconhecido recusado, conferência dos valores (erro com seção e
    campo), gravação atômica e a completude do pedido da DPS com o `[emitente]`. Não há campo de senha nem de token;
  * `cofre`: trait `Cofre`, o cofre do sistema (Gerenciador de Credenciais, no Windows) e um cofre em memória para
    teste;
  * `notas`: pasta de notas sem banco (`<AAAA>/<MM>/<chave>.xml`, `indice.json`, `estado.json`) com numeração da DPS
    que só avança em nota autorizada ou recuperada;
  * `pedido`: o fluxo de emitir e cancelar com a configuração e a pasta de notas.
* Binário `rustnf`: opção global `--config`; precedência opção > variável de ambiente `RUSTNF_*` > arquivo > padrão em
  todo parâmetro; `--ambiente`, `--pfx` e os ajustes de `serve` passam a poder vir do arquivo; a senha do certificado,
  na falta de `--senha-env` e `--senha-stdin`, vem do cofre do sistema (continua impossível passá-la por argumento);
  `emitir` completa o pedido com o `[emitente]`, usa o próximo número da pasta de notas e guarda a nota autorizada;
  `cancelar` marca a nota como cancelada; `calcular` usa as `[[aliquotas]]` quando o JSON não traz `publicadas`.
  Comandos novos: `config mostrar`, `config caminho`, `senha guardar`, `senha apagar` e `notas listar`. Sem arquivo
  de configuração e sem as opções novas, o comportamento é o de antes.
* Assinatura da DPS e dos eventos: o padrão passa a ser RSA com SHA-256 (era SHA-1). O SHA-1 continua disponível:
  `--assinatura sha1`, `RUSTNF_ASSINATURA=sha1`, `[sefin] assinatura = "sha1"` ou, na biblioteca, o campo
  `ClienteNfse::algoritmo`. A conferência de assinaturas continua aceitando SHA-1, SHA-256 e SHA-512.
* Novo crate `rustnf-gui` (`crates/rustnf-gui`): a interface de desktop, em GPUI (`gpui-kit` =0.10.0), só para Windows nesta
  versão (fora do Windows o executável avisa e sai com código 2). Lê e grava o mesmo arquivo de configuração da linha de
  comando e guarda a senha do certificado no cofre do sistema. Faz o primeiro uso em três passos, mostra o Início e as
  Configurações (emitente, Sefin Nacional, certificado, senha, notas, DANFSe, alíquotas, tema), testa o certificado e a
  conexão com a Sefin e mostra sempre o ambiente (Produção ou Produção restrita) e o estado do certificado. Ainda não
  emite, consulta nem cancela nota. A camada `modelo` (estado dos formulários, máscaras, conferência campo a campo) não
  depende de janela e é testada em qualquer sistema.
* `rustnf-gui`: nova tela **Nova nota** (Ctrl+N; botão "Emitir uma nota" no Início): formulário do caso comum (tomador,
  serviço com busca na Lista Nacional, valores, ISS, tributos federais e IBS/CBS) com a sugestão de NBS, `cIndOp` e
  `cClassTrib` pela correlação do Anexo VIII, conferência ao vivo (própria e da biblioteca, cada recusa no campo da tela),
  painel fixo com pendências, valor líquido e prévia do IBS/CBS, rascunho automático em `rascunho.json` na pasta de notas,
  confirmação com o ambiente em destaque (em produção, com marcação explícita) e emissão fora da tela com pedido
  congelado: "Tentar de novo" reenvia o mesmo pedido e "Desistir" avisa que a nota pode ter sido autorizada; nota
  recuperada com dados diferentes dos do formulário é avisada e o formulário fica para emitir de novo. A senha, se
  não estiver no cofre, é pedida para aquela emissão e não é guardada. Feature `demonstracao` (desligada por padrão) e
  exemplo `demonstracao`: a mesma janela com um emissor sem rede. Dependências novas do crate: `serde`, `serde_json` e
  `async-trait` (já usadas pelo resto do workspace).
* `rustnf-app`: `pedido::emitir_pedido_completo` e `pedido::Emitida` devolvem o resultado do cliente como dado tipado
  (rejeição com código, descrição e complemento), e `atomico::gravar` passou a ser público. Nota recuperada (já existia
  na Sefin com a mesma série e o mesmo número de DPS) é comparada com o pedido: `Emitida::diferencas` e, na resposta de
  `emitir_pedido` (linha de comando, HTTP e MCP), `recuperada_difere` dizem em que ela difere; vazio ou ausente quando a
  nota é a do pedido.
* `rustnf-gui`: novas telas **Notas** (Ctrl+L), **Consultar** (Ctrl+K) e **Prévia** (Ctrl+P). Notas lista a pasta de notas
  (busca por texto, filtro por situação, lista virtual, atualização sozinha) e mostra o detalhe da nota lido do XML, com
  "Abrir o DANFSe" (gravado ao lado do XML e aberto no programa padrão), "Mostrar na pasta", **cancelar** (motivo e
  justificativa dentro dos limites da biblioteca, confirmação explícita em produção, desfechos registrado, rejeitado e sem
  resposta, com "Tentar de novo" e "Conferir na Sefin") e "Usar como modelo" (a Nova nota recebe os dados da nota).
  Consultar busca a nota pela chave de acesso (conferida antes, mostrada em grupos), confere se a nota devolvida é a
  pedida e se tem cancelamento registrado, e guarda na pasta de notas a nota do emitente. Prévia é a calculadora avulsa do
  IBS e da CBS, sem certificado nem pasta. A senha do certificado, quando não está no cofre, é pedida pelo mesmo diálogo
  para emitir, cancelar e consultar. A demonstração passa a consultar e cancelar (`--cancelamento`).
* `rustnf-app`: `Emissor::cancelada` (a nota tem o evento de cancelamento registrado?), `emissao::consultar_completa`,
  `pedido::cancelar_pedido_completo` e `pedido::Cancelada` (o resultado do cancelamento como dado tipado; `cancelar_pedido`
  continua devolvendo o JSON de sempre), `Notas::caminho_do_xml_guardado` e `Notas::guardar_danfse` (o PDF fica ao lado do
  XML e não entra no índice nem na numeração).
* Cancelamento recusado por repetição: se a nota já tem o cancelamento registrado (envio anterior sem resposta, ou feito em
  outro lugar), `cancelar_pedido` e `cancelar_pedido_completo` devolvem `registrado` com `recuperada`, marcam a pasta e
  levam as mensagens da recusa em `alertas`. Antes, a nota já cancelada aparecia como cancelamento recusado.
* Configuração: nova seção `[interface]` com `tema` (`sistema`, `claro` ou `escuro`).
* `rustnf-app`: novo módulo `certificado` (abrir o A1 e montar o cliente da Sefin, antes dentro da linha de comando) e
  `cofre::cofre_do_ambiente` (a escolha do cofre por `RUSTNF_COFRE`), compartilhados pela linha de comando e pela
  interface. O comportamento da linha de comando não mudou.
* Biblioteca: `rustnf::danfse::municipio::buscar` agora é pública (código IBGE de 7 dígitos -> nome e UF, pela tabela
  embutida).
* Dependências novas: `toml`, e, só no Windows, `keyring-core` e `windows-native-keyring-store` (todas MIT OR
  Apache-2.0).

* Biblioteca `rustnf`:
  * `nfse`: DPS do leiaute 1.01 (produção) e da produção restrita (NT 009), com as regras do Anexo I conferidas antes
    do envio (entre elas a forma do `totTrib` por regime do Simples Nacional: E0710, E0712 e E0713); pedido de evento;
    leitura da NFS-e, do evento e das mensagens de retorno, com dica para E0037 e E0312; Lista Nacional de serviços;
    correlação do Anexo VIII; `agora_com_folga` para o `dhEmi` (E0008).
  * `nfse::sefin` (feature `sefin`): cliente da Sefin Nacional com mTLS e o A1 só em memória, emissão idempotente,
    consultas, cancelamento e eventos.
  * `assinatura`: XMLDSig com C14N própria e certificado A1 (`.pfx` ou PEM).
  * `reforma`: tabela de CST/cClassTrib, cIndOp, atividades do Simples Nacional e alíquotas 2026-2033.
  * `reforma::previa`: base de cálculo (regime regular e Simples Nacional) e IBS/CBS de um serviço, refazendo a conta
    da calculadora da Receita Federal: decimal exato, um arredondamento por número publicado, "meio para o par".
  * `danfse` (feature `danfse`): DANFSe em PDF (NT 008) a partir do XML da NFS-e autorizada, com o Typst embutido, a
    Liberation Sans (SIL OFL 1.1) e o logotipo da NFS-e. Veja o `NOTICE`.
  * `chave_dfe`: chave de acesso de 50 posições da NFS-e e validação de CPF e CNPJ (numérico e alfanumérico).
* Binário `rustnf` (crate `rustnf-cli`): `dps montar`, `dps conferir`, `calcular`, `danfse`, `emitir`, `consultar`,
  `cancelar` e `serve`. A senha do certificado só entra por variável de ambiente ou stdin. Códigos de saída: 0 ok,
  2 uso ou entrada, 3 recusa de regra, 4 rede ou Sefin, 1 o resto.
* API HTTP (`rustnf serve`): `/health`, `/calcular`, `/dps/conferir`, `/dps/montar`, `/danfse`; `/emitir`,
  `/cancelar` e `/nfse/{chave}` só com `--permitir-emissao` e Bearer de no mínimo 32 caracteres. Escuta em
  `127.0.0.1`, sem CORS, com teto de corpo e tempo limite.
* MCP em `POST /mcp` (Streamable HTTP, resposta JSON, sem estado): ferramentas `calcular_cbs_ibs`, `conferir_dps`,
  `montar_dps`, `gerar_danfse`, mais `emitir_nfse` e `cancelar_nfse` com emissão e Bearer.
* Teste de higiene (`tests/higiene.rs`): termos, endereços, CPF/CNPJ, dependências e aviso da licença em cada `.rs`,
  nos arquivos e nas mensagens de commit. `cargo deny` para licenças e avisos de segurança; `gitleaks` com a
  configuração da raiz.
* Licença: Mozilla Public License 2.0 (`LICENSE`).
