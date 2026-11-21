# Changelog

## Próxima versão

* A API HTTP e o MCP passam a emitir e a cancelar pelo mesmo caminho da linha de comando e da interface. Em
  `POST /emitir` e em `emitir_nfse`, o `[emitente]` da configuração completa o que o pedido não traz e, com
  `[notas].pasta`, o número da DPS pode faltar (vale o próximo da série) e a nota autorizada é guardada na pasta; a
  resposta ganha `guardada`, `numero_dps`, `aviso` e `recuperada_difere`, como no `rustnf emitir`. Em `POST /cancelar`
  e em `cancelar_nfse`, o cancelamento registrado marca a nota na pasta, e a recusa de um cancelamento repetido é
  conferida na Sefin antes de virar erro. Antes, uma nota emitida pelo servidor não entrava na pasta nem na
  numeração. Sem `[emitente]` e sem pasta de notas na configuração, nada muda.
* O servidor emite uma nota por vez: duas emissões simultâneas não pegam mais o mesmo número de DPS.
* `POST /calcular` e `calcular_cbs_ibs` usam as `[[aliquotas]]` da configuração quando o pedido não traz
  `publicadas`, como o `rustnf calcular` já fazia.
* MCP: ferramenta nova `consultar_nfse` (consulta pela chave de acesso), só com `--permitir-emissao` e o Bearer, como
  o `GET /nfse/{chave}` da API HTTP.
* Linha de comando, o que a interface de desktop já fazia e faltava nela:
  * `rustnf certificado mostrar`: titular, documento, emissor, validade, dias restantes e avisos do certificado A1,
    sem usar a rede; código de saída 3 quando ele não assina hoje, para servir a um aviso agendado;
  * `rustnf sefin testar`: uma chamada real à Sefin Nacional (a consulta de uma chave que não existe) que diz se a
    conexão e o certificado foram aceitos;
  * `rustnf danfse --chave <chave>`: o DANFSe de uma nota da pasta de notas, a partir do XML guardado, com a marca de
    cancelada quando a nota está cancelada na pasta;
  * `rustnf notas listar` com `--de`, `--ate`, `--situacao`, `--busca` e saída `--csv` para planilha;
  * `rustnf notas exportar --mes AAAA-MM --para <pasta>`: copia os XML do mês e a lista em CSV, para a contabilidade;
  * `rustnf notas reindexar`: refaz o índice da pasta de notas a partir dos XML guardados, guardando o índice anterior
    ao lado.
* Campo de nome errado no pedido da DPS deixa de ser ignorado em silêncio. `rustnf dps montar`, `dps conferir` e
  `emitir` avisam no stderr o caminho de cada campo que a DPS não conhece, e a resposta em JSON (linha de comando, API
  HTTP e MCP) traz a lista em `campos_desconhecidos`. Com `--estrito` (ou `?estrito=true` em `/dps/conferir`,
  `/dps/montar` e `/emitir`), o pedido é recusado antes de qualquer envio. O padrão continua aceitando o pedido, para
  não quebrar quem já usa.
* `rustnf dps esquema` imprime o JSON Schema do pedido da DPS (`--emissao` para o do `emitir`).
* Biblioteca: `Certificado::resumo` (e `resumo_de_agora`) devolve o que se mostra de um certificado aberto, com o CPF
  mascarado. Camada de aplicação: `notas::Filtro`, `notas::csv`, `Notas::registro`, `Notas::exportar` e
  `Notas::reindexar`.

## 0.2.0 - 2026-11-21

* DANFSe sem o Typst: o PDF passa a ser escrito direto pelo `krilla` (feature `danfse`), a partir da mesma lista de
  operações de desenho que `danfse::dados` já devolvia. O JSON de `dados` não mudou e o leiaute é o mesmo; o texto
  segue selecionável e copiável (Liberation Sans embutida em subconjunto), e a mesma entrada passa a gerar sempre os
  mesmos bytes. A diferença visível é só o contorno das letras: o desenho não aplica mais o kerning do Typst. A
  árvore de dependências da biblioteca com `sefin` e `danfse` caiu de 387 para 207 crates: o DANFSe acrescentava 215
  e passa a acrescentar 35 (o `Cargo.lock` do workspace foi de 1044 para 935). No `deny.toml` saíram cinco exceções
  de segurança que só existiam por causa do Typst (duas do `quick-xml` e três de crates sem mantenedor) e duas
  licenças que nenhuma dependência usa mais. Medido numa compilação de release para Windows: `rustnf.exe` foi de
  48,9 MB para 14,5 MB, `rustnf-gui.exe` de 71,1 MB para 39,7 MB, e gerar um DANFSe pela linha de comando foi de
  cerca de 500 ms para cerca de 180 ms.
* Documentação em português com acentos (README, este arquivo, guia de contribuição, política de segurança, roadmap,
  fontes, NOTICE e modelos de issue). O README ganhou o passo a passo de emissão e passou a dizer a versão mínima do
  Rust (1.92) no lugar de "edição 2021".

## 0.1.0 - 2026-11-21

Primeira versão pública.

* Entrega: o CI passa a compilar e testar a interface de desktop no Windows (job `interface`); novo fluxo `release`
  que, com uma tag de versão, monta `rustnf.exe` e `rustnf-gui.exe` (Windows, com a biblioteca C dentro do executável:
  quem baixa não precisa instalar nada) e `rustnf` (Linux), confere as dependências dos executáveis, calcula os
  SHA-256 e abre a release como rascunho. README com as telas e a instrução de quem baixa pronto.
* Novo crate `rustnf-app` (`crates/rustnf-app`): a camada de aplicação, sem tela, sem servidor e sem `clap`, que a linha
  de comando e qualquer interface de desktop compartilham. Recebeu `nucleo`, `emissao`, `segredo` e o erro (sem o status
  HTTP, que ficou no binário); ganhou:
  * `config`: arquivo de configuração TOML único (`[emitente]`, `[sefin]`, `[notas]`, `[danfse]`, `[servidor]`,
    `[[aliquotas]]`), com padrão em tudo, campo desconhecido recusado, conferência dos valores (erro com seção e
    campo), gravação atômica e a completude do pedido da DPS com o `[emitente]`. Não há campo de senha nem de token;
  * `cofre`: trait `Cofre`, o cofre do sistema (Gerenciador de Credenciais, no Windows) e um cofre em memória para
    teste;
  * `notas`: pasta de notas sem banco (`<AAAA>/<MM>/<chave>.xml`, `indice.json`, `estado.json`) com numeração da DPS
    que só avança em nota autorizada ou recuperada;
  * `pedido`: o fluxo de emitir e cancelar com a configuração e a pasta de notas.
* Binário `rustnf`: opção global `--config`; precedência opção > variável de ambiente `RUSTNF_*` > arquivo > padrão em
  todo parâmetro; `--ambiente`, `--pfx` e os ajustes de `serve` passam a poder vir do arquivo; a senha do certificado,
  na falta de `--senha-env` e `--senha-stdin`, vem do cofre do sistema (continua impossível passá-la por argumento);
  `emitir` completa o pedido com o `[emitente]`, usa o próximo número da pasta de notas e guarda a nota autorizada;
  `cancelar` marca a nota como cancelada; `calcular` usa as `[[aliquotas]]` quando o JSON não traz `publicadas`.
  Comandos novos: `config mostrar`, `config caminho`, `senha guardar`, `senha apagar` e `notas listar`. Sem arquivo
  de configuração e sem as opções novas, o comportamento é o de antes.
* Assinatura da DPS e dos eventos: o padrão passa a ser RSA com SHA-256 (era SHA-1). O SHA-1 continua disponível:
  `--assinatura sha1`, `RUSTNF_ASSINATURA=sha1`, `[sefin] assinatura = "sha1"` ou, na biblioteca, o campo
  `ClienteNfse::algoritmo`. A conferência de assinaturas continua aceitando SHA-1, SHA-256 e SHA-512.
* Novo crate `rustnf-gui` (`crates/rustnf-gui`): a interface de desktop, em GPUI (`gpui-kit` =0.10.0), só para Windows nesta
  versão (fora do Windows o executável avisa e sai com código 2). Lê e grava o mesmo arquivo de configuração da linha de
  comando e guarda a senha do certificado no cofre do sistema. Faz o primeiro uso em três passos, mostra o Início e as
  Configurações (emitente, Sefin Nacional, certificado, senha, notas, DANFSe, alíquotas, tema), testa o certificado e a
  conexão com a Sefin e mostra sempre o ambiente (Produção ou Produção restrita) e o estado do certificado. Ainda não
  emite, consulta nem cancela nota. A camada `modelo` (estado dos formulários, máscaras, conferência campo a campo) não
  depende de janela e é testada em qualquer sistema.
* `rustnf-gui`: nova tela **Nova nota** (Ctrl+N; botão "Emitir uma nota" no Início): formulário do caso comum (tomador,
  serviço com busca na Lista Nacional, valores, ISS, tributos federais e IBS/CBS) com a sugestão de NBS, `cIndOp` e
  `cClassTrib` pela correlação do Anexo VIII, conferência ao vivo (própria e da biblioteca, cada recusa no campo da tela),
  painel fixo com pendências, valor líquido e prévia do IBS/CBS, rascunho automático em `rascunho.json` na pasta de notas,
  confirmação com o ambiente em destaque (em produção, com marcação explícita) e emissão fora da tela com pedido
  congelado: "Tentar de novo" reenvia o mesmo pedido e "Desistir" avisa que a nota pode ter sido autorizada; nota
  recuperada com dados diferentes dos do formulário é avisada e o formulário fica para emitir de novo. A senha, se
  não estiver no cofre, é pedida para aquela emissão e não é guardada. Feature `demonstracao` (desligada por padrão) e
  exemplo `demonstracao`: a mesma janela com um emissor sem rede. Dependências novas do crate: `serde`, `serde_json` e
  `async-trait` (já usadas pelo resto do workspace).
* `rustnf-app`: `pedido::emitir_pedido_completo` e `pedido::Emitida` devolvem o resultado do cliente como dado tipado
  (rejeição com código, descrição e complemento), e `atomico::gravar` passou a ser público. Nota recuperada (já existia
  na Sefin com a mesma série e o mesmo número de DPS) é comparada com o pedido: `Emitida::diferencas` e, na resposta de
  `emitir_pedido` (linha de comando, HTTP e MCP), `recuperada_difere` dizem em que ela difere; vazio ou ausente quando a
  nota é a do pedido.
* `rustnf-gui`: novas telas **Notas** (Ctrl+L), **Consultar** (Ctrl+K) e **Prévia** (Ctrl+P). Notas lista a pasta de notas
  (busca por texto, filtro por situação, lista virtual, atualização sozinha) e mostra o detalhe da nota lido do XML, com
  "Abrir o DANFSe" (gravado ao lado do XML e aberto no programa padrão), "Mostrar na pasta", **cancelar** (motivo e
  justificativa dentro dos limites da biblioteca, confirmação explícita em produção, desfechos registrado, rejeitado e sem
  resposta, com "Tentar de novo" e "Conferir na Sefin") e "Usar como modelo" (a Nova nota recebe os dados da nota).
  Consultar busca a nota pela chave de acesso (conferida antes, mostrada em grupos), confere se a nota devolvida é a
  pedida e se tem cancelamento registrado, e guarda na pasta de notas a nota do emitente. Prévia é a calculadora avulsa do
  IBS e da CBS, sem certificado nem pasta. A senha do certificado, quando não está no cofre, é pedida pelo mesmo diálogo
  para emitir, cancelar e consultar. A demonstração passa a consultar e cancelar (`--cancelamento`).
* `rustnf-app`: `Emissor::cancelada` (a nota tem o evento de cancelamento registrado?), `emissao::consultar_completa`,
  `pedido::cancelar_pedido_completo` e `pedido::Cancelada` (o resultado do cancelamento como dado tipado; `cancelar_pedido`
  continua devolvendo o JSON de sempre), `Notas::caminho_do_xml_guardado` e `Notas::guardar_danfse` (o PDF fica ao lado do
  XML e não entra no índice nem na numeração).
* Cancelamento recusado por repetição: se a nota já tem o cancelamento registrado (envio anterior sem resposta, ou feito em
  outro lugar), `cancelar_pedido` e `cancelar_pedido_completo` devolvem `registrado` com `recuperada`, marcam a pasta e
  levam as mensagens da recusa em `alertas`. Antes, a nota já cancelada aparecia como cancelamento recusado.
* Configuração: nova seção `[interface]` com `tema` (`sistema`, `claro` ou `escuro`).
* `rustnf-app`: novo módulo `certificado` (abrir o A1 e montar o cliente da Sefin, antes dentro da linha de comando) e
  `cofre::cofre_do_ambiente` (a escolha do cofre por `RUSTNF_COFRE`), compartilhados pela linha de comando e pela
  interface. O comportamento da linha de comando não mudou.
* Biblioteca: `rustnf::danfse::municipio::buscar` agora é pública (código IBGE de 7 dígitos -> nome e UF, pela tabela
  embutida).
* Dependências novas: `toml`, e, só no Windows, `keyring-core` e `windows-native-keyring-store` (todas MIT OR
  Apache-2.0).

* Biblioteca `rustnf`:
  * `nfse`: DPS do leiaute 1.01 (produção) e da produção restrita (NT 009), com as regras do Anexo I conferidas antes
    do envio (entre elas a forma do `totTrib` por regime do Simples Nacional: E0710, E0712 e E0713); pedido de evento;
    leitura da NFS-e, do evento e das mensagens de retorno, com dica para E0037 e E0312; Lista Nacional de serviços;
    correlação do Anexo VIII; `agora_com_folga` para o `dhEmi` (E0008).
  * `nfse::sefin` (feature `sefin`): cliente da Sefin Nacional com mTLS e o A1 só em memória, emissão idempotente,
    consultas, cancelamento e eventos.
  * `assinatura`: XMLDSig com C14N própria e certificado A1 (`.pfx` ou PEM).
  * `reforma`: tabela de CST/cClassTrib, cIndOp, atividades do Simples Nacional e alíquotas 2026-2033.
  * `reforma::previa`: base de cálculo (regime regular e Simples Nacional) e IBS/CBS de um serviço, refazendo a conta
    da calculadora da Receita Federal: decimal exato, um arredondamento por número publicado, "meio para o par".
  * `danfse` (feature `danfse`): DANFSe em PDF (NT 008) a partir do XML da NFS-e autorizada, com o Typst embutido, a
    Liberation Sans (SIL OFL 1.1) e o logotipo da NFS-e. Veja o `NOTICE`.
  * `chave_dfe`: chave de acesso de 50 posições da NFS-e e validação de CPF e CNPJ (numérico e alfanumérico).
* Binário `rustnf` (crate `rustnf-cli`): `dps montar`, `dps conferir`, `calcular`, `danfse`, `emitir`, `consultar`,
  `cancelar` e `serve`. A senha do certificado só entra por variável de ambiente ou stdin. Códigos de saída: 0 ok,
  2 uso ou entrada, 3 recusa de regra, 4 rede ou Sefin, 1 o resto.
* API HTTP (`rustnf serve`): `/health`, `/calcular`, `/dps/conferir`, `/dps/montar`, `/danfse`; `/emitir`,
  `/cancelar` e `/nfse/{chave}` só com `--permitir-emissao` e Bearer de no mínimo 32 caracteres. Escuta em
  `127.0.0.1`, sem CORS, com teto de corpo e tempo limite.
* MCP em `POST /mcp` (Streamable HTTP, resposta JSON, sem estado): ferramentas `calcular_cbs_ibs`, `conferir_dps`,
  `montar_dps`, `gerar_danfse`, mais `emitir_nfse` e `cancelar_nfse` com emissão e Bearer.
* Teste de higiene (`tests/higiene.rs`): termos, endereços, CPF/CNPJ, dependências e aviso da licença em cada `.rs`,
  nos arquivos e nas mensagens de commit. `cargo deny` para licenças e avisos de segurança; `gitleaks` com a
  configuração da raiz.
* Licença: Mozilla Public License 2.0 (`LICENSE`).
