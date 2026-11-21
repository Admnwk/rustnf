// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! "Usar como modelo": a nota escolhida vira o formulario da Nova nota (tomador, servico, descricao e valores, o que
//! o XML guardado permite ler). O que nao da para ler fica vazio, e a conferencia da Nova nota aponta. A competencia
//! e' a de hoje.

use super::detalhe::DetalheDaNota;
use super::nota::{CampoNota, DadosNota};
use super::pedido::Contexto;

/// O valor lido da nota, ou vazio quando o detalhe nao o traz (`-`).
fn lido(texto: &str) -> &str {
    if texto == "-" {
        ""
    } else {
        texto
    }
}

/// Os dados da Nova nota a partir de uma nota ja emitida. O ISS retido, os descontos, o endereco e o NBS nao constam
/// do que a biblioteca le da NFS-e: ficam vazios (ou no padrao) para a pessoa conferir.
#[must_use]
pub fn dados_da_nota(n: &DetalheDaNota, ctx: &Contexto) -> DadosNota {
    let mut d = ctx.nota_nova();
    let sem_tomador = n.tomador_documento.trim().is_empty() && n.tomador.trim().is_empty();
    d.sem_tomador = sem_tomador;
    if !sem_tomador {
        d.definir(CampoNota::TomadorDocumento, &n.tomador_documento);
        d.definir(CampoNota::TomadorNome, &n.tomador);
    }
    d.definir(CampoNota::Ctribnac, &n.servico_codigo);
    d.definir(CampoNota::Descricao, &n.servico_descricao);
    d.definir(CampoNota::ValorServico, lido(&n.valor_servico));
    if ctx.regime().mostra_aliquota_iss(d.tributacao) {
        d.definir(CampoNota::AliquotaIss, &n.aliquota_iss);
    }
    d
}

#[cfg(test)]
mod testes {
    use rustnf::nfse::leitura::ler_nfse;
    use rustnf_app::notas::Situacao;

    use super::*;
    use crate::modelo::conferencia::conferir;
    use crate::modelo::pedido::testes::{config_do_caso, ctx_do_caso};

    fn nfse() -> rustnf::nfse::leitura::Nfse {
        ler_nfse(include_str!("../../../rustnf-cli/exemplos/nfse.xml")).unwrap()
    }

    fn detalhe(n: &rustnf::nfse::leitura::Nfse) -> DetalheDaNota {
        DetalheDaNota::da_nfse(n, Situacao::Autorizada)
    }

    #[test]
    fn leva_tomador_servico_descricao_e_valores_e_a_competencia_e_hoje() {
        let ctx = ctx_do_caso();
        let d = dados_da_nota(&detalhe(&nfse()), &ctx);
        assert_eq!(d.tomador_documento, "33.444.555/0001-72");
        assert_eq!(d.tomador_nome, "CLIENTE EXEMPLO SA");
        assert_eq!(d.ctribnac, "01.03.01");
        assert_eq!(d.descricao, "Descrição do Serviço");
        assert_eq!(d.valor_servico, "12,45");
        assert_eq!(d.competencia, ctx.hoje());
        assert_ne!(d.competencia, "23/09/2026");
        assert!(!d.sem_tomador);
        // o que o XML nao traz fica vazio
        assert_eq!(d.nbs, "");
        assert_eq!(d.end_cep, "");
        assert_eq!(d.desc_incondicionado, "");
        assert_eq!(d.irrf, "");
    }

    #[test]
    fn a_aliquota_do_iss_acompanha_o_regime() {
        // ME/EPP (o caso comum) tem aliquota, ja que o regime nao e' MEI nem especial
        let d = dados_da_nota(&detalhe(&nfse()), &ctx_do_caso());
        assert_eq!(d.aliquota_iss, "5,00");
        // MEI nao informa aliquota de ISS: a biblioteca a recusaria
        let mut config = config_do_caso();
        config.emitente.op_simples = 2;
        let mei = Contexto::de_config(&config, "2026-11-21T15:00:00Z".parse().unwrap());
        assert_eq!(dados_da_nota(&detalhe(&nfse()), &mei).aliquota_iss, "");
    }

    #[test]
    fn nota_sem_tomador_vira_formulario_sem_tomador() {
        let mut n = nfse();
        n.tomador_documento.clear();
        n.tomador_nome.clear();
        let d = dados_da_nota(&detalhe(&n), &ctx_do_caso());
        assert!(d.sem_tomador);
        assert_eq!(d.tomador_documento, "");
    }

    #[test]
    fn a_conferencia_da_nova_nota_aponta_o_que_faltou() {
        let ctx = ctx_do_caso();
        // o CNPJ do XML de exemplo e' ficticio e nao passa no digito verificador: a conferencia o aponta, e o teste usa
        // um valido para ver o resto
        let mut n = nfse();
        n.tomador_documento = "11222333000181".into();
        let d = dados_da_nota(&detalhe(&n), &ctx);
        let c = conferir(&d, &ctx, 1);
        // o modelo nao e' uma nota completa: falta ao menos o percentual do Simples e a classificacao do IBS/CBS
        assert!(!c.limpa());
        assert!(c
            .pendencias
            .iter()
            .all(|p| p.campo != Some(CampoNota::ValorServico)));
        assert!(c
            .pendencias
            .iter()
            .all(|p| p.campo != Some(CampoNota::Descricao)));
        assert!(c
            .pendencias
            .iter()
            .all(|p| p.campo != Some(CampoNota::TomadorDocumento)));
    }

    #[test]
    fn o_ibscbs_nasce_conforme_o_regime_como_em_toda_nota_nova() {
        let d = dados_da_nota(&detalhe(&nfse()), &ctx_do_caso());
        assert_eq!(
            d.ibscbs_ativo,
            ctx_do_caso().regime().informa_ibscbs_por_padrao()
        );
    }
}
