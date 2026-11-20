# Changelog

## 0.1.0

Primeira versao publica.

* Biblioteca `rustnf`:
  * `nfse`: DPS do leiaute 1.01 (producao) e da producao restrita (NT 009), com as regras do Anexo I conferidas antes
    do envio (entre elas a forma do `totTrib` por regime do Simples Nacional: E0710, E0712 e E0713); pedido de evento;
    leitura da NFS-e, do evento e das mensagens de retorno, com dica para E0037 e E0312; Lista Nacional de servicos;
    correlacao do Anexo VIII; `agora_com_folga` para o `dhEmi` (E0008).
  * `nfse::sefin` (feature `sefin`): cliente da Sefin Nacional com mTLS e o A1 so' em memoria, emissao idempotente,
    consultas, cancelamento e eventos.
  * `assinatura`: XMLDSig com C14N propria e certificado A1 (`.pfx` ou PEM).
  * `reforma`: tabela de CST/cClassTrib, cIndOp, atividades do Simples Nacional e aliquotas 2026-2033.
  * `reforma::previa`: base de calculo (regime regular e Simples Nacional) e IBS/CBS de um servico, refazendo a conta
    da calculadora da Receita Federal: decimal exato, um arredondamento por numero publicado, "meio para o par".
  * `danfse` (feature `danfse`): DANFSe em PDF (NT 008) a partir do XML da NFS-e autorizada, com o Typst embutido, a
    Liberation Sans (SIL OFL 1.1) e o logotipo da NFS-e. Veja o `NOTICE`.
  * `chave_dfe`: chave de acesso de 50 posicoes da NFS-e e validacao de CPF e CNPJ (numerico e alfanumerico).
* Binario `rustnf` (crate `rustnf-cli`): `dps montar`, `dps conferir`, `calcular`, `danfse`, `emitir`, `consultar`,
  `cancelar` e `serve`. A senha do certificado so' entra por variavel de ambiente ou stdin. Codigos de saida: 0 ok,
  2 uso ou entrada, 3 recusa de regra, 4 rede ou Sefin, 1 o resto.
* API HTTP (`rustnf serve`): `/health`, `/calcular`, `/dps/conferir`, `/dps/montar`, `/danfse`; `/emitir`,
  `/cancelar` e `/nfse/{chave}` so' com `--permitir-emissao` e Bearer de no minimo 32 caracteres. Escuta em
  `127.0.0.1`, sem CORS, com teto de corpo e tempo limite.
* MCP em `POST /mcp` (Streamable HTTP, resposta JSON, sem estado): ferramentas `calcular_cbs_ibs`, `conferir_dps`,
  `montar_dps`, `gerar_danfse`, mais `emitir_nfse` e `cancelar_nfse` com emissao e Bearer.
* Teste de higiene (`tests/higiene.rs`): termos, enderecos, CPF/CNPJ, dependencias e aviso da licenca em cada `.rs`,
  nos arquivos e nas mensagens de commit. `cargo deny` para licencas e avisos de seguranca; `gitleaks` com a
  configuracao da raiz.
* Licenca: Mozilla Public License 2.0 (`LICENSE`).
