# Como contribuir

Obrigado pelo interesse. Este documento diz como rodar os testes, o que uma mudança de regra precisa trazer e o que o
teste de higiene barra.

## Antes de começar

1. Faça uma cópia do repositório na sua conta e crie um ramo para a mudança.
2. Rode os testes (abaixo) antes de mexer, para saber que o ponto de partida está verde.
3. Mudanças grandes (regra nova, rota nova na API, ferramenta nova no MCP): abra uma issue antes e descreva o que
   pretende.

## Como rodar os testes

Os testes não dependem de rede, de certificado real nem de porta fixa. Rode só o que a mudança toca:

```
cargo test -p rustnf --test nfse_nacional     # DPS: XML gerado e recusas do Anexo I
cargo test -p rustnf --test reforma_previa    # previa do IBS/CBS
cargo test -p rustnf --test nfse_sefin        # cliente da Sefin, com transporte falso
cargo test -p rustnf --test danfse            # DANFSe
cargo test -p rustnf-cli                      # CLI, API HTTP e MCP
cargo test -p rustnf --test higiene           # o que nao pode entrar no repositorio
```

Antes de pedir a integração, o CI roda também:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo deny check licenses advisories
```

Rode `cargo` com poucos núcleos se a máquina for compartilhada (`CARGO_BUILD_JOBS=2` e `RUST_TEST_THREADS=2`).

## Testar em produção restrita e relatar

O projeto ainda não tem nota real emitida (veja "Estado" no `README.md`). O teste que mais ajuda é o seu:

1. Use a **produção restrita** (`--ambiente restrita`), com o seu certificado A1. Nada ali tem efeito fiscal. Nem todo
   município está ativo nesse ambiente, e muitos não cadastram lá os códigos de serviço: as recusas E0037 e E0312
   costumam ser do cadastro do ambiente, não da sua nota.
2. Confira antes de enviar: `rustnf dps conferir pedido.json`.
3. Emita: `rustnf emitir pedido.json --pfx certificado.pfx --senha-env SENHA_DO_A1 --ambiente restrita`.
4. Abra um relato pelo modelo **"Relato de emissão"**, tenha dado certo ou não: ambiente, município, versão, o
   código e a mensagem que a Sefin Nacional devolveu.

No relato, **nunca** anexe certificado, senha, token, nem XML ou PDF com dado de pessoa ou empresa real. Troque
nomes e documentos pelos fictícios de `crates/rustnf/tests/higiene_documentos.txt` antes de colar qualquer trecho.
## Estilo do código

* Rust sem `unsafe`; sem `unwrap`, `expect` nem `panic!` fora de teste; erro tipado (enum com `Display` e
  `std::error::Error`, `#[non_exhaustive]` no que é público), nunca `Result<_, String>`.
* Todo arquivo `.rs` começa pelas três linhas do aviso da MPL 2.0 (copie de outro arquivo).
* Texto, comentários e mensagens em português do Brasil; identificadores sem acento.
* Valores e percentuais em inteiros de ponto fixo; nada de ponto flutuante nas contas de dinheiro.
* O binário (`crates/rustnf-cli`) só lê a entrada, chama a biblioteca e escreve a saída: regra fiscal mora em
  `crates/rustnf`.
* Segredo (senha, token) em `rustnf_cli::segredo::Segredo`, que tem `Debug` redigido e zera a memória.

## Contrato para mudar uma regra

Regra fiscal não se muda por opinião, por comportamento de outro programa nem por "funciona assim na prática". Toda
mudança de regra (validação da DPS, cálculo do IBS/CBS, leiaute do XML, texto do DANFSe) precisa trazer, no mesmo
pedido de mudança:

1. **A fonte oficial**, citada no código (comentário) e em `docs/FONTES.md`: o documento, a versão, a aba ou o item, e
   o código da regra (por exemplo, "Anexo I, regra E0710"). Fontes aceitas: documentação técnica do portal da NFS-e,
   Notas Técnicas, XSD oficial, Lei Complementar, calculadora oficial da Receita Federal.
2. **Um caso no contrato**, em `crates/rustnf/tests/casos/*.json`, com a entrada, o resultado esperado escrito a
   partir da fonte (não copiado da saída do próprio código) e, para recusa, o campo e o código da regra.
3. **O teste que falha sem a mudança** e passa com ela.
4. **Uma linha no `CHANGELOG.md`**.

Mudança sem fonte oficial não é aceita, ainda que o resultado pareça certo.

## O teste de higiene

`cargo test -p rustnf --test higiene` varre os arquivos do repositório (rastreados e ainda não rastreados) e as
mensagens de commit. Ele reprova:

* **dado real**: CPF ou CNPJ com dígito verificador válido que não esteja na lista de documentos fictícios
  (`crates/rustnf/tests/higiene_documentos.txt`). Use só os documentos dessa lista nos exemplos e nos testes;
* **endereço (URL) fora de fonte oficial**: só passam os endereços listados no teste (portais do governo, W3C,
  RFC, a licença, o índice de pacotes, a especificação do MCP, este repositório) e os de teste (`127.0.0.1`,
  `localhost`, `exemplo.com.br`);
* **dependência fora do crates.io**: `git`, `registry` alternativo ou `path` para fora do repositório;
* **arquivo `.rs` sem o aviso da licença** nas três primeiras linhas;
* **termos proibidos** (`crates/rustnf/tests/higiene_termos.txt`): caminho de máquina (letra de unidade de disco) e
  rodapé de ferramenta em mensagem de commit. A lista vale para arquivos e para mensagens de commit; mensagem de
  commit descreve o que o código faz, em português neutro. O único rodapé aceito é o de coautoria do projeto,
  exatamente como o teste o define (`rodape_de_autoria`). Quem mantém o projeto roda o mesmo teste com uma lista
  adicional de termos, guardada fora do repositório.

Não altere a lista de termos nem a de endereços para fazer o teste passar: mude o texto que ele acusou. Se um endereço
oficial novo for necessário, explique no pedido de mudança.

## Licença da contribuição

O projeto é distribuído sob a [Mozilla Public License 2.0](LICENSE). Ao enviar uma contribuição, você concorda em
licenciá-la sob a MPL 2.0, e declara que tem o direito de fazê-lo. Não envie código copiado de projeto com licença
incompatível, nem dado de pessoa ou empresa real.

## Relato de vulnerabilidade

Não use issue nem pedido de mudança: veja o [SECURITY.md](SECURITY.md).
