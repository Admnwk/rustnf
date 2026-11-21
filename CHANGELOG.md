# Changelog

## Proxima versao

* Novo crate `rustnf-app` (`crates/rustnf-app`): a camada de aplicacao, sem tela, sem servidor e sem `clap`, que a linha
  de comando e qualquer interface de desktop compartilham. Recebeu `nucleo`, `emissao`, `segredo` e o erro (sem o status
  HTTP, que ficou no binario); ganhou:
  * `config`: arquivo de configuracao TOML unico (`[emitente]`, `[sefin]`, `[notas]`, `[danfse]`, `[servidor]`,
    `[[aliquotas]]`), com padrao em tudo, campo desconhecido recusado, conferencia dos valores (erro com secao e
    campo), gravacao atomica e a completude do pedido da DPS com o `[emitente]`. Nao ha campo de senha nem de token;
  * `cofre`: trait `Cofre`, o cofre do sistema (Gerenciador de Credenciais, no Windows) e um cofre em memoria para
    teste;
  * `notas`: pasta de notas sem banco (`<AAAA>/<MM>/<chave>.xml`, `indice.json`, `estado.json`) com numeracao da DPS
    que so' avanca em nota autorizada ou recuperada;
  * `pedido`: o fluxo de emitir e cancelar com a configuracao e a pasta de notas.
* Binario `rustnf`: opcao global `--config`; precedencia opcao > variavel de ambiente `RUSTNF_*` > arquivo > padrao em
  todo parametro; `--ambiente`, `--pfx` e os ajustes de `serve` passam a poder vir do arquivo; a senha do certificado,
  na falta de `--senha-env` e `--senha-stdin`, vem do cofre do sistema (continua impossivel passa-la por argumento);
  `emitir` completa o pedido com o `[emitente]`, usa o proximo numero da pasta de notas e guarda a nota autorizada;
  `cancelar` marca a nota como cancelada; `calcular` usa as `[[aliquotas]]` quando o JSON nao traz `publicadas`.
  Comandos novos: `config mostrar`, `config caminho`, `senha guardar`, `senha apagar` e `notas listar`. Sem arquivo
  de configuracao e sem as opcoes novas, o comportamento e' o de antes.
* Assinatura da DPS e dos eventos: o padrao passa a ser RSA com SHA-256 (era SHA-1). O SHA-1 continua disponivel:
  `--assinatura sha1`, `RUSTNF_ASSINATURA=sha1`, `[sefin] assinatura = "sha1"` ou, na biblioteca, o campo
  `ClienteNfse::algoritmo`. A conferencia de assinaturas continua aceitando SHA-1, SHA-256 e SHA-512.
* Novo crate `rustnf-gui` (`crates/rustnf-gui`): a interface de desktop, em GPUI (`gpui-kit` =0.10.0), so' para Windows nesta
  versao (fora do Windows o executavel avisa e sai com codigo 2). Le e grava o mesmo arquivo de configuracao da linha de
  comando e guarda a senha do certificado no cofre do sistema. Faz o primeiro uso em tres passos, mostra o Inicio e as
  Configuracoes (emitente, Sefin Nacional, certificado, senha, notas, DANFSe, aliquotas, tema), testa o certificado e a
  conexao com a Sefin e mostra sempre o ambiente (Producao ou Producao restrita) e o estado do certificado. Ainda nao
  emite, consulta nem cancela nota. A camada `modelo` (estado dos formularios, mascaras, conferencia campo a campo) nao
  depende de janela e e' testada em qualquer sistema.
* `rustnf-gui`: nova tela **Nova nota** (Ctrl+N; botao "Emitir uma nota" no Inicio): formulario do caso comum (tomador,
  servico com busca na Lista Nacional, valores, ISS, tributos federais e IBS/CBS) com a sugestao de NBS, `cIndOp` e
  `cClassTrib` pela correlacao do Anexo VIII, conferencia ao vivo (propria e da biblioteca, cada recusa no campo da tela),
  painel fixo com pendencias, valor liquido e previa do IBS/CBS, rascunho automatico em `rascunho.json` na pasta de notas,
  confirmacao com o ambiente em destaque (em producao, com marcacao explicita) e emissao fora da tela com pedido
  congelado: "Tentar de novo" reenvia o mesmo pedido e "Desistir" avisa que a nota pode ter sido autorizada; nota
  recuperada com dados diferentes dos do formulario e' avisada e o formulario fica para emitir de novo. A senha, se
  nao estiver no cofre, e' pedida para aquela emissao e nao e' guardada. Feature `demonstracao` (desligada por padrao) e
  exemplo `demonstracao`: a mesma janela com um emissor sem rede. Dependencias novas do crate: `serde`, `serde_json` e
  `async-trait` (ja usadas pelo resto do workspace).
* `rustnf-app`: `pedido::emitir_pedido_completo` e `pedido::Emitida` devolvem o resultado do cliente como dado tipado
  (rejeicao com codigo, descricao e complemento), e `atomico::gravar` passou a ser publico. Nota recuperada (ja existia
  na Sefin com a mesma serie e o mesmo numero de DPS) e' comparada com o pedido: `Emitida::diferencas` e, na resposta de
  `emitir_pedido` (linha de comando, HTTP e MCP), `recuperada_difere` dizem em que ela difere; vazio ou ausente quando a
  nota e' a do pedido.
* `rustnf-gui`: novas telas **Notas** (Ctrl+L), **Consultar** (Ctrl+K) e **Previa** (Ctrl+P). Notas lista a pasta de notas
  (busca por texto, filtro por situacao, lista virtual, atualizacao sozinha) e mostra o detalhe da nota lido do XML, com
  "Abrir o DANFSe" (gravado ao lado do XML e aberto no programa padrao), "Mostrar na pasta", **cancelar** (motivo e
  justificativa dentro dos limites da biblioteca, confirmacao explicita em producao, desfechos registrado, rejeitado e sem
  resposta, com "Tentar de novo" e "Conferir na Sefin") e "Usar como modelo" (a Nova nota recebe os dados da nota).
  Consultar busca a nota pela chave de acesso (conferida antes, mostrada em grupos), confere se a nota devolvida e' a
  pedida e se tem cancelamento registrado, e guarda na pasta de notas a nota do emitente. Previa e' a calculadora avulsa do
  IBS e da CBS, sem certificado nem pasta. A senha do certificado, quando nao esta no cofre, e' pedida pelo mesmo dialogo
  para emitir, cancelar e consultar. A demonstracao passa a consultar e cancelar (`--cancelamento`).
* `rustnf-app`: `Emissor::cancelada` (a nota tem o evento de cancelamento registrado?), `emissao::consultar_completa`,
  `pedido::cancelar_pedido_completo` e `pedido::Cancelada` (o resultado do cancelamento como dado tipado; `cancelar_pedido`
  continua devolvendo o JSON de sempre), `Notas::caminho_do_xml_guardado` e `Notas::guardar_danfse` (o PDF fica ao lado do
  XML e nao entra no indice nem na numeracao).
* Configuracao: nova secao `[interface]` com `tema` (`sistema`, `claro` ou `escuro`).
* `rustnf-app`: novo modulo `certificado` (abrir o A1 e montar o cliente da Sefin, antes dentro da linha de comando) e
  `cofre::cofre_do_ambiente` (a escolha do cofre por `RUSTNF_COFRE`), compartilhados pela linha de comando e pela
  interface. O comportamento da linha de comando nao mudou.
* Biblioteca: `rustnf::danfse::municipio::buscar` agora e' publica (codigo IBGE de 7 digitos -> nome e UF, pela tabela
  embutida).
* Dependencias novas: `toml`, e, so' no Windows, `keyring-core` e `windows-native-keyring-store` (todas MIT OR
  Apache-2.0).

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
