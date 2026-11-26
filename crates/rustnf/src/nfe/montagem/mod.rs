// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Montagem e assinatura da NF-e (modelo 55), leiaute 4.00 com o IBS/CBS: do pedido em JSON ate o XML assinado,
//! sem rede. **Nao envia** a nota: o envio e' do `nfe::sefaz` (feature `nfe_sefaz`).
//!
//! * [`nota_de_json`] le o pedido ([`Nota`]); [`campos_desconhecidos`] e [`nota_de_json_estrito`] tratam o campo
//!   que o pedido traz e o leiaute nao conhece;
//! * [`conferir`] devolve as rejeicoes do MOC e das Notas Tecnicas que a nota levaria, para corrigir ANTES de gastar
//!   o numero; [`montar`] ja' recusa, com o campo e o codigo da rejeicao, o que o leiaute nao aceita;
//! * [`montar`] escreve o `infNFe` na ordem do XSD (pacote PL_010h) e calcula os totais; a mesma entrada da o mesmo
//!   XML (a data de emissao e o `cNF` vem do pedido);
//! * [`assinar`] assina o `infNFe` com o certificado A1 (RSA-SHA1 e digest SHA-1, os unicos que o XSD da NF-e
//!   aceita);
//! * [`avisos`] (e `Montada::avisos`) lista o que a nota tem de estranho sem ser recusado: PIS/COFINS que nao combinam com
//!   o regime informado e o Simples sem regime regular com CST que exige o `gIBSCBS`. Aviso nunca impede a montagem.
//!
//! Fora desta entrega: NFC-e (modelo 65), o QR Code (`infNFeSupl`: a nota com `tpImp` 6, DANFE simplificado, monta mas
//! nao assina), imposto de importacao, imposto seletivo, `detExport`, armamento, `NVE`, `ISSQN`, `retTrib`, `obsCont`,
//! `obsFisco`, `procRef`, `cana`, `agropecuario` e `impostoDevol`, e o envio. O que o pedido pede e a montagem ainda nao faz
//! e' recusado com erro claro; nada sai pela metade.
//!
//! Fontes: MOC 7.00 (Anexo I), NT 2025.002 (IBS/CBS/IS), NT 2026.005 (Simples Nacional), NT 2026.008 (valor liquido do
//! produto) e o pacote de esquemas PL_010h v1.03; a lista esta em `docs/FONTES.md`. O formato do pedido esta em
//! `docs/NFE_PEDIDO.md`.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};

use super::vigencias::{self, Ambiente};
use crate::chave_dfe;
use crate::reforma::aliquotas::tributos_somam_no_total;
use crate::reforma::calculo::{totalizar, vnftot, ItemIbsCbs};

mod assinar;
mod avisos;
mod cobr;
mod comb;
pub mod difal;
mod esquema;
mod esquema_grupos;
mod exporta;
mod formato;
mod gtin;
mod icms;
mod icms_mono;
mod icms_regras;
mod ide;
mod item;
mod json;
mod json_grupos;
mod modelo;
mod partes;
mod produto;
mod regras;
mod total;
mod transp;
mod uf_dest;

pub use assinar::assinar;
pub use avisos::{avisos, Aviso, RegimePisCofins, TipoAviso};
pub use cobr::{Cobranca, Duplicata, Fatura};
pub use comb::{Cide, Combustivel, Encerrante, OrigemComb};
pub use esquema::{campos_conhecidos, campos_desconhecidos, nota_de_json_estrito};
pub use exporta::{Compra, Exportacao};
pub use formato::{ErroMontagem, Res};
pub use json::nota_de_json;
pub use modelo::{
    Cartao, CompraGov, Destinatario, Emitente, Endereco, Icms, Ipi, Item, Local, Montada, Nota,
    Pagamento, PisCofins, RespTec, Veiculo,
};
pub use produto::{Medicamento, Rastro};
pub use regras::conferir;
pub use transp::{RetencaoIcms, Transportadora, Transporte, VeiculoTransp, VolumeTransp};

use cobr::cobr;
use exporta::{compra, exporta};
#[cfg(feature = "nfe_sefaz")]
pub(crate) use formato::documento as documento_cnpj_cpf;
pub(crate) use formato::na_uf;
pub(crate) use formato::uf_codigo;
#[cfg(feature = "nfe_sefaz")]
pub(crate) use formato::{dh, esc, filtrar, uf_sigla, Dec};
use formato::{documento, erro, erro_r};
use ide::{finalidade, ide};
use item::det;
use partes::{dest, emit, local, regime_sn, vigencia_pl010h};
use total::{inf_adic, pag, resp_tec, total, Ac};
use transp::transp;

/// Espaco de nomes do XML da NF-e.
pub const NS: &str = "http://www.portalfiscal.inf.br/nfe";
/// Versao do leiaute.
pub const VERSAO: &str = "4.00";
/// `xProd` do primeiro item em homologacao com DANFE simplificado (`tpImp` 6).
pub(crate) const HOM_XPROD: &str =
    "NOTA FISCAL EMITIDA EM AMBIENTE DE HOMOLOGACAO - SEM VALOR FISCAL";
/// `xNome` do destinatario em homologacao (regra do MOC: a nota de teste leva este texto).
pub(crate) const HOM_NOME_DEST: &str = "NF-E EMITIDA EM AMBIENTE DE HOMOLOGACAO - SEM VALOR FISCAL";
/// `cEAN` e `cEANTrib` do produto sem GTIN.
pub(crate) const SEM_GTIN: &str = "SEM GTIN";
/// `verProc` quando o pedido nao traz.
pub(crate) const VER_PROC: &str = concat!("rustnf ", env!("CARGO_PKG_VERSION"));

/// Escritor do XML: texto escapado nas folhas, tags abertas e fechadas na mao (a ordem e' a do XSD).
pub(crate) struct X(String);

impl X {
    fn t(&mut self, tag: &str, v: &str) {
        self.0
            .push_str(&format!("<{tag}>{}</{tag}>", formato::esc(v)));
    }
    fn o(&mut self, tag: &str, v: &Option<String>) {
        if let Some(v) = v {
            self.t(tag, v);
        }
    }
    fn abre(&mut self, tag: &str) {
        self.0.push_str(&format!("<{tag}>"));
    }
    fn fecha(&mut self, tag: &str) {
        self.0.push_str(&format!("</{tag}>"));
    }
}

/// `tpImp` da nota: o do pedido ou 1 (DANFE retrato).
pub(crate) fn tp_imp(n: &Nota) -> i64 {
    n.tp_imp.unwrap_or(1)
}

/// `indFinal` da nota: o do pedido ou 0 (normal).
pub(crate) fn ind_final(n: &Nota) -> i64 {
    n.ind_final.unwrap_or(0)
}

/// `indPres` da nota: o do pedido ou 9 (outros).
pub(crate) fn ind_pres(n: &Nota) -> i64 {
    n.ind_pres.unwrap_or(9)
}

/// Sigla da UF do emitente, conferida.
pub(crate) fn uf_nota(n: &Nota) -> Res<String> {
    let uf = n.emit.endereco.uf.trim().to_uppercase();
    if uf_codigo(&uf).is_none() {
        return Err(erro(
            format!("UF do emitente invalida: {:?}", n.emit.endereco.uf),
            "enderEmit/UF",
        ));
    }
    Ok(uf)
}

/// Inicio da vigencia de uma regra no ambiente da nota (`tpAmb` 1 = producao; o resto, homologacao). A chave tem de
/// existir em [`vigencias`]; um teste confere todas as que a montagem usa.
pub(crate) fn vigencia(chave: &str, tp_amb: i64) -> NaiveDate {
    let amb = if tp_amb == 1 {
        Ambiente::Producao
    } else {
        Ambiente::Homologacao
    };
    vigencias::inicio(chave, amb).unwrap_or(NaiveDate::MIN)
}

/// O defeito da chave de 44 posicoes em texto, ou `None` se ela e' valida (inclusive o CPF ou CNPJ do emitente). Alem do
/// 55 e do 65, aceita o modelo 59 (CF-e SAT): a nota pode referenciar cupom fiscal, e as NT tratam esse caso
/// (rejeicoes 375 e 679).
pub(crate) fn erro_chave(c: &str) -> Option<String> {
    let k = match chave_dfe::decompor(c, true) {
        Ok(k) => k,
        Err(e) => return Some(e.to_string()),
    };
    match k.erro {
        None => None,
        Some(chave_dfe::ErroChave::Modelo) if k.modelo == "59" => {
            // so' o modelo falhou na ordem da conferencia; falta conferir o emitente
            let valido = if k.emitente_cpf {
                chave_dfe::cpf_valido(&k.emitente)
            } else {
                chave_dfe::cnpj_valido(&k.emitente)
            };
            let e = if k.emitente_cpf {
                chave_dfe::ErroChave::CpfEmitente
            } else {
                chave_dfe::ErroChave::CnpjEmitente
            };
            (!valido).then(|| e.to_string())
        }
        Some(e) => Some(e.to_string()),
    }
}

/// Monta o `infNFe` completo (sem `Signature`) e a chave de acesso. A entrada que o leiaute nao aceita volta como
/// [`ErroMontagem`], com o campo e, quando ha, o codigo da rejeicao.
///
/// # Errors
///
/// [`ErroMontagem`] com o campo do defeito: modelo diferente de 55, `cNF` ausente, campo fora do leiaute,
/// regra do MOC ou da NT conferida antes do envio.
pub fn montar(n: &Nota) -> Res<Montada> {
    if n.modelo == 65 {
        return Err(erro(
            "modelo 65 (NFC-e) ainda nao e' montado: so' a NF-e modelo 55",
            "mod",
        ));
    }
    if n.modelo != 55 {
        return Err(erro(format!("modelo {}: use 55 (NF-e)", n.modelo), "mod"));
    }
    if n.itens.is_empty() {
        return Err(erro("nota sem itens", "det"));
    }
    if n.itens.len() > 990 {
        return Err(erro("maximo 990 itens", "det"));
    }
    let uf = uf_nota(n)?;
    let emissao_uf = na_uf(&n.emissao, &uf);
    let Some(codigo) = n.codigo else {
        return Err(erro(
            "cNF ausente: informe o codigo numerico (8 digitos); a mesma entrada tem de dar o mesmo XML (veja \
             chave_dfe::gerar_codigo)",
            "cNF",
        ));
    };
    if codigo < 0 || !chave_dfe::codigo_valido(codigo as u64, n.numero.max(0) as u64) {
        return Err(erro_r(
            format!("cNF {codigo} invalido (igual ao nNF ou sequencia proibida)"),
            "cNF",
            "897",
        ));
    }
    let aamm = format!("{:02}{:02}", emissao_uf.year() % 100, emissao_uf.month());
    let (_, doc) = documento(&n.emit.documento, "emit/CNPJ")?;
    let chave = chave_dfe::montar(&chave_dfe::Montar {
        cuf: uf_codigo(&uf).unwrap_or(0),
        aamm: &aamm,
        emitente: &doc,
        modelo: 55,
        serie: n.serie.max(0) as u64,
        numero: n.numero.max(0) as u64,
        tp_emis: n.tp_emis.max(0) as u32,
        codigo: Some(codigo as u64),
    })
    .map_err(|e| erro(e.to_string(), "chave"))?;
    let mut x = X(format!(
        "<NFe xmlns=\"{NS}\"><infNFe Id=\"NFe{chave}\" versao=\"{VERSAO}\">"
    ));
    let regime = regime_sn(n)?;
    vigencia_pl010h(n, emissao_uf.date_naive())?;
    finalidade(n, emissao_uf.year())?;
    ide(&mut x, n, &chave, &uf)?;
    emit(&mut x, n)?;
    dest(&mut x, n)?;
    // XSD: depois do dest, antes do autXML
    local(&mut x, "retirada", n.retirada.as_ref(), n.remover_acento)?;
    local(&mut x, "entrega", n.entrega.as_ref(), n.remover_acento)?;
    let tem_ibscbs = n.itens.iter().any(|i| i.ibscbs.is_some());
    if tem_ibscbs && n.itens.iter().any(|i| i.dfe_ref.is_some()) && !n.refs_nfe.is_empty() {
        return Err(erro_r(
            "referenciamento na nota (NFref) E no item (DFeReferenciado) ao mesmo tempo (VC02-05)",
            "NFref",
            "1010",
        ));
    }
    let mut ac = Ac::default();
    for (i, it) in n.itens.iter().enumerate() {
        det(
            &mut x,
            n,
            i + 1,
            it,
            &mut ac,
            tem_ibscbs,
            emissao_uf.year(),
            regime,
        )?;
    }
    let totais = total(&mut x, n, &ac, &uf)?;
    transp(&mut x, n)?;
    cobr(&mut x, n)?;
    pag(&mut x, n)?;
    inf_adic(&mut x, n)?;
    exporta(&mut x, n)?;
    compra(&mut x, n)?;
    resp_tec(&mut x, n, &chave)?;
    x.0.push_str("</infNFe></NFe>");
    Ok(Montada {
        xml: x.0,
        chave,
        totais,
        avisos: avisos::avisos(n),
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Toda chave de calendario que a montagem consulta existe na tabela de `nfe::vigencias`.
    #[test]
    fn as_chaves_de_vigencia_existem() {
        for chave in [
            "pl_010h",
            "nt2026_008.nb01_30",
            "nt2025_002.v1_53",
            "nt2025_002.vc02_14",
            "nt2025_002.vc02_40",
            "nt2026_002.vc02_04_tpimp6",
            "nt2026_002.v1_11_adiadas",
        ] {
            assert!(vigencias::buscar(chave).is_some(), "{chave}");
        }
    }
}
