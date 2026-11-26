// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `rustnf nfe` por processo real, nos comandos que nao usam a rede: montar, conferir, a pasta de notas da NF-e, a lista dos
//! pendentes e a convivencia com a pasta da NFS-e. Nenhum teste chama a SEFAZ.

mod comum;

use std::io::Write;
use std::process::{Command, Stdio};

use comum::*;
use rustnf_app::notas::nfe::{EventoGuardado, Inutilizacao, NotasNfe, Registro, Situacao};
use rustnf_app::notas::Notas;
use serde_json::{json, Value};

struct Saida {
    codigo: i32,
    out: String,
    err: String,
}

fn rodar(vazia: &Pasta, args: &[&str], stdin: Option<&str>) -> Saida {
    rodar_com_env(vazia, args, stdin, &[])
}

fn rodar_com_env(vazia: &Pasta, args: &[&str], stdin: Option<&str>, env: &[(&str, &str)]) -> Saida {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rustnf"));
    isolar(&mut cmd, &vazia.0);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut filho = cmd.spawn().unwrap();
    if let Some(t) = stdin {
        filho.stdin.take().unwrap().write_all(t.as_bytes()).unwrap();
    } else {
        drop(filho.stdin.take());
    }
    let s = filho.wait_with_output().unwrap();
    Saida {
        codigo: s.status.code().unwrap(),
        out: String::from_utf8_lossy(&s.stdout).into_owned(),
        err: String::from_utf8_lossy(&s.stderr).into_owned(),
    }
}

/// O arquivo de configuracao de um emitente ficticio de SP no Simples, com a pasta de notas dentro de `p`.
fn configurar(p: &Pasta, nfe_extra: &str) -> String {
    let texto = format!(
        "[emitente]\ncnpj = \"11222333000181\"\nnome = \"LOJA DE TESTE LTDA\"\nmunicipio = \"3550308\"\n\
         [notas]\npasta = '{}'\n[nfe]\nuf = \"SP\"\nie = \"111111111111\"\ncrt = 1\n{nfe_extra}\
         [nfe.endereco]\nlogradouro = \"Rua das Flores\"\nnumero = \"100\"\nbairro = \"Centro\"\n\
         municipio = \"Sao Paulo\"\ncep = \"01001000\"\n",
        p.caminho("notas").to_string_lossy()
    );
    p.escrever("rustnf.toml", texto.as_bytes())
}

fn pedido() -> Value {
    json!({
        "emissao": "2026-10-03T09:00:00-03:00",
        "nat_op": "VENDA",
        "dest": {"documento": "52998224725", "nome": "CLIENTE TESTE", "ind_ie_dest": 9,
                 "endereco": {"logradouro": "Rua A", "numero": "1", "bairro": "Centro",
                              "cod_municipio": "3550308", "municipio": "Sao Paulo", "uf": "SP"}},
        "ind_final": 1, "ind_pres": 1,
        "itens": [{"codigo": "1", "descricao": "Cafe torrado 500g", "ncm": "09012100", "cfop": "5102",
                   "unidade": "UN", "quantidade": "2", "valor_unitario": "15.90",
                   "icms": {"codigo": "102"}, "pis": {"cst": "49"}, "cofins": {"cst": "49"}}],
        "pagamentos": [{"t_pag": "01", "valor": "31.80"}]
    })
}

fn chave(serie: u64, numero: u64) -> String {
    rustnf::chave_dfe::montar(&rustnf::chave_dfe::Montar {
        cuf: 35,
        aamm: "2610",
        emitente: "11222333000181",
        modelo: 55,
        serie,
        numero,
        tp_emis: 1,
        codigo: Some(12_345_670 + numero),
    })
    .unwrap()
}

fn registro(numero: u64) -> Registro {
    Registro {
        chave: chave(1, numero),
        serie: "1".into(),
        numero: numero.to_string(),
        emissao: "2026-10-03T09:00:00-03:00".into(),
        valor: "31.80".into(),
        ambiente: "homologacao".into(),
        destinatario_documento: "52998224725".into(),
        destinatario_nome: "CLIENTE TESTE".into(),
        desde: 100,
        ..Registro::default()
    }
}

#[test]
fn montar_completa_o_pedido_com_a_configuracao_e_escreve_o_xml() {
    let p = Pasta::nova();
    let cfg = configurar(&p, "");
    let pedido = p.escrever("pedido.json", pedido().to_string().as_bytes());
    let s = rodar(&p, &["--config", &cfg, "nfe", "montar", &pedido], None);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(
        s.out
            .starts_with("<NFe xmlns=\"http://www.portalfiscal.inf.br/nfe\">"),
        "{}",
        s.out
    );
    for trecho in [
        "<CNPJ>11222333000181</CNPJ>",
        "<xNome>LOJA DE TESTE LTDA</xNome>",
        "<UF>SP</UF>",
        "<IE>111111111111</IE>",
        "<CRT>1</CRT>",
        "<nNF>1</nNF>",
        "<tpAmb>2</tpAmb>",
    ] {
        assert!(s.out.contains(trecho), "faltou {trecho}");
    }
    assert!(!s.out.contains("<Signature"), "montar nao assina");
    assert!(
        std::fs::read_dir(p.caminho("notas").join("nfe"))
            .unwrap()
            .count()
            == 0,
        "montar nao grava nada na pasta"
    );
    // o mesmo pedido pelo stdin
    let s = rodar(
        &p,
        &["--config", &cfg, "nfe", "montar", "-"],
        Some(&pedido_texto()),
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
}

fn pedido_texto() -> String {
    pedido().to_string()
}

#[test]
fn montar_recusa_campo_desconhecido_e_ignorar_avisa() {
    let p = Pasta::nova();
    let cfg = configurar(&p, "");
    let mut ped = pedido();
    ped["campo_inventado"] = json!("x");
    let arq = p.escrever("pedido.json", ped.to_string().as_bytes());
    let s = rodar(&p, &["--config", &cfg, "nfe", "montar", &arq], None);
    assert_eq!(s.codigo, 3);
    assert!(s.err.contains("campo_inventado"), "{}", s.err);
    assert!(s.out.is_empty());
    let s = rodar(
        &p,
        &[
            "--config",
            &cfg,
            "nfe",
            "montar",
            &arq,
            "--ignorar-desconhecidos",
        ],
        None,
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(
        s.err
            .contains("aviso: campo desconhecido no pedido, ignorado: campo_inventado"),
        "{}",
        s.err
    );
}

#[test]
fn conferir_diz_ok_ou_a_recusa_com_o_campo_e_o_codigo_da_saida() {
    let p = Pasta::nova();
    let cfg = configurar(&p, "");
    let bom = p.escrever("bom.json", pedido().to_string().as_bytes());
    let s = rodar(&p, &["--config", &cfg, "nfe", "conferir", &bom], None);
    assert_eq!((s.codigo, s.out.as_str()), (0, "ok\n"), "{}", s.err);
    let s = rodar(
        &p,
        &["--config", &cfg, "nfe", "conferir", &bom, "--json"],
        None,
    );
    let v: Value = serde_json::from_str(&s.out).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["chave"].as_str().unwrap().len(), 44);
    assert_eq!(v["numero"], 1);
    assert_eq!(v["totais"]["vNF"], "31.80");
    assert_eq!(v["avisos"], json!([]));

    let mut ruim = pedido();
    ruim["itens"][0]["ncm"] = json!("123");
    let arq = p.escrever("ruim.json", ruim.to_string().as_bytes());
    let s = rodar(
        &p,
        &["--config", &cfg, "nfe", "conferir", &arq, "--json"],
        None,
    );
    assert_eq!(s.codigo, 3, "{}", s.err);
    let v: Value = serde_json::from_str(&s.out).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["codigo"], "recusa");
    assert!(s.err.starts_with("erro: "), "{}", s.err);
}

#[test]
fn sem_a_uf_do_emitente_o_erro_diz_onde_informar() {
    let p = Pasta::nova();
    let cfg = p.escrever(
        "rustnf.toml",
        b"[emitente]\ncnpj = \"11222333000181\"\nnome = \"LOJA TESTE\"\n[nfe]\ncrt = 1\n",
    );
    let arq = p.escrever("pedido.json", pedido().to_string().as_bytes());
    let s = rodar(&p, &["--config", &cfg, "nfe", "montar", &arq], None);
    // sem pasta de notas e sem numero no pedido, falta o numero; com a UF ausente o aviso vem primeiro
    assert_eq!(s.codigo, 2, "{}", s.err);
    assert!(s.err.contains("[nfe].uf"), "{}", s.err);
}

#[test]
fn valor_de_nfe_que_nao_serve_aponta_a_secao_e_o_campo() {
    let p = Pasta::nova();
    let cfg = configurar(&p, "serie_errada = 1\n");
    let s = rodar(&p, &["--config", &cfg, "nfe", "notas", "numero"], None);
    assert_eq!(s.codigo, 2, "{}", s.err);
    let cfg = p.escrever("rustnf2.toml", b"[nfe]\nambiente = \"restrita\"\n");
    let s = rodar(&p, &["--config", &cfg, "nfe", "notas", "numero"], None);
    assert_eq!(s.codigo, 2);
    assert!(s.err.contains("[nfe] ambiente"), "{}", s.err);
}

/// Enche a pasta como as acoes enchem: notas 1 (autorizada) e 4 (cancelada), a 2 pendente e a faixa 5 a 6 inutilizada.
fn encher(p: &Pasta) -> NotasNfe {
    let n = NotasNfe::abrir(p.caminho("notas")).unwrap();
    for numero in [1, 2, 4] {
        n.guardar_pendente(&registro(numero), "<NFe/>").unwrap();
    }
    for (numero, situacao) in [(1, Situacao::Autorizada), (4, Situacao::Autorizada)] {
        n.concluir(
            &chave(1, numero),
            situacao,
            Some("<nfeProc/>"),
            "135260000000001",
            "100",
        )
        .unwrap();
    }
    let ev = EventoGuardado {
        tipo: "110111".into(),
        sequencia: 1,
        protocolo: "135260000000009".into(),
    };
    n.guardar_evento(&chave(1, 4), &ev, "<procEventoNFe/>", "")
        .unwrap();
    let faixa = Inutilizacao {
        id: "ID35261122233300018155001000000005000000006".into(),
        serie: "1".into(),
        inicio: 5,
        fim: 6,
        ano: "2026".into(),
        protocolo: "135260000000300".into(),
        situacao: Situacao::Inutilizada,
    };
    n.guardar_inutilizacao(&faixa, "<ProcInutNFe/>").unwrap();
    n
}

#[test]
fn notas_numero_mostra_o_proximo_os_pulados_e_so_anda_para_a_frente() {
    let p = Pasta::nova();
    let cfg = configurar(&p, "");
    encher(&p);
    let s = rodar(&p, &["--config", &cfg, "nfe", "notas", "numero"], None);
    assert_eq!(s.codigo, 0, "{}", s.err);
    // 1, 2 e 4 tem nota, 5 e 6 foram inutilizados: o 3 foi pulado
    assert!(s.out.contains("serie 1: proxima NF-e 7"), "{}", s.out);
    assert!(s.out.contains("numeros pulados"), "{}", s.out);
    assert!(s.out.contains(": 3\n"), "{}", s.out);
    let s = rodar(
        &p,
        &[
            "--config",
            &cfg,
            "nfe",
            "notas",
            "numero",
            "--proximo",
            "100",
        ],
        None,
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(s.out.contains("proxima NF-e 100"), "{}", s.out);
    assert!(
        !s.out.contains("pulados"),
        "o que veio de outro sistema nao e' numero pulado: {}",
        s.out
    );
    let s = rodar(
        &p,
        &[
            "--config",
            &cfg,
            "nfe",
            "notas",
            "numero",
            "--proximo",
            "50",
        ],
        None,
    );
    assert_eq!(s.codigo, 2, "{}", s.out);
    assert!(s.err.contains("NF-e"), "{}", s.err);
    // outra serie comeca em 1
    let s = rodar(
        &p,
        &["--config", &cfg, "nfe", "notas", "numero", "--serie", "2"],
        None,
    );
    assert!(s.out.contains("serie 2: proxima NF-e 1"), "{}", s.out);
}

#[test]
fn notas_listar_em_texto_json_csv_e_com_filtros() {
    let p = Pasta::nova();
    let cfg = configurar(&p, "");
    encher(&p);
    let s = rodar(&p, &["--config", &cfg, "nfe", "notas", "listar"], None);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert_eq!(s.out.lines().count(), 3, "{}", s.out);
    assert!(
        s.out.contains("nfe 4\tserie 1\t31.80\tcancelada\t"),
        "{}",
        s.out
    );
    assert!(s.out.contains("\tpendente\t"), "{}", s.out);

    let s = rodar(
        &p,
        &[
            "--config",
            &cfg,
            "nfe",
            "notas",
            "listar",
            "--json",
            "--situacao",
            "cancelada",
        ],
        None,
    );
    let v: Value = serde_json::from_str(&s.out).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["numero"], "4");
    assert_eq!(v[0]["eventos"][0]["tipo"], "110111");

    let s = rodar(
        &p,
        &["--config", &cfg, "nfe", "notas", "listar", "--csv"],
        None,
    );
    assert!(
        s.out
            .starts_with("emissao;serie;numero;valor;situacao;ambiente;protocolo;"),
        "{}",
        s.out
    );
    assert_eq!(s.out.lines().count(), 4);

    let s = rodar(
        &p,
        &[
            "--config",
            &cfg,
            "nfe",
            "notas",
            "listar",
            "--de",
            "2026-11-01",
        ],
        None,
    );
    assert!(s.out.is_empty());
    assert!(s.err.contains("nenhuma nota"), "{}", s.err);
    let s = rodar(
        &p,
        &[
            "--config",
            &cfg,
            "nfe",
            "notas",
            "listar",
            "--de",
            "01/11/2026",
        ],
        None,
    );
    assert_eq!(s.codigo, 2);

    // as faixas inutilizadas
    let s = rodar(
        &p,
        &[
            "--config",
            &cfg,
            "nfe",
            "notas",
            "listar",
            "--situacao",
            "inutilizada",
        ],
        None,
    );
    assert!(
        s.out
            .contains("numeros 5 a 6\tinutilizada\tprotocolo 135260000000300"),
        "{}",
        s.out
    );
    let s = rodar(
        &p,
        &[
            "--config",
            &cfg,
            "nfe",
            "notas",
            "listar",
            "--situacao",
            "inutilizada",
            "--json",
        ],
        None,
    );
    assert_eq!(serde_json::from_str::<Value>(&s.out).unwrap()[0]["fim"], 6);
    let s = rodar(
        &p,
        &[
            "--config",
            &cfg,
            "nfe",
            "notas",
            "listar",
            "--situacao",
            "inutilizada",
            "--csv",
        ],
        None,
    );
    assert_eq!(s.codigo, 2);
}

#[test]
fn notas_exportar_e_reindexar() {
    let p = Pasta::nova();
    let cfg = configurar(&p, "");
    encher(&p);
    let destino = p.texto("contabilidade");
    let s = rodar(
        &p,
        &[
            "--config", &cfg, "nfe", "notas", "exportar", "--mes", "2026-10", "--para", &destino,
        ],
        None,
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(
        s.out
            .contains("3 nota(s) na lista (notas.csv), 4 XML copiado(s)"),
        "{}",
        s.out
    );
    assert!(p.caminho("contabilidade").join("notas.csv").is_file());
    let s = rodar(
        &p,
        &[
            "--config", &cfg, "nfe", "notas", "exportar", "--mes", "2026-11", "--para", &destino,
        ],
        None,
    );
    assert_eq!(s.codigo, 0);
    assert!(s.err.contains("nenhuma nota nesse mes"), "{}", s.err);
    let s = rodar(
        &p,
        &[
            "--config", &cfg, "nfe", "notas", "exportar", "--mes", "2026-13", "--para", &destino,
        ],
        None,
    );
    assert_eq!(s.codigo, 2);

    // os arquivos desta pasta sao de mentira (nao sao NF-e): o indice refeito sai vazio e conta o que ficou de fora
    let s = rodar(&p, &["--config", &cfg, "nfe", "notas", "reindexar"], None);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(
        s.out.contains("indice refeito: 0 nota(s), 0 cancelada(s)"),
        "{}",
        s.out
    );
    assert!(s.err.contains("ficaram de fora"), "{}", s.err);
}

#[test]
fn pendentes_lista_sem_rede_e_diz_como_retomar() {
    let p = Pasta::nova();
    let cfg = configurar(&p, "");
    let s = rodar(&p, &["--config", &cfg, "nfe", "pendentes"], None);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(s.err.contains("nenhuma nota pendente"));
    encher(&p);
    let s = rodar(&p, &["--config", &cfg, "nfe", "pendentes"], None);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert_eq!(
        s.out.lines().count(),
        1,
        "so' a nota 2 esta pendente: {}",
        s.out
    );
    assert!(
        s.out.contains("nfe 2\tserie 1\t31.80\tpendente\t"),
        "{}",
        s.out
    );
    assert!(s.out.contains("consultas 0"), "{}", s.out);
    assert!(
        s.err.contains("rustnf nfe pendentes --retomar"),
        "{}",
        s.err
    );
    let s = rodar(&p, &["--config", &cfg, "nfe", "pendentes", "--json"], None);
    let v: Value = serde_json::from_str(&s.out).unwrap();
    assert_eq!(v[0]["situacao"], "pendente");
}

#[test]
fn a_pasta_da_nfse_e_a_da_nfe_convivem_cada_uma_com_os_seus_comandos() {
    let p = Pasta::nova();
    let cfg = configurar(&p, "");
    let nfse = Notas::abrir(p.caminho("notas")).unwrap();
    guardar_nota_do_danfse(&nfse);
    encher(&p);
    // a NFS-e lista so' a dela, e a numeracao da DPS segue a de sempre
    let s = rodar(&p, &["--config", &cfg, "notas", "listar"], None);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert_eq!(s.out.lines().count(), 1);
    assert!(s.out.contains("nfse "), "{}", s.out);
    let s = rodar(&p, &["--config", &cfg, "notas", "numero"], None);
    assert!(s.out.contains("proxima DPS"), "{}", s.out);
    // a NF-e lista so' a dela
    let s = rodar(&p, &["--config", &cfg, "nfe", "notas", "listar"], None);
    assert_eq!(s.out.lines().count(), 3);
    assert!(!s.out.contains("nfse "));
    // o `reindexar` da NFS-e nao toma a pasta da NF-e por uma pasta de notas
    let s = rodar(&p, &["--config", &cfg, "notas", "reindexar"], None);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(s.out.contains("indice refeito"), "{}", s.out);
    let s = rodar(&p, &["--config", &cfg, "nfe", "notas", "listar"], None);
    assert_eq!(s.out.lines().count(), 3);
}

#[test]
fn comando_que_precisa_da_rede_sem_a_uf_ou_o_certificado_para_antes_de_enviar() {
    let p = Pasta::nova();
    // sem UF no arquivo: erro de uso antes de abrir certificado
    let cfg = p.escrever("rustnf.toml", b"[nfe]\ncrt = 1\n");
    let s = rodar(&p, &["--config", &cfg, "nfe", "status"], None);
    assert_eq!(s.codigo, 2, "{}", s.err);
    assert!(s.err.contains("[nfe].uf"), "{}", s.err);
    // com UF e sem certificado
    let cfg = p.escrever("rustnf2.toml", b"[nfe]\nuf = \"SP\"\n");
    let s = rodar(&p, &["--config", &cfg, "nfe", "status"], None);
    assert_eq!(s.codigo, 2, "{}", s.err);
    assert!(s.err.contains("--pfx"), "{}", s.err);
    // a senha nunca vem por argumento
    let s = rodar(
        &p,
        &["--config", &cfg, "nfe", "status", "--senha", "x"],
        None,
    );
    assert_eq!(s.codigo, 2);
}

#[test]
fn o_exemplo_traz_a_secao_da_nfe_e_ela_so_muda_o_que_o_emitente_precisa() {
    let c = rustnf_app::config::Config::ler(&exemplo()).unwrap();
    c.validar().unwrap();
    let padrao = rustnf_app::config::Config::default().nfe;
    assert_eq!(
        (c.nfe.uf.as_str(), c.nfe.crt, c.nfe.serie.as_str()),
        ("SP", 1, "1")
    );
    assert_eq!(c.nfe.endereco.cep, "01001000");
    // o que o exemplo nao personaliza e' o que o programa assume
    assert_eq!(c.nfe.ambiente, padrao.ambiente);
    assert_eq!(c.nfe.tempo_limite_s, padrao.tempo_limite_s);
    assert_eq!(c.nfe.tentativas_retorno, padrao.tentativas_retorno);
    assert_eq!(c.nfe.intervalo_retorno_s, padrao.intervalo_retorno_s);
    assert_eq!(c.nfe.canhoto, padrao.canhoto);
    assert_eq!(c.nfe.contingencia, padrao.contingencia);
    // a secao da NFC-e do exemplo e' o padrao: quem nao a personaliza emite em homologacao, QR Code 3, bobina de 80 mm
    assert_eq!(c.nfce, rustnf_app::config::Config::default().nfce);
}

#[test]
fn config_mostrar_traz_a_secao_nfe_e_a_variavel_do_ambiente_a_muda() {
    let vazia = Pasta::nova();
    let cfg = vazia.escrever(
        "rustnf.toml",
        b"[nfe]
uf = \"PR\"
crt = 3
",
    );
    let s = rodar(&vazia, &["--config", &cfg, "config", "mostrar"], None);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(s.out.contains("[nfe]"), "{}", s.out);
    assert!(s.out.contains("uf = \"PR\""), "{}", s.out);
    assert!(s.out.contains("ambiente = \"homologacao\""), "{}", s.out);
    let s = rodar_com_env(
        &vazia,
        &["--config", &cfg, "config", "mostrar", "--json"],
        None,
        &[
            ("RUSTNF_NFE_AMBIENTE", "producao"),
            ("RUSTNF_NFE_SERIE", "7"),
        ],
    );
    let v: Value = serde_json::from_str(&s.out).unwrap();
    assert_eq!(v["config"]["nfe"]["ambiente"], "producao");
    assert_eq!(v["config"]["nfe"]["serie"], "7");
    assert_eq!(
        v["variaveis"],
        json!(["RUSTNF_NFE_AMBIENTE", "RUSTNF_NFE_SERIE"])
    );
    // o ambiente da NFS-e nao muda
    assert_eq!(v["config"]["sefin"]["ambiente"], "restrita");
    // valor que nao serve aponta o campo
    let s = rodar_com_env(
        &vazia,
        &["config", "mostrar"],
        None,
        &[("RUSTNF_NFE_AMBIENTE", "restrita")],
    );
    assert_eq!(s.codigo, 2);
    assert!(s.err.contains("[nfe] ambiente"), "{}", s.err);
}
