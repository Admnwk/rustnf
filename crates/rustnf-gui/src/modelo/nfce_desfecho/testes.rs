// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use rustnf_app::nfce::{Emissao, Gravada, Pendente, Prazo};
use rustnf_app::notas::nfe::Registro;

use super::*;

fn gravada() -> Gravada {
    Gravada {
        chave: "35261011222333000181650010000000419000000019".into(),
        serie: "1".into(),
        numero: 41,
        prazo: "05/10/2026 23:59".into(),
        motivo: "a SEFAZ nao respondeu ao status do servico antes do envio",
        avisos: Vec::new(),
    }
}

fn pendente(offline: bool, prazo: Prazo, recusa: &str) -> Pendente {
    Pendente {
        registro: Registro {
            serie: "1".into(),
            numero: "41".into(),
            offline,
            recusa: recusa.into(),
            ..Registro::default()
        },
        prazo,
        limite: if offline {
            "05/10/2026 23:59".into()
        } else {
            String::new()
        },
    }
}

#[test]
fn a_off_line_diz_que_o_cupom_vale_e_o_prazo_vem_da_aplicacao() {
    let d = DesfechoCaixa::de(Ok(Emissao::Offline(Box::new(gravada()))));
    assert!(d.tem_cupom() && d.deixa_pendente());
    assert_eq!(d.titulo(), "NFC-e nº 41 emitida sem internet");
    let t = d.texto();
    assert!(t.contains("O cupom vale"), "{t}");
    assert!(t.contains("05/10/2026 23:59"), "{t}");
    assert!(!t.contains("rustnf nfce"), "nada de linha de comando: {t}");
}

#[test]
fn recusa_antes_de_enviar_diz_o_que_fazer_no_caixa_sem_passo_de_assistente() {
    let e = Erro::recusa(
        "nNF",
        "o numero 41 da serie 1 esta com uma NFC-e pendente (chave 35261011222333000181650010000000419000000019): retome os pendentes",
    );
    let d = DesfechoCaixa::do_erro(&e);
    let DesfechoCaixa::Recusada { pendencias } = &d else {
        panic!("{d:?}")
    };
    assert!(pendencias[0].como_resolver.contains("Fechar o caixa"));
    assert!(!d.texto().contains("passo"));
    assert!(!d.deixa_pendente() && !d.tem_cupom());
    let csc = DesfechoCaixa::do_erro(&Erro::Uso(
        "o QR Code versao 2 precisa do CSC e ele nao esta guardado neste ambiente".into(),
    ));
    assert!(
        csc.texto().contains("Configurações, NFC-e"),
        "{}",
        csc.texto()
    );
}

#[test]
fn sem_resposta_nunca_manda_emitir_de_novo() {
    let d = DesfechoCaixa::SemResposta {
        chave: "k".into(),
        numero: "7".into(),
        serie: "1".into(),
    };
    assert!(d.deixa_pendente() && !d.tem_cupom());
    assert!(d.texto().contains("Não emita outro cupom"));
    assert!(d.texto().contains("MESMA chave"));
}

#[test]
fn o_fechamento_conta_as_notas_e_diz_o_prazo_mais_curto() {
    assert!(aviso_do_fechamento(&[]).is_none());
    let l = vec![
        pendente(true, Prazo::Folga(7200), ""),
        pendente(false, Prazo::Sem, ""),
    ];
    let a = aviso_do_fechamento(&l).unwrap();
    assert!(a.starts_with("2 NFC-e para transmitir"), "{a}");
    assert!(a.contains("Transmita antes de 05/10/2026 23:59"), "{a}");
    let v = vec![pendente(true, Prazo::Vencido(60), "")];
    assert!(aviso_do_fechamento(&v).unwrap().contains("JÁ VENCEU"));
    let um = aviso_do_fechamento(&[pendente(false, Prazo::Sem, "")]).unwrap();
    assert!(um.starts_with("1 NFC-e"), "{um}");
}

#[test]
fn cada_pendente_tem_a_frase_do_seu_caso() {
    let f = |p: &Pendente| frase_do_pendente(p);
    assert!(f(&pendente(true, Prazo::Folga(7200), "")).contains("transmita até 05/10/2026 23:59"));
    assert!(f(&pendente(true, Prazo::AVencer(600), "")).contains("agora"));
    assert!(f(&pendente(true, Prazo::Vencido(600), "")).contains("VENCEU"));
    assert!(f(&pendente(false, Prazo::Sem, "")).contains("não emita outro cupom"));
    assert!(
        f(&pendente(true, Prazo::Folga(60), "cStat 778: NCM")).contains("recusou a transmissão")
    );
    assert_eq!(
        rotulo_do_pendente(&pendente(true, Prazo::Sem, "")),
        "Sem internet (off-line)"
    );
    assert_eq!(
        rotulo_do_pendente(&pendente(false, Prazo::Sem, "")),
        "Sem resposta"
    );
}
