// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os campos que o pedido JSON conhece, por caminho. Serve a duas coisas: apontar o campo que o pedido traz e a
//! montagem nao le (nome errado, maiuscula, campo que a montagem ainda nao tem) e listar o formato para a
//! documentacao (`docs/NFE_PEDIDO.md`, conferida por teste contra esta lista).

use serde_json::Value;

use super::esquema_grupos::{COBR, COMBUSTIVEL, COMPRA, EXPORTA, MED, RASTRO, TRANSP};
use super::formato::{erro, Res};
use super::json::nota_de_json;
use super::modelo::Nota;

/// Um campo do pedido: folha, objeto com filhos ou lista de objetos.
pub(super) struct Campo {
    pub(super) nome: &'static str,
    pub(super) filhos: &'static [Campo],
    pub(super) lista: bool,
    /// Objeto de forma livre: nada abaixo dele e' conferido.
    pub(super) livre: bool,
}

pub(super) const fn folha(nome: &'static str) -> Campo {
    Campo {
        nome,
        filhos: &[],
        lista: false,
        livre: false,
    }
}

pub(super) const fn obj(nome: &'static str, filhos: &'static [Campo]) -> Campo {
    Campo {
        nome,
        filhos,
        lista: false,
        livre: false,
    }
}

pub(super) const fn lista(nome: &'static str, filhos: &'static [Campo]) -> Campo {
    Campo {
        nome,
        filhos,
        lista: true,
        livre: false,
    }
}

const fn livre(nome: &'static str) -> Campo {
    Campo {
        nome,
        filhos: &[],
        lista: false,
        livre: true,
    }
}

const ENDERECO: &[Campo] = &[
    folha("logradouro"),
    folha("numero"),
    folha("bairro"),
    folha("cod_municipio"),
    folha("municipio"),
    folha("uf"),
    folha("cep"),
    folha("complemento"),
    folha("cod_pais"),
    folha("pais"),
    folha("fone"),
];

const LOCAL: &[Campo] = &[
    folha("documento"),
    obj("endereco", ENDERECO),
    folha("nome"),
    folha("email"),
    folha("ie"),
];

const EMIT: &[Campo] = &[
    folha("documento"),
    folha("nome"),
    obj("endereco", ENDERECO),
    folha("crt"),
    folha("ie"),
    folha("fantasia"),
    folha("iest"),
    folha("im"),
    folha("cnae"),
    folha("tp_regime_ibscbs"),
    folha("isuf"),
];

const DEST: &[Campo] = &[
    folha("documento"),
    folha("id_estrangeiro"),
    folha("nome"),
    obj("endereco", ENDERECO),
    folha("ind_ie_dest"),
    folha("ie"),
    folha("isuf"),
    folha("im"),
    folha("email"),
];

const ICMS: &[Campo] = &[
    folha("codigo"),
    folha("orig"),
    folha("mod_bc"),
    folha("vbc"),
    folha("p_red_bc"),
    folha("p_icms"),
    folha("v_icms"),
    folha("vbc_fcp"),
    folha("p_fcp"),
    folha("v_fcp"),
    folha("vbc_st_ret"),
    folha("p_st"),
    folha("v_icms_substituto"),
    folha("v_icms_st_ret"),
    folha("p_cred_sn"),
    folha("v_cred_icms_sn"),
    folha("mod_bc_st"),
    folha("p_mva_st"),
    folha("p_red_bc_st"),
    folha("vbc_st"),
    folha("p_icms_st"),
    folha("v_icms_st"),
    folha("vbc_fcp_st"),
    folha("p_fcp_st"),
    folha("v_fcp_st"),
    folha("v_icms_op"),
    folha("p_dif"),
    folha("v_icms_dif"),
    folha("p_fcp_dif"),
    folha("v_fcp_dif"),
    folha("v_fcp_efet"),
    folha("v_icms_deson"),
    folha("mot_des_icms"),
    folha("ind_deduz_deson"),
    folha("v_icms_st_deson"),
    folha("mot_des_icms_st"),
    folha("partilha"),
    folha("p_bc_op"),
    folha("uf_st"),
    folha("repasse"),
    folha("vbc_st_dest"),
    folha("v_icms_st_dest"),
    folha("vbc_fcp_st_ret"),
    folha("p_fcp_st_ret"),
    folha("v_fcp_st_ret"),
    folha("p_red_bc_efet"),
    folha("vbc_efet"),
    folha("p_icms_efet"),
    folha("v_icms_efet"),
    folha("c_benef_rbc"),
];

const PIS_COFINS: &[Campo] = &[
    folha("cst"),
    folha("vbc"),
    folha("aliquota"),
    folha("valor"),
];

const IPI: &[Campo] = &[
    folha("cst"),
    folha("c_enq"),
    folha("vbc"),
    folha("aliquota"),
    folha("valor"),
];

const VEICULO: &[Campo] = &[
    folha("tp_op"),
    folha("chassi"),
    folha("c_cor"),
    folha("x_cor"),
    folha("pot"),
    folha("cilin"),
    folha("peso_l"),
    folha("peso_b"),
    folha("n_serie"),
    folha("tp_comb"),
    folha("n_motor"),
    folha("cmt"),
    folha("dist"),
    folha("ano_mod"),
    folha("ano_fab"),
    folha("tp_pint"),
    folha("tp_veic"),
    folha("esp_veic"),
    folha("vin"),
    folha("cond_veic"),
    folha("c_mod"),
    folha("c_cor_denatran"),
    folha("lota"),
    folha("tp_rest"),
];

const ICMS_UF_DEST: &[Campo] = &[
    folha("vbc"),
    folha("aliquota_interna"),
    folha("aliquota_inter"),
    folha("ano"),
    folha("fcp"),
    folha("vbc_uf_dest"),
];

const ALIQ: &[Campo] = &[folha("ibs_uf"), folha("ibs_mun"), folha("cbs")];
const PAR: &[Campo] = &[folha("ibs"), folha("cbs")];
const PV: &[Campo] = &[folha("p"), folha("v")];
const TRIB_PV: &[Campo] = &[obj("ibs_uf", PV), obj("ibs_mun", PV), obj("cbs", PV)];
const CRED_PRES_TRIB: &[Campo] = &[folha("p"), folha("cond_sus")];
const MONO_SUB: &[Campo] = &[
    folha("q"),
    folha("ad_rem"),
    folha("vbc"),
    folha("p_uf"),
    folha("p_mun"),
    folha("p"),
    folha("v"),
];
const MONO_TRIB: &[Campo] = &[
    obj("padrao", MONO_SUB),
    obj("reten", MONO_SUB),
    obj("ret", MONO_SUB),
    obj("pbio", MONO_SUB),
];

/// O `ibscbs` do item e' a entrada do calculo do IBS/CBS de mercadoria ([`crate::reforma::calculo::EntradaItem`]).
const IBSCBS: &[Campo] = &[
    folha("cst"),
    folha("cclass_trib"),
    folha("vbc"),
    folha("data"),
    obj("aliquotas", ALIQ),
    livre("publicadas"),
    folha("simples_nacional"),
    // texto (o mesmo percentual nos tres entes) ou objeto com os tres
    obj("p_dif", ALIQ),
    folha("p_dev_cbs"),
    obj(
        "trib_regular",
        &[
            folha("cst"),
            folha("cclass_trib"),
            folha("ibs_uf"),
            folha("ibs_mun"),
            folha("cbs"),
        ],
    ),
    obj(
        "cred_pres",
        &[
            folha("c_cred_pres"),
            folha("vbc"),
            obj("ibs", CRED_PRES_TRIB),
            obj("cbs", CRED_PRES_TRIB),
        ],
    ),
    folha("deduz_cred_pres"),
    obj("estorno", PAR),
    obj("mono", &[obj("ibs", MONO_TRIB), obj("cbs", MONO_TRIB)]),
    obj("transf_cred", PAR),
    obj(
        "ajuste_compet",
        &[folha("compet_apur"), folha("ibs"), folha("cbs")],
    ),
    obj(
        "cred_pres_zfm",
        &[folha("compet_apur"), folha("tp"), folha("v")],
    ),
    folha("ind_doacao"),
    folha("tp_nf_debito"),
    obj("compra_gov", &[folha("p_redutor"), obj("trib", TRIB_PV)]),
    folha("validar_vigencia"),
    obj("trib_sn", &[obj("ibs", PV), obj("cbs", PV)]),
    folha("fin_nfe"),
    folha("tp_nf_credito"),
    folha("cbs_zero_alc_zfm"),
];

const ITEM: &[Campo] = &[
    folha("codigo"),
    folha("descricao"),
    folha("ncm"),
    folha("cfop"),
    folha("unidade"),
    folha("quantidade"),
    folha("valor_unitario"),
    obj("icms", ICMS),
    obj("pis", PIS_COFINS),
    obj("cofins", PIS_COFINS),
    folha("valor_produto"),
    folha("gtin"),
    folha("gtin_trib"),
    folha("unidade_trib"),
    folha("quantidade_trib"),
    folha("valor_unitario_trib"),
    folha("cest"),
    folha("cbenef"),
    folha("extipi"),
    folha("frete"),
    folha("seguro"),
    folha("desconto"),
    folha("outros"),
    folha("ind_tot"),
    obj("ipi", IPI),
    obj("ibscbs", IBSCBS),
    folha("v_tot_trib"),
    folha("inf_ad_prod"),
    folha("x_ped"),
    folha("n_item_ped"),
    folha("dfe_ref"),
    folha("valor_unitario_liquido"),
    folha("valor_produto_liquido"),
    folha("c_ativ_sn"),
    folha("v_icms_previsto"),
    obj("icms_uf_dest", ICMS_UF_DEST),
    folha("difal_dispensado"),
    obj("veiculo", VEICULO),
    obj("combustivel", COMBUSTIVEL),
    obj("med", MED),
    lista("rastro", RASTRO),
];

const PAGAMENTO: &[Campo] = &[
    folha("t_pag"),
    folha("valor"),
    folha("ind_pag"),
    folha("x_pag"),
    obj(
        "cartao",
        &[
            folha("tp_integra"),
            folha("cnpj"),
            folha("t_band"),
            folha("c_aut"),
        ],
    ),
];

const NOTA: &[Campo] = &[
    folha("modelo"),
    folha("serie"),
    folha("numero"),
    folha("emissao"),
    folha("nat_op"),
    folha("cmun_fg"),
    obj("emit", EMIT),
    lista("itens", ITEM),
    lista("pagamentos", PAGAMENTO),
    obj("dest", DEST),
    folha("tp_amb"),
    folha("tp_emis"),
    folha("tp_nf"),
    folha("id_dest"),
    folha("fin_nfe"),
    folha("ind_final"),
    folha("ind_pres"),
    folha("ind_intermed"),
    folha("tp_imp"),
    folha("proc_emi"),
    folha("ver_proc"),
    folha("codigo"),
    folha("saida"),
    folha("cmun_fg_ibs"),
    folha("dh_cont"),
    folha("x_just"),
    folha("refs_nfe"),
    folha("troco"),
    folha("mod_frete"),
    folha("inf_ad_fisco"),
    folha("inf_cpl"),
    obj(
        "resp_tec",
        &[
            folha("cnpj"),
            folha("contato"),
            folha("email"),
            folha("fone"),
            folha("id_csrt"),
            folha("csrt"),
        ],
    ),
    folha("remover_acento"),
    folha("tp_nf_debito"),
    folha("tp_nf_credito"),
    folha("pag_antecipado"),
    obj(
        "compra_gov",
        &[
            folha("tp_ente"),
            folha("p_redutor"),
            folha("tp_oper"),
            folha("refs"),
        ],
    ),
    folha("c_ind_op"),
    obj("retirada", LOCAL),
    obj("entrega", LOCAL),
    obj("transp", TRANSP),
    obj("cobr", COBR),
    obj("exporta", EXPORTA),
    obj("compra", COMPRA),
];

fn achar(v: &Value, campos: &[Campo], caminho: &str, saida: &mut Vec<String>) {
    let Some(o) = v.as_object() else { return };
    for (chave, valor) in o {
        let aqui = if caminho.is_empty() {
            chave.clone()
        } else {
            format!("{caminho}.{chave}")
        };
        let Some(c) = campos.iter().find(|c| c.nome == chave) else {
            saida.push(aqui);
            continue;
        };
        if c.livre || c.filhos.is_empty() {
            continue;
        }
        if c.lista {
            for (i, el) in valor.as_array().into_iter().flatten().enumerate() {
                achar(el, c.filhos, &format!("{aqui}[{}]", i + 1), saida);
            }
        } else {
            achar(valor, c.filhos, &aqui, saida);
        }
    }
}

/// Os campos do pedido que o leiaute da montagem nao conhece, por caminho (`itens[1].icms.cstt`), em ordem. Lista
/// vazia = o pedido so' traz campo conhecido. O pedido que nao e' objeto devolve lista vazia: quem diz esse erro e' a
/// leitura.
#[must_use]
pub fn campos_desconhecidos(pedido: &Value) -> Vec<String> {
    let mut saida = Vec::new();
    achar(pedido, NOTA, "", &mut saida);
    saida.sort();
    saida
}

/// Como [`nota_de_json`], mas recusa o pedido que traz campo desconhecido (modo estrito): o primeiro vai no `campo` do
/// erro e todos na mensagem.
///
/// # Errors
///
/// [`super::ErroMontagem`] pelo campo desconhecido ou por qualquer defeito de [`nota_de_json`].
pub fn nota_de_json_estrito(pedido: &Value) -> Res<Nota> {
    let achados = campos_desconhecidos(pedido);
    if let Some(primeiro) = achados.first() {
        return Err(erro(
            format!(
                "campo desconhecido no pedido: {} (confira o nome em docs/NFE_PEDIDO.md)",
                achados.join(", ")
            ),
            primeiro,
        ));
    }
    nota_de_json(pedido)
}

fn listar(campos: &[Campo], prefixo: &str, saida: &mut Vec<String>) {
    for c in campos {
        let nome = if prefixo.is_empty() {
            c.nome.to_string()
        } else {
            format!("{prefixo}.{}", c.nome)
        };
        saida.push(nome.clone());
        let base = if c.lista { format!("{nome}[]") } else { nome };
        listar(c.filhos, &base, saida);
    }
}

/// Todos os caminhos que o pedido conhece (`itens[].icms.codigo`), na ordem do leiaute. A documentacao do pedido tem
/// de citar cada um.
#[must_use]
pub fn campos_conhecidos() -> Vec<String> {
    let mut saida = Vec::new();
    listar(NOTA, "", &mut saida);
    saida
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    #[test]
    fn acha_o_campo_desconhecido_em_qualquer_nivel() {
        let p = json!({
            "numero": 1,
            "emitt": {},
            "emit": {"endereco": {"ufx": "SP"}, "crt": 3},
            "itens": [{"icms": {"codigo": "00"}}, {"icms": {"cstt": "00"}, "ibscbs": {"cst": "000", "cClassTrib": "1"}}],
            "dest": {"documento": "1"}
        });
        assert_eq!(
            campos_desconhecidos(&p),
            vec![
                "emit.endereco.ufx".to_string(),
                "emitt".to_string(),
                "itens[2].ibscbs.cClassTrib".to_string(),
                "itens[2].icms.cstt".to_string(),
            ]
        );
    }

    #[test]
    fn pedido_so_com_campo_conhecido_nao_tem_aviso() {
        let p = json!({"numero": 1, "emit": {"endereco": {"uf": "SP"}}, "itens": [{"ibscbs": {"publicadas": {"2027": {"x": 1}}}}]});
        assert!(campos_desconhecidos(&p).is_empty());
        assert!(campos_desconhecidos(&json!(3)).is_empty());
    }

    #[test]
    fn a_lista_de_campos_nao_repete_caminho() {
        let mut v = campos_conhecidos();
        let n = v.len();
        v.sort();
        v.dedup();
        assert_eq!(v.len(), n);
        assert!(v.contains(&"itens[].icms.codigo".to_string()));
    }
}
