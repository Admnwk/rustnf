# Pedido da NF-e e da NFC-e em JSON

O pedido que `rustnf::nfe::montagem::nota_de_json` le para montar a NF-e (modelo 55) ou a NFC-e (modelo 65), leiaute 4.00.
A montagem NAO envia a nota a SEFAZ.

* Pela linha de comando (`rustnf nfe`), o que o pedido nao traz vem do arquivo de configuracao: `emit` (documento, nome, `crt`, `ie`,
  fantasia e endereco), `serie`, `numero` (o proximo da pasta de notas), `tp_amb`, `emissao`, `cmun_fg`, `codigo`,
  `regime_pis_cofins` e, na contingencia, `tp_emis`, `dh_cont` e `x_just`. O que o pedido traz vale sempre.
* Valor monetario e percentual vao em texto (`"10.50"`) ou inteiro; numero com ponto flutuante e' recusado. Os campos que
  a tabela diz "em centavos" sao inteiros.
* Data e hora em ISO 8601 COM fuso (`2026-10-03T09:00:00-03:00`); sem fuso e' erro (nao se chuta o horario de Brasilia).
* `codigo` (o `cNF`, 8 digitos) vem no pedido: a mesma entrada da o mesmo XML. Quem nao tem um gera com
  `chave_dfe::gerar_codigo`.
* O texto e' filtrado como manda o leiaute (sem espaco repetido, sem quebra de linha) e, com `remover_acento` (padrao: sim),
  sem acento.
* Campo que o pedido traz e a montagem nao conhece (nome errado, campo que a montagem ainda nao tem) e' ignorado, com
  aviso em `campos_desconhecidos`; `nota_de_json_estrito` recusa. `itens[].ibscbs.publicadas` e' de forma livre.
* Ainda sem campo no pedido (a montagem recusa o que pede e nao faz): imposto de importacao (`II`, `DI`, `adi`),
  imposto seletivo, `detExport`, armamento, `NVE`, `ISSQN`, `retTrib`, `obsCont`, `obsFisco`, `procRef`, `cana`,
  `agropecuario` e `impostoDevol`.
* **NFC-e (`modelo` 65).** O mesmo pedido, com os padroes do modelo: `tp_imp` 4, `ind_final` 1 e `ind_pres` 1 quando o
  pedido nao diz, `mod_frete` 9. O destinatario (`dest`) e' opcional: sem ele a NFC-e sai sem o grupo `dest`; com ele, so'
  o CPF, o CNPJ ou o `id_estrangeiro` e `ind_ie_dest` 9 sao obrigatorios (nome e endereco sao opcionais, e acima de R$
  10.000,00 a regra 752 exige o endereco). A montagem recusa, com o codigo da rejeicao, o que o MOC 7.00 e a NT 2026.002
  v1.11 rejeitam na NFC-e: `dhSaiEnt` (705), entrada (706), operacao fora do estado (707), `tpImp` fora de 4 e 5 (709),
  finalidade diferente de 1 (715), `ind_final` 0 (716), `ind_pres` fora de 1, 4 e 5 (717), entrega a domicilio sem
  destinatario (787), destinatario contribuinte (789), referencias (708), IE de substituto (718), local de retirada
  (669), CFOP fora da lista (725) e a combinacao CFOP x CST ou CSOSN (382 e 386), CST e CSOSN fora da lista (766 e 383),
  item que nao soma no total (774), veiculo (736), IPI (742), DIFAL (807), partilha e repasse do ICMS (741 e 740), MEI
  (782 e 337), frete (753), grupos de transporte (758 e 759), compra (762), meio de pagamento 99 (436) e os limites de
  valor (780, 750 e 752). `conferir` lista todas na frente. **De fora, por depender da SEFAZ ou do relogio:** o atraso
  da data de emissao (704), a UF que nao aceita a NFC-e (702) ou a entrega a domicilio (785) e o limite de valor por UF
  (a montagem usa o padrao da NT: R$ 10.000,00 sem destinatario e R$ 200.000,00 no total); **de fora, por nao haver o
  campo na montagem:** arma (738), RECOPI (348), cana (763), PIS e COFINS ST (746 e 749), II (743) e devolucao de
  tributos (390). As regras facultativas por UF (751, 391 e 392) e a G01-10 (670, "implementacao futura" na NT) nao
  sao aplicadas.
* **Contingencia off-line (`tp_emis` 9).** `tp_emis` 9, `dh_cont` e `x_just` (15 a 256 caracteres) no pedido; `idDest` 1,
  `finNFe` 1, `indFinal` 1 e `indPres` 1 (o MOC, Anexo IV, item 4, e as regras acima). O prazo para transmitir e' o fim
  do primeiro dia util depois da emissao: `rustnf::nfe::montagem::prazo_de_transmissao`. A nota segue com a MESMA chave.
* **QR Code e CSC.** O `infNFeSupl` (`qrCode` e `urlChave`) nao e' campo do pedido: entra na assinatura
  (`montagem::assinar` e `assinar_com`), que le o QR Code da nota montada. Sem CSC o QR Code e' o da versao 3 (NT
  2025.001: no off-line leva a assinatura RSA SHA-1 dos parametros, feita com o certificado A1 da nota). A versao 2
  usa o CSC (de 16 a 36 caracteres alfanumericos) e o `idCSC` que a SEFAZ da UF fornece pelo portal dela: o CSC **nao vai no
  JSON do pedido** (o pedido pode ser guardado, enviado ou colado num chamado), vai por argumento a
  `assinar_com(&montada, &certificado, &OpcoesQr { versao, csc })`. `Csc` esconde o valor no `Debug` e nenhuma mensagem de
  erro o repete; quem guarda o CSC e' a aplicacao, junto do certificado (segredo write-only, como a senha do A1). O
  emitente pessoa fisica (CPF, produtor rural) so' usa a versao 3 (rejeicao 444), e a NF-e com `tp_imp` 6 tambem (672), com a
  URL da NFC-e (regra ZX02-20, observacao 3). O endereco do QR Code e o da consulta por chave vem de
  `dados/nfce_qrcode.json`, com a fonte de cada linha; a UF que a tabela nao cobre (AM, MA e a homologacao de RR) recusa a
  assinatura com o motivo.

## Avisos da montagem

`montar` devolve, junto do XML, a lista `Montada::avisos` (e `avisos(&Nota)` a calcula sozinha). Aviso **nunca impede a
montagem e nao muda o XML**; o que o leiaute recusa continua sendo `ErroMontagem`. Cada aviso traz o tipo
(`TipoAviso::codigo`), o item (ou nenhum, quando e' da nota) e o texto.

| codigo | quando |
|---|---|
| `pis_cofins_regime` | Com `regime_pis_cofins` informado e emissao ate 2026: no `cumulativo`, CST 01 com aliquota diferente de 0,65% (PIS) ou 3% (COFINS) e CST de credito (50 a 56 e 60 a 67); no `nao_cumulativo`, CST 01 com aliquota diferente de 1,65% (PIS) ou 7,6% (COFINS), com a ressalva das receitas que a Lei 10.637, art. 8o, e a Lei 10.833, art. 10, mantem no cumulativo; emitente do Simples (CRT diferente de 3) com o campo informado. **Nao avisam**: aliquota zero, monofasico, substituicao, suspensao, isencao, sem incidencia (CST 02 a 09), outras operacoes (49 e 99) e aquisicoes (70 a 75 e 98). |
| `simples_sem_gibscbs` | CRT 1 sem `tpRegimeIBSCBS` e item com CST que exige o `gIBSCBS`: a montagem omite o grupo pela tabela 2.3 da NT 2026.005 v1.00, mas a regra UB13-30 da NT 2025.002 v1.53 (rejeicao 1022) o exige; as duas NT ainda nao se conciliam e a SEFAZ pode rejeitar. |

O pedido que traz campo que o leiaute nao conhece tem outro canal: `campos_desconhecidos`, antes de a `Nota` existir.

Cada tabela abaixo e' um bloco do pedido; o campo vai pelo caminho (`emit.endereco.uf`; `itens[]` e' a lista de itens).


## Nota (raiz)

| campo | o que e' |
|---|---|
| `modelo` | Modelo: 55 (NF-e) ou 65 (NFC-e). |
| `serie` | `serie`. |
| `numero` | `nNF`. |
| `emissao` | `dhEmi`, com fuso. |
| `nat_op` | `natOp`. |
| `cmun_fg` | `cMunFG`. |
| `emit` | Emitente. |
| `itens` | Itens (de 1 a 990). |
| `pagamentos` | Pagamentos (de 1 a 100). |
| `dest` | Destinatario. Opcional na NFC-e (consumidor nao identificado); a NF-e exige. |
| `tp_amb` | `tpAmb`: 1 producao, 2 homologacao. |
| `tp_emis` | `tpEmis`. |
| `tp_nf` | `tpNF`: 0 entrada, 1 saida. |
| `id_dest` | `idDest`. |
| `fin_nfe` | `finNFe`. |
| `ind_final` | `indFinal`; vazio = 0 na NF-e e 1 na NFC-e. |
| `ind_pres` | `indPres`; vazio = 9 na NF-e e 1 (presencial) na NFC-e. |
| `ind_intermed` | `indIntermed`. |
| `tp_imp` | `tpImp`; vazio = 1 na NF-e e 4 (DANFE NFC-e) na NFC-e. O 6 (DANFE Simplificado Tipo 2) leva o QR Code, como a NFC-e. |
| `proc_emi` | `procEmi`. |
| `ver_proc` | `verProc`; o pedido JSON sem ele leva `rustnf <versao>`. |
| `codigo` | `cNF` (8 digitos). Obrigatorio na montagem: ela nao sorteia, para a mesma entrada dar o mesmo XML (veja `chave_dfe::gerar_codigo`). |
| `saida` | `dhSaiEnt`. |
| `cmun_fg_ibs` | `cMunFGIBS`. |
| `dh_cont` | `dhCont`. |
| `x_just` | `xJust`. |
| `refs_nfe` | `NFref/refNFe`. |
| `troco` | `vTroco`. |
| `mod_frete` | `modFrete`. |
| `inf_ad_fisco` | `infAdFisco`. |
| `inf_cpl` | `infCpl`. |
| `resp_tec` | `infRespTec`. |
| `remover_acento` | Tira acento e caractere fora do ASCII dos textos (padrao do pedido JSON: sim). |
| `tp_nf_debito` | B25.1 `tpNFDebito` (NT 2025.002): "01" a "08", so' com `finNFe` 6. |
| `tp_nf_credito` | B25.2 `tpNFCredito`: "01" a "06", so' com `finNFe` 5. |
| `pag_antecipado` | BC02 `gPagAntecipado/refNFe`: chaves das notas de debito 06 a abater (de 1 a 99). |
| `compra_gov` | BB01 `gCompraGov`. O item leva o `gTribCompraGov` com o mesmo `pRedutor`. |
| `c_ind_op` | B25d `cIndOp` (NT 2025.002): so' codigo que o Anexo VII admite na NF-e ([`crate::reforma::indop`]). |
| `retirada` | F01: local de retirada (obrigatorio com `cIndOp` 010104 ou 010105, B25d-30). |
| `entrega` | G01: local de entrega. |
| `transp` | X03 a X34: transportadora, retencao do ICMS do transporte, veiculo, reboques, vagao, balsa e volumes. O `modFrete` e' `mod_frete`. |
| `cobr` | Y01 `cobr`: fatura e duplicatas. |
| `exporta` | ZA01 `exporta`: obrigatorio na exportacao (`tpNF` 1 e `idDest` 3, rejeicao 355) e proibido fora dela (356). |
| `compra` | ZB01 `compra`: nota de empenho, pedido e contrato. |
| `regime_pis_cofins` | `cumulativo` ou `nao_cumulativo`: o regime do PIS e da COFINS do emitente (CRT 3). **Nao vai ao XML**: so' alimenta o aviso de CST ou aliquota que nao combinam com o regime. Sem ele, sem aviso. |

## emit

| campo | o que e' |
|---|---|
| `emit.documento` | CNPJ (numerico ou alfanumerico) ou CPF. |
| `emit.nome` | `xNome`. |
| `emit.endereco` | `enderEmit`. |
| `emit.crt` | `CRT`: 1 Simples Nacional, 2 Simples com excesso de sublimite, 3 regime normal, 4 MEI. |
| `emit.ie` | `IE`. |
| `emit.fantasia` | `xFant`. |
| `emit.iest` | `IEST`. |
| `emit.im` | `IM`. |
| `emit.cnae` | `CNAE`, so' sai com a inscricao municipal. |
| `emit.tp_regime_ibscbs` | C21a, NT 2026.005: 1 Simples com regime regular do IBS/CBS; 2, 3 e 4 regime normal (CRT 3) com a opcao pelo Simples pendente. Ausente para os demais. |
| `emit.isuf` | C22 `ISUFEmit` (NT 2025.002): inscricao na Suframa do emitente da ZFM ou das ALC. |

## emit.endereco

| campo | o que e' |
|---|---|
| `emit.endereco.logradouro` | `xLgr`. |
| `emit.endereco.numero` | `nro`. |
| `emit.endereco.bairro` | `xBairro`. |
| `emit.endereco.cod_municipio` | `cMun`: codigo IBGE, 7 digitos. |
| `emit.endereco.municipio` | `xMun`. |
| `emit.endereco.uf` | `UF`; `EX` para o exterior. |
| `emit.endereco.cep` | `CEP`, 8 digitos. |
| `emit.endereco.complemento` | `xCpl`. |
| `emit.endereco.cod_pais` | `cPais`; vazio = 1058 (Brasil) no emitente e no destinatario. |
| `emit.endereco.pais` | `xPais`; vazio = BRASIL no emitente e no destinatario. |
| `emit.endereco.fone` | `fone`. |

## itens[]

| campo | o que e' |
|---|---|
| `itens[].codigo` | `cProd`. |
| `itens[].descricao` | `xProd`. |
| `itens[].ncm` | `NCM`: 2 ou 8 digitos. |
| `itens[].cfop` | `CFOP`, 4 digitos. Conferido contra a Tabela de CFOP do Portal (`cfop`): na NF-e 55 com `fin_nfe` 4, ou nota de credito (`fin_nfe` 5) de `tp_nf_credito` 03 ou 06, so' CFOP de devolucao (`indDevol` 1; rejeicao 327); o 1.949 e o 2.949 so' no `fin_nfe` 4; o 5.949 e o 6.949 so' com o NCM 27112100 (gas natural); emitente MEI (`crt` 4) com `id_dest` diferente de 3, nessa mesma situacao, so' 1.202, 1.553, 2.202, 2.553, 5.202 e 6.202 (1179); fora de `fin_nfe` 2 e 4 o CFOP de devolucao e' recusado (328), salvo na nota de credito 03, 04 e 06. As tres regras valem so' para o modelo 55. CFOP fora da tabela, fora da vigencia na emissao ou de combustivel (`indComb` 1 ou 2) sem o grupo `combustivel` nao sao recusados: saem em `Montada.avisos` (770 e 660, que o MOC deixa a criterio da UF). |
| `itens[].unidade` | `uCom`. |
| `itens[].quantidade` | `qCom`. |
| `itens[].valor_unitario` | `vUnCom`. |
| `itens[].icms` | ICMS. Ausente so' na nota de credito ou de debito (`finNFe` 5 ou 6, B25-80); nas demais e' a rejeicao 1002. |
| `itens[].pis` | PIS. Ausente so' na nota de credito ou de debito; nas demais e' a rejeicao 745. |
| `itens[].cofins` | COFINS. Ausente so' na nota de credito ou de debito; nas demais e' a rejeicao 748. |
| `itens[].valor_produto` | `vProd`; vazio = `qCom` x `vUnCom` arredondado a 2 casas. |
| `itens[].gtin` | `cEAN`; vazio = `SEM GTIN`. |
| `itens[].gtin_trib` | `cEANTrib`; vazio = o `cEAN`. |
| `itens[].unidade_trib` | `uTrib`; vazio = `uCom`. |
| `itens[].quantidade_trib` | `qTrib`; vazio = `qCom`. |
| `itens[].valor_unitario_trib` | `vUnTrib`; vazio = `vUnCom`. |
| `itens[].cest` | `CEST`. |
| `itens[].cbenef` | `cBenef`. |
| `itens[].extipi` | `EXTIPI`. |
| `itens[].frete` | `vFrete`. |
| `itens[].seguro` | `vSeg`. |
| `itens[].desconto` | `vDesc`. |
| `itens[].outros` | `vOutro`. |
| `itens[].ind_tot` | `indTot`: 0 ou 1. |
| `itens[].ipi` | IPI. |
| `itens[].ibscbs` | IBS/CBS do item, ja' calculado ([`crate::reforma::calculo::ibscbs_do_item`]). |
| `itens[].v_tot_trib` | `vTotTrib`. |
| `itens[].inf_ad_prod` | `infAdProd`. |
| `itens[].x_ped` | `xPed`. |
| `itens[].n_item_ped` | `nItemPed`. |
| `itens[].dfe_ref` | `DFeReferenciado`: chave do documento e `nItem` dele (devolucao, VC02-14). |
| `itens[].valor_unitario_liquido` | I11b `vUnComLiq` (NT 2026.008): valor unitario sem tributo; abre a sequencia I11a. |
| `itens[].valor_produto_liquido` | I11c `vProdLiq`; vazio = `qCom` x `vUnComLiq` arredondado a 2 casas. |
| `itens[].c_ativ_sn` | I17d `cAtivSN` (NT 2026.005): 01, 02, 03, 04 ou 90; so' emitente do Simples ou com a opcao pendente. |
| `itens[].v_icms_previsto` | NB02 `vICMSPrevisto` (NT 2026.008): ICMS do fornecimento futuro, no lugar do grupo de ICMS; so' com `tpNFDebito` 06 ou recebimento antecipado de compra governamental. |
| `itens[].icms_uf_dest` | NA01 `ICMSUFDest` (DIFAL), calculado por [`difal::calcular`]. |
| `itens[].difal_dispensado` | Dispensa do DIFAL pelas excecoes da rejeicao 694: `cfop_retorno` ou `cfop_remessa`. |
| `itens[].veiculo` | J `veicProd`. |
| `itens[].combustivel` | LA `comb`: combustivel liquido e lubrificante. Um grupo especifico so' por item (`veicProd`, `med` ou `comb`). |
| `itens[].med` | K `med`: medicamento. Exige o `rastro` (rejeicao 873), salvo as excecoes da regra K01-20. |
| `itens[].rastro` | I80 `rastro`: lotes do item, de 0 a 500. |

## itens[].icms

| campo | o que e' |
|---|---|
| `itens[].icms.codigo` | CST (2 digitos) ou CSOSN (3 digitos), conforme o CRT do emitente. Os CST do ICMS monofasico (02, 15, 53 e 61) valem tambem no CRT 1. |
| `itens[].icms.orig` | `orig`: 0 a 8. |
| `itens[].icms.mod_bc` | `modBC`. |
| `itens[].icms.vbc` | `vBC`. |
| `itens[].icms.p_red_bc` | `pRedBC`. |
| `itens[].icms.p_icms` | `pICMS`. |
| `itens[].icms.v_icms` | `vICMS`. |
| `itens[].icms.vbc_fcp` | `vBCFCP`. |
| `itens[].icms.p_fcp` | `pFCP`. |
| `itens[].icms.v_fcp` | `vFCP`. |
| `itens[].icms.vbc_st_ret` | `vBCSTRet`. |
| `itens[].icms.p_st` | `pST`. |
| `itens[].icms.v_icms_substituto` | `vICMSSubstituto`. |
| `itens[].icms.v_icms_st_ret` | `vICMSSTRet`. |
| `itens[].icms.p_cred_sn` | `pCredSN`. |
| `itens[].icms.v_cred_icms_sn` | `vCredICMSSN`. |
| `itens[].icms.mod_bc_st` | ST propria: CST 10, 30 e 70 e CSOSN 201, 202 e 203 exigem; CST 90 e CSOSN 900 levam se informada. N18: 0 tabelado, 1 lista negativa, 2 positiva, 3 neutra, 4 MVA, 5 pauta, 6 valor da operacao. |
| `itens[].icms.p_mva_st` | N19 `pMVAST`: so' com `modBCST` 4 (rejeicoes 932 e 933). |
| `itens[].icms.p_red_bc_st` | `pRedBCST`. |
| `itens[].icms.vbc_st` | `vBCST`. |
| `itens[].icms.p_icms_st` | N22 `pICMSST`, sem o FCP. |
| `itens[].icms.v_icms_st` | `vICMSST`. |
| `itens[].icms.vbc_fcp_st` | N23a `vBCFCPST`: FCP retido por ST (grupo opcional). |
| `itens[].icms.p_fcp_st` | N23b `pFCPST` (zero e' a rejeicao 881). |
| `itens[].icms.v_fcp_st` | N23d `vFCPST`. |
| `itens[].icms.v_icms_op` | Diferimento (CST 51), todos opcionais: N16a `vICMSOp`, o ICMS sem o diferimento. |
| `itens[].icms.p_dif` | N16b `pDif`: percentual do diferimento (100 = total). |
| `itens[].icms.v_icms_dif` | N16c `vICMSDif`: `vICMSOp` x `pDif`. |
| `itens[].icms.p_fcp_dif` | N17d `pFCPDif`. |
| `itens[].icms.v_fcp_dif` | N17e `vFCPDif`. |
| `itens[].icms.v_fcp_efet` | N17f `vFCPEfet`. |
| `itens[].icms.v_icms_deson` | N28a `vICMSDeson`: CST 20, 30, 40, 41, 50, 70 e 90; sai so' com valor maior que zero. |
| `itens[].icms.mot_des_icms` | N28 `motDesICMS`: codigo do motivo, na enumeracao do XSD de cada grupo. |
| `itens[].icms.ind_deduz_deson` | N28b `indDeduzDeson`: 1 = o `vICMSDeson` sai do `vItem` e do `vNF` (W16-10, excecao 4). |
| `itens[].icms.v_icms_st_deson` | N33a `vICMSSTDeson`: ICMS-ST desonerado (CST 10, 70 e 90); nao tem total no `ICMSTot`. |
| `itens[].icms.mot_des_icms_st` | N33b `motDesICMSST`. |
| `itens[].icms.partilha` | Partilha do ICMS entre a UF de origem e a do destino (grupo `ICMSPart`): CST 10 e 90. Exige o ICMS proprio e a ST, sem FCP proprio. |
| `itens[].icms.p_bc_op` | N25 `pBCOp`. |
| `itens[].icms.uf_st` | N24 `UFST` (`EX` = exterior). |
| `itens[].icms.repasse` | Repasse do ICMS-ST retido (grupo `ICMSST`): CST 41 e 60. |
| `itens[].icms.vbc_st_dest` | N31 `vBCSTDest`. |
| `itens[].icms.v_icms_st_dest` | N32 `vICMSSTDest`. |
| `itens[].icms.vbc_fcp_st_ret` | N27a `vBCFCPSTRet`: FCP retido anteriormente por ST (os tres campos ou nenhum). |
| `itens[].icms.p_fcp_st_ret` | N27b `pFCPSTRet`. |
| `itens[].icms.v_fcp_st_ret` | N27d `vFCPSTRet`. |
| `itens[].icms.p_red_bc_efet` | N34 `pRedBCEfet`: ICMS efetivo (os quatro campos ou nenhum). |
| `itens[].icms.vbc_efet` | N35 `vBCEfet`. |
| `itens[].icms.p_icms_efet` | N36 `pICMSEfet`. |
| `itens[].icms.v_icms_efet` | N37 `vICMSEfet`. |
| `itens[].icms.c_benef_rbc` | N14a `cBenefRBC` (NT 2019.001): beneficio fiscal da reducao de base quando acumula com o diferimento (CST 51) ou no CST 90. Confere-se o formato do XSD; se o codigo existe na tabela da UF e' assunto da UF. |
| `itens[].icms.q_bc_mono` | N37a `qBCMono`: quantidade tributada do ICMS proprio, ate 11 inteiros e 4 casas (CST 02, 15 e 53). Obrigatoria (rejeicao 767). |
| `itens[].icms.ad_rem_icms` | N38 `adRemICMS`: aliquota ad rem do ICMS proprio, ate 3 inteiros e 4 casas (CST 02, 15 e 53; obrigatoria no 02 e no 15). |
| `itens[].icms.v_icms_mono` | N39 `vICMSMono`: valor do ICMS proprio (CST 02, 15 e 53). No 02 e no 15, na nota normal (`finNFe` 1), e' `qBCMono` x `adRemICMS` arredondado a 2 casas, com tolerancia de 0,01 (rejeicao 962). No 53 e' o devido (operacao menos diferido) e a tag so' sai com valor. |
| `itens[].icms.q_bc_mono_reten` | N39a `qBCMonoReten`: quantidade sujeita a retencao (CST 15). Obrigatoria (rejeicao 768). |
| `itens[].icms.ad_rem_icms_reten` | N40 `adRemICMSReten`: aliquota ad rem do ICMS com retencao (CST 15; obrigatoria). |
| `itens[].icms.v_icms_mono_reten` | N41 `vICMSMonoReten`: valor do ICMS com retencao (CST 15; obrigatorio). Na nota normal e' `qBCMonoReten` x `adRemICMSReten` (x 0,3333 nos codigos ANP do biodiesel e do diesel B da regra N41-20), arredondado a 2 casas, tolerancia de 0,01 (rejeicao 964). Soma no `vItem` e no `vNF`. |
| `itens[].icms.p_red_ad_rem` | N47 `pRedAdRem`: percentual de reducao da aliquota ad rem, de 0 a 100 com 2 casas (CST 15); vai junto com `mot_red_ad_rem`. |
| `itens[].icms.mot_red_ad_rem` | N48 `motRedAdRem`: 1 transporte coletivo de passageiros, 9 outros (CST 15). |
| `itens[].icms.v_icms_mono_op` | N41a `vICMSMonoOp`: ICMS da operacao como se nao houvesse o diferimento (CST 53). O percentual do diferimento (N42 `pDif`, ate 100) e' o `p_dif` do grupo. |
| `itens[].icms.v_icms_mono_dif` | N43 `vICMSMonoDif`: ICMS diferido (CST 53). |
| `itens[].icms.q_bc_mono_ret` | N43a `qBCMonoRet`: quantidade tributada retida anteriormente (CST 61). Obrigatoria (rejeicao 769). |
| `itens[].icms.ad_rem_icms_ret` | N44 `adRemICMSRet`: aliquota ad rem do ICMS retido anteriormente (CST 61; obrigatoria). |
| `itens[].icms.v_icms_mono_ret` | N45 `vICMSMonoRet`: valor do ICMS retido anteriormente (CST 61; obrigatorio; sem rejeicao de conta, a NT revogou a 966). |

O **ICMS monofasico sobre combustiveis** (NT 2023.001 v1.60) usa os CST 02 (proprio), 15 (proprio com retencao), 53
(recolhimento diferido) e 61 (cobrado anteriormente), com os campos `q_bc_mono*`, `ad_rem_icms*` e `v_icms_mono*` acima, e
o emitente do Simples (CRT 1) tambem os informa (regra N12-20, excecao). O item leva o grupo `combustivel` (sem ele, rejeicao
959) e nao leva `vbc`, `p_icms`, `v_icms` nem os campos de outros CST. A saida a nao contribuinte (`ind_ie_dest` 9) so' aceita o
61 (e o 02 na operacao interna): CST 15 ou 53, ou 02 interestadual, e' a rejeicao 508. A nota de entrada (`tp_nf` 0) nao tem
essa recusa. Os seis totais (`qBCMono`, `vICMSMono`, `qBCMonoReten`, `vICMSMonoReten`, `qBCMonoRet`, `vICMSMonoRet`) saem no
`ICMSTot` so' com valor, somados dos itens. As rejeicoes 961, 963, 965 e 966 estao revogadas na NT e nao sao conferidas; a
comparacao do `cProdANP` com a Tabela de Combustiveis Sujeitos a Tributacao Monofasica (959 pela tabela e 960) fica fora, porque
a tabela e' publicada a parte.

## itens[].pis

| campo | o que e' |
|---|---|
| `itens[].pis.cst` | CST: 01 e 02 (aliquota), 04 a 09 (nao tributado), 49 a 99 (outras operacoes). |
| `itens[].pis.vbc` | `vBC`. |
| `itens[].pis.aliquota` | `pPIS` ou `pCOFINS`. |
| `itens[].pis.valor` | `vPIS` ou `vCOFINS`. |

## itens[].cofins

| campo | o que e' |
|---|---|
| `itens[].cofins.cst` | CST: 01 e 02 (aliquota), 04 a 09 (nao tributado), 49 a 99 (outras operacoes). |
| `itens[].cofins.vbc` | `vBC`. |
| `itens[].cofins.aliquota` | `pPIS` ou `pCOFINS`. |
| `itens[].cofins.valor` | `vPIS` ou `vCOFINS`. |

## itens[].ipi

| campo | o que e' |
|---|---|
| `itens[].ipi.cst` | CST: 00, 49, 50 e 99 tributam; 01 a 05 e 51 a 55 nao. |
| `itens[].ipi.c_enq` | `cEnq`: 3 digitos. |
| `itens[].ipi.vbc` | `vBC`. |
| `itens[].ipi.aliquota` | `pIPI`. |
| `itens[].ipi.valor` | `vIPI`. |

## itens[].ibscbs

| campo | o que e' |
|---|---|
| `itens[].ibscbs.cst` | CST do IBS/CBS (tabela oficial da NT 2025.002). |
| `itens[].ibscbs.cclass_trib` | cClassTrib (tabela oficial); tem de pertencer ao CST e valer na NF-e. |
| `itens[].ibscbs.vbc` | Base de calculo do IBS/CBS do item (decimal em texto). |
| `itens[].ibscbs.data` | Data do fato gerador (AAAA-MM-DD): decide as aliquotas e a vigencia da tabela. |
| `itens[].ibscbs.aliquotas` | Aliquotas informadas (percentual em texto) no lugar das do calendario da lei. |
| `itens[].ibscbs.publicadas` | Aliquotas publicadas por ano (resolucao do Senado); objeto livre, veja `reforma::aliquotas`. |
| `itens[].ibscbs.simples_nacional` | Operacao de optante do Simples Nacional (as aliquotas de 2026 nao se aplicam a ele). |
| `itens[].ibscbs.p_dif` | Percentual de diferimento: texto (o mesmo para os tres entes) ou objeto com `ibs_uf`, `ibs_mun` e `cbs`. |
| `itens[].ibscbs.p_dev_cbs` | Percentual de devolucao de tributo da CBS (`gDevTrib`). |
| `itens[].ibscbs.trib_regular` | Tributacao regular (`gTribRegular`), exigida por alguns cClassTrib. |
| `itens[].ibscbs.cred_pres` | Credito presumido da operacao (`gCredPresOper`). |
| `itens[].ibscbs.deduz_cred_pres` | O credito presumido deduz do valor do IBS/CBS do item. |
| `itens[].ibscbs.estorno` | Estorno de credito (`gEstornoCred`). |
| `itens[].ibscbs.mono` | Monofasia (`gIBSCBSMono`), CST 620. |
| `itens[].ibscbs.transf_cred` | Transferencia de credito (`gTransfCred`), CST 800. |
| `itens[].ibscbs.ajuste_compet` | Ajuste de competencia (`gAjusteCompet`), CST 811. |
| `itens[].ibscbs.cred_pres_zfm` | Credito presumido de IBS na Zona Franca (`gCredPresIBSZFM`). |
| `itens[].ibscbs.ind_doacao` | Operacao de doacao (`indDoacao`). |
| `itens[].ibscbs.tp_nf_debito` | `tpNFDebito` da nota, quando o grupo depende dele (UB13-30). |
| `itens[].ibscbs.compra_gov` | Composicao do tributo na compra governamental (`gTribCompraGov`). |
| `itens[].ibscbs.validar_vigencia` | Confere a vigencia do cClassTrib na data (padrao: sim). |
| `itens[].ibscbs.trib_sn` | `gTribSN` do Simples Nacional (NT 2026.005): aliquota efetiva e valor prontos. |
| `itens[].ibscbs.fin_nfe` | `finNFe` da nota (vem dela quando ausente). |
| `itens[].ibscbs.tp_nf_credito` | `tpNFCredito` da nota (vem dela quando ausente). |
| `itens[].ibscbs.cbs_zero_alc_zfm` | pCBS zero na area incentivada ZFM/ALC (UB56-10, excecao 3). |

## itens[].ibscbs.aliquotas

| campo | o que e' |
|---|---|
| `itens[].ibscbs.aliquotas.ibs_uf` | Aliquota do IBS estadual. |
| `itens[].ibscbs.aliquotas.ibs_mun` | Aliquota do IBS municipal. |
| `itens[].ibscbs.aliquotas.cbs` | Aliquota da CBS. |

## itens[].ibscbs.p_dif

| campo | o que e' |
|---|---|
| `itens[].ibscbs.p_dif.ibs_uf` | Diferimento do IBS estadual. |
| `itens[].ibscbs.p_dif.ibs_mun` | Diferimento do IBS municipal. |
| `itens[].ibscbs.p_dif.cbs` | Diferimento da CBS. |

## itens[].ibscbs.trib_regular

| campo | o que e' |
|---|---|
| `itens[].ibscbs.trib_regular.cst` | CST da tributacao regular. |
| `itens[].ibscbs.trib_regular.cclass_trib` | cClassTrib da tributacao regular. |
| `itens[].ibscbs.trib_regular.ibs_uf` | Aliquota efetiva regular do IBS estadual (vazio = calcula). |
| `itens[].ibscbs.trib_regular.ibs_mun` | Aliquota efetiva regular do IBS municipal (vazio = calcula). |
| `itens[].ibscbs.trib_regular.cbs` | Aliquota efetiva regular da CBS (vazio = calcula). |

## itens[].ibscbs.cred_pres

| campo | o que e' |
|---|---|
| `itens[].ibscbs.cred_pres.c_cred_pres` | Codigo do credito presumido (`cCredPres`, 2 digitos; tem de existir na tabela cCredPres v1.70, rejeicao 1055). |
| `itens[].ibscbs.cred_pres.vbc` | Base do credito presumido (vazio = a base do item). |
| `itens[].ibscbs.cred_pres.ibs` | Credito presumido do IBS (so' nos codigos que o exigem; informado onde nao cabe = 1053, ausente onde e' exigido = 1054). |
| `itens[].ibscbs.cred_pres.cbs` | Credito presumido da CBS (so' nos codigos que a exigem; informado onde nao cabe = 1050, ausente onde e' exigida = 1058). |

## itens[].ibscbs.cred_pres.ibs

| campo | o que e' |
|---|---|
| `itens[].ibscbs.cred_pres.ibs.p` | Percentual do credito presumido do IBS. |
| `itens[].ibscbs.cred_pres.ibs.cond_sus` | Credito com condicao suspensiva. |

## itens[].ibscbs.cred_pres.cbs

| campo | o que e' |
|---|---|
| `itens[].ibscbs.cred_pres.cbs.p` | Percentual do credito presumido da CBS. |
| `itens[].ibscbs.cred_pres.cbs.cond_sus` | Credito com condicao suspensiva. |

## itens[].ibscbs.estorno

| campo | o que e' |
|---|---|
| `itens[].ibscbs.estorno.ibs` | Valor do IBS estornado. |
| `itens[].ibscbs.estorno.cbs` | Valor da CBS estornada. |

## itens[].ibscbs.mono

| campo | o que e' |
|---|---|
| `itens[].ibscbs.mono.ibs` | Monofasia do IBS. |
| `itens[].ibscbs.mono.cbs` | Monofasia da CBS. |

## itens[].ibscbs.mono.ibs

| campo | o que e' |
|---|---|
| `itens[].ibscbs.mono.ibs.padrao` | Subgrupo padrao (`gMonoPadrao`). |
| `itens[].ibscbs.mono.ibs.reten` | Subgrupo de retencao (`gMonoReten`). |
| `itens[].ibscbs.mono.ibs.ret` | Subgrupo retido (`gMonoRet`). |
| `itens[].ibscbs.mono.ibs.pbio` | Diferenca do biocombustivel (`gpBioDiferenca`). |

## itens[].ibscbs.mono.ibs.padrao

| campo | o que e' |
|---|---|
| `itens[].ibscbs.mono.ibs.padrao.q` | Quantidade. |
| `itens[].ibscbs.mono.ibs.padrao.ad_rem` | Aliquota ad rem. |
| `itens[].ibscbs.mono.ibs.padrao.vbc` | Base (ad valorem). |
| `itens[].ibscbs.mono.ibs.padrao.p_uf` | Aliquota estadual. |
| `itens[].ibscbs.mono.ibs.padrao.p_mun` | Aliquota municipal. |
| `itens[].ibscbs.mono.ibs.padrao.p` | Aliquota. |
| `itens[].ibscbs.mono.ibs.padrao.v` | Valor pronto. |

## itens[].ibscbs.mono.ibs.reten

| campo | o que e' |
|---|---|
| `itens[].ibscbs.mono.ibs.reten.q` | Quantidade. |
| `itens[].ibscbs.mono.ibs.reten.ad_rem` | Aliquota ad rem. |
| `itens[].ibscbs.mono.ibs.reten.vbc` | Base (ad valorem). |
| `itens[].ibscbs.mono.ibs.reten.p_uf` | Aliquota estadual. |
| `itens[].ibscbs.mono.ibs.reten.p_mun` | Aliquota municipal. |
| `itens[].ibscbs.mono.ibs.reten.p` | Aliquota. |
| `itens[].ibscbs.mono.ibs.reten.v` | Valor pronto. |

## itens[].ibscbs.mono.ibs.ret

| campo | o que e' |
|---|---|
| `itens[].ibscbs.mono.ibs.ret.q` | Quantidade. |
| `itens[].ibscbs.mono.ibs.ret.ad_rem` | Aliquota ad rem. |
| `itens[].ibscbs.mono.ibs.ret.vbc` | Base (ad valorem). |
| `itens[].ibscbs.mono.ibs.ret.p_uf` | Aliquota estadual. |
| `itens[].ibscbs.mono.ibs.ret.p_mun` | Aliquota municipal. |
| `itens[].ibscbs.mono.ibs.ret.p` | Aliquota. |
| `itens[].ibscbs.mono.ibs.ret.v` | Valor pronto. |

## itens[].ibscbs.mono.ibs.pbio

| campo | o que e' |
|---|---|
| `itens[].ibscbs.mono.ibs.pbio.q` | Quantidade. |
| `itens[].ibscbs.mono.ibs.pbio.ad_rem` | Aliquota ad rem. |
| `itens[].ibscbs.mono.ibs.pbio.vbc` | Base (ad valorem). |
| `itens[].ibscbs.mono.ibs.pbio.p_uf` | Aliquota estadual. |
| `itens[].ibscbs.mono.ibs.pbio.p_mun` | Aliquota municipal. |
| `itens[].ibscbs.mono.ibs.pbio.p` | Aliquota. |
| `itens[].ibscbs.mono.ibs.pbio.v` | Valor pronto. |

## itens[].ibscbs.mono.cbs

| campo | o que e' |
|---|---|
| `itens[].ibscbs.mono.cbs.padrao` | Subgrupo padrao (`gMonoPadrao`). |
| `itens[].ibscbs.mono.cbs.reten` | Subgrupo de retencao (`gMonoReten`). |
| `itens[].ibscbs.mono.cbs.ret` | Subgrupo retido (`gMonoRet`). |
| `itens[].ibscbs.mono.cbs.pbio` | Diferenca do biocombustivel (`gpBioDiferenca`). |

## itens[].ibscbs.mono.cbs.padrao

| campo | o que e' |
|---|---|
| `itens[].ibscbs.mono.cbs.padrao.q` | Quantidade. |
| `itens[].ibscbs.mono.cbs.padrao.ad_rem` | Aliquota ad rem. |
| `itens[].ibscbs.mono.cbs.padrao.vbc` | Base (ad valorem). |
| `itens[].ibscbs.mono.cbs.padrao.p_uf` | Aliquota estadual. |
| `itens[].ibscbs.mono.cbs.padrao.p_mun` | Aliquota municipal. |
| `itens[].ibscbs.mono.cbs.padrao.p` | Aliquota. |
| `itens[].ibscbs.mono.cbs.padrao.v` | Valor pronto. |

## itens[].ibscbs.mono.cbs.reten

| campo | o que e' |
|---|---|
| `itens[].ibscbs.mono.cbs.reten.q` | Quantidade. |
| `itens[].ibscbs.mono.cbs.reten.ad_rem` | Aliquota ad rem. |
| `itens[].ibscbs.mono.cbs.reten.vbc` | Base (ad valorem). |
| `itens[].ibscbs.mono.cbs.reten.p_uf` | Aliquota estadual. |
| `itens[].ibscbs.mono.cbs.reten.p_mun` | Aliquota municipal. |
| `itens[].ibscbs.mono.cbs.reten.p` | Aliquota. |
| `itens[].ibscbs.mono.cbs.reten.v` | Valor pronto. |

## itens[].ibscbs.mono.cbs.ret

| campo | o que e' |
|---|---|
| `itens[].ibscbs.mono.cbs.ret.q` | Quantidade. |
| `itens[].ibscbs.mono.cbs.ret.ad_rem` | Aliquota ad rem. |
| `itens[].ibscbs.mono.cbs.ret.vbc` | Base (ad valorem). |
| `itens[].ibscbs.mono.cbs.ret.p_uf` | Aliquota estadual. |
| `itens[].ibscbs.mono.cbs.ret.p_mun` | Aliquota municipal. |
| `itens[].ibscbs.mono.cbs.ret.p` | Aliquota. |
| `itens[].ibscbs.mono.cbs.ret.v` | Valor pronto. |

## itens[].ibscbs.mono.cbs.pbio

| campo | o que e' |
|---|---|
| `itens[].ibscbs.mono.cbs.pbio.q` | Quantidade. |
| `itens[].ibscbs.mono.cbs.pbio.ad_rem` | Aliquota ad rem. |
| `itens[].ibscbs.mono.cbs.pbio.vbc` | Base (ad valorem). |
| `itens[].ibscbs.mono.cbs.pbio.p_uf` | Aliquota estadual. |
| `itens[].ibscbs.mono.cbs.pbio.p_mun` | Aliquota municipal. |
| `itens[].ibscbs.mono.cbs.pbio.p` | Aliquota. |
| `itens[].ibscbs.mono.cbs.pbio.v` | Valor pronto. |

## itens[].ibscbs.transf_cred

| campo | o que e' |
|---|---|
| `itens[].ibscbs.transf_cred.ibs` | Valor do IBS transferido. |
| `itens[].ibscbs.transf_cred.cbs` | Valor da CBS transferida. |

## itens[].ibscbs.ajuste_compet

| campo | o que e' |
|---|---|
| `itens[].ibscbs.ajuste_compet.compet_apur` | Competencia de apuracao (AAAA-MM). |
| `itens[].ibscbs.ajuste_compet.ibs` | Valor do IBS do ajuste. |
| `itens[].ibscbs.ajuste_compet.cbs` | Valor da CBS do ajuste. |

## itens[].ibscbs.cred_pres_zfm

| campo | o que e' |
|---|---|
| `itens[].ibscbs.cred_pres_zfm.compet_apur` | Competencia de apuracao. |
| `itens[].ibscbs.cred_pres_zfm.tp` | Tipo (`tpCredPresIBSZFM`, 0 a 4). |
| `itens[].ibscbs.cred_pres_zfm.v` | Valor do credito. |

## itens[].ibscbs.compra_gov

| campo | o que e' |
|---|---|
| `itens[].ibscbs.compra_gov.p_redutor` | Redutor do art. 472 da LC 214/2025 (o mesmo da nota). |
| `itens[].ibscbs.compra_gov.trib` | Aliquota e valor de cada ente. |

## itens[].ibscbs.compra_gov.trib

| campo | o que e' |
|---|---|
| `itens[].ibscbs.compra_gov.trib.ibs_uf` | Ente da compra governamental. |
| `itens[].ibscbs.compra_gov.trib.ibs_mun` | Ente da compra governamental. |
| `itens[].ibscbs.compra_gov.trib.cbs` | Ente da compra governamental. |

## itens[].ibscbs.compra_gov.trib.ibs_uf

| campo | o que e' |
|---|---|
| `itens[].ibscbs.compra_gov.trib.ibs_uf.p` | Aliquota (percentual). |
| `itens[].ibscbs.compra_gov.trib.ibs_uf.v` | Valor do tributo. |

## itens[].ibscbs.compra_gov.trib.ibs_mun

| campo | o que e' |
|---|---|
| `itens[].ibscbs.compra_gov.trib.ibs_mun.p` | Aliquota (percentual). |
| `itens[].ibscbs.compra_gov.trib.ibs_mun.v` | Valor do tributo. |

## itens[].ibscbs.compra_gov.trib.cbs

| campo | o que e' |
|---|---|
| `itens[].ibscbs.compra_gov.trib.cbs.p` | Aliquota (percentual). |
| `itens[].ibscbs.compra_gov.trib.cbs.v` | Valor do tributo. |

## itens[].ibscbs.trib_sn

| campo | o que e' |
|---|---|
| `itens[].ibscbs.trib_sn.ibs` | `gIBSSN`. |
| `itens[].ibscbs.trib_sn.cbs` | `gCBSSN`. |

## itens[].ibscbs.trib_sn.ibs

| campo | o que e' |
|---|---|
| `itens[].ibscbs.trib_sn.ibs.p` | Aliquota efetiva do Simples (percentual). |
| `itens[].ibscbs.trib_sn.ibs.v` | Valor. |

## itens[].ibscbs.trib_sn.cbs

| campo | o que e' |
|---|---|
| `itens[].ibscbs.trib_sn.cbs.p` | Aliquota efetiva do Simples (percentual). |
| `itens[].ibscbs.trib_sn.cbs.v` | Valor. |

## itens[].icms_uf_dest

| campo | o que e' |
|---|---|
| `itens[].icms_uf_dest.vbc` | Base do ICMS em centavos (inteiro). |
| `itens[].icms_uf_dest.aliquota_interna` | Aliquota interna da UF do destino (percentual em texto). |
| `itens[].icms_uf_dest.aliquota_inter` | Aliquota interestadual: 4, 7 ou 12. |
| `itens[].icms_uf_dest.ano` | Ano do fato gerador (decide a partilha). |
| `itens[].icms_uf_dest.fcp` | Aliquota do FCP da UF do destino (percentual em texto, ate 2). |
| `itens[].icms_uf_dest.vbc_uf_dest` | Base do destino em centavos, quando diferente da do remetente. |

## itens[].veiculo

| campo | o que e' |
|---|---|
| `itens[].veiculo.tp_op` | `tpOp`: 0 outros, 1 concessionaria, 2 faturamento direto, 3 venda direta a grande consumidor. No faturamento direto o `vNF` e o `vItem` nao somam o ICMS-ST (W16-10, excecao 1) e a nota exige o local de entrega (rejeicao 478). |
| `itens[].veiculo.chassi` | `chassi`: 17 posicoes. |
| `itens[].veiculo.c_cor` | `cCor`. |
| `itens[].veiculo.x_cor` | `xCor`. |
| `itens[].veiculo.pot` | `pot`. |
| `itens[].veiculo.cilin` | `cilin`. |
| `itens[].veiculo.peso_l` | `pesoL`. |
| `itens[].veiculo.peso_b` | `pesoB`. |
| `itens[].veiculo.n_serie` | `nSerie`. |
| `itens[].veiculo.tp_comb` | `tpComb`. |
| `itens[].veiculo.n_motor` | `nMotor`. |
| `itens[].veiculo.cmt` | `CMT`. |
| `itens[].veiculo.dist` | `dist`. |
| `itens[].veiculo.ano_mod` | `anoMod`. |
| `itens[].veiculo.ano_fab` | `anoFab`. |
| `itens[].veiculo.tp_pint` | `tpPint`. |
| `itens[].veiculo.tp_veic` | `tpVeic`. |
| `itens[].veiculo.esp_veic` | `espVeic`. |
| `itens[].veiculo.vin` | `VIN`: `R` remarcado ou `N` normal. |
| `itens[].veiculo.cond_veic` | `condVeic`. |
| `itens[].veiculo.c_mod` | `cMod`. |
| `itens[].veiculo.c_cor_denatran` | `cCorDENATRAN`. |
| `itens[].veiculo.lota` | `lota`. |
| `itens[].veiculo.tp_rest` | `tpRest`. |

## itens[].combustivel

| campo | o que e' |
|---|---|
| `itens[].combustivel.c_prod_anp` | LA02 `cProdANP`: 9 digitos (codificacao do SIMP, da ANP). |
| `itens[].combustivel.desc_anp` | LA03 `descANP`: de 2 a 95 caracteres. |
| `itens[].combustivel.uf_cons` | LA06 `UFCons`: sigla da UF de consumo, ou `EX`. |
| `itens[].combustivel.p_glp` | LA03a `pGLP`: so' no GLP (`cProdANP` 210203001); ausente = 0. Em outro produto e' a rejeicao 461. |
| `itens[].combustivel.p_gn_n` | LA03b `pGNn`: so' no GLP; ausente = 0. |
| `itens[].combustivel.p_gn_i` | LA03c `pGNi`: so' no GLP; ausente = 0. `pGLP`, `pGNn` e `pGNi` somam 100 (rejeicao 855). |
| `itens[].combustivel.v_part` | LA03d `vPart`: so' no GLP, valor por quilograma sem ICMS; obrigatorio nele (rejeicao 856). O GLP exige `uTrib` em `kg` (rejeicao 817). |
| `itens[].combustivel.codif` | LA04 `CODIF`: de 1 a 21 digitos. |
| `itens[].combustivel.q_temp` | LA05 `qTemp`: 4 casas; zero ou ausente = a tag nao sai (o XSD nao aceita zero). |
| `itens[].combustivel.cide` | LA07 `CIDE`. |
| `itens[].combustivel.encerrante` | LA11 `encerrante`: so' com o CFOP 5656 ou 5667 (rejeicao 379). |
| `itens[].combustivel.p_bio` | LA17 `pBio`: maior que 0 e ate 100; zero ou ausente = a tag nao sai. |
| `itens[].combustivel.origens` | LA18 `origComb`, de 0 a 30. Gas natural no GLP (`pGNn` ou `pGNi` diferente de 0) exige ao menos uma, salvo consumidor final, `finNFe` 2 ou 4, CFOP 5922 ou 6922 e `tpImp` 6 (rejeicao 909). |

## itens[].combustivel.cide

| campo | o que e' |
|---|---|
| `itens[].combustivel.cide.q_bc_prod` | LA08 `qBCProd`: base em quantidade (ate 12 inteiros e 4 casas). |
| `itens[].combustivel.cide.v_aliq_prod` | LA09 `vAliqProd`: aliquota em reais por unidade (4 casas). |
| `itens[].combustivel.cide.v_cide` | LA10 `vCIDE` (2 casas). |

## itens[].combustivel.encerrante

| campo | o que e' |
|---|---|
| `itens[].combustivel.encerrante.n_bico` | LA12 `nBico`: de 0 a 999. |
| `itens[].combustivel.encerrante.n_tanque` | LA14 `nTanque`: de 0 a 999. |
| `itens[].combustivel.encerrante.v_enc_ini` | LA15 `vEncIni` (3 casas). |
| `itens[].combustivel.encerrante.v_enc_fin` | LA16 `vEncFin` (3 casas); tem de ser maior que o `vEncIni` (rejeicao 380). |
| `itens[].combustivel.encerrante.n_bomba` | LA13 `nBomba`: de 0 a 999; ausente = a tag nao sai. |

## itens[].combustivel.origens[]

| campo | o que e' |
|---|---|
| `itens[].combustivel.origens[].ind_import` | LA19 `indImport`: 0 nacional, 1 importado. |
| `itens[].combustivel.origens[].c_uf_orig` | LA20 `cUFOrig`: codigo IBGE da UF (`"35"`) ou a sigla (`"SP"`); sai o codigo. |
| `itens[].combustivel.origens[].p_orig` | LA21 `pOrig`: maior que 0 e ate 100, ate 4 casas. |

## itens[].med

| campo | o que e' |
|---|---|
| `itens[].med.c_prod_anvisa` | K01a `cProdANVISA`: numero do registro (11 ou 13 digitos) ou o literal `ISENTO`. |
| `itens[].med.x_motivo_isencao` | K01b `xMotivoIsencao`: numero da decisao que isenta o registro (de 1 a 255 caracteres). |
| `itens[].med.v_pmc` | K06 `vPMC`: preco maximo ao consumidor (2 casas). |

## itens[].rastro[]

| campo | o que e' |
|---|---|
| `itens[].rastro[].n_lote` | I81 `nLote`: de 1 a 20 caracteres. |
| `itens[].rastro[].q_lote` | I82 `qLote`: ate 8 inteiros e 3 casas. |
| `itens[].rastro[].d_fab` | I83 `dFab`: `AAAA-MM-DD`; nao pode ser depois da emissao (rejeicao 877). |
| `itens[].rastro[].d_val` | I84 `dVal`: `AAAA-MM-DD`; nao pode ser antes da fabricacao (rejeicao 870). |
| `itens[].rastro[].c_agreg` | I85 `cAgreg`: de 1 a 20 caracteres. |

## transp

| campo | o que e' |
|---|---|
| `transp.transporta` | X03 `transporta`: a transportadora; o grupo nao vai vazio. |
| `transp.ret_transp` | X11 `retTransp`: retencao do ICMS do transporte; todos os campos sao obrigatorios. |
| `transp.veic` | X18 `veicTransp`: o veiculo trator. Nao vai na operacao interestadual (`idDest` 2, rejeicao 868). |
| `transp.reboque` | X22 `reboque`: reboques e dolly, ate 5. Nao vai na operacao interestadual (868). |
| `transp.vagao` | X25a `vagao`: identificacao do vagao, de 1 a 20 caracteres. O XSD aceita so' o veiculo (com os reboques), o vagao ou a balsa. |
| `transp.balsa` | X25b `balsa`: identificacao da balsa, de 1 a 20 caracteres. |
| `transp.vol` | X26 `vol`: volumes, de 0 a 5000; o volume nao vai vazio. |

## transp.transporta

| campo | o que e' |
|---|---|
| `transp.transporta.documento` | X04 `CNPJ` ou X05 `CPF`, com digito verificador valido (rejeicoes 542 e 543). |
| `transp.transporta.nome` | X06 `xNome`: de 2 a 60 caracteres. |
| `transp.transporta.ie` | X07 `IE`: de 2 a 14 digitos ou `ISENTO`; com ela a UF e' obrigatoria (rejeicao 559). A validade da IE na UF (544) e' do cadastro e nao e' conferida. |
| `transp.transporta.endereco` | X08 `xEnder`: endereco completo, de 1 a 60 caracteres. |
| `transp.transporta.municipio` | X09 `xMun`: de 1 a 60 caracteres. |
| `transp.transporta.uf` | X10 `UF`: sigla, ou `EX` para o exterior. |

## transp.ret_transp

| campo | o que e' |
|---|---|
| `transp.ret_transp.v_serv` | X12 `vServ`: valor do servico (2 casas). |
| `transp.ret_transp.v_bc_ret` | X13 `vBCRet`: base da retencao (2 casas). |
| `transp.ret_transp.p_icms_ret` | X14 `pICMSRet`: aliquota da retencao, de 0 a 100. |
| `transp.ret_transp.v_icms_ret` | X15 `vICMSRet`: valor do ICMS retido (2 casas). |
| `transp.ret_transp.cfop` | X16 `CFOP`: 4 digitos, iniciado por 1, 2, 3, 5, 6 ou 7. Tem de ser CFOP de transporte da Tabela de CFOP do Portal (`indTransp` 1), senao a rejeicao 722. |
| `transp.ret_transp.c_mun_fg` | X17 `cMunFG`: codigo IBGE, 7 digitos. |

## transp.veic

| campo | o que e' |
|---|---|
| `transp.veic.placa` | X19 `placa`: `XXX9999`, `XXX999`, `XX9999`, `XXXX999` ou 7 letras e numeros (placa Mercosul); espaco e hifen sao tirados. |
| `transp.veic.uf` | `UF`: sigla, ou `EX`. |
| `transp.veic.rntc` | `RNTC`: registro na ANTT, de 1 a 20 caracteres. |

## transp.reboque[]

| campo | o que e' |
|---|---|
| `transp.reboque[].placa` | X23 `placa`: `XXX9999`, `XXX999`, `XX9999`, `XXXX999` ou 7 letras e numeros (placa Mercosul); espaco e hifen sao tirados. |
| `transp.reboque[].uf` | `UF`: sigla, ou `EX`. |
| `transp.reboque[].rntc` | `RNTC`: registro na ANTT, de 1 a 20 caracteres. |

## transp.vol[]

| campo | o que e' |
|---|---|
| `transp.vol[].q_vol` | X27 `qVol`: de 1 a 15 digitos. |
| `transp.vol[].esp` | X28 `esp`: especie, de 1 a 60 caracteres. |
| `transp.vol[].marca` | X29 `marca`: de 1 a 60 caracteres. |
| `transp.vol[].n_vol` | X30 `nVol`: de 1 a 60 caracteres. |
| `transp.vol[].peso_l` | X31 `pesoL`: peso liquido em kg (ate 12 inteiros e 3 casas). |
| `transp.vol[].peso_b` | X32 `pesoB`: peso bruto em kg (ate 12 inteiros e 3 casas). |
| `transp.vol[].lacres` | X33 `lacres` / X34 `nLacre`: lista com o numero de cada lacre, de 1 a 60 caracteres, ate 5000. |

## cobr

| campo | o que e' |
|---|---|
| `cobr.fat` | Y02 `fat`: a fatura. Com `cobr`, `nFat`, `vOrig` e `vLiq` sao obrigatorios (rejeicao 905). |
| `cobr.dup` | Y07 `dup`, de 0 a 120; a soma bate com o `vLiq` (rejeicao 851). |

## cobr.fat

| campo | o que e' |
|---|---|
| `cobr.fat.n_fat` | Y03 `nFat`: de 1 a 60 caracteres. |
| `cobr.fat.v_orig` | Y04 `vOrig`: valor original (2 casas). |
| `cobr.fat.v_desc` | Y05 `vDesc`: desconto, nao maior que o original (rejeicao 901); ausente ou zero = a tag nao sai. |
| `cobr.fat.v_liq` | Y06 `vLiq`: valor liquido; igual ao original menos o desconto (rejeicao 902). |

## cobr.dup[]

| campo | o que e' |
|---|---|
| `cobr.dup[].n_dup` | Y08 `nDup`: tres algarismos sequenciais a partir de `001` (rejeicao 852). |
| `cobr.dup[].d_venc` | Y09 `dVenc`: `AAAA-MM-DD`, em ordem crescente e nao antes da emissao (rejeicoes 900 e 850). |
| `cobr.dup[].v_dup` | Y10 `vDup`: valor da parcela, maior que zero (2 casas). |

## exporta

| campo | o que e' |
|---|---|
| `exporta.uf_saida_pais` | ZA02 `UFSaidaPais`: sigla da UF de embarque (sem `EX`). |
| `exporta.x_loc_exporta` | ZA03 `xLocExporta`: local de embarque, de 1 a 60 caracteres. |
| `exporta.x_loc_despacho` | ZA04 `xLocDespacho`: local de despacho, de 1 a 60 caracteres. |

## compra

| campo | o que e' |
|---|---|
| `compra.x_n_emp` | ZB02 `xNEmp`: nota de empenho, de 1 a 22 caracteres. |
| `compra.x_ped` | ZB03 `xPed`: pedido, de 1 a 60 caracteres. |
| `compra.x_cont` | ZB04 `xCont`: contrato, de 1 a 60 caracteres. |

## pagamentos[]

| campo | o que e' |
|---|---|
| `pagamentos[].t_pag` | `tPag`. |
| `pagamentos[].valor` | `vPag`. |
| `pagamentos[].ind_pag` | `indPag`. |
| `pagamentos[].x_pag` | `xPag`: obrigatorio com `tPag` 99. |
| `pagamentos[].cartao` | `card`. |

## pagamentos[].cartao

| campo | o que e' |
|---|---|
| `pagamentos[].cartao.tp_integra` | `tpIntegra`. |
| `pagamentos[].cartao.cnpj` | CNPJ da credenciadora. |
| `pagamentos[].cartao.t_band` | `tBand`. |
| `pagamentos[].cartao.c_aut` | `cAut`. |

## dest

| campo | o que e' |
|---|---|
| `dest.documento` | CNPJ ou CPF. |
| `dest.id_estrangeiro` | `idEstrangeiro`. |
| `dest.nome` | `xNome`. |
| `dest.endereco` | `enderDest`. |
| `dest.ind_ie_dest` | `indIEDest`: 1, 2 ou 9. |
| `dest.ie` | `IE`. |
| `dest.isuf` | `ISUF`. |
| `dest.im` | `IM`. |
| `dest.email` | `email`. |

## dest.endereco

| campo | o que e' |
|---|---|
| `dest.endereco.logradouro` | `xLgr`. |
| `dest.endereco.numero` | `nro`. |
| `dest.endereco.bairro` | `xBairro`. |
| `dest.endereco.cod_municipio` | `cMun`: codigo IBGE, 7 digitos. |
| `dest.endereco.municipio` | `xMun`. |
| `dest.endereco.uf` | `UF`; `EX` para o exterior. |
| `dest.endereco.cep` | `CEP`, 8 digitos. |
| `dest.endereco.complemento` | `xCpl`. |
| `dest.endereco.cod_pais` | `cPais`; vazio = 1058 (Brasil) no emitente e no destinatario. |
| `dest.endereco.pais` | `xPais`; vazio = BRASIL no emitente e no destinatario. |
| `dest.endereco.fone` | `fone`. |

## resp_tec

| campo | o que e' |
|---|---|
| `resp_tec.cnpj` | CNPJ do responsavel. |
| `resp_tec.contato` | `xContato`. |
| `resp_tec.email` | `email`. |
| `resp_tec.fone` | `fone`. |
| `resp_tec.id_csrt` | `idCSRT`. |
| `resp_tec.csrt` | Codigo de seguranca do responsavel tecnico; com ele a nota leva `idCSRT` e `hashCSRT`. |

## compra_gov

| campo | o que e' |
|---|---|
| `compra_gov.tp_ente` | BB02: 1 Uniao, 2 Estado, 3 Distrito Federal, 4 Municipio, 5 consorcio publico, 6 CGIBS. |
| `compra_gov.p_redutor` | BB03: redutor do art. 472 da LC 214/2025. |
| `compra_gov.tp_oper` | BB04: 1 fornecimento com pagamento posterior, 2 recebimento do pagamento (fornecimento feito), 3 fornecimento com pagamento feito, 4 recebimento do pagamento (fornecimento posterior). |
| `compra_gov.refs` | BB05 `refDFeAnt`: chaves dos DF-e anteriores (so' com `tpOperGov` 2 e 3; de 0 a 99). |

## retirada

| campo | o que e' |
|---|---|
| `retirada.documento` | CNPJ ou CPF do local. |
| `retirada.endereco` | Endereco. |
| `retirada.nome` | `xNome`. |
| `retirada.email` | `email`. |
| `retirada.ie` | `IE`. |

## retirada.endereco

| campo | o que e' |
|---|---|
| `retirada.endereco.logradouro` | `xLgr`. |
| `retirada.endereco.numero` | `nro`. |
| `retirada.endereco.bairro` | `xBairro`. |
| `retirada.endereco.cod_municipio` | `cMun`: codigo IBGE, 7 digitos. |
| `retirada.endereco.municipio` | `xMun`. |
| `retirada.endereco.uf` | `UF`; `EX` para o exterior. |
| `retirada.endereco.cep` | `CEP`, 8 digitos. |
| `retirada.endereco.complemento` | `xCpl`. |
| `retirada.endereco.cod_pais` | `cPais`; vazio = 1058 (Brasil) no emitente e no destinatario. |
| `retirada.endereco.pais` | `xPais`; vazio = BRASIL no emitente e no destinatario. |
| `retirada.endereco.fone` | `fone`. |

## entrega

| campo | o que e' |
|---|---|
| `entrega.documento` | CNPJ ou CPF do local. |
| `entrega.endereco` | Endereco. |
| `entrega.nome` | `xNome`. |
| `entrega.email` | `email`. |
| `entrega.ie` | `IE`. |

## entrega.endereco

| campo | o que e' |
|---|---|
| `entrega.endereco.logradouro` | `xLgr`. |
| `entrega.endereco.numero` | `nro`. |
| `entrega.endereco.bairro` | `xBairro`. |
| `entrega.endereco.cod_municipio` | `cMun`: codigo IBGE, 7 digitos. |
| `entrega.endereco.municipio` | `xMun`. |
| `entrega.endereco.uf` | `UF`; `EX` para o exterior. |
| `entrega.endereco.cep` | `CEP`, 8 digitos. |
| `entrega.endereco.complemento` | `xCpl`. |
| `entrega.endereco.cod_pais` | `cPais`; vazio = 1058 (Brasil) no emitente e no destinatario. |
| `entrega.endereco.pais` | `xPais`; vazio = BRASIL no emitente e no destinatario. |
| `entrega.endereco.fone` | `fone`. |
