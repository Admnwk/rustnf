# Como contribuir

Obrigado pelo interesse. Este documento diz como rodar os testes, o que uma mudanca de regra precisa trazer e o que o
teste de higiene barra.

## Antes de comecar

1. Faca uma copia do repositorio na sua conta e crie um ramo para a mudanca.
2. Rode os testes (abaixo) antes de mexer, para saber que o ponto de partida esta verde.
3. Mudancas grandes (regra nova, rota nova na API, ferramenta nova no MCP): abra uma issue antes e descreva o que
   pretende.

## Como rodar os testes

Os testes nao dependem de rede, de certificado real nem de porta fixa. Rode so' o que a mudanca toca:

```
cargo test -p rustnf --test nfse_nacional     # DPS: XML gerado e recusas do Anexo I
cargo test -p rustnf --test reforma_previa    # previa do IBS/CBS
cargo test -p rustnf --test nfse_sefin        # cliente da Sefin, com transporte falso
cargo test -p rustnf --test danfse            # DANFSe
cargo test -p rustnf-cli                      # CLI, API HTTP e MCP
cargo test -p rustnf --test higiene           # o que nao pode entrar no repositorio
```

Antes de pedir a integracao, o CI roda tambem:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo deny check licenses advisories
```

A feature `danfse` embute o Typst e demora para compilar na primeira vez. Rode `cargo` com poucos nucleos se a maquina
for compartilhada (`CARGO_BUILD_JOBS=2` e `RUST_TEST_THREADS=2`).

## Testar em producao restrita e relatar

O projeto ainda nao tem nota real emitida (veja "Estado" no `README.md`). O teste que mais ajuda e' o seu:

1. Use a **producao restrita** (`--ambiente restrita`), com o seu certificado A1. Nada ali tem efeito fiscal. Nem todo
   municipio esta ativo nesse ambiente, e muitos nao cadastram la os codigos de servico: as recusas E0037 e E0312
   costumam ser do cadastro do ambiente, nao da sua nota.
2. Confira antes de enviar: `rustnf dps conferir pedido.json`.
3. Emita: `rustnf emitir pedido.json --pfx certificado.pfx --senha-env SENHA_DO_A1 --ambiente restrita`.
4. Abra um relato pelo modelo **"Relato de emissao"**, tenha dado certo ou nao: ambiente, municipio, versao, o
   codigo e a mensagem que a Sefin Nacional devolveu.

No relato, **nunca** anexe certificado, senha, token, nem XML ou PDF com dado de pessoa ou empresa real. Troque
nomes e documentos pelos ficticios de `crates/rustnf/tests/higiene_documentos.txt` antes de colar qualquer trecho.
## Estilo do codigo

* Rust sem `unsafe`; sem `unwrap`, `expect` nem `panic!` fora de teste; erro tipado (enum com `Display` e
  `std::error::Error`, `#[non_exhaustive]` no que e' publico), nunca `Result<_, String>`.
* Todo arquivo `.rs` comeca pelas tres linhas do aviso da MPL 2.0 (copie de outro arquivo).
* Texto, comentarios e mensagens em portugues do Brasil; identificadores sem acento.
* Valores e percentuais em inteiros de ponto fixo; nada de ponto flutuante nas contas de dinheiro.
* O binario (`crates/rustnf-cli`) so' le a entrada, chama a biblioteca e escreve a saida: regra fiscal mora em
  `crates/rustnf`.
* Segredo (senha, token) em `rustnf_cli::segredo::Segredo`, que tem `Debug` redigido e zera a memoria.

## Contrato para mudar uma regra

Regra fiscal nao se muda por opiniao, por comportamento de outro programa nem por "funciona assim na pratica". Toda
mudanca de regra (validacao da DPS, calculo do IBS/CBS, leiaute do XML, texto do DANFSe) precisa trazer, no mesmo
pedido de mudanca:

1. **A fonte oficial**, citada no codigo (comentario) e em `docs/FONTES.md`: o documento, a versao, a aba ou o item, e
   o codigo da regra (por exemplo, "Anexo I, regra E0710"). Fontes aceitas: documentacao tecnica do portal da NFS-e,
   Notas Tecnicas, XSD oficial, Lei Complementar, calculadora oficial da Receita Federal.
2. **Um caso no contrato**, em `crates/rustnf/tests/casos/*.json`, com a entrada, o resultado esperado escrito a
   partir da fonte (nao copiado da saida do proprio codigo) e, para recusa, o campo e o codigo da regra.
3. **O teste que falha sem a mudanca** e passa com ela.
4. **Uma linha no `CHANGELOG.md`**.

Mudanca sem fonte oficial nao e' aceita, ainda que o resultado pareca certo.

## O teste de higiene

`cargo test -p rustnf --test higiene` varre os arquivos do repositorio (rastreados e ainda nao rastreados) e as
mensagens de commit. Ele reprova:

* **dado real**: CPF ou CNPJ com digito verificador valido que nao esteja na lista de documentos ficticios
  (`crates/rustnf/tests/higiene_documentos.txt`). Use so' os documentos dessa lista nos exemplos e nos testes;
* **endereco (URL) fora de fonte oficial**: so' passam os enderecos listados no teste (portais do governo, W3C,
  RFC, a licenca, o indice de pacotes, a especificacao do MCP, este repositorio) e os de teste (`127.0.0.1`,
  `localhost`, `exemplo.com.br`);
* **dependencia fora do crates.io**: `git`, `registry` alternativo ou `path` para fora do repositorio;
* **arquivo `.rs` sem o aviso da licenca** nas tres primeiras linhas;
* **termos proibidos** (`crates/rustnf/tests/higiene_termos.txt`): caminho de maquina (letra de unidade de disco) e
  rodape de ferramenta em mensagem de commit. A lista vale para arquivos e para mensagens de commit; mensagem de
  commit descreve o que o codigo faz, em portugues neutro. O unico rodape aceito e' o de coautoria do projeto,
  exatamente como o teste o define (`rodape_de_autoria`). Quem mantem o projeto roda o mesmo teste com uma lista
  adicional de termos, guardada fora do repositorio.

Nao altere a lista de termos nem a de enderecos para fazer o teste passar: mude o texto que ele acusou. Se um endereco
oficial novo for necessario, explique no pedido de mudanca.

## Licenca da contribuicao

O projeto e' distribuido sob a [Mozilla Public License 2.0](LICENSE). Ao enviar uma contribuicao, voce concorda em
licencia-la sob a MPL 2.0, e declara que tem o direito de faze-lo. Nao envie codigo copiado de projeto com licenca
incompativel, nem dado de pessoa ou empresa real.

## Relato de vulnerabilidade

Nao use issue nem pedido de mudanca: veja o [SECURITY.md](SECURITY.md).
