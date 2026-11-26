// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O servidor com a NFC-e (e, quando o teste pede, tambem a NF-e) sobre o transporte falso da SEFAZ, com emitente e documentos
//! ficticios. As respostas da SEFAZ sao as de `nfe_falso`: a NFC-e usa os mesmos servicos de autorizacao, consulta e evento.

use axum::Router;
use chrono::{TimeZone, Utc};
use rustnf::nfe::sefaz::ClienteSefaz;
use rustnf_app::cofre::{Cofre, CofreEmMemoria};
use rustnf_app::config::Config as ConfigApp;
use rustnf_app::nfce;
use rustnf_app::nfe::config_sefaz;
use rustnf_app::notas::nfe::NotasNfe;
use rustnf_app::segredo::Segredo;
use rustnf_cli::http::nfe::dinamico;
use rustnf_cli::http::Estado;
use rustnf_cli::http::{roteador, Config, ServidorNfce, ServidorNfe, Token};
use serde_json::{json, Value};

use super::nfe_falso::{FalsoNfe, Resposta};
use super::{a1, emissor_falso, Falso, Pasta, TOKEN};

/// O valor do CSC dos testes: ficticio, so' para procurar vazamento.
pub const CSC_TESTE: &str = "ZZTESTECSC0123456789ABCDEF000000";

/// A configuracao de um emitente ficticio de SP no Simples, com a pasta de notas dentro de `p`. Com `v2`, o QR Code versao 2
/// (o `idCSC` e' sempre `1`).
pub fn config_nfce(p: &Pasta, v2: bool) -> ConfigApp {
    let texto = format!(
        "[emitente]\ncnpj = \"11222333000181\"\nnome = \"LOJA DE TESTE LTDA\"\nmunicipio = \"3550308\"\n\
         [notas]\npasta = '{}'\n[nfe]\nuf = \"SP\"\nie = \"111111111111\"\ncrt = 1\n\
         [nfe.endereco]\nlogradouro = \"Rua das Flores\"\nnumero = \"100\"\nbairro = \"Centro\"\n\
         municipio = \"Sao Paulo\"\ncep = \"01001000\"\n[nfce]\nid_csc = \"1\"\nqr_versao = {}\n",
        p.caminho("notas").to_string_lossy(),
        if v2 { 2 } else { 3 }
    );
    ConfigApp::de_texto(&texto).unwrap()
}

/// O pedido de uma NFC-e de venda de cafe, sem emitente, destinatario nem numero.
pub fn pedido_nfce() -> Value {
    json!({
        "emissao": "2026-10-03T09:00:00-03:00",
        "nat_op": "VENDA",
        "ind_final": 1, "ind_pres": 1,
        "itens": [{"codigo": "1", "descricao": "Cafe torrado 500g", "ncm": "09012100", "cfop": "5102",
                   "unidade": "UN", "quantidade": "2", "valor_unitario": "15.90",
                   "icms": {"codigo": "102"}, "pis": {"cst": "49"}, "cofins": {"cst": "49"}}],
        "pagamentos": [{"t_pag": "01", "valor": "31.80"}]
    })
}

fn cliente(
    config: &ConfigApp,
    falso: &FalsoNfe,
) -> ClienteSefaz<rustnf_cli::http::nfe::TransporteDin> {
    ClienteSefaz::com_transporte(config_sefaz(config).unwrap(), dinamico(falso.clone()))
        .unwrap()
        .com_relogio(|| Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap())
        .com_dormir(|_| Box::pin(async {}))
}

fn estado(nfce: ServidorNfce) -> Estado {
    Estado::com_emissao(
        Config::default(),
        emissor_falso(Falso::default()),
        Token::novo(TOKEN.into()).unwrap(),
    )
    .com_nfce(nfce)
}

/// O servidor com emissao e a NFC-e sobre o transporte falso com o `roteiro` (QR Code versao 3, sem CSC). Devolve tambem o
/// transporte (para contar o que sobrou do roteiro) e a pasta `nfce/` das notas.
pub fn app_nfce(
    pasta: &Pasta,
    roteiro: Vec<(&'static str, Resposta)>,
) -> (Router, FalsoNfe, NotasNfe) {
    let config = config_nfce(pasta, false);
    let notas = NotasNfe::abrir_nfce(pasta.caminho("notas")).unwrap();
    let falso = FalsoNfe::novo(roteiro);
    let nfce = ServidorNfce::com_cliente(
        &config,
        Some(notas.clone()),
        cliente(&config.como_nfce(), &falso),
        a1(),
        nfce::opcoes_qr(&CofreEmMemoria::novo(), &config.como_nfce()).unwrap(),
    );
    (roteador(estado(nfce)), falso, notas)
}

/// Como [`app_nfce`], mas com a NF-e ligada ao MESMO transporte: as duas notas de modelos diferentes na mesma serie numerica.
/// Devolve a pasta da NFC-e e a da NF-e.
pub fn app_nfce_e_nfe(
    pasta: &Pasta,
    roteiro: Vec<(&'static str, Resposta)>,
) -> (Router, FalsoNfe, NotasNfe, NotasNfe) {
    let config = config_nfce(pasta, false);
    let notas_nfce = NotasNfe::abrir_nfce(pasta.caminho("notas")).unwrap();
    let notas_nfe = NotasNfe::abrir(pasta.caminho("notas")).unwrap();
    let falso = FalsoNfe::novo(roteiro);
    let nfce = ServidorNfce::com_cliente(
        &config,
        Some(notas_nfce.clone()),
        cliente(&config.como_nfce(), &falso),
        a1(),
        nfce::opcoes_qr(&CofreEmMemoria::novo(), &config.como_nfce()).unwrap(),
    );
    let nfe = ServidorNfe::com_cliente(
        config.clone(),
        Some(notas_nfe.clone()),
        cliente(&config, &falso),
        a1(),
    );
    (
        roteador(estado(nfce).com_nfe(nfe)),
        falso,
        notas_nfce,
        notas_nfe,
    )
}

/// O servidor com o QR Code versao 2 e o CSC de teste guardado no cofre em memoria, lido ao subir (como o `rustnf serve`).
pub fn app_nfce_com_csc(
    pasta: &Pasta,
    roteiro: Vec<(&'static str, Resposta)>,
) -> (Router, FalsoNfe, NotasNfe) {
    let config = config_nfce(pasta, true);
    let cofre = CofreEmMemoria::novo();
    nfce::csc::guardar(
        &cofre,
        &config.como_nfce(),
        &Segredo::novo(CSC_TESTE.into()),
    )
    .unwrap();
    let notas = NotasNfe::abrir_nfce(pasta.caminho("notas")).unwrap();
    let falso = FalsoNfe::novo(roteiro);
    let nfce = ServidorNfce::com_cliente(
        &config,
        Some(notas.clone()),
        cliente(&config.como_nfce(), &falso),
        a1(),
        nfce::opcoes_qr(&cofre, &config.como_nfce()).unwrap(),
    );
    (roteador(estado(nfce)), falso, notas)
}

/// O servidor com o QR Code versao 2 e SEM CSC guardado (a NFC-e sem emissao, com a frase do que fazer) e a NF-e pronta.
pub fn app_nfce_sem_csc(
    pasta: &Pasta,
    roteiro_nfe: Vec<(&'static str, Resposta)>,
) -> (Router, FalsoNfe) {
    let config = config_nfce(pasta, true);
    let erro =
        nfce::opcoes_qr(&CofreEmMemoria::novo() as &dyn Cofre, &config.como_nfce()).unwrap_err();
    let notas_nfce = NotasNfe::abrir_nfce(pasta.caminho("notas")).unwrap();
    let falso = FalsoNfe::novo(roteiro_nfe);
    let nfe = ServidorNfe::com_cliente(
        config.clone(),
        Some(NotasNfe::abrir(pasta.caminho("notas")).unwrap()),
        cliente(&config, &falso),
        a1(),
    );
    let nfce = ServidorNfce::sem_emissao(&config, Some(notas_nfce), erro.to_string());
    (roteador(estado(nfce).com_nfe(nfe)), falso)
}

/// O servidor com emissao e SEM configuracao: a NFC-e diz o que configurar.
pub fn app_nfce_sem_configuracao() -> Router {
    roteador(Estado::com_emissao(
        Config::default(),
        emissor_falso(Falso::default()),
        Token::novo(TOKEN.into()).unwrap(),
    ))
}

/// O servidor so' de leitura, com a configuracao da NFC-e.
pub fn app_nfce_leitura(pasta: &Pasta) -> Router {
    let nfce = ServidorNfce::sem_emissao(
        &config_nfce(pasta, false),
        Some(NotasNfe::abrir_nfce(pasta.caminho("notas")).unwrap()),
        "o servidor nao foi iniciado com --permitir-emissao",
    );
    roteador(Estado::somente_leitura(Config::default()).com_nfce(nfce))
}

/// Todo texto que o servidor gravou na pasta de notas, para procurar o que nunca deve estar la.
pub fn tudo_que_foi_gravado(pasta: &Pasta) -> String {
    fn ler(dir: &std::path::Path, saida: &mut String) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                ler(&p, saida);
            } else {
                saida.push_str(&String::from_utf8_lossy(
                    &std::fs::read(&p).unwrap_or_default(),
                ));
            }
        }
    }
    let mut s = String::new();
    ler(&pasta.0, &mut s);
    s
}
