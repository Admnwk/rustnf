# Pedido da NF-e em JSON

O pedido que `rustnf::nfe::montagem::nota_de_json` le para montar a NF-e (modelo 55, leiaute 4.00). A montagem NAO envia
a nota a SEFAZ.

* Valor monetario e percentual vao em texto (`"10.50"`) ou inteiro; numero com ponto flutuante e' recusado. Os campos que
  a tabela diz "em centavos" sao inteiros.
* Data e hora em ISO 8601 COM fuso (`2026-10-03T09:00:00-03:00`); sem fuso e' erro (nao se chuta o horario de Brasilia).
* `codigo` (o `cNF`, 8 digitos) vem no pedido: a mesma entrada da o mesmo XML. Quem nao tem um gera com
  `chave_dfe::gerar_codigo`.
* O texto e' filtrado como manda o leiaute (sem espaco repetido, sem quebra de linha) e, com `remover_acento` (padrao: sim),
  sem acento.
* Campo que o pedido traz e a montagem nao conhece (nome errado, campo que a montagem ainda nao tem) e' ignorado, com
  aviso em `campos_desconhecidos`; `nota_de_json_estrito` recusa. `itens[].ibscbs.publicadas` e' de forma livre.
* Fora desta entrega (nao ha campo no pedido): cobranca (`cobr`), transportadora, volumes e lacres, exportacao, compra,
  rastreabilidade, medicamento, combustivel, armamento e o QR Code. O modelo 65 (NFC-e) e' recusado.

Cada tabela abaixo e' um bloco do pedido; o campo vai pelo caminho (`emit.endereco.uf`; `itens[]` e' a lista de itens).


## Nota (raiz)

| campo | o que e' |
|---|---|
| `modelo` | Modelo: so' o 55 e' montado. |
| `serie` | `serie`. |
| `numero` | `nNF`. |
| `emissao` | `dhEmi`, com fuso. |
| `nat_op` | `natOp`. |
| `cmun_fg` | `cMunFG`. |
| `emit` | Emitente. |
| `itens` | Itens (de 1 a 990). |
| `pagamentos` | Pagamentos (de 1 a 100). |
| `dest` | Destinatario. |
| `tp_amb` | `tpAmb`: 1 producao, 2 homologacao. |
| `tp_emis` | `tpEmis`. |
| `tp_nf` | `tpNF`: 0 entrada, 1 saida. |
| `id_dest` | `idDest`. |
| `fin_nfe` | `finNFe`. |
| `ind_final` | `indFinal`; vazio = 0. |
| `ind_pres` | `indPres`; vazio = 9. |
| `ind_intermed` | `indIntermed`. |
| `tp_imp` | `tpImp`; vazio = 1. O 6 (DANFE simplificado) pede o QR Code: a nota monta, mas nao e' assinada. |
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
| `itens[].cfop` | `CFOP`. |
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

## itens[].icms

| campo | o que e' |
|---|---|
| `itens[].icms.codigo` | CST (2 digitos) ou CSOSN (3 digitos), conforme o CRT do emitente. |
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
| `itens[].ibscbs.cred_pres.c_cred_pres` | Codigo do credito presumido (`cCredPres`, 2 digitos). |
| `itens[].ibscbs.cred_pres.vbc` | Base do credito presumido (vazio = a base do item). |
| `itens[].ibscbs.cred_pres.ibs` | Credito presumido do IBS. |
| `itens[].ibscbs.cred_pres.cbs` | Credito presumido da CBS. |

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
