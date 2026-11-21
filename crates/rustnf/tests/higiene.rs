// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Gate de higiene: o que nao pode entrar no repositorio.
//!
//! * termo proibido (`higiene_termos.txt`, uma expressao regular por linha) em arquivo ou em mensagem de commit;
//!   quem mantem o projeto soma uma lista propria, guardada fora do repositorio, apontando a variavel de
//!   ambiente `RUSTNF_HIGIENE_PRIVADA` para a pasta dela (`termos.txt`, `amostras.txt` e `limpos.txt`); em
//!   `termos.txt`, linha que comeca por `!` e' excecao: um trecho de terceiros que nao e' rastro;
//! * endereco (URL) fora da lista de fontes publicas aceitas;
//! * CPF ou CNPJ com digito verificador valido que nao esteja em `higiene_documentos.txt`;
//! * dependencia que nao venha do crates.io, ou por caminho para fora do repositorio;
//! * arquivo `.rs` sem o aviso da licenca.
//!
//! Varre os arquivos rastreados e os ainda nao rastreados que nao estejam ignorados (o problema aparece antes do
//! commit). Arquivo binario e' varrido pelos trechos de texto que carrega. Nao precisa de rede.

use regex::Regex;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const AVISO_LICENCA: &str =
    "// This Source Code Form is subject to the terms of the Mozilla Public\n\
// License, v. 2.0. If a copy of the MPL was not distributed with this\n\
// file, You can obtain one at https://mozilla.org/MPL/2.0/.\n";

/// Enderecos aceitos: documento oficial, norma tecnica, a licenca, o indice de pacotes e este repositorio.
const URLS_ACEITAS: &[&str] = &[
    r"^https?://([a-z0-9-]+\.)*gov\.br(/|$)",
    r"^https?://www\.w3\.org(/|$)",
    r"^https://www\.rfc-editor\.org/",
    r"^https://mozilla\.org/MPL/",
    r"^https://modelcontextprotocol\.io(/|$)",
    r"^https://github\.com/ReceitaFederal/",
    r"^https://github\.com/Admnwk/rustnf(/|$)",
    r"^https://github\.com/rust-lang/crates\.io-index$",
    r"^https://(docs\.rs|crates\.io)/",
    // licenca SIL OFL e a fundicao, como vem no texto da licenca e no metadado das fontes Liberation
    r"^http://scripts\.sil\.org/OFL$",
    r"^http://www\.ascendercorp\.com/",
    // enderecos de teste: maquina local, dominio de exemplo e nomes sem ponto
    r"^https?://(127\.0\.0\.1|localhost)([:/]|$)",
    r"^https?://([a-z0-9-]+\.)*exemplo\.com\.br([:/]|$)",
    r"^https?://[a-z0-9-]+([:/]|$)",
];

fn pasta_do_crate() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn raiz_do_repo() -> PathBuf {
    pasta_do_crate().join("..").join("..")
}

fn ler_do_crate(rel: &str) -> String {
    let caminho = pasta_do_crate().join("tests").join(rel);
    std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("ler tests/{rel}: {e}"))
}

fn linhas_uteis(texto: &str) -> impl Iterator<Item = &str> {
    texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

fn carregar_termos(texto: &str) -> Vec<(String, Regex)> {
    linhas_uteis(texto)
        .map(|l| {
            let re = Regex::new(&format!("(?i){l}"))
                .unwrap_or_else(|e| panic!("termo invalido `{l}` em higiene_termos.txt: {e}"));
            (l.to_string(), re)
        })
        .collect()
}

fn termos() -> Vec<(String, Regex)> {
    let mut t = termos_publicos();
    t.extend(termos_privados());
    t
}

fn termos_publicos() -> Vec<(String, Regex)> {
    let t = carregar_termos(&ler_do_crate("higiene_termos.txt"));
    assert!(!t.is_empty(), "lista de termos vazia");
    t
}

/// Variavel de ambiente com a pasta da lista de quem mantem o projeto. Ela fica fora do repositorio.
const LISTA_PRIVADA: &str = "RUSTNF_HIGIENE_PRIVADA";

/// As linhas uteis de um arquivo da pasta privada; `None` sem a variavel. Variavel apontando para pasta sem o
/// arquivo e' erro: melhor parar do que varrer com a lista pela metade.
fn da_pasta_privada(arquivo: &str) -> Option<Vec<String>> {
    let pasta = PathBuf::from(std::env::var_os(LISTA_PRIVADA)?);
    let caminho = pasta.join(arquivo);
    let texto = std::fs::read_to_string(&caminho)
        .unwrap_or_else(|e| panic!("{LISTA_PRIVADA}: nao consegui ler `{arquivo}`: {e}"));
    Some(linhas_uteis(&texto).map(String::from).collect())
}

fn termos_privados() -> Vec<(String, Regex)> {
    match da_pasta_privada("termos.txt") {
        Some(linhas) => {
            let so_termos: Vec<&str> = linhas
                .iter()
                .map(String::as_str)
                .filter(|l| !l.starts_with(MARCA_DE_EXCECAO))
                .collect();
            carregar_termos(&so_termos.join("\n"))
        }
        None => {
            eprintln!(
                "lista privada ausente ({LISTA_PRIVADA}): so' os termos publicos foram usados"
            );
            Vec::new()
        }
    }
}

/// Marca, em `termos.txt` da pasta privada, a linha que e' excecao e nao termo.
const MARCA_DE_EXCECAO: char = '!';

/// Trechos de terceiros que um termo da lista privada acusaria sem serem rastro (nome de um crate no `Cargo.lock`,
/// campo de uma API de biblioteca): as linhas de `termos.txt` que comecam por `!`, cada uma com uma expressao
/// regular. So' o trecho casado e' ignorado; o resto da linha continua sendo varrido.
fn excecoes_privadas() -> Vec<Regex> {
    da_pasta_privada("termos.txt")
        .unwrap_or_default()
        .iter()
        .filter_map(|l| l.strip_prefix(MARCA_DE_EXCECAO))
        .enumerate()
        .map(|(i, l)| {
            // nem a expressao vai para a mensagem: ela pode parar num registro publico
            Regex::new(l).unwrap_or_else(|_| panic!("excecao privada numero {} invalida", i + 1))
        })
        .collect()
}

/// Devolve `rotulo:linha: termo -> trecho` para cada ocorrencia.
fn achados(texto: &str, rotulo: &str, termos: &[(String, Regex)]) -> Vec<String> {
    achados_fora_de(texto, rotulo, termos, &[])
}

/// Como [`achados`], apagando antes de cada linha os trechos que casam com uma das `excecoes`.
fn achados_fora_de(
    texto: &str,
    rotulo: &str,
    termos: &[(String, Regex)],
    excecoes: &[Regex],
) -> Vec<String> {
    let mut saida = Vec::new();
    for (i, linha) in texto.lines().enumerate() {
        let mut limpa = std::borrow::Cow::Borrowed(linha);
        for e in excecoes {
            if e.is_match(&limpa) {
                limpa = std::borrow::Cow::Owned(e.replace_all(&limpa, " ").into_owned());
            }
        }
        let linha = limpa.as_ref();
        for (nome, re) in termos {
            if let Some(m) = re.find(linha) {
                saida.push(format!(
                    "{rotulo}:{}: termo `{nome}` casou com `{}`",
                    i + 1,
                    m.as_str()
                ));
            }
        }
    }
    saida
}

fn git(args: &[&str]) -> Option<Vec<u8>> {
    let saida = Command::new("git")
        .args(args)
        .current_dir(raiz_do_repo())
        .output()
        .ok()?;
    saida.status.success().then_some(saida.stdout)
}

/// Trechos de texto (8 ou mais caracteres imprimiveis seguidos) de um arquivo binario, um por linha: e' onde um
/// metadado de imagem, de fonte ou de PDF carregaria um nome ou um caminho.
fn texto_de_binario(bytes: &[u8]) -> String {
    let mut saida = String::new();
    let mut atual = String::new();
    for b in bytes.iter().copied().chain(std::iter::once(0)) {
        if b == b' ' || b.is_ascii_graphic() {
            atual.push(char::from(b));
        } else {
            if atual.len() >= 8 {
                saida.push_str(&atual);
                saida.push('\n');
            }
            atual.clear();
        }
    }
    saida
}

/// (caminho relativo, texto a varrer) de cada arquivo do repositorio; `None` se o git nao estiver disponivel.
fn arquivos() -> Option<Vec<(String, String)>> {
    let lista = git(&[
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
    ])?;
    let mut saida = Vec::new();
    for rel in lista.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let rel = String::from_utf8_lossy(rel).to_string();
        let Ok(bytes) = std::fs::read(raiz_do_repo().join(&rel)) else {
            continue; // apagado da arvore e ainda no indice
        };
        let texto = if bytes.contains(&0) {
            texto_de_binario(&bytes)
        } else {
            String::from_utf8_lossy(&bytes).to_string()
        };
        saida.push((rel, texto));
    }
    assert!(
        !saida.is_empty(),
        "nenhum arquivo varrido: o gate nao esta olhando nada"
    );
    Some(saida)
}

fn arquivos_ou_aviso() -> Vec<(String, String)> {
    arquivos().unwrap_or_else(|| {
        eprintln!("git indisponivel: varredura de arquivos ignorada");
        Vec::new()
    })
}

fn reprovar(titulo: &str, problemas: &[String]) {
    assert!(problemas.is_empty(), "{titulo}:\n{}", problemas.join("\n"));
}

// ---------------------------------------------------------------- termos

#[test]
fn arquivos_do_repo_sem_termo_proibido() {
    let termos = termos();
    let excecoes = excecoes_privadas();
    let mut problemas = Vec::new();
    for (rel, texto) in arquivos_ou_aviso() {
        problemas.extend(achados_fora_de(&texto, &rel, &termos, &excecoes));
    }
    reprovar("termos proibidos", &problemas);
}

#[test]
fn a_excecao_so_libera_o_trecho_dela() {
    let termos = carregar_termos(r"\bazul\b");
    let excecoes = vec![Regex::new("ceu-azul").unwrap()];
    let acha = |texto: &str| achados_fora_de(texto, "x", &termos, &excecoes).len();
    assert_eq!(acha("name = \"ceu-azul\""), 0);
    assert_eq!(acha("o mar azul"), 1, "fora do trecho continua acusando");
    assert_eq!(
        acha("ceu-azul e mar azul"),
        1,
        "o resto da linha e' varrido"
    );
    assert_eq!(achados("name = \"ceu-azul\"", "x", &termos).len(), 1);
}

/// O rodape de coautoria do projeto: a unica linha de mensagem de commit que pode citar esses nomes. E' montado em
/// pedacos para este arquivo nao casar com a propria lista de termos.
fn rodape_de_autoria() -> String {
    [
        "Co-Authored-B",
        "y: Clau",
        "de Opus 5.5, Fable 5.2 <noreply@anthrop",
        "ic.com>",
    ]
    .concat()
}

/// A mensagem com a linha do rodape de autoria em branco (so' a linha exata; qualquer variacao e' varrida).
fn sem_o_rodape_de_autoria(msg: &str) -> String {
    let rodape = rodape_de_autoria();
    msg.lines()
        .map(|l| if l.trim() == rodape { "" } else { l })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn so_o_rodape_exato_de_autoria_passa() {
    let t = termos();
    let rodape = rodape_de_autoria();
    let mensagem = format!("Corrige o calculo da base\n\nDetalhe da mudanca.\n\n{rodape}\n");
    assert!(achados(&sem_o_rodape_de_autoria(&mensagem), "commit", &t).is_empty());
    // qualquer variacao continua reprovada: outro nome, texto antes ou depois, outro endereco
    for variacao in [
        rodape.replace("Opus 5.5, Fable 5.2", "Outro Nome"),
        format!("{rodape} e mais alguem"),
        format!("Revisado por {rodape}"),
        rodape.replace("noreply", "contato"),
    ] {
        let mensagem = format!("Corrige o calculo da base\n\n{variacao}\n");
        assert!(
            !achados(&sem_o_rodape_de_autoria(&mensagem), "commit", &t).is_empty(),
            "variacao do rodape passou: {variacao}"
        );
    }
}

#[test]
fn historico_sem_termo_proibido() {
    let termos = termos();
    let Some(saida) = git(&["log", "--all", "--format=%H%x1f%B%x1e"]) else {
        eprintln!("historico indisponivel: varredura de commits ignorada");
        return;
    };
    let texto = String::from_utf8_lossy(&saida).to_string();
    let mut problemas = Vec::new();
    for registro in texto
        .split('\u{1e}')
        .map(str::trim)
        .filter(|r| !r.is_empty())
    {
        let (hash, msg) = registro.split_once('\u{1f}').unwrap_or((registro, ""));
        problemas.extend(achados(
            &sem_o_rodape_de_autoria(msg),
            &format!("commit {}", &hash[..hash.len().min(10)]),
            &termos,
        ));
    }
    reprovar("termos proibidos em mensagens de commit", &problemas);
}

#[test]
fn o_scanner_acusa_cada_termo_da_lista() {
    // Um exemplo por termo, na ordem do arquivo, para provar que nenhuma linha da lista esta cega; as palavras
    // sao montadas em pedacos para este arquivo nao casar com a propria lista.
    let t = termos_publicos();
    let amostras: Vec<String> = vec![
        format!("{}:{}pasta", "C", char::from(92u8)),
        format!("Co-Authored-B{}: alguem", "y"),
        format!("Generated wit{} uma ferramenta", "h"),
    ];
    assert_eq!(
        amostras.len(),
        t.len(),
        "cada termo da lista precisa de uma amostra aqui, na mesma ordem"
    );
    for ((nome, re), a) in t.iter().zip(&amostras) {
        assert!(re.is_match(a), "o termo `{nome}` nao acusou: {a}");
    }
}

/// A lista privada, quando presente, traz uma amostra por termo (`amostras.txt`, na mesma ordem).
#[test]
fn a_lista_privada_acusa_cada_termo_dela() {
    let Some(amostras) = da_pasta_privada("amostras.txt") else {
        return;
    };
    let t = termos_privados();
    assert!(!t.is_empty(), "lista privada vazia");
    assert_eq!(
        amostras.len(),
        t.len(),
        "cada termo privado precisa de uma amostra, na mesma ordem"
    );
    for (i, ((_, re), a)) in t.iter().zip(&amostras).enumerate() {
        // nem o termo nem a amostra vao para a mensagem: ela pode parar num registro publico
        assert!(
            re.is_match(a),
            "o termo privado numero {} nao acusou a amostra dele",
            i + 1
        );
    }
}

#[test]
fn o_scanner_nao_acusa_texto_limpo() {
    let t = termos();
    let mut limpos = vec![
        "Fonte: https://www.gov.br/nfse (Anexo I). Casas decimais: 2. Chave de acesso de 44 digitos."
            .to_string(),
        "rustnf serve --porta 8080; o agente operador do fundo; a porta do servidor".to_string(),
    ];
    limpos.extend(da_pasta_privada("limpos.txt").unwrap_or_default());
    for (i, l) in limpos.iter().enumerate() {
        assert!(
            achados(l, "limpo", &t).is_empty(),
            "o texto limpo numero {} foi acusado",
            i + 1
        );
    }
}
#[test]
fn binario_e_varrido_pelo_texto_que_carrega() {
    let t = termos();
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0, 1, 2, 3, 0xff];
    bytes.extend_from_slice(
        format!(
            "Software: C:{}Users{}alguem",
            char::from(92u8),
            char::from(92u8)
        )
        .as_bytes(),
    );
    bytes.extend_from_slice(&[0, 0xfe, b'a', b'b', 0]);
    let texto = texto_de_binario(&bytes);
    assert!(
        !achados(&texto, "imagem", &t).is_empty(),
        "caminho de maquina dentro de binario nao foi acusado: {texto:?}"
    );
    assert!(achados(
        &texto_de_binario(&[0x89, 0, 1, b'a', b'b', 0, 0xff]),
        "imagem",
        &t
    )
    .is_empty());
}

// ---------------------------------------------------------------- enderecos

fn urls_fora_da_lista(texto: &str, rotulo: &str, aceitas: &[Regex], achar: &Regex) -> Vec<String> {
    let mut saida = Vec::new();
    for (i, linha) in texto.lines().enumerate() {
        for m in achar.find_iter(linha) {
            let url = m.as_str().trim_end_matches(['.', ',', ';', ':']);
            if !aceitas.iter().any(|re| re.is_match(url)) {
                saida.push(format!("{rotulo}:{}: endereco fora da lista: {url}", i + 1));
            }
        }
    }
    saida
}

fn regex_de_urls() -> (Vec<Regex>, Regex) {
    let aceitas = URLS_ACEITAS
        .iter()
        .map(|p| Regex::new(&format!("(?i){p}")).expect("padrao de URL valido"))
        .collect();
    let achar = Regex::new(r#"https?://[^\s"'<>()\[\]{}\\`|]+"#).expect("padrao valido");
    (aceitas, achar)
}

#[test]
fn enderecos_so_de_fonte_publica_aceita() {
    let (aceitas, achar) = regex_de_urls();
    let mut problemas = Vec::new();
    for (rel, texto) in arquivos_ou_aviso() {
        problemas.extend(urls_fora_da_lista(&texto, &rel, &aceitas, &achar));
    }
    reprovar(
        "enderecos que nao sao fonte oficial (ver URLS_ACEITAS em tests/higiene.rs)",
        &problemas,
    );
}

#[test]
fn o_filtro_de_enderecos_acusa_e_libera() {
    let (aceitas, achar) = regex_de_urls();
    let fora = |t: &str| urls_fora_da_lista(t, "x", &aceitas, &achar);
    // os enderecos recusados sao montados em pedacos para este arquivo passar no proprio filtro
    let s = "https:";
    assert_eq!(
        fora(&format!("ver {s}//github.com/alguem/projeto-qualquer.")).len(),
        1
    );
    assert_eq!(
        fora(&format!(
            "ver http:{}svn.exemplo.org/trunk e {s}//x.y.z/a",
            "//"
        ))
        .len(),
        2
    );
    assert_eq!(fora(&format!("{s}//www.gov.br.exemplo.net/nfse")).len(), 1);
    assert!(
        fora("https://www.gov.br/nfse, https://sefin.nfse.gov.br/SefinNacional/nfse").is_empty()
    );
    assert!(fora(
        "xmlns=\"http://www.sped.fazenda.gov.br/nfse\" e http://www.w3.org/2000/09/xmldsig#"
    )
    .is_empty());
    assert!(fora("https://github.com/ReceitaFederal/calculadora-rtc-regime-regular").is_empty());
    assert!(fora("http://127.0.0.1:8080/x http://a/b https://api.exemplo.com.br").is_empty());
}

// ---------------------------------------------------------------- CPF e CNPJ

/// CPF e CNPJ com digito verificador valido achados no texto: numero isolado de 11 ou 14 digitos, a inscricao
/// dentro do identificador da DPS (`DPS` + 42 digitos), dentro da chave da NFS-e (50 digitos) e dentro da chave
/// de 44 digitos.
fn documentos_validos(texto: &str, res: &[Regex; 4]) -> BTreeSet<String> {
    let mut candidatos: Vec<String> = Vec::new();
    for m in res[0].find_iter(texto) {
        candidatos.push(m.as_str().to_string());
    }
    for re in &res[1..] {
        for c in re.captures_iter(texto) {
            candidatos.push(c[1].to_string());
        }
    }
    let mut saida = BTreeSet::new();
    for c in candidatos {
        if c.len() == 14 && rustnf::chave_dfe::cnpj_valido(&c) {
            saida.insert(c.clone());
        }
        // CPF solto, ou CPF com 3 zeros a esquerda no campo de 14 posicoes
        let cpf = c
            .strip_prefix("000")
            .filter(|_| c.len() == 14)
            .unwrap_or(c.as_str());
        if cpf.len() == 11 && rustnf::chave_dfe::cpf_valido(cpf) {
            saida.insert(cpf.to_string());
        }
    }
    saida
}

fn regex_de_documentos() -> [Regex; 4] {
    [
        Regex::new(r"\b(\d{11}|\d{14})\b").expect("padrao valido"),
        Regex::new(r"DPS\d{7}[12](\d{14})\d{20}").expect("padrao valido"),
        Regex::new(r"\b\d{7}[12][12](\d{14})\d{27}\b").expect("padrao valido"),
        // chave de 44 posicoes de documento fiscal referenciado: UF, AAMM e o CNPJ do emitente
        Regex::new(r"\b\d{6}(\d{14})\d{24}\b").expect("padrao valido"),
    ]
}

#[test]
fn cpf_e_cnpj_so_os_ficticios_da_lista() {
    let aceitos: BTreeSet<String> = linhas_uteis(&ler_do_crate("higiene_documentos.txt"))
        .map(String::from)
        .collect();
    let res = regex_de_documentos();
    let mut problemas = Vec::new();
    for (rel, texto) in arquivos_ou_aviso() {
        if rel == "Cargo.lock" {
            continue; // so' nomes, versoes e somas de verificacao
        }
        for d in documentos_validos(&texto, &res) {
            if !aceitos.contains(&d) {
                problemas.push(format!("{rel}: {d}"));
            }
        }
    }
    reprovar(
        "CPF/CNPJ com digito valido fora de tests/higiene_documentos.txt (use um ficticio da lista)",
        &problemas,
    );
}

#[test]
fn o_filtro_de_documentos_acha_solto_e_embutido() {
    let res = regex_de_documentos();
    let cnpj = "11222333000181";
    let cpf = "52998224725";
    let solto = documentos_validos(&format!("\"cnpj\": \"{cnpj}\", cpf {cpf}."), &res);
    assert!(solto.contains(cnpj) && solto.contains(cpf));
    let na_dps = format!("DPS{}2{cnpj}{}{}", "3550308", "00001", "000000000000001");
    assert!(documentos_validos(&na_dps, &res).contains(cnpj));
    let na_chave = format!(
        "{}22{cnpj}{}{}{}{}",
        "3550308", "0000000000001", "2601", "123456789", "0"
    );
    assert_eq!(na_chave.len(), 50);
    assert!(documentos_validos(&na_chave, &res).contains(cnpj));
    let cpf_na_dps = format!("DPS{}1000{cpf}{}{}", "3550308", "00001", "000000000000001");
    assert!(documentos_validos(&cpf_na_dps, &res).contains(cpf));
    // digito verificador errado e numero de outro tamanho nao sao documento
    assert!(documentos_validos("11222333000180 52998224724 1234567890123", &res).is_empty());
}

// ---------------------------------------------------------------- dependencias

#[test]
fn dependencias_so_do_indice_publico() {
    let raiz = raiz_do_repo();
    let mut problemas = Vec::new();
    let trava = std::fs::read_to_string(raiz.join("Cargo.lock")).expect("ler Cargo.lock");
    for (i, linha) in trava.lines().enumerate() {
        if let Some(origem) = linha.strip_prefix("source = ") {
            if origem != "\"registry+https://github.com/rust-lang/crates.io-index\"" {
                problemas.push(format!("Cargo.lock:{}: {origem}", i + 1));
            }
        }
    }
    let caminho = Regex::new(r#"\bpath\s*=\s*"([^"]*)""#).expect("padrao valido");
    let de_fora = Regex::new(r#"\b(git|registry|registry-index)\s*=\s*""#).expect("padrao valido");
    for (rel, texto) in arquivos_ou_aviso() {
        if !rel.ends_with("Cargo.toml") {
            continue;
        }
        let pasta = raiz.join(&rel);
        let pasta = pasta.parent().unwrap_or(Path::new("."));
        for (i, linha) in texto.lines().enumerate() {
            if de_fora.is_match(linha) {
                problemas.push(format!("{rel}:{}: {}", i + 1, linha.trim()));
            }
            for c in caminho.captures_iter(linha) {
                let alvo = pasta.join(&c[1]);
                let dentro = match (alvo.canonicalize(), raiz.canonicalize()) {
                    (Ok(a), Ok(r)) => a.starts_with(r),
                    _ => false,
                };
                if !dentro {
                    problemas.push(format!(
                        "{rel}:{}: caminho para fora do repositorio: {}",
                        i + 1,
                        &c[1]
                    ));
                }
            }
        }
    }
    reprovar("dependencia que nao vem do crates.io", &problemas);
}

// ---------------------------------------------------------------- licenca

#[test]
fn todo_arquivo_rs_comeca_pelo_aviso_da_licenca() {
    let mut problemas = Vec::new();
    for (rel, texto) in arquivos_ou_aviso() {
        if rel.ends_with(".rs") && !texto.replace("\r\n", "\n").starts_with(AVISO_LICENCA) {
            problemas.push(rel);
        }
    }
    reprovar(
        "arquivo .rs sem o aviso da MPL 2.0 nas tres primeiras linhas (copie de outro arquivo)",
        &problemas,
    );
}
