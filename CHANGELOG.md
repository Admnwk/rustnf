# Changelog

## Próxima versão

* **O EPEC no lugar de uma NF-e que ficou sem resposta**, na aplicação, na linha de comando (`rustnf nfe epec --substitui
  <chave>`), na API (`substitui` no corpo de `POST /nfe/epec`) e no MCP (`registrar_epec`). Antes de escrever, li o item 2.3.3 do
  Manual de Contingência da NF-e (MOC 7.00, Anexo III) e o 3.3.1 da Visão Geral: a nota pendente de retorno recebe **nova
  numeração** para sair em contingência, então o EPEC continua com o próximo número livre (o código de antes já estava certo) e o
  que faltava era ligar as duas notas. A nota sem resposta passa a ter `substituida_por` no índice (e o EPEC, `substitui`;
  índice antigo, sem os campos, lê igual) e **a retomada só a consulta, nunca a reenvia**; confirmei no `limbo.rs` da biblioteca
  que só reenvia quando recebe a NF-e assinada. Quando a SEFAZ responde, a retomada diz o que a norma manda: se **autorizou**, a
  venda já está no EPEC e a nota se **cancela** (o `cancelar` de sempre); se **não tem** a nota (217), o número se **inutiliza**,
  e só esse caso, com o 217 anotado no índice, entra numa faixa de `inutilizar` (qualquer outra pendente continua recusada). A
  nota inutilizada sai da pasta com a faixa e a NF-e assinada dela fica guardada em `nfe/inutilizacao/<ano>/`. A chave do
  `--substitui` é conferida antes de qualquer rede (da pasta, pendente, do mesmo emitente e ambiente, ainda não substituída) e o
  EPEC recusado não deixa vínculo. `pendentes` e a retomada trazem `substituida_por` e `a_fazer` (`consultar`, `cancelar` ou
  `inutilizar`). A janela continua sem oferecer o EPEC com nota sem resposta: fica para a próxima entrega.
* **Correção: a consulta que a SEFAZ recusa não tira mais a nota da pasta.** A consulta pela chave só lê a situação da nota
  (MOC 7.00, Visão Geral, 5.4), então um `cStat` de recusa nela (o consumo indevido 656, o ambiente divergente 252, a chave
  inválida 236...) é recusa do pedido, não da nota. Antes ele virava `Desfecho::Rejeitada`: a varredura do limbo dava a nota
  como final e a retomada, a consulta da pasta e a nota substituída pelo EPEC a tiravam da pasta junto com a NF-e assinada,
  liberando o número de uma nota que podia estar autorizada. Agora a consulta devolve o desfecho novo
  `Desfecho::ConsultaRecusada` (com `cStat` e motivo; `deve_consultar()` verdadeiro), `limbo::decidir` mantém o estado, a
  varredura avisa sem reenviar e a pasta fica como estava (a nota substituída mantém o vínculo, e o 217 anotado antes deixa de
  liberar a inutilização). No JSON da CLI, da API e do MCP o `status` é `consulta_recusada` (antes `rejeitada`), com saída 4,
  como a nota sem resposta. A janela diz "A SEFAZ não aceitou a consulta" e que a nota continua como estava.
* Montagem da NF-e e da NFC-e: emitente de **GO ou RJ** com CST 20, 30, 40, 41, 50, 70 ou 90 sem o valor e o motivo do
  ICMS desonerado é recusado antes de assinar, com a rejeição 934 (regra N12-90 da NT 2019.001 v1.70, que as duas UF
  aplicam pelo quadro 3.6.1). Ficam fora a devolução, a nota de ajuste e o CST 90 sem redução de base. As demais UF não
  aplicam a regra e nada muda para elas.
* `nfe::cstat`: os códigos 507, 664, 665 e 666, criados pela NT 2019.001 v1.70 (seção 5) e ausentes do MOC, entram na
  tabela com a mensagem oficial. Antes, a resposta da SEFAZ com um deles era classificada como código desconhecido.
* Interface de desktop, polimento das NF-e recebidas: a nota autorizada sem resposta mostra **"Falta responder"** (antes
  "Sem resposta sua"), e a cancelada pelo emitente ou de uso negado não mostra selo de resposta, porque não há o que responder;
  o texto de **"Nada novo"** deixa de repetir o título e diz quando a próxima busca pode ser feita; ao marcar **"Comprei, mas a venda não
  aconteceu"** o campo da justificativa recebe o foco e a janela rola até ele; o botão de produção (e o de cancelar em
  produção) passa de 4,5:1 de contraste nos temas claro e escuro; e o Tab no Início leva a vista até o botão focado.

## 0.6.0 - 2026-11-27

* Interface de desktop: as **NF-e recebidas (as compras) na janela**: buscar as notas que fornecedores emitiram contra a empresa,
  ver a lista e o detalhe, baixar a nota completa, abrir o DANFE e **responder à nota** (a manifestação do destinatário).
  **Nenhuma chamada real ao Ambiente Nacional e nenhum certificado real foram usados pela interface**: tudo foi testado com um
  Ambiente Nacional de mentira (transporte roteirizado, sem rede) e o cofre em memória.
  * Notas ganha a opção **Recebidas (compras)** ao lado de NFS-e, NF-e e NFC-e (lembrada como as outras; a Nova nota continua
    oferecendo só o que se emite). A lista mostra data, fornecedor (nome e CNPJ), valor, a situação da NOTA (autorizada, cancelada
    pelo emitente, uso negado) e a resposta da EMPRESA ("sem resposta sua", ciência, confirmada, desconhecida, não realizada), com
    busca e filtro; um aviso diz quantas notas esperam resposta.
  * **Buscar notas novas** pede a distribuição ao Ambiente Nacional e diz o resultado em palavras ("3 notas novas", "nada novo", "o
    Ambiente Nacional pediu para esperar até HH:MM"). A espera e o bloqueio por consultar demais são da biblioteca: a tela mostra
    a última busca e o horário, o botão fica desligado até lá, e clicar não chama nada.
  * Detalhe: com a nota completa, o **mesmo detalhe e DANFE da NF-e emitida** (as seções foram generalizadas, não copiadas); com só
    o resumo, o que ele traz e "Baixar a nota completa", que explica que isso registra a **ciência da operação** (evento fiscal) e
    pede a confirmação. O desconhecimento não libera a nota completa, e a nota cancelada não oferece resposta.
  * **Responder à nota**: um diálogo com as quatro respostas em palavras de quem compra ("Recebi a mercadoria, está certo", "Vi a
    nota, ainda vou conferir", "Não comprei isto", "Comprei, mas a venda não aconteceu", esta com justificativa de 15 a 255). O que
    o Ambiente Nacional recusaria (ciência ou desconhecimento de nota cancelada, ciência depois da resposta final, a mesma resposta
    duas vezes) aparece ANTES de enviar, pela mesma função da biblioteca. Ambiente em destaque, marcação "Entendo que..." em
    produção (sem ela, nenhuma chamada), sem clique duplo e desfecho com o que fazer. **O prazo da resposta não é calculado pelo
    programa** (a biblioteca não o confere: quem decide é o Ambiente Nacional, que recusa com 596): a tela só repete a recusa.
  * Na aplicação: `rustnf_app::nfe::fila` (última busca e espera), `danfe_da_recebida` e, na pasta dos recebidos, onde guardar o
    DANFE. A SEFAZ de mentira da demonstração ganhou a distribuição e a resposta do destinatário (`gzip` e `base64` só na
    demonstração e nos testes, as mesmas versões que a biblioteca já usa).
  * Demonstração: `--nfe-recebidas normal|espera|vazio` e a pasta de ensaio com três notas recebidas (só o resumo, completa com
    ciência e cancelada pelo emitente), mais três para a janela buscar.

* Interface de desktop: a **NFC-e (modelo 65) na janela, como um caixa**. **Nenhuma NFC-e real, nenhuma SEFAZ real, nenhum
  certificado real e nenhum CSC real foram usados pela interface**: tudo foi testado com uma SEFAZ de mentira (transporte
  roteirizado, sem rede) e o cofre em memória.
  * Tela **Caixa** (barra lateral e seletor "NFC-e" da Nova nota): busca do produto pelo código ou nome no cadastro da NF-e,
    quantidade, cupom com o total grande, desconto, CPF opcional, pagamento (dinheiro com troco, Pix, cartão, mais de uma forma),
    produto novo com o NCM obrigatório, rascunho que sobrevive e **Enter, Enter, Ctrl+Enter** como caminho inteiro. Depois de
    autorizada, "Abrir o cupom" (PDF) e "Salvar para a impressora (ESC/POS)": o programa só gera o arquivo.
  * Desfechos: rejeitada (o número volta livre), sem resposta (só o Retomar; nunca uma segunda NFC-e para o mesmo cupom) e a
    SEFAZ fora antes de enviar, que emite em **contingência off-line** sozinha e diz o prazo para transmitir. **Fechar o caixa**
    lista o que falta transmitir e "Transmitir agora" reenvia a mesma nota, com a mesma chave.
  * Produção: a primeira venda da sessão pede "Entendo que a nota tem efeito fiscal" e a marcação vale por sessão do caixa, não
    por cupom (o caixa ficaria lento; o selo do ambiente fica à vista). Sem a marcação, nenhuma chamada.
  * Configurações, **NFC-e**: ambiente, série, `idCSC`, versão do QR Code, cupom e motivo da off-line; o **CSC** vai ao cofre e a
    tela só diz guardado ou não (com o cofre desligado ela explica e o caixa não emite com o QR Code versão 2).
  * Notas ganha o seletor **NFS-e | NF-e | NFC-e**; a lista da NFC-e reusa a da NF-e, com o detalhe, o cupom, "Conferir na
    SEFAZ", cancelar (com o aviso do prazo nacional ANTES de enviar, sem bloquear) e Retomar. Não há carta de correção nem
    inutilização da NFC-e (a aplicação não as tem). O diálogo e a lista da NF-e foram generalizados para o modelo 65, não copiados.
  * Demonstração: `--nfce-emissao autorizada|rejeitada|sem-resposta|fora` e a pasta de ensaio com NFC-e autorizada, cancelada,
    off-line dentro do prazo e sem resposta.
  * **Fica para depois:** a via do estabelecimento da off-line e a impressão direta em impressora.

* Interface de desktop: a **emissão da NF-e (modelo 55) pela janela**, num assistente de cinco passos para quem vende (para
  quem, o que vendeu, como recebeu, como entrega, conferir e enviar). **Nenhuma NF-e real, nenhuma SEFAZ real, nenhum EPEC real e
  nenhum certificado real foram usados pela interface**: tudo foi testado com uma SEFAZ de mentira (transporte roteirizado, sem
  rede). A Nova nota passa a perguntar NFS-e ou NF-e (a escolha é a da tela Notas, lembrada de uma vez para outra).
  * O termo fiscal sai de uma tabela pequena (`modelo/nfe_fiscal.rs`) com a regra e a fonte de cada escolha (destino, origem,
    ICMS por regime, PIS e COFINS, frete, presença, pagamento) e um teste que confere cada CFOP na tabela da biblioteca. Onde a
    pergunta simples não decide, a tela mostra as opções; o que a pessoa digita em "Detalhes fiscais" nunca é sobrescrito.
  * Cadastro de produtos (`produtos.json`, tolerante e atômico), tela **Produtos**, "Usar como modelo" no detalhe da NF-e, "cliente
    já usado" e rascunho próprio (`rascunho_nfe.json`).
  * Conferência com os erros e avisos da montagem da aplicação em palavras e com o passo onde se corrige; em produção, sem a
    marcação "Entendo que a nota tem efeito fiscal", nenhuma chamada. Sem resposta: só o Retomar (mesma chave, nunca outra
    nota). Contingência EPEC com o motivo de 15 a 255 letras, **não oferecida enquanto há nota sem resposta na pasta**.
  * Venda para outro estado a consumidor final sem inscrição estadual: o CFOP é 6.107 (produção própria) ou 6.108 (mercadoria de
    terceiros), e no regime normal a nota leva o DIFAL (`icms_uf_dest`), que a biblioteca exige (rejeição 694). A tela pede só a
    alíquota interna do ICMS do estado do cliente e o fundo de combate à pobreza (não há tabela de alíquotas dos estados no
    programa), calcula o resto pela biblioteca e mostra "ICMS para o estado do cliente (DIFAL)" na conferência. Cliente isento
    de inscrição e ICMS já cobrado antes a quem não é contribuinte: a tela mostra os CFOP possíveis e a pessoa escolhe.
  * O cliente sem inscrição estadual de outro estado que leva a mercadoria do estabelecimento (no balcão ou com veículo dele)
    faz venda interna: CFOP 5.xxx, sem DIFAL. Só é venda para outro estado quando quem vende entrega lá. É o entendimento das
    Secretarias da Fazenda de SP (Resposta à Consulta 26844/2022) e do RS; a regra é da legislação de cada estado, e a tela
    diz o porquê no resumo.
  * Demonstração: `--nfe-emissao autorizada|rejeitada|sem-resposta|fora` e uma pasta de ensaio com três produtos.
  * Os testes da NFS-e e da lista da NF-e não mudaram de expectativa.

* Interface de desktop: a **NF-e (modelo 55)** na janela, sem a emissão (a emissão está no item acima). A tela Notas
  ganha a escolha "NFS-e | NF-e (venda de mercadoria)", lembrada de uma vez para outra na pasta de notas
  (`preferencias.json`, ignorado se ilegível). **Nenhuma NF-e real, nenhuma SEFAZ real e nenhum certificado real foram usados
  pela interface**: tudo foi testado com uma SEFAZ de mentira (transporte roteirizado, sem rede).
  * Configurações, seção "NF-e": ambiente próprio (homologação ou produção, separado do da NFS-e), UF, IE, regime em
    palavras, série, fantasia, endereço, contingência SVC (motivo de 15 a 255 caracteres), canhoto do DANFE e tempos. Grava
    pelo mesmo Salvar e no mesmo arquivo da NFS-e; `ca_pem` e `[nfe.urls]` seguem como estão no arquivo. O certificado é o
    da NFS-e. "Testar conexão com a SEFAZ" só pergunta o status do serviço.
  * Lista e detalhe da NF-e (busca, filtro, selos), com a nota sem resposta em destaque e o botão Retomar; abrir o DANFE
    (com a marca de cancelada), mostrar na pasta, copiar a chave e "Conferir na SEFAZ".
  * Cancelar, carta de correção (limites, quantas já foram feitas, aviso de que a nova substitui as anteriores, o que a
    carta não corrige e o texto oficial) e inutilizar números, no desenho do cancelamento da NFS-e: ambiente em destaque,
    justificativa com contador, marcação explícita em produção (sem ela, nenhuma chamada), desfecho com o que fazer.
    O cancelamento recusado com 501 diz em palavras que o prazo é o da legislação de cada estado; a interface não conta prazo.
  * O selo do ambiente da barra de título mostra o ambiente do documento que está na tela.
  * Demonstração: `--nfe-evento registrado|rejeitado|sem-resposta` e uma pasta de ensaio com NF-e autorizada, cancelada e
    sem resposta. A feature `demonstracao` passa a ligar o certificado de teste da biblioteca.
  * Os testes da NFS-e não mudaram de expectativa. Os textos da barra de título mudaram de "emissor de NFS-e nacional" para
    "emissor de notas fiscais".

* Interface de desktop: `gpui-kit` 0.10.0 -> 0.11.0 (GPUI 0.3.12 -> 0.3.14). As telas saem iguais e a janela continua
  centrada na tela principal. Uma mudança no teclado: logo ao abrir a janela, antes de qualquer clique, o Tab já leva o
  foco ao próximo controle (e o PageUp/PageDown passa a rolar a tela a partir dele); antes o Tab só andava depois que
  algum campo ou botão tivesse recebido o foco.
* Dependência `base64` 0.22 -> 0.23 (biblioteca, aplicação e linha de comando); o codificador usado continua o mesmo.

## 0.5.0 - 2026-11-26

Ao atualizar: o índice da pasta de notas ganha duas situações (`substituida` e `analise_fiscal`). **Um índice que tenha
alguma delas não é lido pela versão 0.4.0** (ela o trata como ilegível); o índice antigo, só com `autorizada` e
`cancelada`, continua sendo lido por esta versão.

* NFC-e (modelo 65) na aplicação e na linha de comando (`rustnf nfce`; `crates/rustnf-app` e `crates/rustnf-cli`; a feature
  `danfce` da biblioteca passa a ser usada por elas). **Nenhuma NFC-e real, nenhuma SEFAZ real, nenhum certificado A1 real,
  nenhum CSC real e nenhuma impressão em impressora real**; sem dependência nova. A API HTTP e o MCP da NFC-e vêm no item
  seguinte; a interface de desktop da NFC-e fica para depois.
  * Seção `[nfce]` do `rustnf.toml` (`ambiente`, `serie`, `qr_versao`, `id_csc`, `largura_danfce`, `colunas_escpos`,
    `motivo_offline`), que compartilha o emitente, o endereço, a IE, o CRT, o certificado, o cofre, as URLs de simulador e a
    pasta de notas da NF-e. A seção igual ao padrão não é gravada, então um arquivo que não a usa sai idêntico.
  * O CSC (valor) vai só para o cofre, sob a chave `csc:<documento>:<ambiente>:<idCSC>` (por `rustnf nfce csc guardar`,
    `testar` e `apagar`; lido de uma linha do stdin ou de `--csc-env`, nunca de argumento). Com `qr_versao = 2` e sem CSC no
    cofre, a emissão recusa antes de numerar.
  * A NFC-e tem pasta própria, `<pasta>/nfce/`, com índice, estado e numeração só do modelo 65 (a série 1 do 65 e a série 1
    do 55 são sequências distintas). **A pasta `nfe/` e os comandos e saídas da NF-e 55 não mudam um byte**: o índice só
    ganha os campos novos (`offline`, `prazo`, `recusa`) quando a nota os tem, e o `rustnf nfe` agora recusa o pedido do
    modelo 65 dizendo que ele se emite por `rustnf nfce` (era a recusa "só o modelo 55").
  * `rustnf nfce emitir` (envio sempre síncrono, `--offline`, `--offline-se-necessario`, `--danfce`), `consultar`,
    `cancelar` (com o aviso do prazo nacional de 24 horas, MOC 7.00, Visão Geral, regra 2P12-14, que nunca bloqueia o
    envio), `status`, `pendentes` (`--retomar`, `--fechar-caixa` que sai com 4 se sobrar nota pendente) e `danfce` (`--pdf` ou
    `--escpos`, `--largura`, `--colunas`, `--resumido`, `--via-estabelecimento`; gera arquivo, nunca fala com impressora).
  * A NFC-e emitida off-line que a SEFAZ rejeita na transmissão não sai da pasta nem libera o número: fica pendente, com a
    recusa anotada e mostrada por `pendentes`. A retransmissão corrigida com a mesma chave ainda é manual.
  * **Não feito:** cancelamento por substituição (110112) e inutilização da NFC-e (a biblioteca não os tem); prazo de
    cancelamento de cada UF (sem fonte oficial no projeto).
* NFC-e na API HTTP e no MCP do `rustnf serve`, no MESMO servidor e com as mesmas regras de segurança da NF-e. **Nenhuma NFC-e
  real, nenhuma SEFAZ real, nenhum CSC real e nenhum cliente MCP de terceiros**; sem dependência nova. As rotas de NFS-e e de
  NF-e, as ferramentas delas e o `tools/list` delas não mudam (o `tools/list` só ganha as ferramentas da NFC-e).
  * Rotas: `POST /nfce/danfce` (sempre no ar: PDF de bobina ou, com `?formato=escpos`, bytes ESC/POS; `largura`, `colunas`,
    `resumido`, `via_estabelecimento`, `cancelada`), e, só com `--permitir-emissao` e o Bearer (sem a opção nem existem, 404):
    `POST /nfce/emitir` (com `offline` e `offline_se_necessario` no corpo), `GET /nfce/{chave}`, `POST /nfce/cancelar` (o aviso do
    prazo de 24 horas vai em `aviso_prazo` e nunca bloqueia), `GET /nfce/status`, `GET /nfce/pendentes` (com `pode_fechar_caixa`,
    no lugar do código de saída 4 do `--fechar-caixa`) e `POST /nfce/pendentes/retomar`. O mesmo envelope de erro e os mesmos
    códigos da NF-e: a NFC-e não autorizada volta com o JSON inteiro e o envelope `erro` (422 ou 502), para a chave da pendente
    não se perder. `GET /health` ganha `nfce` e `nfce_csc_guardado` (só booleanos); os campos antigos não mudam.
  * Ferramentas do MCP: `gerar_danfce` (leitura) e, só com `--permitir-emissao` e o Bearer, `emitir_nfce`, `consultar_nfce`,
    `cancelar_nfce`, `status_sefaz_nfce` (separada do `status_sefaz`: a NFC-e não tem SVC), `pendentes_nfce` e
    `retomar_pendentes_nfce`. O `tools/list` sem o Bearer passa de sete a oito ferramentas e, com ele, de 26 a 33.
  * **O CSC só se guarda pela linha de comando.** Nenhuma rota nem ferramenta o grava, lê ou apaga; o servidor o lê do cofre uma
    vez, ao subir. Pedido com campo de CSC é recusado antes de qualquer envio, mesmo com `?estrito=false`, e o valor não aparece em
    resposta nem na pasta de notas (testes de HTTP e de MCP).
  * `rustnf serve` sobe com a `[nfce]` do mesmo arquivo da linha de comando. Sem `[nfce]` utilizável (falta o `[nfe].uf`, ou o
    QR Code versão 2 sem CSC guardado), as rotas de efeito fiscal da NFC-e respondem 400 dizendo o que fazer; o DANFC-e, a NF-e e
    a NFS-e seguem no ar. A NFC-e tem fila própria de emissão: duas NFC-e na mesma série não gastam o mesmo número, e uma NFC-e e
    uma NF-e ao mesmo tempo numeram cada uma a sua.
  * **Não feito:** interface de desktop da NFC-e; `/nfce/conferir` e `/nfce/montar` (o pedido se confere e se monta dentro de
    `/nfce/emitir`, antes de qualquer envio); DANFC-e a partir da chave da pasta pela API (o corpo é o XML).
* Tabela de CFOP oficial embutida (módulo público `cfop`: `info`, `e_devolucao`, `vigente`, `origens`, `devolucoes_de`,
  `tabela`, `versao`; dado em `dados/cfop.json`, 619 CFOP com os indicadores da planilha publicada em 04/09/2026, IT
  2023.002 v2.00). `devolucoes_de("5405")` dá `["1411"]` e `origens("1202")` dá `["5102", "5104", "5106"]`, só pela
  descrição oficial do CFOP de devolução. A partida do módulo e das regras veio da `gaialib`, conferida contra a
  planilha e as Notas Técnicas. **Muda o que a NF-e 55 aceita:** o `montar` passa a recusar, e o `conferir` a listar, as
  rejeições 327 (devolução ou nota de crédito 03 e 06 só com CFOP de devolução; 1.949 e 2.949 só no `finNFe` 4; 5.949
  e 6.949 só com o NCM 27112100), 1179 (MEI) e 328 (CFOP de devolução fora da devolução). A retenção do ICMS do
  transporte (`transp.ret_transp.cfop`) passa a recusar com a 722 o CFOP que não é de transporte. CFOP fora da
  tabela ou da vigência e CFOP de combustível sem o grupo `comb` saem como aviso em `Montada.avisos` (770 e 660,
  que o MOC deixa à critério da UF). Três casos de contrato usavam CFOP que a regra nova recusa e foram corrigidos
  (a venda `vc30_1130_venda` tinha o 2.201, de devolução, e as notas de crédito 03 e 06 `credito_03_ref_alheia`,
  `credito_03_ref_nfce` e `credito_06_falta_ref_item` tinham o 5.949; viraram 6.102 e 2.202).
* Correção no DANFSe: a NFS-e sem incidência do ISSQN (`tribISSQN` 4) passa a trazer no bloco "Tributação Municipal
  (ISSQN)" só a frase "OPERAÇÃO NÃO SUJEITA AO ISSQN", como manda a nota 4 do item 2.4.5 da Nota Técnica 008 v1.02 (antes
  o bloco saía inteiro). Imunidade e exportação continuam com o bloco inteiro.
* DANFC-e (feature nova `danfce` = `nfe` + `danfse`, sem dependência nova): `danfce::dados` (conteúdo por divisão I a IX do
  Manual de Padrões do DANFE NFC-e e QR Code 6.0, em JSON estável), `danfce::pdf` (bobina de 80 mm ou 58 mm, altura pelo
  conteúdo, QR Code de nível M com zona quieta de 10%, escritor e fonte do DANFSe) e `danfce::escpos` (bytes para
  impressora térmica: 48, 42 ou 32 colunas, PC850, PC860 ou Windows-1252, QR Code pelo comando nativo, corte opcional; sem abrir
  impressora). Só sai cupom de nota autorizada, cancelada ou emitida em contingência off-line ainda pendente (com
  "EMITIDA EM CONTINGÊNCIA / Pendente de autorização" abaixo do cabeçalho e da identificação, "Via consumidor" e a
  segunda via "Via do Estabelecimento"); nota sem protocolo fora da contingência, denegada, rejeitada e NF-e 55 são
  recusadas com o motivo. O documento do consumidor sai como está no XML (o Manual não permite máscara); nome e endereço são
  opcionais (`consumidor_com_nome_e_endereco`) e saem sempre na entrega a domicílio. **Nenhuma impressão em impressora
  real, nenhum DANFC-e conferido por fiscal ou SEFAZ e nenhum QR Code lido por aplicativo de consulta de SEFAZ.**
  * `nfe::ler` passa a ler o modelo 65 (`nfeProc`, `procNFe` e `NFe` solta, com o `infNFeSupl`); a NFC-e só existe no leiaute
    4.00 (o 3.10 é recusado com `ErroLeitura::NfceLeiaute`) e outros modelos continuam recusados. A NF-e 55 sai idêntica.
    O caso de erro "modelo 65" de `nfe_leitura.json` virou o do modelo 57. `nfe::conferir` e os pedidos da SEFAZ não mudam.
  * Os formatos de impressão do DANFE (valor, quantidade, documento, data no fuso da UF) passaram para `nfe::impressao`, sem
    mudar um byte do DANFE.
* NFC-e (modelo 65) na biblioteca (features `nfe` e `nfe_sefaz`): montagem, QR Code, envio e contingência off-line. A
  partida da leitura e da montagem da NF-e e do cálculo do QR Code veio da `gaialib`; o resto, das fontes oficiais de
  `docs/FONTES.md`. **Nenhum envio a uma SEFAZ real, nenhum certificado A1 real, nenhum CSC real e nenhum QR Code lido por
  um aplicativo de consulta de SEFAZ**; sem dependência nova.
  * `montagem::montar` e `conferir` montam o modelo 65 com as regras do MOC 7.00 e da NT 2026.002 v1.11 e o código da
    rejeição (38 casos); as mesmas regras valem para a NF-e com `tpImp` 6. O destinatário é opcional na NFC-e. A NF-e com
    `tpImp` 4 ou 5 (710) e com `tpEmis` 9 sem `tpImp` 6 (711) passa a ser recusada (antes não havia a regra).
  * `montagem::assinar` e a nova `assinar_com` (`OpcoesQr`, `Csc`): a NFC-e e a NF-e com `tpImp` 6 saem com o `infNFeSupl` entre o
    `infNFe` e a `Signature`; QR Code versão 3 (sem CSC, o padrão) e versão 2 (com CSC), on-line e off-line, com a tabela
    de endereços por UF e ambiente (`dados/nfce_qrcode.json`, com a fonte de cada linha; AM, MA e a homologação de RR
    ficam de fora e recusam com o motivo). O `assinar` deixa de recusar a nota com `tpImp` 6. O CSC não vai no pedido
    JSON (`docs/NFE_PEDIDO.md`).
  * `sefaz::tabela::url_nfce` e `dados/nfce_ws.json`: os endereços da NFC-e (8 autorizadores, 27 UF); `ClienteSefaz`
    autoriza (sempre em lote síncrono), consulta e cancela (110111) pela chave do modelo 65, e `status_nfce`;
    `autorizar_assincrono` recusa a NFC-e. `sefaz::contingencia::passo_offline` e `montagem::prazo_de_transmissao` dizem o
    que fazer com a NFC-e emitida em contingência off-line e até quando. `pedidos::chave_valida_nfce`, `cons_sit_nfce` e
    `evento::cancelamento_nfce` são novos; os pedidos que eram da NF-e continuam recusando a chave do modelo 65.
  * **Fica para depois:** as faces (aplicação, linha de comando, API HTTP e MCP), o cancelamento por
    substituição (110112) e a inutilização da NFC-e.
* NF-e (modelo 55), distribuição de DF-e e manifestação do destinatário, na biblioteca (feature `nfe_sefaz`; as faces estão no item seguinte). **Nenhuma chamada ao Ambiente Nacional real e nenhum certificado A1
  real**; sem dependência nova.
  * `rustnf::nfe::sefaz::dfe`: o pedido `distDFeInt` 1.01 (`distNSU`, `consNSU`, `consChNFe`; CNPJ, inclusive o
    alfanumérico, ou CPF), o cliente `ClienteSefaz::distribuir`, `distribuir_nsu`, `distribuir_chave` e `sincronizar`
    (endereço do Ambiente Nacional pela tabela oficial; `Servico::DistribuicaoDfe` é novo) e o `Cursor` que a aplicação
    persiste (`para_json` e `de_json`). O cursor só anda para o `ultNSU` devolvido, nunca pula nem repete um NSU nem
    volta; o NSU que falta no meio do lote vira lacuna e `sincronizar` o busca por `consNSU` (faixa maior que 500 NSU
    não se persegue: o Ambiente Nacional só guarda 90 dias); depois do 137 e do `ultNSU` igual ao `maxNSU` a
    distribuição espera uma hora (a consulta de um NSU ou de uma chave segue livre), e o 656 bloqueia toda consulta por
    uma hora, também na consulta por chave. Cada `docZip` vem aberto, com limite de 20 MB na descompressão, e lido
    (`resNFe`, `resEvento`, `procNFe`, `procEventoNFe`); esquema desconhecido, XML que não lê e `docZip` que não abre
    voltam no mesmo `Doc`, com o texto cru e o motivo, nunca descartados.
  * `ClienteSefaz::manifestar` e `evento::manifestacao`: os eventos 210200, 210210, 210220 e 210240 no Ambiente Nacional
    (`cOrgao` 91, autor o destinatário, `nSeqEvento` 1), com a justificativa por tipo (obrigatória na operação não
    realizada, opcional no desconhecimento, proibida nos outros dois), a conferência das combinações que o MOC recusa (650,
    651, 655 e 573) e do CNPJ-base ou CPF do certificado; o prazo (596) não é conferido. A resposta perdida se resolve como nos outros eventos: a
    consulta da NF-e antes de qualquer reenvio.
  * **Muda o que se aceita:** `155` ("cancelamento homologado fora de prazo") só vale como registro no cancelamento; num
    evento de outro tipo passa a ser rejeição. `pedidos::envelope` embrulha o `nfeDadosMsg` na operação
    `nfeDistDFeInteresse` quando o serviço é a distribuição (os outros serviços não mudam).
* NF-e: documentos RECEBIDOS e manifestação do destinatário na aplicação (`rustnf-app`), na linha de comando (`rustnf nfe
  recebidas sincronizar|listar|baixar|manifestar|cursor`), na API HTTP (`/nfe/recebidas/...`) e no MCP (`sincronizar_recebidas`,
  `listar_recebidas`, `baixar_recebida` e `manifestar_recebida`). **Nenhuma chamada ao Ambiente Nacional real e nenhum
  certificado A1 real**; sem dependência nova nas crates publicadas (`flate2` e `base64`, que a biblioteca já usa, entram só
  como dependência de teste de `rustnf-app` e `rustnf-cli`).
  * Pasta `nfe/recebidas/` dentro da `[notas].pasta`, separada das notas emitidas: `<AAAA>/<MM>/<chave>-resNFe.xml`,
    `-procNFe.xml`, `-<tipo>-<seq>-procEventoNFe.xml` e `-resEvento.xml`, `outros/` (o que não tem chave de NF-e ou não abriu),
    `indice.json` (chave, emitente, valor, data e situação: `resumo`, `completa`, `ciencia`, `confirmada`, `desconhecida`,
    `nao_realizada`, `cancelada`, `denegada`) e `cursor-<ambiente>-<documento>.json`, um por ambiente e interessado. A pasta e
    a configuração da 0.4.0 continuam lidas; `nfe notas reindexar` não enxerga os recebidos.
  * O cursor só anda DEPOIS de os documentos do lote e o índice estarem gravados: queda no meio de um lote refaz o mesmo lote
    e nunca pula NSU. O documento que não abriu é guardado e relido por `consNSU` na sincronização seguinte. O 137 e o 656
    ficam gravados no cursor: antes da hora a chamada responde quando tentar, sem ir à rede, e sai com 0 (esperar não é
    falha); o 656 sai com 3.
  * Manifestação (210200, 210210, 210220, 210240) com o que a pasta sabe conferindo o pedido antes da rede (ciência depois de
    conclusiva, repetição, cancelada ou denegada); o evento registrado vai para a pasta ao lado da nota. `baixar --ciencia`
    registra a ciência antes de consultar a chave, porque o Ambiente Nacional só entrega a NF-e completa a quem manifestou
    (NT 2014.002 v1.02d, item 3, texto do serviço e nota 1 da tabela de documentos; o desconhecimento não a libera).
  * `[nfe.urls]` aceita `distribuicao` (endereço do simulador). Na API e no MCP, tudo o que fala com o Ambiente Nacional exige
    `--permitir-emissao` e o Bearer; `listar_recebidas` não usa a rede, mas lê os documentos da empresa e exige o mesmo.
    `tools/list` autenticado passa de 22 para 26 ferramentas; sem o Bearer continua com 7.
* NF-e: o código do crédito presumido (`cCredPres`) passa a ser conferido pela tabela oficial (IT 2025.002 v1.70, 13
  códigos), não só pelo formato de 2 dígitos, no grupo `gCredPresOper` do item (`reforma::calculo`) e no evento
  211110. Cada regra da NT 2025.002 v1.53 aparece com a sua rejeição: código inexistente (1055; `UB122-10`,
  `2P25a-10`); grupo do IBS informado onde o código não o permite (1053; `UB123-10`, `2P26-10`) ou ausente onde o
  exige (1054; `UB123-20`, `2P26-20`); o mesmo para a CBS (1050 e 1058; `UB127-10`, `UB127-20`, `2P30-10`, `2P30-20`).
  Indicador vazio na planilha conta como "não permite". Novo módulo público `reforma::credpres` (`registro`,
  `descricao`, `tabela`, `versao`, `conferir_grupos`), com o dado em `dados/ccredpres.json`. **Muda o que se aceita:**
  um pedido que a SEFAZ recusaria agora é recusado antes do envio; no teste do evento 211110, o código `01` com só o
  grupo do IBS passou para `08` (o `01` exige IBS e CBS: seria a rejeição 1058). As colunas "apropria via nota", "via
  evento", `indNFe` e a vigência estão na consulta, mas nenhuma regra da NT as transforma em rejeição, então a
  montagem não as aplica. Nenhuma SEFAZ real foi chamada.
* Apresentação: o README, a descrição dos crates `rustnf` e `rustnf-cli` e o repositório passam a dizer que o rustnf
  emite NFS-e e NF-e (modelo 55), com um quadro do que cada uma tem hoje; a interface de desktop segue só com a NFS-e.
* NF-e: a produção do pacote de esquemas PL_010h (NT 2026.005 v1.00, Simples Nacional, e NT 2026.008, valor líquido do
  produto) passa de 16/11 para **14/12/2026**, pelo aviso do Portal Nacional da NF-e de 13/11/2026. Até 13/12, em produção,
  a montagem recusa os campos do PL_010h (`tpRegimeIBSCBS`, `gTribSN`...) com a rejeição 225 que a SEFAZ daria; em
  homologação nada muda (desde 26/10/2026). A NT 2025.002 v1.53 não foi adiada e segue em 16/11.
* NF-e (modelo 55), faces, parte 2: API HTTP (`/nfe/`) e MCP no mesmo `rustnf serve`, sobre as mesmas funções de
  `rustnf_app::nfe` que a linha de comando usa. **Nenhuma NF-e real foi emitida, nenhum certificado A1 real foi usado e
  nenhuma SEFAZ real foi chamada**; uma dependência nova no `rustnf-cli` (`async-trait`, já usada pela biblioteca).
  * Rotas: `POST /nfe/conferir`, `/nfe/montar` (nunca assina) e `/nfe/danfe` (sempre no ar, sem rede); e, só com
    `--permitir-emissao` e o Bearer (sem a opção, 404): `POST /nfe/emitir`, `/nfe/epec`, `/nfe/cancelar`,
    `/nfe/corrigir`, `/nfe/inutilizar`, `/nfe/pendentes/retomar`, `GET /nfe/{chave}`, `/nfe/status` e `/nfe/pendentes`.
    `GET /health` ganha `"nfe"` (a NF-e pode falar com a SEFAZ); os campos que já existiam não mudam. O que a SEFAZ não
    autoriza volta com o JSON completo e o envelope `erro` (422 recusa, 502 sem resposta); campo desconhecido recusa o
    pedido antes de qualquer envio.
  * Ferramentas MCP: `conferir_nfe`, `montar_nfe`, `gerar_danfe` (leitura) e `emitir_nfe`, `consultar_nfe`,
    `cancelar_nfe`, `corrigir_nfe`, `inutilizar_nfe`, `status_sefaz`, `registrar_epec` e `retomar_pendentes_nfe` (só com
    emissão e o Bearer). `tools/list` sem o Bearer passa de 4 para 7 ferramentas e, com ele, de 11 para 22; nada da NFS-e
    mudou.
  * O servidor numera, reserva faixa e retoma uma operação por vez: duas emissões simultâneas na mesma série não gastam o
    mesmo número. Sem `[nfe].uf`, as ações de efeito fiscal da NF-e respondem em palavras o que configurar.
  * `rustnf_app::nfe` ganha `conferencia_json` e `erro_da_retomada`, que a linha de comando também passa a usar (mesma saída).
* NF-e (modelo 55), faces, parte 1: a camada de aplicação (`rustnf-app`) e a linha de comando `rustnf nfe` (a API HTTP e o
  MCP vieram na parte 2, e a interface de desktop vem depois). Nenhuma crate nova. **Nenhuma NF-e real foi emitida, nenhum
  certificado A1 real foi usado e nenhuma SEFAZ real foi chamada.**
  * Configuração: seção `[nfe]` (`ambiente`, `uf`, `ie`, `crt`, `regime_pis_cofins`, `serie`, `contingencia` e
    `motivo_contingencia`, `fantasia`, `tempo_limite_s`, `tentativas_retorno`, `intervalo_retorno_s`, `canhoto`, `ca_pem`),
    `[nfe.endereco]` e `[nfe.urls]` (endereços de um simulador local), e as variáveis `RUSTNF_NFE_AMBIENTE`,
    `RUSTNF_NFE_SERIE`, `RUSTNF_NFE_CONTINGENCIA` e `RUSTNF_NFE_TEMPO_LIMITE`. O CNPJ ou CPF, o nome e o município do
    `[emitente]`, o certificado e o cofre do `[sefin]`, a folga do relógio e a pasta de `[notas]` valem também para a NF-e.
    O ambiente da NF-e (`homologacao` por padrão) é independente do `[sefin].ambiente` da NFS-e. O arquivo da versão 0.4.0
    continua sendo lido sem mudança.
  * Pasta de notas da NF-e em `<pasta>/nfe/`, separada da NFS-e: `nfeProc`, NF-e assinada pendente, eventos, faixas
    inutilizadas, `indice.json` e `estado.json` por série, com a mesma gravação atômica e a mesma trava. A NF-e assinada é
    gravada como `pendente` **antes** de ir à rede. Numeração: autorizada e denegada gastam o número, rejeitada não,
    número pulado se inutiliza (MOC 7.00, Anexo I, regras 2B08-10, 2B08-20, 2B08-40 e 3B08-100, e Visão Geral, 5.3).
    Situações: `autorizada`, `denegada`, `cancelada`, `pendente`, `inutilizada` e `epec`. A pasta e a numeração da NFS-e
    não mudam.
  * Aplicação (`rustnf_app::nfe`), uma função por ação: emitir (completa o pedido com a configuração, confere, monta,
    assina, grava pendente, envia e grava o resultado), consultar, cancelar, corrigir (a sequência da carta sai da pasta),
    inutilizar, status, EPEC, retomar os pendentes (a varredura do limbo da biblioteca sobre a pasta: consulta a mesma
    chave e reenvia a mesma NF-e, sem nota gêmea) e DANFE. As mensagens dizem em palavras o que houve e o que fazer, com
    o `cStat`.
  * Linha de comando: `rustnf nfe montar`, `conferir`, `emitir`, `consultar`, `cancelar`, `corrigir`, `inutilizar`,
    `status`, `epec`, `pendentes [--retomar]`, `danfe` e `notas listar|exportar|reindexar|numero`, com as mesmas opções
    globais, `--json` e códigos de saída (0 ok, 2 uso, 3 recusa de regra, 4 sem resposta ou serviço parado). `completar`
    cobre os comandos novos.
  * **Fica de fora:** interface de desktop, NFC-e, manifestação do destinatário e
    documentos recebidos.
* NF-e (modelo 55), DANFE em PDF (feature `danfe` = `nfe` + `danfse`, nenhuma crate nova): `rustnf::danfe::{dados, pdf}`
  desenham o DANFE A4 retrato direto no PDF, com o escritor do DANFSe (que segue com os mesmos bytes), em Liberation
  Serif (a métrica da Times New Roman que o item 3.7 do Anexo II manda usar; mesma licença SIL OFL 1.1). Blocos e
  medidas do item 3.8.1 do Anexo II do MOC 7.00, corpos de fonte mínimos do 3.7 (os campos crescem na altura em vez de o
  texto descer abaixo do corpo), itens em várias folhas com o cabeçalho de identificação repetido e "FOLHA nn/total"
  (3.5 e 3.10.2), continuação das informações complementares, canhoto e logotipo opcionais. O código de barras é o
  CODE-128 da chave (conjunto C; B só onde a chave tem letra), com margem clara de 10 módulos e módulo de pelo menos
  0,2 mm; `danfe::barras::decodificar` lê as barras de volta. Situações: homologação, cancelada, denegada, nota sem
  protocolo (só prévia, com marca), contingência com formulário de segurança (campos 1 e 2 do item 3.9.2, com o
  código de barras dos dados da NF-e), SVC e EPEC (protocolo do evento nas `Opcoes`, como sai de
  `nfe::sefaz::protocolo_epec_danfe`), com as expressões e o motivo e a hora de entrada em contingência que o Anexo III
  e a Visão Geral mandam imprimir. O leiaute da Reforma Tributária (NT 2026.010 v1.00: bloco IBS/CBS/IS, CRT, QR Code
  e as bases, alíquotas e valores por item) vale para a emissão a partir de 01/12/2026 (`nfe::vigencias`); antes disso
  vale o do MOC, e `Leiaute::Rtc` o força. Contrato em `tests/casos/danfe.json`. **Fica de fora:** NFC-e (DANFE NFC-e),
  as faces (aplicação, linha de comando, API, MCP e interface), o DANFE em paisagem e as colunas de ICMS ST por item.
  O ponto de partida foi o desenho equivalente da `gaialib`. **Nenhum DANFE foi conferido por fiscal ou SEFAZ e nenhum
  foi impresso em papel.**
* NF-e (modelo 55), ICMS monofásico sobre combustíveis (NT 2023.001 v1.60): os grupos `ICMS02`, `ICMS15`, `ICMS53` e
  `ICMS61` (CST 02, 15, 53 e 61, também no CRT 1) no pedido (13 campos novos em `itens[].icms`), na montagem e nos totais
  do `ICMSTot` (`qBCMono`, `vICMSMono`, `qBCMonoReten`, `vICMSMonoReten`, `qBCMonoRet`, `vICMSMonoRet`, só com valor). Regras
  conferidas antes do envio: 959 (sem o grupo `comb`), 508 (não contribuinte), 767, 768 e 769 (quantidade obrigatória), 962 e
  964 (valor = quantidade x alíquota ad rem, com o fator 0,3333 do biodiesel e do diesel B; só na nota normal) e a soma do
  `vICMSMonoReten` no `vNF` e no `vItem` (W16-10 e VB01-10). Os campos fora do XSD e os de outro CST são recusados. As regras
  revogadas (961, 963, 965 e 966) e a conferência do `cProdANP` com a Tabela de Combustíveis Monofásicos (960, e 959 pela
  tabela) não entram. O ponto de partida foi a correção equivalente da `gaialib`. O percentual de redução da alíquota ad rem
  zero é aceito, como no XSD. **Nenhuma chamada a uma SEFAZ real foi feita.**
* NF-e (modelo 55), cliente dos webservices da SEFAZ, parte 2: eventos, inutilização e contingência SVC, na mesma
  feature `nfe_sefaz` (nenhuma crate nova). **O DANFE, as faces (aplicação, linha de comando, API, MCP, interface), a
  distribuição de DF-e e a manifestação do destinatário não estão nesta parte. Nenhuma chamada a uma SEFAZ real foi
  feita.**
  * `nfe::sefaz::evento` (puro): `cancelamento` (110111, `nProt` de 15 ou 17 dígitos, justificativa de 15 a 255),
    `carta_correcao` (110110, sequência de 1 a 20, texto fixo de `xCondUso` do esquema; a carta nova substitui as
    anteriores), `epec` (110140, dados tirados da NF-e com `tpEmis` 4, `cOrgao` 91 do Ambiente Nacional) e `evento_rtc`
    (os 13 eventos de apuração do IBS/CBS da NT 2025.002 v1.53 mais o 110001; o 211120, eliminado pela NT, e os do
    Fisco ficam de fora). `Montado::assinar` assina o `infEvento` com o A1 e monta o `envEvento`.
  * `nfe::sefaz::inutilizacao`: o `inutNFe` com as regras da tabela 5-12 (faixa de até 10.000 números, ano de 2006 até o
    atual) e `disponivel`/`eliminada_em`: AC, BA e SP perderam o serviço em 11/11/2026 nos dois ambientes (NT 2023.002
    v1.02) e o cliente devolve o erro em palavras sem chamar a rede. O `inutNFe` é só de CNPJ, como o esquema nacional.
  * `nfe::sefaz::svc`: a SVC de cada UF pelo Ato COTEPE/ICMS 39/12 (SVC-AN: `tpEmis` 6; SVC-RS: `tpEmis` 7) e o que a
    SVC atende, pelo manual de contingência (MOC 7.00, Anexo III, 2.1.3.4). A nota com `tpEmis` 6 ou 7 vai ao endereço
    da SVC, e a consulta, o recibo e o cancelamento dela também; a carta de correção de uma nota da SVC vai à SEFAZ de
    origem (a SVC não a registra) e a inutilização nunca vai à SVC. O `tpEmis` que não é o da SVC da UF é recusado antes
    da rede (rejeição 713). `status_svc` consulta o status da SVC (107, 113 e 114). A montagem passa a exigir `dhCont`
    e `xJust` nos `tpEmis` 6 e 7 (Anexo III, 2.1.3.5; o Anexo I não tem rejeição para isso). A página de homologação do
    Portal lista o PI na SVC-RS e o `NfeInutilizacao` na SVC-AN; o Ato e o manual dizem o contrário.
  * `ClienteSefaz`: `cancelar`, `corrigir`, `registrar_epec`, `registrar_evento_rtc` e `inutilizar`, com o resultado
    tipado em palavras (`DesfechoEvento`: registrado, registrado fora do prazo, sem vínculo, prazo vencido, EPEC
    bloqueado, rejeitado, indisponível, não registrado, sem resposta; `DesfechoInutilizacao`). Evento sem resposta
    consulta a nota (que devolve os eventos) antes de qualquer reenvio: achou, devolve o registro; a consulta respondeu
    sem o evento, reenviar o MESMO evento é seguro; sem resposta nem da consulta, manda consultar de novo. A duplicidade
    573 também se resolve pela consulta. `resposta_evento` lê o `retEnvEvento` e o `retInutNFe` e monta o
    `procEventoNFe` e o `ProcInutNFe` pelo texto cru.
  * A resposta com o XML escapado dentro do `nfeResultMsg` (texto em vez de elemento) passa a ser lida, em todos os
    serviços.
  * Correção: a frase da consulta que acha só o EPEC (124) saía com uma fileira de espaços no meio.
  * Os pedidos, a carta de correção, o EPEC, os 14 eventos de apuração, o `inutNFe` e os documentos de distribuição foram
    validados nos esquemas XSD oficiais fora do repositório. Os eventos, a inutilização e a SVC partiram da `gaialib`;
    aqui foram reescritos e cobertos por casos novos.
* NF-e (modelo 55), cliente dos webservices da SEFAZ, parte 1, atrás da nova feature `nfe_sefaz` (= `nfe` + as
  dependências que a `sefin` já usa; nenhuma crate nova). **Nenhuma chamada a uma SEFAZ real foi feita.**
  * `nfe::sefaz::tabela`: endereços dos webservices lidos das páginas oficiais "Relação de Serviços Web" (produção e
    homologação; 15 tabelas, 96 endereços por ambiente), UF -> autorizador e a lista da contingência.
  * `nfe::sefaz::pedidos` e `resposta`: `consStatServ`, `enviNFe` (síncrono e assíncrono), `consReciNFe` e `consSitNFe`;
    envelope SOAP 1.2 sem `nfeCabecMsg`; leitura de `retConsStatServ`, `retEnviNFe`, `retConsReciNFe` e `retConsSitNFe`. A
    NF-e vai no lote como veio assinada, sem reescrever um byte, e o `nfeProc` usa o `protNFe` recortado do texto da
    resposta.
  * `nfe::sefaz::ClienteSefaz`: `status`, `autorizar` (síncrono), `autorizar_assincrono` (só até a data de corte da NT
    2023.002 v1.02 de cada ambiente), `retorno` (consulta do recibo com espera e tentativas configuráveis) e `consultar`.
    Resposta perdida vira consulta pela chave antes de qualquer reenvio; duplicidade 204 devolve a nota já autorizada;
    539 diz qual é a nota que tem o número. O resultado diz em palavras se a nota foi autorizada, denegada, rejeitada ou
    ficou sem resposta, e o que fazer.
  * `nfe::sefaz::limbo`: a varredura das notas enviadas sem resultado (espera crescente, aviso, reenvio da mesma NF-e).
  * Transporte real: `reqwest` com mTLS pelo certificado A1, raízes vigentes da AC Raiz da ICP-Brasil somadas às do
    `webpki-roots` só neste cliente (`dados/icp_brasil_raizes.crt`), tempo limite configurável, resposta limitada a 16
    MB e raiz extra para simulador local. O que o `nfse::sefin` já fazia para montar o cliente TLS agora mora num módulo
    comum (`https`), sem mudar o comportamento dele.
  * A recuperação da nota e a varredura do limbo partiram da `gaialib`; aqui foram reescritas e cobertas por casos novos.
* NF-e (modelo 55), base da biblioteca, atrás da feature `nfe` (sem dependência nova). **A NF-e ainda não emite (o envio vem no item do cliente da SEFAZ, acima)**: só
  chave, leitura e tabelas.
  * `chave_dfe`: a chave de acesso de 44 posições (NF-e e NFC-e). `montar` recusa campo que não cabe, UF ou modelo
    desconhecido, CPF ou CNPJ do emitente com dígito verificador inválido e `cNF` que a regra B09-30 recusa; `validar`,
    `decompor`, `formatar`, `limpar` e `achar` (texto de OCR e URL de QR Code) completam. O emitente CPF da chave é o da
    série 910 a 969 e o da Nota Fiscal Fácil (`tpEmis` 3, 5º dígito do número igual a 2); na série 890 a 899 a chave leva
    o CNPJ da SEFAZ e `nfe::conferir` não a compara com o emitente do XML (NT 2021.002 v1.12, itens 2.1 e 2.2). O que a
    NFS-e usa não mudou.
  * `nfe::ler`: lê o `nfeProc` ou a `NFe` solta (leiautes 3.10 e 4.00) para a struct `Nfe`, com decimal exato e data com
    fuso; recusa DTD, XML de outro documento, modelo diferente de 55 e campo fora do formato, com erro tipado.
    **A leitura não confere a assinatura XML nem o `digVal`.** `nfe::conferir` aponta divergência entre a chave e o `ide`,
    entre os totais e a soma dos itens e entre as duplicatas e a fatura.
  * `nfe::cstat`: tabela dos 953 códigos de retorno (MOC 7.00 Anexo I e Visão Geral, e Notas Técnicas), com a mensagem oficial e a
    classe (autorizada, cancelada, denegada, rejeitada, processamento). O `cStat` 110 (Uso Denegado) é classe denegada.
  * `nfe::vigencias`: calendário das Notas Técnicas por ambiente, com as datas conferidas no cronograma de cada NT.
  * A chave de acesso e a leitura da NF-e partiram da `gaialib`; aqui foram reescritas e cobertas por casos novos, com
    documentos fictícios.
* NF-e (modelo 55), montagem e assinatura, atrás da feature `nfe` (sem dependência nova). **O envio vem no item do cliente da SEFAZ, acima**:
  não há cliente da SEFAZ, DANFE, linha de comando, API nem interface para ela.
  * `nfe::montagem`: o pedido em JSON (`nota_de_json`, formato em `docs/NFE_PEDIDO.md`) vira a `Nota` e o XML `NFe` 4.00
    do modelo 55 (`montar`), na ordem do XSD PL_010h: ide (referenciadas, intermediador, contingência, `cIndOp`), emit,
    dest, retirada e entrega, det/prod (veículo novo, CEST, benefício fiscal), ICMS completo (CST e CSOSN, ST, diferimento,
    desoneração, partilha, repasse, FCP, ICMS efetivo), IPI, PIS, COFINS, ICMSUFDest (DIFAL), IBS/CBS, compra
    governamental, pagamento antecipado, valor líquido do produto, totais (`ICMSTot`, `IBSCBSTot`, `vNFTot`), transporte
    (`modFrete`), pagamento e cartão, informações adicionais e responsável técnico. A mesma entrada dá o mesmo XML: o
    `cNF` e a data de emissão vêm do pedido.
  * `conferir` devolve as rejeições do MOC e das Notas Técnicas (`<cStat>-Rejeicao: ...`) que a nota levaria, antes de
    gastar o número; `montar` recusa, com o campo e o código da rejeição, o que o leiaute não aceita.
  * `assinar` assina o `infNFe` com o A1 pelo módulo `assinatura`: RSA-SHA1 e digest SHA-1, os únicos que o
    `xmldsig-core-schema_v1.01.xsd` da NF-e aceita (a NFS-e usa SHA-256). O DANFE simplificado (`tpImp` 6) monta mas não
    assina: pede o QR Code, que ainda não existe. O modelo 65 (NFC-e) é recusado.
  * Campo desconhecido no pedido: aviso (`campos_desconhecidos`) e, no modo estrito (`nota_de_json_estrito`), recusa.
  * `reforma::calculo` (feature `nfe`): o IBS/CBS de mercadoria por item e o total (`ibscbs_do_item`, `totalizar`,
    `vnftot`); mais `grupo_exigido`, `tributos_somam_no_total`, `mono_modalidade` e `indop::erro_nfe`.
  * Simples Nacional (NT 2026.005, tabela da seção 2.3): o Simples sem regime regular do IBS/CBS (CRT 1, sem
    `tpRegimeIBSCBS`) leva o item SEM o `gIBSCBS`, mesmo quando o CST o exige, e COM o `gTribSN` (`gIBSSN` e
    `gCBSSN`); a montagem exige o `gTribSN` nesse regime. No CRT 2 sem regime regular o `gCBS` vai com a alíquota
    zerada.
  * A montagem, o cálculo e as regras partiram da `gaialib`; aqui foram reescritos e cobertos por casos novos, com
    documentos fictícios e o XML validado no XSD oficial.
* NF-e (modelo 55), grupos da montagem (feature `nfe`, sem dependência nova). **A NF-e ainda não é enviada.**
  * Item: `comb` (combustível, com CIDE, encerrante, origem 0 a 30 e GLP), `med` (medicamento) e `rastro` (lotes, até 500);
    um grupo específico só por item (`veicProd`, `med` ou `comb`). Nota: `cobr` (fatura e duplicatas), `transp` completo
    (transportadora, `retTransp`, veículo, reboques, vagão, balsa, volumes e lacres), `exporta` e `compra`, na ordem do XSD.
    Rejeições conferidas: 461, 855, 856, 817, 379, 380, 909 e 858 (combustível), 873, 877 e 870 (medicamento e lote),
    905, 901, 902, 852, 900, 850 e 851 (cobrança), 542, 543, 559 e 868 (transporte), 355 e 356 (exportação).
  * **DIFAL**: as regras 694 e 695 passam a ler a UF do local de entrega e o `cProdANP` do combustível, que a montagem nunca
    repassava a elas. Antes, a entrega na UF do emitente continuava exigindo o `ICMSUFDest`, a entrega em outra UF continuava
    recusando o grupo e o combustível derivado de petróleo fora da lista da ANP não dispensava nem proibia o grupo.
    Nenhum XML de caso anterior mudou.
  * **Canal de avisos**: `Montada` ganha `avisos` e há `montagem::avisos(&Nota)`; aviso nunca impede a montagem nem muda o
    XML. Campo novo e opcional do pedido, `regime_pis_cofins` (`cumulativo` ou `nao_cumulativo`, não vai ao XML), para o
    aviso de CST e alíquota de PIS/COFINS que não combinam com o regime (Leis 10.637/2002, 10.833/2003, 9.715/1998 e
    9.718/1998). Segundo aviso: Simples sem regime regular com CST que exige o `gIBSCBS` (tabela 2.3 da NT 2026.005 contra a
    regra UB13-30 da NT 2025.002, rejeição 1022).
  * Quem monta `Nota`, `Item` ou `Montada` por literal de struct precisa dos campos novos (`transp`, `cobr`, `exporta`,
    `compra`, `regime_pis_cofins`; `combustivel`, `med`, `rastro`; `avisos`). `montar` não mudou de assinatura.
* Reforma: o aviso ao optante do Simples Nacional passa a explicar a opção pelo regime regular do IBS e da CBS (LC 123/2006,
  art. 13, §§ 9º a 11, incluídos pela LC 214/2025, art. 517, na redação da LC 227/2026, com efeito a partir de 1º/1/2027):
  vale por semestre iniciado em janeiro ou julho, é irretratável no semestre e é feita em setembro ou março, antes dele.
* Consulta da DPS: `rustnf consultar --dps <id>` (ou `--serie` e `--numero`, com o `[emitente]` da configuração),
  `GET /dps/{id}` e a ferramenta MCP `consultar_dps` devolvem a chave da NFS-e que a DPS gerou, e a nota com
  `--com-nota`. O Id fora da forma é recusado antes da rede.
* Consulta de evento: `rustnf evento consultar`, `GET /nfse/{chave}/eventos/{tipo}/{sequencia}` e `consultar_evento`.
  O evento devolvido é conferido contra a chave e o tipo pedidos: a resposta de outra nota ou de outro tipo, ou um
  corpo que não é um evento, é erro, não sucesso. Isso vale também para a conferência do cancelamento.
* Eventos além do cancelamento: `rustnf evento registrar`, `POST /eventos` e `registrar_evento` registram os eventos
  que o contribuinte pede (por exemplo o 101103, pedido de análise fiscal); o 101103 registrado marca a nota da pasta
  como em análise fiscal. O cancelamento (101101) e os eventos do fisco e do sistema são recusados antes de qualquer
  envio, dizendo o que fazer ou quem os registra. `rustnf evento tipos` lista os tipos do contribuinte.
* Atualização das situações em lote: `rustnf notas atualizar`, `POST /notas/atualizar` e `atualizar_situacoes`
  conferem na Sefin as notas da pasta (cancelada, substituída, em análise fiscal), uma nota e uma consulta por vez,
  com intervalo entre as consultas (200 ms por padrão). Nota que a pasta dá como cancelada ou substituída e que a
  Sefin não mostra assim nunca volta para autorizada: vai para as falhas, como divergência.
* A situação da nota na pasta passa a ter `substituida` (105102) e `analise_fiscal` (101103 sem decisão do fisco);
  aparecem no selo, no filtro e no detalhe da lista de notas da interface, que não ganhou tela nova. O `reindexar`
  aproveita todas as situações do índice anterior, não só a cancelada.
* Interface de desktop, **eventos na tela**: "Conferir situação na Sefin" nas Notas (as notas que a lista mostra, com
  andamento, Parar, confirmação acima de 50 notas e resultado em palavras: o que mudou de-para, quantas estavam iguais e,
  de cada falha, o que fazer); "Histórico na Sefin" no detalhe da nota; "Pedir análise fiscal do cancelamento" (mesmo
  diálogo do cancelamento, oferecido também quando a Sefin recusa o cancelamento por prazo, E0822; o pedido sem
  resposta é conferido, não reenviado às cegas); Consultar pela série e número da
  DPS; e, para a nota recebida (de outro emitente, com o emitente da configuração como tomador), "Confirmar que o serviço
  foi prestado" e "Recusar esta nota". `rustnf-app`: `atualizar_lista` (a conferência em lote com acompanhamento e parada)
  e `marcar_analise_fiscal` pública. O emissor de demonstração responde consulta de DPS e de evento e registra eventos por
  roteiro (`--evento`). Testados com a demonstração e com a janela no ambiente de teste do GPUI: **nenhuma consulta nem
  evento real foi feito pela interface**.
* Correção: quando a resposta do registro de um evento se perdia, a biblioteca só olhava a sequência 1 do tipo e dava
  por registrado um pedido novo de um tipo que já tinha evento (o 101103 depois de um 105105, por exemplo). Agora ela
  percorre as sequências do tipo e só aceita o evento que traz o mesmo `dhEvento` do pedido; não achando, devolve
  "pendente" para reenviar o mesmo pedido.
* `rustnf-app`: dependência direta de `tokio` (só o relógio, para o intervalo entre as consultas); a interface de
  desktop e as demais faces usam o executor de quem chama.

## 0.4.0 - 2026-11-25

Ao atualizar: a configuração nova (`aliquota_iss`) é opcional e vazia por padrão, e a prévia do IBS/CBS só muda no
empate exato de arredondamento (veja abaixo).

* Nova nota: "Buscar endereço pelo CEP" preenche logradouro, bairro e município (código IBGE) do tomador. Só no
  clique: o CEP vai ao ViaCEP e, se ele não responder, à BrasilAPI; sem resposta, o endereço é preenchido à mão. Não
  traz dependência nova (o cliente HTTP já era o da Sefin).
* Quem já emitia por outro sistema continua a numeração de lá: `rustnf notas numero --proximo <N>` ou, na interface,
  Configurações > Notas e DANFSe > "Continuar deste número". A série passa a seguir do número informado; a
  numeração só anda para a frente (um número já usado faria a Sefin devolver a nota antiga). Sem `--proximo`, o
  comando mostra a próxima DPS da série.
* ISS: alíquota padrão no `[emitente]` (`aliquota_iss`, vazio = não informar), que completa a DPS só onde ela leva
  a alíquota (ISS tributável, sem regime especial, fora do MEI e, para ME/EPP, com ISS retido). Na interface, o campo
  fica em Configurações > Emitente, aparece como dica na Nova nota, e o resumo de valores mostra o ISS estimado (ou
  "calculado pela Sefin" sem alíquota conhecida); o ISS retido sai do valor líquido.
* Interface de desktop pelo teclado: o Tab leva a tela até o campo que ganha o foco (antes ele podia ficar fora de
  vista), PageUp/PageDown rolam a tela mesmo com o foco num campo, e as setas e Ctrl+Home/Ctrl+End rolam quando o
  foco não está num campo. Na lista de notas as setas continuam andando pela lista.
* Retorno E0008 da Sefin (hora de emissão depois da hora de processamento) ganha a dica de acertar o relógio ou
  aumentar `folga_relogio_s`.
* Prévia do IBS/CBS: a leitura de alíquota ou redução com mais de 4 casas passa a arredondar "meio para o par", como
  os valores da prévia e a calculadora oficial; só muda no empate exato (ex.: `0,00005`).
* `rustnf completar <shell>`: imprime o script que completa comandos e opções com Tab no PowerShell, bash, zsh,
  fish ou elvish (veja "Linha de comando" no README).
* Dependência `url` 2.5.8 -> 2.5.9 (RUSTSEC-2026-0592, estouro de pilha em `Url::origin()`; chega pela interface
  de desktop).
* Dependências: atualização semanal proposta pelo Dependabot, agrupada, sempre passando pelo CI antes de entrar; o
  kit da interface de desktop continua sendo trocado à mão.
* Versão mínima do Rust declarada no `Cargo.toml` (`rust-version = "1.92"`) e conferida no CI, que compila o
  workspace nessa versão além do Rust estável.
* Exemplo da biblioteca: `cargo run -p rustnf --example montar_e_assinar --features cert_a1_teste` monta uma DPS a
  partir do pedido em JSON, assina com um certificado A1 de teste e confere a assinatura, sem rede.
* CI: documentação gerada com aviso virando erro, o exemplo da biblioteca roda, e o histórico inteiro passa pelo
  gitleaks (binário oficial conferido pelo SHA-256); as ações do GitHub foram para as versões em Node.js 24.
* Release: cada arquivo publicado ganha um atestado de procedência do GitHub, conferível com
  `gh attestation verify <arquivo> --repo Admnwk/rustnf`.
* Dependências `roxmltree` 0.21 e `qrcode` 0.15 (os DANFSe do contrato saem byte a byte iguais aos da 0.3.0) e correções
  de `clap`, `regex`, `toml`, `keyring-core` e `x509-cert`

## 0.3.0 - 2026-11-21

Atenção ao atualizar: a emissão passa a recusar o pedido com campo de nome errado (veja "Mudança de comportamento"
abaixo); a API HTTP e o MCP passam a usar a pasta de notas e o `[emitente]` da configuração, como a linha de comando.

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
* **Mudança de comportamento:** campo de nome errado no pedido da DPS deixa de ser ignorado em silêncio. A
  **emissão recusa** o pedido com campo que a DPS não conhece, antes de abrir o certificado e de qualquer envio: em
  `rustnf emitir` (código 3; `--ignorar-desconhecidos` emite mesmo assim, sem o campo, e `--estrito` continua aceito),
  em `POST /emitir` (422; `?estrito=false` emite sem o campo) e em `emitir_nfse` (`isError`). `rustnf dps montar` e
  `dps conferir`, `/dps/conferir` e `/dps/montar` e `conferir_dps` avisam e devolvem a lista em
  `campos_desconhecidos`; com `--estrito` ou `?estrito=true`, também recusam.
* `rustnf dps esquema` imprime o JSON Schema do pedido da DPS (`--emissao` para o do `emitir`).
* Interface de desktop:
  * correção na busca da tela Notas: um nome de tomador comprido com número no meio ("Clínica 24 Horas ...") era
    tomado por chave de acesso e não achava nada. A busca passa a usar a mesma regra da linha de comando: só é chave
    o texto que começa pelos 7 dígitos do município;
  * "Abrir a pasta", na tela Notas, mostra a pasta de notas no gerenciador de arquivos;
  * "Copiar o XML", no detalhe da nota, põe o XML inteiro na área de transferência;
  * Ctrl+Shift+D descarta, nas Configurações, o que foi alterado e não foi salvo (o botão "Descartar" já existia; a
    ação não tinha tecla);
  * período na tela Notas ("Todo o período", "Este mês", "Mês passado", "Este ano"), junto com a busca e o filtro;
  * "Exportar a lista", na tela Notas: grava numa pasta escolhida o XML de cada nota da lista e a lista em CSV;
  * a coluna DPS da lista não quebra mais um número de DPS longo em duas linhas: o meio dá lugar a reticências;
  * tomador já usado: na Nova nota, ao digitar o CPF ou CNPJ completo de quem já recebeu nota, os campos vazios do
    tomador (nome, inscrição municipal, e-mail, telefone) e o endereço vêm da nota mais recente dele na pasta de
    notas, com um aviso de qual nota. O que a pessoa já digitou fica; com parte do endereço digitada, o endereço não
    é mexido.
* `rustnf-gui.exe` ganha ícone (o da janela e o do arquivo) e a versão nas propriedades do arquivo (aba "Detalhes"),
  gerada a partir da versão do pacote. O CI e o fluxo de release conferem os dois no executável, e a release só sai
  se a versão gravada for a da tag.
* Biblioteca: a NFS-e lida (`nfse::leitura::Nfse`) traz também a inscrição municipal, o e-mail, o telefone e o
  endereço nacional do tomador (`tomador_im`, `tomador_email`, `tomador_fone`, `tomador_cep`, `tomador_municipio`,
  `tomador_logradouro`, `tomador_numero`, `tomador_complemento`, `tomador_bairro`). O JSON de `emitir` e de
  `consultar` (linha de comando, API HTTP e MCP) ganha esses campos em `nfse`.
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
