# rustnf

Emissor aberto de **NFS-e** do padrão nacional (Sefin Nacional / ADN) e de **NF-e** (modelo 55) e **NFC-e** (modelo 65,
na biblioteca), com as SEFAZ, escrito em Rust: uma biblioteca, um binário (`rustnf`) que oferece **linha de comando**, **API HTTP** e **servidor MCP** no mesmo
executável, e uma **interface de desktop** para Windows (`rustnf-gui`).

| | NFS-e | NF-e (modelo 55) | NFC-e (modelo 65) |
|---|---|---|---|
| montar, conferir as regras e assinar | sim | sim | sim, com o QR Code (versões 2 e 3) |
| emitir, consultar, cancelar | sim (Sefin Nacional) | sim (SEFAZ da UF, contingência SVC e EPEC) | sim (SEFAZ da UF, só em lote síncrono); contingência off-line (`tpEmis` 9) |
| outros eventos | análise fiscal e os eventos do contribuinte | carta de correção, eventos da apuração do IBS/CBS, inutilização | ainda não |
| documento auxiliar | DANFSe (PDF) | DANFE (PDF, A4 retrato) | DANFC-e (PDF de bobina de 80 e 58 mm e bytes ESC/POS, feature `danfce`); o DANFE Simplificado Tipo 2 da NF-e (`tpImp` 6) sai da assinatura com o QR Code, sem o PDF |
| notas recebidas de terceiros | | distribuição de DF-e e manifestação do destinatário | não se aplica |
| linha de comando, API HTTP e MCP | sim | sim | sim |
| interface de desktop | sim | sim, em um assistente por passos (emitir com EPEC, produtos, nota anterior como modelo, configurar, ver, DANFE, conferir, cancelar, carta, inutilizar, retomar) | sim: o **Caixa** (venda rápida pelo teclado, cupom, contingência off-line, fechar o caixa) e a NFC-e na lista de Notas (cupom, conferir, cancelar, retomar) |

A NFC-e tem a biblioteca (features `nfe`, `nfe_sefaz` e `danfce`: leitura do modelo 65, montagem, QR Code, envio, contingência
off-line e o DANFC-e, `rustnf::danfce::{dados, pdf, escpos}`, veja `crates/rustnf/examples/danfce.rs`), a camada de aplicação,
a linha de comando (`rustnf nfce`), a API HTTP (`/nfce/`) e o MCP (`*_nfce`). A interface de desktop tem o **Caixa** (veja [Caixa (NFC-e)](#caixa-nfc-e)).

> **Aviso: este projeto NÃO é software oficial** da Receita Federal, do Comitê Gestor da NFS-e, de qualquer
> prefeitura nem de qualquer Secretaria da Fazenda. Ele segue a documentação pública (fontes em `docs/FONTES.md`); a
> responsabilidade pelo uso em produção é de quem o opera. Emitir, cancelar ou manifestar nota em produção tem efeito
> fiscal. Teste antes na produção restrita (NFS-e) ou em homologação (NF-e).

![Nova nota: o formulário, a conferência e a prévia do IBS/CBS](docs/imagens/nova-nota.png)

![Notas: a pasta de notas, com busca e filtro](docs/imagens/notas.png)

As telas acima são da interface de desktop, com dados fictícios.

## Emitir uma NFS-e: passo a passo

Para a NF-e, veja o [passo a passo da NF-e](#passo-a-passo). Pela interface de desktop (Windows):

1. Baixe o `.zip` da versão mais recente em [Releases](https://github.com/Admnwk/rustnf/releases), descompacte e abra o
   `rustnf-gui.exe` (sobre o aviso do Windows no primeiro uso, veja [Baixar pronto](#baixar-pronto)).
2. No primeiro uso, preencha os três passos: os dados do **emitente**, o **certificado A1** (o arquivo `.pfx` e a senha,
   com o botão "Testar certificado") e a **pasta das notas**.
3. Confira, no canto superior direito, que o ambiente é **Produção restrita (testes)**. É o padrão; nele as notas não
   têm efeito fiscal.
4. Abra **Nova nota** e preencha o tomador, o serviço e os valores. O painel ao lado mostra o que falta; quando ele
   disser "Tudo certo", clique em **Emitir nota** e confirme.
5. A nota autorizada aparece em **Notas**: abra-a para ver os dados, gerar o DANFSe em PDF ou cancelar.
6. Quando os testes derem certo, troque o ambiente para **Produção** em Configurações. A partir daí cada nota tem valor
   fiscal, e a tela pede uma confirmação a mais antes de emitir.

Pela linha de comando (Windows ou Linux), com o pedido em JSON (modelo em `crates/rustnf-cli/exemplos/dps.json`):

```
$ export SENHA_A1='...'
$ rustnf dps conferir pedido.json
$ rustnf emitir pedido.json --pfx certificado.pfx --senha-env SENHA_A1 --ambiente restrita
```

Os detalhes de cada passo estão em [Interface de desktop](#interface-de-desktop) e em
[Linha de comando](#linha-de-comando).

## Estado

O rustnf segue a documentação oficial, e cada parte foi conferida contra ela:

* o XML é validado nos esquemas oficiais (XSD): 210 NF-e e NFC-e no pacote PL_010h v1.03, além dos eventos, da
  inutilização e da distribuição de DF-e;
* as regras vêm do MOC, das notas técnicas e do Anexo I da NFS-e, cada uma com o código de rejeição que a SEFAZ ou a
  Sefin Nacional daria;
* a prévia do IBS/CBS é conferida nos casos da calculadora oficial da Receita Federal;
* o QR Code da NFC-e é conferido por um cálculo independente; o DANFE, contra o Anexo II do MOC, e o DANFC-e, contra o
  Manual do DANFC-e;
* o envio, a resposta perdida, a duplicidade, a contingência e a retomada com a mesma chave (sem nota gêmea) foram
  percorridos com um transporte de teste e, na NF-e e na NFC-e, contra um servidor simulado.

Falta a prova que só o uso real dá: uma SEFAZ ou a Sefin Nacional aceitar a nota assinada com um certificado A1 de
verdade. É aí que você ajuda, e sem risco: comece pela produção restrita (NFS-e) ou pela homologação (NF-e e NFC-e),
onde a nota não tem efeito fiscal. Conte como foi pelo modelo **"Relato de emissão"** (aba Issues). O relato de quem
conseguiu vale tanto quanto o de quem achou erro.

Parte por parte:

| parte | como foi conferida | o que só o teste real prova |
|---|---|---|
| XML da DPS e regras antes do envio | contratos em `crates/rustnf/tests/casos/`, comparados texto a texto; regras lidas no Anexo I oficial | a Sefin Nacional aceitar uma DPS gerada aqui |
| assinatura | conferida pelo próprio verificador e por um XML assinado fora da biblioteca | a Sefin Nacional validar uma assinatura feita aqui, em SHA-256 (o padrão) ou em SHA-1 (`--assinatura sha1`) |
| cliente da Sefin Nacional | transporte falso (resposta perdida, 5xx, E0014, consulta da nota, da DPS e de evento, cancelamento, pedido de análise fiscal, conferência da situação das notas em lote) | uma chamada real, com TLS mútuo e certificado A1; nenhuma consulta de DPS, consulta ou registro de evento e nenhuma atualização de situação foi feita contra a Sefin, e o que ela responde aos eventos de manifestação (202201 e seguintes) e às sequências de um mesmo tipo de evento não foi conferido |
| prévia do IBS/CBS | casos de teste da calculadora oficial da Receita Federal | comparar com os valores de uma NFS-e autorizada |
| montagem e assinatura da NF-e (feature `nfe`) | 188 XML de contrato validados no XSD oficial 4.00 (pacote PL_010h v1.03), regras com o código da rejeição, assinatura conferida pelo verificador da própria biblioteca | nenhum envio a SEFAZ e nenhum XML conferido por uma SEFAZ real; o XML não foi comparado com uma nota autorizada |
| cliente da SEFAZ para a NF-e (feature `nfe_sefaz`) | transporte falso (autorizada, denegada, rejeitada, lote em processamento, resposta perdida, duplicidade 204 e 539, Fault, HTTP de erro, corte do assíncrono), esquemas XSD oficiais para os pedidos e o `nfeProc` e um simulador local (fora do repositório) | nenhuma chamada a uma SEFAZ real e nenhum certificado A1 real; o espaço de nomes e a ação SOAP de cada serviço não foram conferidos no WSDL publicado; as raízes da ICP-Brasil não foram testadas contra o servidor de uma SEFAZ |
| eventos, inutilização e contingência SVC do cliente da SEFAZ (feature `nfe_sefaz`) | transporte falso (cancelamento homologado, fora do prazo e recusado por prazo, carta de correção, EPEC, evento da apuração do IBS/CBS, evento sem resposta que a consulta acha, não acha ou também não responde, duplicidade 573, inutilização 102, 563 e 998, AC, BA e SP sem o serviço, SVC por UF), 68 validações nos esquemas XSD oficiais (`envEvento` de cada tipo, `inutNFe`, `procEventoNFe`, `ProcInutNFe`) e um simulador local (fora do repositório) | nenhuma chamada a uma SEFAZ real e nenhum certificado A1 real; os WSDL de `NFeRecepcaoEvento4` e `NFeInutilizacao4` não foram lidos; o `cOrgao` 91 do EPEC e o `cOrgao` da UF nos eventos da apuração seguem o texto do MOC e da NT, sem confirmação de uma SEFAZ |
| distribuição de DF-e e manifestação do destinatário (feature `nfe_sefaz`) | transporte falso (138 com resumo, NF-e completa, resumo de evento e evento; 137 e a espera de uma hora; 656 e o bloqueio; rejeição; serviço parado; `docZip` corrompido, acima do limite de descompressão e de esquema desconhecido; lacuna de NSU fechada por `consNSU`; NSU repetido, recuado ou sem avanço; salto grande; laço `sincronizar` com `gravar` falhando; os quatro eventos 210200, 210210, 210220 e 210240, cada regra de justificativa e de combinação, certificado de outro CNPJ-base, resposta perdida com a consulta que acha, não acha ou também não responde, duplicidade 573), 42 validações nos esquemas XSD oficiais (pedido `distDFeInt`, `envEvento` de cada tipo e o `detEvento` pelo esquema do tipo, `procEventoNFe`, `retDistDFeInt`, `resNFe` e `resEvento`) e um simulador local (fora do repositório) percorrendo a fila sem pular NSU, manifestando e provocando o 656 | nenhuma chamada ao Ambiente Nacional real e nenhum certificado A1 real; o WSDL de `NFeDistribuicaoDFe` não foi lido (a operação `nfeDistDFeInteresse` e o espaço de nomes vêm da NT 2014.002 e de uma biblioteca pública gerada do WSDL, não do próprio WSDL); o prazo da manifestação (rejeição 596) não é conferido; não se viu uma consulta real da NF-e devolver as manifestações (a recuperação depende disso); o procNFe lido da distribuição não foi validado no esquema completo da NF-e (só o pacote da distribuição e o dos eventos) |
| DANFSe | contrato de conteúdo e leiaute | comparar com um DANFSe emitido pelo portal |
| DANFE da NF-e (feature `danfe`) | contrato de conteúdo, plano das folhas e desenho; medidas, código de barras (decodificado de volta na chave) e corpos de fonte conferidos nas operações; fotos das folhas conferidas contra o Anexo II | nenhum DANFE conferido por fiscal ou SEFAZ e nenhum impresso em papel; a leitura do código de barras por um leitor óptico |
| DANFC-e (feature `danfce`) | 12 casos de conteúdo por divisão do Manual (consumidor identificado e não identificado, vários itens, pagamentos com troco, acréscimo e desconto, homologação, contingência off-line pendente nas duas vias, cancelada, resumido, 58 mm) e 10 recusas com o motivo; PDF com bytes estáveis, uma página, texto extraível e QR decodificado de volta no `qrCode` do XML (leitor independente); ESC/POS com bytes estáveis, colunas 48, 42 e 32, páginas de código e o comando do QR (nível M) com o texto exato | nenhuma impressão em impressora real, nenhum DANFC-e conferido por fiscal ou SEFAZ e nenhum QR lido por aplicativo de consulta de SEFAZ |
| NFC-e (modelo 65, features `nfe` e `nfe_sefaz`) | 8 XML de contrato (a NFC-e simples, com consumidor, off-line, de entrega e a NF-e com `tpImp` 6) e os 202 da NF-e, 210 ao todo, validados no esquema oficial 4.00 (pacote PL_010h v1.03); 38 regras do MOC e da NT 2026.002 com o código da rejeição; o QR Code (versões 2 e 3, on-line e off-line) com o hash SHA-1 e a assinatura RSA conferidos por um cálculo independente (Python, fora do repositório) e o `infNFeSupl` aceito pelo esquema; transporte falso (autorização síncrona no endereço da NFC-e, consulta, cancelamento, status, lote assíncrono recusado, contingência off-line) e um simulador local (fora do repositório), que aceitou a NFC-e on-line (QR versões 2 e 3), a off-line, a consulta e o cancelamento | nenhum envio a uma SEFAZ real, nenhum certificado A1 real e nenhum CSC real; nenhum QR Code lido por um aplicativo de consulta de SEFAZ; os endereços do QR Code e da consulta por chave vêm das páginas do Portal Nacional da NFC-e (AM, MA e a homologação de RR ficam fora da tabela), a da consulta por chave é de 2018; os endereços dos webservices da NFC-e e o mapa UF-autorizador não foram conferidos num WSDL nem numa chamada; o cancelamento por substituição (110112) e a inutilização da NFC-e não foram feitos |
| faces da NF-e (camada `rustnf-app`, `rustnf nfe`, API HTTP `/nfe/` e MCP) | transporte falso (autorizada, denegada, rejeitada que não gasta número, número já autorizado com outra chave, resposta perdida com a nota pendente e retomada sem nota gêmea, reenvio da mesma NF-e, cancelamento, carta de correção numerada pela pasta, inutilização, EPEC e sua transmissão, status), pasta de notas e configuração; DANFE gerado da nota guardada; um simulador local, fora do repositório, pela linha de comando, pela API (curl) e por um cliente MCP mínimo escrito para o teste; rotas e ferramentas por requisição em memória, com transporte falso | nenhuma NF-e real foi emitida, consultada nem cancelada por estas faces, nenhum certificado A1 real foi usado e nenhuma SEFAZ real foi chamada (nem a de homologação); nenhum cliente MCP de terceiros leu as ferramentas |
| faces da NFC-e (camada `rustnf-app` e `rustnf nfce`: CSC no cofre, pasta `nfce/` de numeração própria, emissão, off-line, retomada, pendentes e fechamento do caixa, cancelamento com aviso de prazo, DANFC-e como arquivo) | transporte falso (autorizada, rejeitada que não gasta número, denegada, resposta perdida com a nota pendente e retomada da mesma chave, off-line com o prazo, transmissão da off-line e a recusa dela anotada, decisão automática antes do envio, cancelamento, pendentes e `--fechar-caixa` com o código de saída, CSC que não vaza para JSON, `Debug` nem arquivo da pasta, numeração do 65 separada do 55 e pasta antiga do 55 lida igual), cofre em memória, DANFC-e gerado da pasta e do XML (PDF e ESC/POS com bytes estáveis) e um simulador local, fora do repositório, pela linha de comando | nenhuma NFC-e real, nenhuma SEFAZ real (nem a de homologação), nenhum certificado A1 real, nenhum CSC real e nenhuma impressão em impressora real; o prazo de cancelamento de cada UF (só o nacional de 24 horas tem fonte), o cancelamento por substituição (110112) e a inutilização da NFC-e não foram feitos; a interface de desktop da NFC-e tem a linha própria abaixo |
| API HTTP e MCP da NFC-e (`/nfce/` e as 7 ferramentas `*_nfce`, no mesmo servidor da NF-e) | rotas e ferramentas sobre o transporte falso e uma pasta temporária: um teste por rota e ferramenta e desfecho (autorizada, rejeitada, sem resposta e retomada da mesma chave, off-line e transmissão, cancelamento com aviso de prazo, status, pendentes e fechamento do caixa), 404 sem `--permitir-emissao`, 401 sem o Bearer, `tools/list` com e sem o Bearer, campo desconhecido e CSC recusados sem envio, CSC que não aparece em resposta, no schema nem na pasta de notas, duas NFC-e e uma NFC-e com uma NF-e ao mesmo tempo, NFC-e sem configuração utilizável; e o servidor de verdade contra um simulador local (fora do repositório), por `curl` e por um cliente MCP mínimo | nenhuma NFC-e real, nenhuma SEFAZ real, nenhum CSC real e nenhum cliente MCP de terceiros |
| faces dos documentos recebidos e da manifestação (`rustnf nfe recebidas`, `/nfe/recebidas/` e 4 ferramentas do MCP) | transporte falso (vários lotes até o 137, queda no meio de um lote sem pular NSU, 656 gravado e a chamada seguinte que diz a hora sem ir à rede, documento que não abre e é relido, ciência, confirmação e as recusas antes da rede, baixa por chave com e sem ciência, pasta e configuração da 0.4.0) e um simulador local (fora do repositório) pela linha de comando, pela API e por um cliente MCP mínimo | nenhuma chamada ao Ambiente Nacional real e nenhum certificado A1 real; não há interface de desktop para os recebidos |
| interface de desktop, NF-e (`rustnf-gui`, só Windows: assistente de emissão em cinco passos, cadastro de produtos, nota anterior como modelo, contingência EPEC, configurar, lista, detalhe, DANFE, conferir, cancelar, carta de correção, inutilizar e retomar) | testes do modelo (a tabela das escolhas fiscais com o CFOP conferido na tabela da biblioteca, o pedido que o assistente monta passando pela montagem da aplicação nos casos típicos do Simples e do regime normal, o que a pessoa editou à mão nunca sobrescrito, cadastro com arquivo ruim, rascunho, a nota anterior lida do XML, texto de cada erro e desfecho) e da janela no ambiente de teste do GPUI com uma SEFAZ de mentira (transporte roteirizado, sem rede): o assistente do início ao fim (autorizada, com o produto novo entrando no cadastro), o produto do cadastro, a rejeitada que se corrige e sai com o mesmo número, a nota sem resposta que só o Retomar resolve (um único envio, nenhuma nota gêmea), a SEFAZ fora e o EPEC (que não é oferecido com nota sem resposta na pasta), a produção sem a marcação (nenhuma chamada), o clique duplo, a nota anterior como modelo e a tela de produtos; uso manual pelo exemplo de demonstração | nenhuma NF-e real, nenhuma SEFAZ real (nem a de homologação), nenhum EPEC real e nenhum certificado real pela interface; a venda ao exterior, o IPI, a venda por intermediador e o valor unitário com mais de duas casas não passam pelo assistente (vão pela linha de comando); o DANFE aberto no leitor de PDF de outra máquina; o lado Linux só pelo CI |
| interface de desktop, NFC-e (`rustnf-gui`, só Windows: o Caixa pelo teclado com cupom, desconto, CPF opcional e pagamento com troco, o cupom como arquivo, a contingência off-line, "Fechar o caixa", a seção NFC-e das Configurações com o CSC no cofre, e a NFC-e na lista de Notas com detalhe, conferir, cancelar e retomar) | testes do modelo (o pedido do cupom passando pelo `montar_pedido` estrito da aplicação com o contexto da NFC-e: dinheiro com troco, Pix, cartão, duas formas, desconto rateado, com e sem CPF, Simples e regime normal; troco e totais; o CSC fora de texto de tela e de `Debug`; o texto de cada desfecho; o prazo da off-line e o aviso de fechamento vindos da aplicação) e da janela no ambiente de teste do GPUI com uma SEFAZ de mentira: a venda do início ao fim pelo teclado até o cupom em PDF e ESC/POS, a rejeitada que sai com o mesmo número, a sem resposta que só o Retomar resolve (um único envio), a SEFAZ fora antes de enviar (off-line, Fechar o caixa e Transmitir com a MESMA chave), a produção sem a marcação (nenhuma chamada), o CSC sem cofre e o CSC guardado no cofre sem tocar o arquivo nem a nota, o rascunho que sobrevive, o cancelamento com o aviso do prazo e o Retomar da off-line pela lista; uso manual pelo exemplo de demonstração | nenhuma NFC-e real, nenhuma SEFAZ real (nem a de homologação), nenhum certificado real e nenhum CSC real pela interface; a impressão em impressora real (o programa só gera o arquivo do cupom); a inutilização e a carta de correção (a NFC-e não tem carta, e a aplicação não inutiliza o modelo 65) e a via do estabelecimento da off-line não estão na janela; o cupom aberto no leitor de PDF de outra máquina; o lado Linux só pelo CI |
| interface de desktop, NF-e recebidas (`rustnf-gui`, só Windows: a opção Recebidas (compras) em Notas, buscar notas novas com a última busca e a espera, o detalhe do resumo e da nota completa, baixar com a ciência, o DANFE e as quatro respostas à nota) | testes do modelo (a lista com situação da nota e resposta da empresa separadas, busca e filtro, 2 mil registros; o texto de cada resultado de buscar, baixar e responder; as regras do diálogo conferidas contra a função da biblioteca em todas as combinações) e da janela no ambiente de teste do GPUI com um Ambiente Nacional de mentira: buscar traz as notas novas, a espera desliga o botão com o horário e clicar não chama nada, o resumo baixado com a ciência vira a nota completa com DANFE, cada uma das quatro respostas, o que a aplicação recusa antes de enviar, a produção sem a marcação (nenhuma chamada), a recusa do Ambiente Nacional e a resposta sem retorno; uso manual pelo exemplo de demonstração | nenhuma chamada real ao Ambiente Nacional (nem a de homologação) e nenhum certificado real pela interface; o prazo da resposta (o programa não o calcula e nenhuma fonte dele foi usada); a busca de documentos que o Ambiente Nacional guarda há mais de 90 dias; o lado Linux só pelo CI |
| interface de desktop (`rustnf-gui`, só Windows) | testes do modelo (máscaras, conferência, pedido, prévia, sugestão, rascunho, lista das notas, detalhe, cancelamento, conferência em lote, histórico de eventos, pedido de análise fiscal, confirmação e recusa de nota recebida, consulta pela chave e pela DPS e prévia avulsa), dos fluxos com emissor de demonstração e pasta temporária e da janela no ambiente de teste do GPUI; uso manual da janela em modo de depuração e pelo exemplo de demonstração | emitir, consultar ou cancelar uma nota real pela janela: nenhuma nota real foi emitida, consultada nem cancelada por esta interface, e nenhuma consulta de DPS ou de evento, nenhuma conferência de situação e nenhum evento (análise fiscal, confirmação, recusa) foi feito por ela contra a Sefin; só o emissor de demonstração respondeu; o DANFSe aberto no leitor de PDF de outra máquina; uma chamada real à Sefin pelo botão "Testar conexão"; exame da janela em outras escalas de tela; o executável de release aberto numa máquina sem as ferramentas de desenvolvimento |

Na NFS-e, se a assinatura for recusada, repita com `--assinatura sha1` e diga no relato qual das duas passou.
Correção é bem-vinda por pedido de mudança, com a fonte oficial citada e um caso no contrato (veja `CONTRIBUTING.md`).

## O que faz

| comando | o que faz |
|---|---|
| `rustnf dps montar` | lê o JSON de um pedido e escreve o XML da DPS (não assinado) |
| `rustnf dps conferir` | confere as regras do Anexo I e diz "ok" ou a recusa (campo e motivo) |
| `rustnf dps esquema` | imprime o JSON Schema do pedido da DPS |
| `rustnf calcular` | prévia da base de cálculo e do IBS/CBS (Reforma Tributária, LC 214/2025), como a calculadora da Receita Federal |
| `rustnf danfse` | gera o DANFSe em PDF (NT 008) a partir do XML da NFS-e autorizada, ou de uma nota da pasta de notas (`--chave`) |
| `rustnf emitir` | monta, assina com o certificado A1 e envia a DPS à Sefin Nacional (idempotente) |
| `rustnf consultar` | consulta uma NFS-e pela chave de acesso, ou a chave que uma DPS gerou (`--dps <id>` ou `--numero`) |
| `rustnf cancelar` | cancela uma NFS-e (evento 101101) |
| `rustnf evento consultar` | consulta um evento da NFS-e (chave, tipo e sequência) |
| `rustnf evento registrar` | registra um evento que o contribuinte pede, como o pedido de análise fiscal (101103) |
| `rustnf evento tipos` | lista os tipos de evento que o contribuinte registra, com a descrição e o autor |
| `rustnf serve` | sobe a API HTTP e o servidor MCP |
| `rustnf config mostrar` / `caminho` | a configuração efetiva e de onde veio; o arquivo em uso |
| `rustnf senha guardar` / `apagar` | guarda a senha do certificado no cofre do sistema (Windows) |
| `rustnf notas listar` | lista as notas guardadas na pasta de notas, com filtro por período, situação e texto; em texto, JSON ou CSV |
| `rustnf notas exportar` | copia para uma pasta os XML das notas de um mês e a lista delas em CSV (o que vai para a contabilidade) |
| `rustnf notas reindexar` | refaz o índice da pasta de notas a partir dos XML guardados |
| `rustnf notas atualizar` | confere na Sefin a situação das notas da pasta (cancelada, substituída, em análise fiscal) e atualiza o índice |
| `rustnf notas numero` | mostra a próxima DPS da série; com `--proximo`, continua a numeração de quem já emitia por outro sistema |
| `rustnf nfe montar` / `conferir` / `emitir` / `consultar` / `cancelar` / `corrigir` / `inutilizar` / `status` / `epec` / `pendentes` / `danfe` / `notas` | a NF-e (modelo 55): veja [NF-e](#nf-e-modelo-55) |
| `rustnf certificado mostrar` | mostra o titular, o documento, a validade e os avisos do certificado A1 |
| `rustnf sefin testar` | testa a conexão com a Sefin Nacional e a aceitação do certificado |

Nenhuma regra fiscal mora no binário: ele lê a entrada, chama a biblioteca (`crates/rustnf`) e escreve a saída. A
biblioteca monta a DPS (leiaute 1.01 de produção e, com `"leiaute": "nt009"`, o da NT 009 com os Releases 1 e 2, que a
produção restrita usa desde 24/11/2026), confere as regras antes do envio,
lê o retorno, calcula a prévia do IBS/CBS, gera o DANFSe e fala com a Sefin Nacional por mTLS com o A1 em memória.
Valores e percentuais usam inteiros de ponto fixo, nunca ponto flutuante. A DPS não leva IBS/CBS calculado (quem
calcula é a Sefin Nacional): `calcular` é só uma prévia; vale o que voltar na NFS-e.

A biblioteca tem também, atrás da feature `nfe`, a base da NF-e (modelo 55): a chave de acesso de 44 posições
(montar, conferir, decompor e achar em texto), o calendário das Notas Técnicas, a leitura do XML autorizado para uma
struct e a tabela de códigos de retorno (`cStat`) do MOC. Monta e assina o XML da NF-e 4.00 a partir de um pedido em JSON
(`rustnf::nfe::montagem`, formato em [docs/NFE_PEDIDO.md](docs/NFE_PEDIDO.md)), com as regras do MOC e das Notas Técnicas
conferidas antes e o IBS/CBS de mercadoria (inclusive o Simples Nacional da NT 2026.005). A montagem cobre também
o combustível (grupo `comb`), o medicamento e a rastreabilidade do item, a cobrança (`cobr`), o transporte completo
(transportadora, veículo, reboques, vagão, balsa, volumes e lacres), a exportação e a compra, e devolve avisos (PIS e
COFINS que não combinam com o regime informado; Simples sem regime regular e `gIBSCBS`) junto do XML. Com a feature `nfe_sefaz`, **a
biblioteca ENVIA a NF-e assinada**: `rustnf::nfe::sefaz` traz os endereços oficiais dos webservices por UF e ambiente,
o SOAP 1.2 com certificado A1, o status do serviço, a autorização (síncrona, e assíncrona até a data de corte da NT
2023.002 v1.02), o retorno do recibo e a consulta pela chave. Se a resposta se perde, o cliente consulta a chave antes de
qualquer reenvio e nunca produz nota com outra chave; o resultado diz em palavras se a nota foi autorizada, denegada,
rejeitada ou ficou sem resposta (veja `crates/rustnf/examples/nfe_sefaz.rs`). O mesmo cliente **registra eventos**:
cancelamento (o prazo é o da legislação da UF: fora dele a SEFAZ recusa com 501 ou homologa com 155, e o resultado diz
qual), carta de correção (a carta nova substitui as anteriores), EPEC no Ambiente Nacional e os eventos da apuração do
IBS/CBS da NT 2025.002, todos assinados com o A1; guarda o `procEventoNFe` pelo texto cru; e, se a resposta do evento
se perde, consulta a nota antes de qualquer reenvio (nunca cria um segundo cancelamento nem uma carta de sequência
nova por causa disso). **Inutiliza numeração** (`inutNFe`), e nas UF que acabaram com o serviço (AC, BA e SP desde
11/11/2026, NT 2023.002 v1.02) devolve o erro em palavras sem chamar a rede (`nfe::sefaz::inutilizacao::disponivel`
diz se a UF ainda tem o serviço). **Envia à contingência SVC**: a nota com `tpEmis` 6 ou 7 vai ao endereço da SVC-AN ou
da SVC-RS da UF (Ato COTEPE/ICMS 39/12), e a consulta e o cancelamento dela também (a SVC não registra carta de
correção nem inutiliza: MOC 7.00, Anexo III). Com a feature `danfe`, **gera o DANFE em PDF**
(`rustnf::danfe::pdf`, A4 retrato, várias folhas, MOC 7.00 Anexo II e o leiaute da NT 2026.010; veja
`crates/rustnf/examples/danfe.rs`). A NF-e tem a camada de aplicação (`rustnf-app`), a linha de comando (`rustnf nfe`, veja [NF-e](#nf-e-modelo-55)), a API HTTP (`/nfe/`) e o MCP, e a
interface de desktop (veja [Nova NF-e](#nova-nf-e-assistente-de-venda)); o modelo 65 (NFC-e) tem o seu caminho. A leitura não
confere a assinatura XML.

Com a mesma feature, a biblioteca **baixa os documentos de que um CNPJ ou CPF é interessado** (`rustnf::nfe::sefaz::dfe`, `NFeDistribuicaoDFe` do Ambiente Nacional) e **registra a manifestação do destinatário** (210200 confirmação, 210210 ciência, 210220 desconhecimento e 210240 operação não realizada, `ClienteSefaz::manifestar`). A distribuição devolve cada documento aberto (gzip com limite de tamanho) e lido: resumo da NF-e (`resNFe`), NF-e completa (lida pelo mesmo leitor da NF-e), resumo de evento e evento; o que não é reconhecido ou não abre volta cru, nunca é descartado. O `Cursor` guarda o último NSU, a espera de uma hora (depois do 137, do `ultNSU` igual ao `maxNSU` e do 656) e as lacunas: a biblioteca decide o próximo NSU e quando esperar, nunca pula nem repete um NSU e fecha a lacuna por `consNSU`; a aplicação só persiste o cursor, a cada lote, junto dos documentos (veja `crates/rustnf/examples/nfe_dfe.rs`). A aplicação, a linha de comando (`rustnf nfe recebidas`), a API HTTP e o MCP também as têm: veja [Documentos recebidos](#documentos-recebidos).

## Instalação

### Baixar pronto

Cada versão publicada traz, na página de Releases, um `.zip` para Windows (64 bits) com `rustnf.exe` (linha de comando)
e `rustnf-gui.exe` (interface de desktop), e um `.tar.gz` com a linha de comando para Linux. Não há instalador:
descompacte e execute. As versões anteriores continuam na mesma página, para quem preferir. No Windows (10 ou 11)
não é preciso instalar mais nada: a biblioteca C vai dentro do executável. Para emitir você precisa do seu certificado
A1 (`.pfx`) e da senha dele.

O executável **não tem assinatura de código**. No primeiro uso o Windows pode mostrar "O Windows protegeu o
computador" (SmartScreen): clique em "Mais informações" e depois em "Executar assim mesmo". Antes, confira o
SHA-256 do arquivo baixado com o `SHA256SUMS.txt` da mesma release (no PowerShell:
`Get-FileHash arquivo.zip -Algorithm SHA256`).

### Compilar

Precisa do Rust estável 1.92 ou mais novo: é a versão mínima que as dependências declaram. O projeto é compilado e
testado com a versão estável atual.

```
git clone https://github.com/Admnwk/rustnf
cd rustnf
cargo build --release -p rustnf-cli
```

O executável fica em `target/release/rustnf` (`rustnf.exe` no Windows). A interface de desktop é outro executável, só
para Windows (veja [Interface de desktop](#interface-de-desktop)). A biblioteca e a camada de aplicação
(`crates/rustnf-app`: operações, configuração, cofre e pasta de notas, sem tela nem servidor) são compartilhadas pelo
binário e por qualquer interface que use o mesmo arquivo de configuração. Não é preciso nenhum programa externo: o
DANFSe em PDF é escrito pelo próprio binário, com as fontes embutidas.

## Linha de comando

Convenções: dado no stdout, diagnóstico no stderr; `--json` onde a saída padrão é texto; onde se espera um arquivo,
`-` lê do stdin. Códigos de saída:

| código | significado |
|---|---|
| 0 | ok |
| 1 | falha interna |
| 2 | erro de uso ou de entrada (opção errada, JSON inválido, arquivo ausente, certificado que não abre) |
| 3 | recusa de regra (a DPS não passa no Anexo I, a prévia não pode calcular, a Sefin rejeitou) |
| 4 | erro de rede ou resposta de erro da Sefin Nacional |

Os exemplos abaixo usam os arquivos de `crates/rustnf-cli/exemplos/` (só documentos fictícios).

Para completar comandos e opções com Tab, gere o script do seu shell (powershell, bash, zsh, fish ou elvish):

```sh
rustnf completar powershell | Out-String | Invoke-Expression   # PowerShell; ponha no $PROFILE para valer sempre
source <(rustnf completar bash)                                 # bash
```

### Montar e conferir a DPS

```
$ rustnf dps montar crates/rustnf-cli/exemplos/dps.json
<?xml version="1.0" encoding="UTF-8"?><DPS xmlns="http://www.sped.fazenda.gov.br/nfse" versao="1.01"><infDPS Id="DPS3550308..."> ...

$ rustnf dps conferir crates/rustnf-cli/exemplos/dps.json
ok

$ rustnf dps conferir pedido-com-erro.json --json     # saida 3
{"campo": "infDPS/prest/regTrib/regApTribSN", "codigo": "recusa", "mensagem": "...", "ok": false}
```

O JSON do pedido tem a forma de `rustnf::nfse::Dps` (veja `crates/rustnf/tests/casos/nfse_nacional.json` para dezenas
de casos). Valores aceitam número ou texto. `rustnf dps esquema` imprime o JSON Schema do pedido, com a descrição de
cada campo (`--emissao` para o do `emitir`, que não exige o que a configuração completa).

**Campo de nome errado.** Um campo que a DPS não conhece (erro de digitação, campo no grupo errado) não entra no XML.
O `emitir` o **recusa** (código 3) antes de abrir o certificado e de qualquer envio: uma nota sem o desconto que se
quis dar tem valor fiscal. Para emitir mesmo assim, ignorando o campo, use `--ignorar-desconhecidos`. `dps montar` e
`dps conferir` só avisam no stderr, com o caminho (`valores.desconto_incondicional`), e a saída `--json` traz a lista em
`campos_desconhecidos`; com `--estrito`, eles também recusam:

```
$ rustnf dps conferir pedido.json
ok
aviso: campo desconhecido no pedido, ignorado: valores.desconto_incondicional (confira o nome)

$ rustnf dps conferir pedido.json --estrito          # saida 3
erro: valores.desconto_incondicional: campo desconhecido no pedido: confira o nome
```

### Prévia do IBS/CBS

```
$ rustnf calcular crates/rustnf-cli/exemplos/calculo.json
{
  "bases": { "regime_regular": "80.00", "simples": null, "avisos": [], "erros": [] },
  "recusado": false,
  "tributos": { "base": "80.00", "ibs": "0.08", "cbs": { "aliquota": "0.90", "devido": "0.72", ... }, ... }
}
```

O JSON tem `{"base": {...}}` e/ou `{"tributos": {...}}`. Se vierem os dois e `tributos.base` faltar, vale a base do
regime regular calculada. Se a base registrar recusas de regra (`bases.erros`), a conta sai mesmo assim, `recusado` é
`true` e o código de saída é 3.

### DANFSe

```
$ rustnf danfse crates/rustnf-cli/exemplos/nfse.xml -o danfse.pdf
DANFSe gravado: 116902 bytes
$ rustnf danfse nfse.xml -o danfse.pdf --situacao cancelada --sem-canhoto
```

`--situacao` aceita `valida` (padrão), `cancelada` e `substituida` (marca d'água da NT 008). `-o -` escreve o PDF no
stdout.

Com a [pasta de notas](#pasta-de-notas) configurada, basta a chave de acesso (colada com ou sem espaços):

```
$ rustnf danfse --chave <chave>
<pasta>/2026/10/<chave>.pdf
$ rustnf danfse --chave <chave> -o danfse.pdf
```

O PDF sai do XML guardado, com a marca de cancelada quando a nota está cancelada na pasta. Sem `-o`, ele fica ao lado
do XML e o caminho dele sai no stdout.

### Emitir, consultar e cancelar

Precisam do certificado A1 (`.pfx`/`.p12`), do ambiente e da senha. **A senha nunca vem por argumento** (apareceria na
lista de processos e no histórico do shell): só por variável de ambiente (`--senha-env`) ou pela primeira linha do
stdin (`--senha-stdin`).

```
$ export SENHA_A1='...'
$ rustnf emitir pedido.json --pfx certificado.pfx --senha-env SENHA_A1 --ambiente restrita
autorizada
chave: <50 posicoes>
numero: 77

$ printf '%s\n' "$SENHA_A1" | rustnf consultar --chave <chave> --pfx certificado.pfx --senha-stdin --ambiente restrita --json

$ rustnf cancelar --chave <chave> --motivo 1 --justificativa "Valor digitado errado na nota" \
    --pfx certificado.pfx --senha-env SENHA_A1 --ambiente restrita
```

* `--ambiente producao|restrita` prevalece sobre o `ambiente` do pedido. Sem arquivo de configuração ele é obrigatório;
  com arquivo (ou `RUSTNF_AMBIENTE`), vale o `[sefin].ambiente`. `--pfx` segue a mesma regra.
* Sem `--senha-env` nem `--senha-stdin`, a senha vem do cofre do sistema, se houver uma guardada (veja
  [Configuração](#configuração)).
* Sem `emissao` (`dhEmi`) no pedido, vale "agora menos 60 s" (`--folga-relogio`), para o relógio local adiantado não
  cair na regra E0008.
* A emissão é idempotente: resposta perdida, erro 5xx ou E0014 fazem a biblioteca consultar a DPS antes de reenviar o
  MESMO XML. Nunca renumere a DPS (`numero`) para "tentar de novo".
* A DPS e os eventos são assinados com RSA e SHA-256. `--assinatura sha1` (ou `RUSTNF_ASSINATURA`, ou
  `[sefin].assinatura`) troca para SHA-1; o leiaute não fixa o algoritmo.
* Motivos de cancelamento: `1` erro na emissão, `2` serviço não prestado, `9` outros; a justificativa é obrigatória.
* `--json` imprime o resultado completo (inclui a NFS-e lida e os alertas); sem ele, um resumo.
* O pedido com campo desconhecido é recusado antes de abrir o certificado; `--ignorar-desconhecidos` emite mesmo
  assim, sem o campo (veja [Montar e conferir a DPS](#montar-e-conferir-a-dps)).

## Configuração

Um arquivo TOML único, lido pela linha de comando e por qualquer outra interface. Exemplo comentado, só com dados
fictícios: [`crates/rustnf-cli/exemplos/rustnf.toml`](crates/rustnf-cli/exemplos/rustnf.toml).

**Onde ele mora.** `--config <arquivo>` (arquivo inexistente é erro); sem a opção, a variável de ambiente
`RUSTNF_CONFIG` (inexistente também é erro); sem ela, o local padrão: no Windows `%APPDATA%\rustnf\rustnf.toml`, nos
outros sistemas `$XDG_CONFIG_HOME/rustnf/rustnf.toml` ou `$HOME/.config/rustnf/rustnf.toml`. Se nem o local padrão
existir, valem os padrões e nada muda em relação a usar só opções. `rustnf config caminho` mostra o que vale e
`rustnf config mostrar [--json]` mostra a configuração efetiva e de onde veio.

**Precedência**, em todo parâmetro: opção da chamada > variável de ambiente `RUSTNF_*` > arquivo > padrão. As
variáveis: `RUSTNF_AMBIENTE`, `RUSTNF_PFX`, `RUSTNF_ASSINATURA`, `RUSTNF_TEMPO_LIMITE_SEFIN`, `RUSTNF_FOLGA_RELOGIO`,
`RUSTNF_SERIE`, `RUSTNF_NOTAS_PASTA`, `RUSTNF_CANHOTO`, `RUSTNF_ENDERECO`, `RUSTNF_PORTA`, `RUSTNF_LIMITE_CORPO`,
`RUSTNF_TEMPO_LIMITE` e `RUSTNF_ORIGENS_PERMITIDAS` (separadas por vírgula); para a NF-e, `RUSTNF_NFE_AMBIENTE`, `RUSTNF_NFE_SERIE`, `RUSTNF_NFE_CONTINGENCIA` e `RUSTNF_NFE_TEMPO_LIMITE`. Campo desconhecido no arquivo é recusado,
e os valores são conferidos (ambiente, código do município, CNPJ/CPF, números, anos de alíquota repetidos...) com a
seção e o campo no erro.

| seção | campos |
|---|---|
| `[emitente]` | `cnpj` ou `cpf`, `nome`, `inscricao_municipal`, `municipio` (IBGE, 7 dígitos), `op_simples` (1, 2 ou 3), `regime_apuracao` (0 = não informar), `regime_especial`, `serie`, `aliquota_iss` (vazio = não informar) |
| `[sefin]` | `ambiente` (`producao` ou `restrita`), `pfx`, `assinatura` (`sha256` ou `sha1`), `tempo_limite_s`, `folga_relogio_s` |
| `[notas]` | `pasta` (vazio = não guardar) |
| `[danfse]` | `canhoto` |
| `[nfe]`, `[nfe.endereco]`, `[nfe.urls]` | a NF-e: `ambiente`, `uf`, `ie`, `crt`, `regime_pis_cofins`, `serie`, `contingencia`, `motivo_contingencia`, `fantasia`, `tempo_limite_s`, `tentativas_retorno`, `intervalo_retorno_s`, `canhoto`, `ca_pem`, o endereço e os endereços do simulador; veja [NF-e](#nf-e-modelo-55) |
| `[servidor]` | `endereco`, `porta`, `limite_corpo`, `tempo_limite_s`, `origens_permitidas` |
| `[interface]` | `tema` (`sistema`, `claro` ou `escuro`; padrão `sistema`): preferência da interface de desktop, a linha de comando não a usa |
| `[[aliquotas]]` | `ano`, `cbs_referencia`, `cbs`, `ibs_uf`, `ibs_mun`, `fonte` (uma tabela por ano) |

* **`[emitente]`** completa o pedido da DPS no `emitir` (linha de comando, API HTTP e MCP), só nos campos que o pedido
  não traz (nunca sobrescreve): `prestador.documento`, `nome`, `im`, `op_simples`, `apuracao_sn`, `regime_especial`,
  `cmun_emissor` e `serie`. A `aliquota_iss` (ex. `2.00`) completa `valores.municipal.aliquota` só onde a DPS leva a
  alíquota: ISS tributável, sem regime especial, fora do MEI e, para ME/EPP, com o ISS retido. Configure-a só se o seu
  município não for conveniado ao sistema nacional (ou se for ME/EPP com ISS retido): em município conveniado a Sefin
  usa a alíquota do município e recusa a informada. Na interface, a Nova nota mostra o ISS estimado no resumo.
* **`[[aliquotas]]`** valem no `calcular` (linha de comando, API HTTP e MCP) quando o JSON não traz `publicadas`: sem
  elas, a prévia de 2027 em diante usa o que a biblioteca assume; com elas, usa o número publicado.
* **`[servidor]`** é o que `rustnf serve` usa quando a opção não é dada.
* **`[interface]`** guarda só o tema da interface de desktop; `sistema` segue o tema do Windows.
* **Sem segredo no arquivo.** Não existe campo de senha nem de token; `senha` ou `token` no arquivo é recusado como
  campo desconhecido.

### Cofre da senha do certificado

A senha do certificado vem de `--senha-env`, de `--senha-stdin` ou, na falta das duas, do **cofre do sistema**. No
Windows é o Gerenciador de Credenciais (serviço `rustnf`; o item é identificado pelo CNPJ do `[emitente]` ou, sem
ele, pelo nome do arquivo `.pfx`). Nos outros sistemas o cofre não é usado: continue com variável de ambiente ou stdin.
`RUSTNF_COFRE=desligado` desliga o cofre.

```
$ rustnf senha guardar --config rustnf.toml --senha-env SENHA_A1     # confere que a senha abre o .pfx e grava
$ rustnf emitir pedido.json --config rustnf.toml                    # sem --senha-env: usa o cofre
$ rustnf senha apagar --config rustnf.toml
```

A senha continua **impossível** de passar por argumento.

### Conferir o certificado e a conexão

```
$ rustnf certificado mostrar --config rustnf.toml
Certificado OK: OFICINA TESTE (11.***.***/****-81), valido ate 20/11/2027.
titular: OFICINA TESTE
documento: 11222333000181
emissor: AC EXEMPLO (ICP-Brasil)
validade: de 2026-11-20 a 2027-11-20 (364 dias)

$ rustnf sefin testar --config rustnf.toml
Conexao com o SEFIN ok, certificado aceito.
ambiente: restrita
```

* **`certificado mostrar`** abre o `.pfx` e não usa a rede. Os avisos (vence em 30 dias ou menos, não é da
  ICP-Brasil, é de outro CNPJ que o do `[emitente]`) saem no stderr. O CPF do titular aparece mascarado. Sai com
  código 3 quando o certificado não assina hoje (vencido ou ainda não válido), o que serve para um aviso agendado:
  com `--json`, o campo `dias_restantes` diz quanto falta.
* **`sefin testar`** faz **uma chamada real** à Sefin Nacional do ambiente configurado: a consulta de uma chave que
  não existe. A resposta "não encontrada" prova que a conexão e o certificado foram aceitos; nada é emitido nem
  alterado. Sai com código 3 se o certificado não assina hoje (nesse caso nada vai para a rede) e com 4 se a Sefin
  não respondeu ou recusou o certificado.

As duas aceitam `--pfx`, `--senha-env`, `--senha-stdin` e `--json`, com as mesmas regras de
### Consultar a DPS, eventos e a situação das notas

Mesmas opções de certificado, senha e ambiente de `emitir`, `consultar` e `cancelar`; `--json` onde há saída em texto.
Estes comandos foram conferidos com transporte de teste; a chamada à Sefin Nacional real é o que o teste real prova
(veja [Estado](#estado)).

```
# a chave que uma DPS gerou: pelo Id (45 posicoes) ou pela serie e o numero (o Id sai do [emitente] da configuracao)
$ rustnf consultar --dps <id da DPS> --pfx certificado.pfx --senha-env SENHA_A1 --ambiente restrita
$ rustnf consultar --serie 1 --numero 42 --com-nota --pfx certificado.pfx --senha-env SENHA_A1 --ambiente restrita

# um evento da nota (a sequencia padrao e' 1) e os tipos que o contribuinte registra
$ rustnf evento tipos
$ rustnf evento consultar --chave <chave> --tipo 101103 [--sequencia 2] --pfx certificado.pfx --senha-env SENHA_A1 --ambiente restrita

# pedido de analise fiscal para cancelamento (101103); com [notas].pasta, a nota passa a "em analise fiscal"
$ rustnf evento registrar --chave <chave> --tipo 101103 --motivo 1 --justificativa "Valor digitado errado na nota" \
    --pfx certificado.pfx --senha-env SENHA_A1 --ambiente restrita

# confere na Sefin a situacao das notas da pasta, de uma em uma
$ rustnf notas atualizar --de 2026-10-01 --ate 2026-10-31 --pfx certificado.pfx --senha-env SENHA_A1 --ambiente restrita
```

* `consultar --dps` e `--numero` devolvem a chave da NFS-e que a DPS gerou; com `--com-nota`, a nota também. DPS sem
  NFS-e sai com 4 (não encontrada).
* `evento consultar` só dá sucesso quando o evento devolvido é o da chave e do tipo pedidos; o evento que não existe
  sai com 4.
* `evento registrar` recusa (saída 3), antes de abrir o certificado, o cancelamento (101101, que tem o comando
  `cancelar` e marca a pasta de notas) e os eventos que o fisco ou o próprio sistema registram (105102, 105104,
  105105, 305101...). `evento tipos` mostra o que sobra, com a descrição e o autor. O autor do evento é o titular do
  certificado. Quando a resposta do pedido se perde, a biblioteca procura o evento pelo `dhEvento` do pedido em todas
  as sequências do tipo, e não aceita um evento antigo no lugar.
* `notas atualizar [--de --ate --situacao --busca] [--intervalo-ms N]` consulta uma nota e uma consulta por vez, com
  um intervalo entre elas (200 ms por padrão, cortesia com o serviço). A regra, parando no primeiro evento que decide:
  101101 ou 105104, cancelada; 105102, substituída; 101103 sem decisão (mais pedidos do que 105105), em análise fiscal;
  senão, autorizada, e a Sefin precisa confirmar que a nota existe. Imprime o que mudou e o que falhou e sai com 4 se
  alguma nota não pôde ser conferida (sem resposta, recusa, nota não encontrada). Uma nota que a pasta dá como
  cancelada ou substituída e que a Sefin não mostra assim **nunca volta para autorizada**: ela vai para as falhas, como
  divergência, e o índice não muda.

### Pasta de notas

Com `[notas].pasta`, o `emitir` guarda cada nota autorizada, sem banco de dados:

```
<pasta>/<AAAA>/<MM>/<chave>.xml   o XML da NFS-e autorizada que a Sefin devolve
<pasta>/indice.json               chave, numero, serie, data, valor, situacao, documento e nome do tomador
<pasta>/estado.json               o ultimo numero de DPS usado em cada serie
<pasta>/<AAAA>/<MM>/<chave>.pdf   o DANFSe, quando ja foi gerado para a nota (nao entra no indice)
```

Sem `numero` no pedido, o `emitir` usa o próximo da série. O número só avança em nota autorizada ou recuperada
(rejeitada ou pendente não gasta número) e nunca volta atrás. Nota recuperada é a que já existia na Sefin com
aquela série e aquele número de DPS: se os dados dela não forem os do pedido (sobra de uma tentativa anterior), a
resposta traz `recuperada_difere` com o que difere (tomador, valor, código do serviço, competência) e a nota do pedido
NÃO foi emitida; emita de novo, que o número já avançou. A situação de cada nota no índice é `autorizada`, `cancelada`, `substituida` ou `analise_fiscal` (um índice com as duas últimas não é lido pela versão 0.4.0). O `cancelar` marca a nota como cancelada no índice; o `evento registrar` de um 101103 a marca em análise fiscal; o `notas atualizar` confere todas na Sefin; se a Sefin
recusar o pedido e a nota já tiver o cancelamento registrado (envio anterior sem resposta, ou feito em outro lugar), a
resposta é `registrado` com `recuperada: true`, e as mensagens da recusa vêm em `alertas`.
As gravações são atômicas (arquivo temporário e `rename`); o nome do arquivo vem só da chave de acesso conferida. A
emissão e o cancelamento pela API HTTP e pelo MCP usam a mesma pasta, do mesmo jeito.

```
$ rustnf notas listar                                       # todas, da mais recente para a mais antiga
$ rustnf notas listar --de 2026-10-01 --ate 2026-10-31      # um periodo (os dois dias inclusive)
$ rustnf notas listar --situacao cancelada --busca "padaria"
$ rustnf notas listar --csv > notas.csv                     # para planilha
$ rustnf notas exportar --mes 2026-10 --para contador/2026-10
$ rustnf notas reindexar
$ rustnf notas numero --proximo 1501                       # ja emitia por outro sistema: a ultima DPS foi a 1500
```

* **`listar`** filtra por período (`--de`, `--ate`), por situação (`--situacao autorizada|cancelada`) e por texto
  (`--busca`: nome do tomador sem acento, CPF/CNPJ dele, número da NFS-e ou da DPS, ou uma chave de acesso colada); os
  filtros valem juntos. `--json` e `--csv` trocam a saída. O CSV usa `;` como separador, vírgula decimal no valor e uma
  linha de títulos; texto que uma planilha leria como fórmula ganha um apóstrofo na frente.
* **`exportar`** copia para a pasta de destino o XML de cada nota do mês (`<chave>.xml`) e a lista delas
  (`notas.csv`). São cópias: nada sai da pasta de notas.
* **`reindexar`** refaz o `indice.json` a partir dos XML guardados, para quando o índice se perde ou fica ilegível. O
  índice que havia fica copiado como `indice.anterior.json`. O XML não diz se a nota foi cancelada: a marca de
  cancelada é aproveitada do índice anterior quando ele ainda é legível; se não for, todas voltam como autorizadas e o
  comando avisa. A numeração da DPS não muda.
* **`numero`** mostra a próxima DPS da série (a do `[emitente]`, ou `--serie`). Quem já emitia por outro sistema passa
  `--proximo` com o número seguinte ao da última DPS de lá: a série continua dali. A numeração só anda para a frente,
  porque um número já usado faria a Sefin devolver a nota antiga em vez de autorizar a nova. Na interface:
  Configurações > Notas e DANFSe > "Continuar deste número".

## NF-e (modelo 55)

A NF-e usa a mesma camada de aplicação da NFS-e (`crates/rustnf-app`): o mesmo arquivo de configuração, o mesmo
certificado A1 e a mesma senha no cofre, a mesma pasta de notas e a mesma gravação atômica. A biblioteca monta, assina,
envia e entende a resposta da SEFAZ; a linha de comando `rustnf nfe` completa o pedido com a configuração, numera pela
pasta, guarda o que a SEFAZ devolve e diz o resultado em palavras.

Foi conferida com transporte de teste e com um servidor simulado; a primeira NF-e aceita por uma SEFAZ real é a prova
que falta, e a homologação é o lugar de tentar (veja [Estado](#estado)). A NF-e também tem API HTTP e MCP no mesmo servidor (veja [NF-e pela API](#nf-e-pela-api) e o [MCP](#mcp)); **a interface de desktop da NF-e emite por um assistente de cinco passos** (veja [Nova NF-e](#nova-nf-e-assistente-de-venda)). A NFC-e (modelo 65) tem a sua seção, [NFC-e (modelo 65)](#nfc-e-modelo-65): o `rustnf nfe` recusa o pedido do modelo 65 e diz que ele se emite por `rustnf nfce`.

### Passo a passo

1. **Configure.** No mesmo `rustnf.toml` da NFS-e, acrescente a seção `[nfe]` com a UF, a inscrição estadual, o regime
   (`crt`) e o endereço do emitente (o CNPJ, o nome e o município continuam sendo os do `[emitente]`, e o certificado é o
   do `[sefin].pfx`):

   ```toml
   [nfe]
   uf = "SP"
   ie = "111111111111"
   crt = 1                      # 1 Simples Nacional, 3 regime normal, 4 MEI
   [nfe.endereco]
   logradouro = "Rua das Flores"
   numero = "100"
   bairro = "Centro"
   municipio = "Sao Paulo"
   cep = "01001000"
   ```

   O ambiente começa em **homologação** (`tpAmb` 2): a nota de teste não vale. A senha do certificado vai para o cofre
   com `rustnf senha guardar`, como na NFS-e. O arquivo de exemplo, com todos os campos, é
   [`crates/rustnf-cli/exemplos/rustnf.toml`](crates/rustnf-cli/exemplos/rustnf.toml).
2. **Escreva o pedido** em JSON (campos em [docs/NFE_PEDIDO.md](docs/NFE_PEDIDO.md)). O que o pedido não traz vem da
   configuração: o emitente, a UF, o ambiente, a série, o número (o próximo da pasta), o `cNF` e a data de emissão.
3. **Confira, sem rede e sem gastar número:** `rustnf nfe conferir pedido.json`. Diz "ok" ou a recusa com o campo e o
   código da rejeição que a SEFAZ devolveria, e mostra os avisos (por exemplo, PIS e COFINS que não combinam com o
   regime). `rustnf nfe montar pedido.json` escreve só o XML, sem assinar.
4. **Veja se o serviço está no ar:** `rustnf nfe status`.
5. **Emita em homologação:** `rustnf nfe emitir pedido.json --senha-env SENHA_A1`. A NF-e assinada é gravada na pasta
   como **pendente antes** de ir à rede; a resposta da SEFAZ fecha a nota (`autorizada`, `denegada`) ou, se foi
   rejeitada, tira a nota da pasta e devolve o número.
6. **Gere o DANFE:** `rustnf nfe danfe --chave <chave>` grava o PDF ao lado do XML, na pasta de notas.
7. **Se a conexão cair no meio:** a nota fica `pendente` e o comando sai com código 4. **Não emita outra nota no lugar:**
   rode `rustnf nfe pendentes --retomar`. Ele consulta a mesma chave e, se a SEFAZ diz que nunca recebeu, reenvia a mesma
   NF-e assinada que está na pasta. Não nasce nota gemea.
8. **Cancele ou corrija:** `rustnf nfe cancelar --chave <chave> --justificativa "..."` (de 15 a 255 caracteres; o prazo é o
   da legislação da UF) e `rustnf nfe corrigir --chave <chave> --texto "..."` (carta de correção: a sequência 1, 2, 3 sai da
   pasta, e **a carta nova substitui as anteriores**, então o texto leva todas as correções que ainda valem).
9. **Número pulado:** `rustnf nfe notas numero` mostra o próximo e os números pulados; os que não vão mais ser usados se
   inutilizam com `rustnf nfe inutilizar --inicio 2 --fim 3 --justificativa "..."`.

Só depois de tudo certo em homologação troque o ambiente (`--ambiente producao`, `RUSTNF_NFE_AMBIENTE` ou
`[nfe].ambiente`): a partir daí cada nota tem valor fiscal.

### Comandos

| comando | o que faz |
|---|---|
| `rustnf nfe montar` | monta o XML da NF-e a partir do pedido (sem assinar, sem rede, sem gastar número) |
| `rustnf nfe conferir` | confere as regras do MOC e das Notas Técnicas e mostra os avisos; sem rede |
| `rustnf nfe emitir` | completa, confere, monta, assina, grava como pendente, envia e grava o resultado |
| `rustnf nfe consultar` | consulta a NF-e pela chave e põe a nota da pasta em dia com o que a SEFAZ diz |
| `rustnf nfe cancelar` | cancela a NF-e (evento 110111) |
| `rustnf nfe corrigir` | registra uma carta de correção (evento 110110), com a sequência tirada da pasta |
| `rustnf nfe inutilizar` | inutiliza uma faixa de numeração (a UF que acabou com o serviço devolve o erro sem chamar a rede) |
| `rustnf nfe status` | o estado do serviço da SEFAZ (ou da SVC, com `--svc`) |
| `rustnf nfe epec` | emite em contingência EPEC: registra o evento no Ambiente Nacional; a mesma NF-e é transmitida depois. Se a venda tinha uma NF-e que ficou sem resposta, `--substitui <chave>` (a chave dela, da lista de `pendentes`): o EPEC sai com o próximo número livre, como o Manual de Contingência manda, e a nota sem resposta nunca mais é reenviada |
| `rustnf nfe pendentes` | lista as notas pendentes e em EPEC; com `--retomar`, consulta, reenvia a mesma NF-e ou transmite a do EPEC. A nota que um EPEC substituiu só é consultada, e a retomada diz o que fazer com ela: **cancelar** (se a SEFAZ a autorizou) ou **inutilizar** o número (se a SEFAZ não a tem, e então `rustnf nfe inutilizar` aceita esse número) |
| `rustnf nfe danfe` | gera o DANFE em PDF do XML ou de uma nota da pasta (`--chave`) |
| `rustnf nfe recebidas sincronizar` / `listar` / `baixar` / `manifestar` / `cursor` | as NF-e recebidas (distribuição de DF-e) e a manifestação do destinatário; veja [Documentos recebidos](#documentos-recebidos) |
| `rustnf nfe notas listar` / `exportar` / `reindexar` / `numero` | a pasta de notas da NF-e, como `rustnf notas` na NFS-e; `listar --situacao inutilizada` lista as faixas |

Opções das ações que falam com a SEFAZ: `--pfx`, `--senha-env` ou `--senha-stdin` (ou a senha do cofre), `--ambiente
homologacao|producao`, `--json`. A NF-e é sempre assinada em SHA-1 (o único algoritmo que o esquema da NF-e aceita), então
não há `--assinatura`. Códigos de saída de `rustnf nfe`:

| código | significado |
|---|---|
| 0 | nota autorizada, evento registrado, faixa inutilizada, serviço em operação, retomada sem pendência |
| 2 | erro de uso ou de entrada (opção errada, JSON inválido, falta de configuração, número já gasto) |
| 3 | recusa de regra: o pedido que a conferência recusa, nota rejeitada ou denegada, cancelamento fora do prazo |
| 4 | sem resposta da SEFAZ ou serviço parado: a nota **pode** ter sido autorizada; consulte, nunca emita outra |

Com `--json`, o resultado completo sai no stdout mesmo quando o código de saída é 3 ou 4.

### Tabela da `[nfe]`

Campo desconhecido é recusado e os valores são conferidos, com a seção e o campo no erro. O que a NF-e compartilha com
a NFS-e **não** se repete aqui: o CNPJ ou CPF e o nome (`[emitente]`), o município (`[emitente].municipio`, que também é o
`cMun` do endereço quando `[nfe.endereco].cod_municipio` está vazio e o `cMunFG`), o certificado (`[sefin].pfx` e o cofre
da senha), a folga do relógio (`[sefin].folga_relogio_s`, tirada do `dhEmi` que o pedido não traz) e a pasta
(`[notas].pasta`, com a subpasta `nfe/`). O ambiente, a série, o tempo limite e o canhoto têm campo próprio porque a NFS-e
usa outro serviço, outra numeração e outro documento.

| campo | o que é | padrão |
|---|---|---|
| `ambiente` | `homologacao` ou `producao`; variável `RUSTNF_NFE_AMBIENTE` | `homologacao` |
| `uf` | sigla da UF do emitente: escolhe o autorizador e vai no `cUF` da chave | (obrigatório para emitir) |
| `ie` | inscrição estadual, só dígitos | vazio |
| `crt` | 1 Simples Nacional, 2 Simples com excesso de sublimite, 3 regime normal, 4 MEI | 0 (a emissão recusa) |
| `regime_pis_cofins` | `cumulativo` ou `nao_cumulativo` (CRT 3): liga o aviso de CST ou alíquota que não combina | vazio |
| `serie` | série da NF-e, de 0 a 999; variável `RUSTNF_NFE_SERIE` | `1` |
| `contingencia` | `normal` ou `svc` (SEFAZ Virtual de Contingência, `tpEmis` 6 ou 7 conforme a UF); variável `RUSTNF_NFE_CONTINGENCIA` | `normal` |
| `motivo_contingencia` | de 15 a 255 caracteres; obrigatório com `svc` e usado no EPEC | vazio |
| `fantasia` | nome fantasia (`xFant`) | vazio |
| `tempo_limite_s` | tempo limite de cada chamada, em segundos; variável `RUSTNF_NFE_TEMPO_LIMITE` | 60 |
| `tentativas_retorno` | consultas do recibo de um lote que segue em processamento | 5 |
| `intervalo_retorno_s` | espera entre elas (a SEFAZ rejeita consulta em rajada) | 3 |
| `canhoto` | canhoto de recebimento no DANFE | `true` |
| `ca_pem` | arquivo PEM com uma raiz de confiança a mais (simulador ou homologação própria) | vazio |
| `[nfe.urls]` | endereços que trocam os da tabela oficial, por serviço: `status`, `autorizacao`, `retorno`, `consulta`, `eventos`, `inutilizacao`, `distribuicao` (`https`, ou `http` só no próprio computador) | vazio |
| `[nfe.endereco]` | `logradouro`, `numero`, `complemento`, `bairro`, `cod_municipio`, `municipio`, `cep`, `fone` | vazio |

### Pasta de notas da NF-e

Dentro da mesma `[notas].pasta` da NFS-e, separada dela (a NFS-e não lê a subpasta `nfe/` e a NF-e não toca nos arquivos
da NFS-e, nem nos da versão 0.4.0):

```
<pasta>/nfe/<AAAA>/<MM>/<chave>-procNFe.xml                     o nfeProc: autorizada ou denegada
<pasta>/nfe/<AAAA>/<MM>/<chave>-NFe.xml                         a NF-e assinada, gravada ANTES de ir à rede (pendente)
<pasta>/nfe/<AAAA>/<MM>/<chave>-<tipo>-<seq>-procEventoNFe.xml  cada evento registrado (cancelamento, carta, EPEC)
<pasta>/nfe/inutilizacao/<AAAA>/<serie>-<ini>-<fim>-procInutNFe.xml   cada faixa inutilizada
<pasta>/nfe/indice.json                                         uma linha por nota e por faixa
<pasta>/nfe/estado.json                                         o último número consumido em cada série
<pasta>/nfe/<AAAA>/<MM>/<chave>-procNFe.pdf                     o DANFE, quando já foi gerado (não entra no índice)
```

A situação no índice é `autorizada`, `denegada`, `cancelada`, `pendente`, `inutilizada` (as faixas) ou `epec`. As
gravações são atômicas (arquivo temporário e `rename`) e protegidas por uma trava; o nome de cada arquivo vem só da
chave de acesso conferida.

**Regras de numeração** (a fonte é o MOC 7.00, que o repositório lista em `docs/FONTES.md`):

* Nota **autorizada** ou **denegada** gasta o número: a SEFAZ o guarda e recusa outro igual (Anexo I, regras 2B08-10 e
  2B08-20, rejeição 539 para o mesmo número com outra chave; 2B08-40, rejeição 205, para a denegada). A nota cancelada
  também continua gastando o número (2B08-30, rejeição 218).
* Nota **rejeitada** não gasta: a regra de duplicidade só olha a NF-e "já cadastrada", e o MOC (Visão Geral, 4.2, consumo
  indevido) manda corrigir a causa e reenviar. A rejeitada sai da pasta e o número volta a ficar livre. O MOC não diz isso
  numa frase só: é o que as duas regras juntas mostram.
* Número **pulado**, que não vai mais ser usado, se **inutiliza** (Visão Geral, 5.3; a rejeição 206 vale para quem tentar
  emiti-lo depois, regra 3B08-100).
* Nota **pendente** (enviada sem resultado) conta como gasta até se saber: ela pode ter sido autorizada. Por isso a
  NF-e assinada é gravada antes do envio.
* Quando a SEFAZ responde 539 (o número já foi autorizado com outra chave), o número é consumido sem nota na pasta: a nota
  que vale é a outra.

### Documentos recebidos

As NF-e que OUTRAS empresas emitiram contra o CNPJ do `[emitente]` (a empresa é a destinatária) se baixam do Ambiente
Nacional pela distribuição de DF-e (`NFeDistribuicaoDFe`, NT 2014.002). A mesma camada de aplicação, a mesma pasta de notas
e o mesmo certificado A1 servem: `rustnf nfe recebidas ...`. Foi conferido com transporte de
teste e com um servidor simulado; a chamada ao Ambiente Nacional real é o que o teste real prova (veja
[Estado](#estado)). A interface de desktop tem a opção **Recebidas (compras)** na tela Notas (veja abaixo).

**Pela interface de desktop (Windows):** em Notas, escolha **Recebidas (compras)**. A lista mostra, para cada nota, a data, o
fornecedor (nome e CNPJ), o valor, o que aconteceu com a nota (autorizada, cancelada pelo emitente, uso negado) e o que a empresa já
respondeu (sem resposta, ciência, confirmada, desconhecida, não realizada). **Buscar notas novas** pede ao Ambiente Nacional as
notas novas e diz em palavras o que veio; se ele mandou esperar, o botão fica desligado e a tela mostra até que horas (a espera é da
biblioteca: a tela nunca a contorna). Na nota só com o resumo, **Baixar a nota completa** registra a **ciência da operação** (um
evento fiscal; em produção pede a marcação "Entendo que...") e traz a nota, com o mesmo detalhe e o mesmo DANFE da NF-e emitida.
**Responder à nota** oferece as quatro respostas ("Recebi a mercadoria, está certo", "Vi a nota, ainda vou conferir", "Não comprei
isto", "Comprei, mas a venda não aconteceu", esta com justificativa de 15 a 255 caracteres) e já desliga as que o Ambiente Nacional
recusaria. **O programa não calcula o prazo da resposta**: quem o confere é o Ambiente Nacional.

Passo a passo:

1. **Configure** o mesmo `rustnf.toml` da emissão (`[emitente].cnpj`, `[nfe].uf` e `[nfe].ambiente`; o certificado é o do
   `[sefin].pfx`). Comece em `homologacao`: cada ambiente tem o seu cursor e os seus NSU.
2. **Baixe o que há:** `rustnf nfe recebidas sincronizar`. Ele lê a fila do último NSU até o fim, de 50 em 50 documentos, e
   diz quantos documentos vieram e de que tipo. **Antes de a empresa manifestar, o Ambiente Nacional entrega só o resumo
   (`resNFe`) de cada nota**, o suficiente para saber quem emitiu, quanto vale e decidir.
3. **Veja a lista:** `rustnf nfe recebidas listar` (só lê a pasta; `--situacao`, `--busca`, `--json`).
4. **Manifeste:** `rustnf nfe recebidas manifestar --chave <chave> --tipo ciencia`. **A manifestação é um evento fiscal:**
   ela é assinada com o certificado da empresa e registrada no Ambiente Nacional; vale a última manifestação conclusiva e
   não há como desfazer. Os tipos são `ciencia` (210210, ainda não conclusiva), `confirmacao` (210200), `desconhecimento`
   (210220) e `nao-realizada` (210240, com `--justificativa` de 15 a 255 caracteres).
5. **Baixe a nota completa.** Depois de uma manifestação de ciência, confirmação ou operação não realizada, a próxima
   `sincronizar` traz o `procNFe`; `rustnf nfe recebidas baixar --chave <chave>` busca uma só, e com `--ciencia` dá a
   ciência antes se a nota ainda não tem manifestação. O desconhecimento da operação NÃO libera a nota completa (NT
   2014.002 v1.02d, item 3, texto do serviço e nota 1 da tabela de documentos).
6. **Respeite a espera.** Quando a fila acaba, o Ambiente Nacional manda esperar uma hora; consulta antes disso, ou repetida,
   leva o bloqueio 656 (também por uma hora). O programa grava a hora no cursor e, antes dela, responde quando tentar
   **sem chamar a rede** (saída 0: esperar não é falha). Não apague o cursor para "recomeçar": é o caminho do 656.

```
<pasta>/nfe/recebidas/<AAAA>/<MM>/<chave>-resNFe.xml                       o resumo (só ele, até a empresa manifestar)
<pasta>/nfe/recebidas/<AAAA>/<MM>/<chave>-procNFe.xml                      a NF-e completa
<pasta>/nfe/recebidas/<AAAA>/<MM>/<chave>-<tipo>-<seq>-procEventoNFe.xml   evento completo (de terceiros ou a sua manifestação)
<pasta>/nfe/recebidas/<AAAA>/<MM>/<chave>-<tipo>-<seq>-resEvento.xml       resumo de evento
<pasta>/nfe/recebidas/outros/<NSU>-...                                     o que não tem chave de NF-e ou não abriu
<pasta>/nfe/recebidas/indice.json                                          uma linha por NF-e recebida
<pasta>/nfe/recebidas/cursor-<ambiente>-<documento>.json                   o cursor de NSU de cada ambiente e interessado
```

`<AAAA>/<MM>` são o ano e o mês da chave (o da emissão). Fica separada das notas emitidas: a pasta e a configuração da
versão 0.4.0 continuam sendo lidas como estavam, e `rustnf nfe notas reindexar` não enxerga os recebidos. A situação no
índice é `resumo`, `completa`, `ciencia`, `confirmada`, `desconhecida`, `nao_realizada`, `cancelada` (o emitente cancelou) ou
`denegada`; a manifestação que o índice guarda é a que passou por este programa ou voltou na distribuição.

**Quando o cursor anda.** A cada lote, o programa grava os documentos e o índice e SÓ ENTÃO o cursor (cada gravação é atômica
e protegida por trava). Uma queda no meio deixa o cursor no lote anterior: a próxima `sincronizar` pede o mesmo lote e regrava
os mesmos arquivos (o nome vem da chave, então repetir não duplica nada). O NSU que falta no meio do lote é buscado um a um
pela biblioteca (`consNSU`), e o documento que não abriu fica guardado e é relido na vez seguinte.

| comando | o que faz | usa a rede |
|---|---|---|
| `rustnf nfe recebidas sincronizar` | baixa do último NSU ao fim da fila (`--lotes N`, padrão 20), respeitando a espera e o bloqueio | sim |
| `rustnf nfe recebidas listar` | lista as NF-e recebidas da pasta (`--situacao`, `--busca`, `--json`) | não |
| `rustnf nfe recebidas baixar` | baixa a NF-e de uma `--chave`; `--ciencia` registra a ciência antes, se preciso | sim (a ciência é evento fiscal) |
| `rustnf nfe recebidas manifestar` | `--chave`, `--tipo ciencia\|confirmacao\|desconhecimento\|nao-realizada`, `--justificativa` | sim (**evento fiscal**) |
| `rustnf nfe recebidas cursor` | o último NSU, quando se pode consultar de novo e o que não abriu | não |

Códigos de saída: 0 ok (inclusive "ainda não é hora de consultar"), 2 uso ou entrada, 3 recusa (bloqueio 656, rejeição, nota
não localizada ou só o resumo, manifestação recusada), 4 sem resposta ou serviço parado do Ambiente Nacional. O endereço do
simulador vai em `[nfe.urls].distribuicao` (e `eventos`, para a manifestação).

## NFC-e (modelo 65)

A NFC-e é a nota do cupom de varejo: a venda ao consumidor no balcão, com o DANFC-e (o cupom) e um QR Code. Ela usa a mesma
camada de aplicação, o mesmo arquivo de configuração, o mesmo certificado A1 e a mesma senha no cofre da NF-e, mas tem
**numeração própria**, prazos próprios e o CSC. A biblioteca monta, assina, envia e desenha o cupom; a linha de comando
`rustnf nfce` completa o pedido com a configuração, numera pela pasta, guarda o que a SEFAZ devolve, diz o resultado em
palavras e avisa o que falta enviar ao fechar o caixa.

Foi conferida com transporte de teste e com um servidor simulado; a NFC-e aceita por uma SEFAZ real, com o CSC de
verdade, e o cupom numa impressora real são o que o teste real prova (veja [Estado](#estado)). Na interface de desktop a NFC-e é o
[Caixa](#caixa-nfc-e); a API HTTP e o MCP estão em [NFC-e pela API](#nfc-e-pela-api).

| | NFS-e | NF-e (55) | NFC-e (65) |
|---|---|---|---|
| para que serve | serviço | venda de mercadoria entre empresas ou a consumidor | venda de varejo presencial ao consumidor |
| comando | `rustnf emitir` | `rustnf nfe ...` | `rustnf nfce ...` |
| pasta de notas | `<pasta>/` | `<pasta>/nfe/` | `<pasta>/nfce/` |
| numeração | a da DPS | por série, própria | por série, **própria** (a série 1 do 65 não é a série 1 do 55) |
| envio | Sefin Nacional | SEFAZ da UF (lote síncrono) | SEFAZ da UF, **sempre síncrono** |
| QR Code | não | não (só com `tpImp` 6) | **sim**: versão 3 (padrão, sem CSC) ou versão 2 (com CSC) |
| sem internet | — | SVC ou EPEC | **contingência off-line** (`tpEmis` 9): a nota vai depois, com prazo |
| impressão | DANFSe | DANFE | **DANFC-e** (PDF de bobina de 80 ou 58 mm, ou bytes ESC/POS) |

### Passo a passo (do CSC ao fechamento do caixa)

Tudo em homologação primeiro: a nota de homologação não vale. Os mesmos `[emitente]`, `[nfe]` (UF, IE, CRT, endereço) e
`[sefin].pfx` da NF-e servem; o que é só da NFC-e fica em `[nfce]` (exemplo em `crates/rustnf-cli/exemplos/rustnf.toml`).

1. **Escolha o QR Code.** O padrão é a versão 3, que não pede CSC: não precisa fazer nada. Se a SEFAZ da sua UF ainda exige a
   versão 2, ela lhe dá um **CSC** (um código de 16 a 36 letras e números) e um **idCSC** (um número). Ponha o idCSC no
   arquivo e use `qr_versao = 2`:

   ```toml
   [nfce]
   qr_versao = 2
   id_csc = "1"
   ```

2. **Guarde o CSC no cofre do sistema** (nunca no arquivo, nunca na linha de comando): o CSC vem de uma linha do stdin ou de
   uma variável de ambiente que você apaga depois.

   ```
   echo CODIGO-QUE-A-SEFAZ-DEU | rustnf nfce csc guardar            # ou: --csc-env MINHA_VARIAVEL
   rustnf nfce csc testar                                             # diz se há CSC guardado para o ambiente e o idCSC; nunca o mostra
   rustnf nfce csc apagar
   ```

   O item do cofre é próprio do emitente, do ambiente e do idCSC: trocar de ambiente ou de idCSC não enxerga o CSC do outro.
   Com `qr_versao = 2` e sem CSC guardado, `rustnf nfce emitir` recusa **antes de numerar** qualquer nota.

3. **Emita.** O pedido é o da NF-e (`docs/NFE_PEDIDO.md`), sem destinatário e sem `modelo` (o 65 vem sozinho):

   ```
   rustnf nfce emitir pedido.json --danfce       # grava a nota e o cupom em PDF na pasta de notas
   ```

   Se a resposta da SEFAZ se perder, a nota fica **pendente** com a mesma chave: nunca emita outra no lugar.
   `rustnf nfce pendentes --retomar` a consulta e, se a SEFAZ diz que não a recebeu, reenvia a MESMA nota.

4. **Sem internet: emita off-line.** `rustnf nfce emitir pedido.json --offline --danfce` assina e guarda a nota como pendente
   (`tpEmis` 9), **sem falar com a SEFAZ**, e diz até quando você tem para transmiti-la. Entregue o cupom ao consumidor (o
   DANFC-e da off-line sai com as mensagens do Manual; a via do estabelecimento sai com `rustnf nfce danfce <chave>
   --via-estabelecimento --pdf via.pdf`). Quando a internet voltar: `rustnf nfce pendentes --retomar`.
   `--offline-se-necessario` pergunta antes ao serviço de status da NFC-e: se ele não responde, emite off-line; se responde,
   envia. Depois de **enviar** sem resposta a nota nunca vira off-line (seria uma segunda nota para o mesmo cupom): fica
   pendente e se retoma.

5. **Ao fechar o caixa**, rode `rustnf nfce pendentes --fechar-caixa`. Ele lista, em palavras, cada NFC-e que ainda depende da
   SEFAZ e sai com **código 4** se houver alguma (para um script de fechamento detectar). Ele diz a ação e o prazo ("rode
   `rustnf nfce pendentes --retomar` com internet antes de 05/10/2026 23:59"); prazo vencido ou a menos de uma hora aparece
   em destaque (`!!`). Com `--retomar` na mesma chamada, ele transmite e só então confere o que sobrou.

6. **Cancele** com `rustnf nfce cancelar --chave ... --justificativa "..."`. Antes de enviar, o programa avisa se o prazo
   nacional de 24 horas depois da autorização já acabou (o aviso nunca bloqueia: a SEFAZ da UF é quem decide e responde 501
   se recusar).

### Comandos

| comando | o que faz |
|---|---|
| `rustnf nfce csc guardar` / `testar` / `apagar` | o CSC no cofre do sistema; nunca por argumento, nunca mostrado |
| `rustnf nfce emitir` | completa, confere, monta, assina (com o QR Code da configuração), grava como pendente, envia e grava o resultado; `--offline`, `--offline-se-necessario`, `--danfce` |
| `rustnf nfce consultar` | consulta a NFC-e pela chave e põe a nota da pasta em dia |
| `rustnf nfce cancelar` | cancela (evento 110111), com o aviso do prazo antes de enviar |
| `rustnf nfce pendentes` | lista o que falta enviar, com o prazo; `--retomar` consulta, reenvia ou transmite a off-line; `--fechar-caixa` sai com 4 se sobrar pendente |
| `rustnf nfce status` | o estado do serviço de autorização da NFC-e da UF |
| `rustnf nfce danfce <chave \| arquivo.xml>` | grava o DANFC-e como arquivo: `--pdf arq.pdf` ou `--escpos arq.bin`, `--largura 80\|58`, `--colunas 48\|42\|32`, `--resumido`, `--via-estabelecimento`; **nunca fala com impressora** |

Os códigos de saída são os de `rustnf nfe` (0, 2, 3 e 4) e, com `--fechar-caixa`, o 4 também significa "ainda há nota
pendente". Opções das ações que falam com a SEFAZ: `--pfx`, `--senha-env` ou `--senha-stdin` (ou a senha do cofre),
`--ambiente homologacao|producao` (o de `[nfce].ambiente`), `--json`.

### Tabela da `[nfce]`

| campo | padrão | o que é |
|---|---|---|
| `ambiente` | `homologacao` | `homologacao` ou `producao`: o `tpAmb` e a SEFAZ da NFC-e |
| `serie` | `1` | série do 65, de 0 a 999 (sequência própria) |
| `qr_versao` | `3` | 3 (sem CSC) ou 2 (com CSC) |
| `id_csc` | vazio | idCSC, de 1 a 6 dígitos; obrigatório com `qr_versao = 2` |
| `largura_danfce` | `80` | bobina do cupom: 80 ou 58 mm |
| `colunas_escpos` | `0` | 0 = pela largura (48 ou 32), ou 48, 42, 32 |
| `motivo_offline` | vazio | `xJust` da off-line (15 a 255 caracteres); vazio = texto padrão |

### Pasta de notas da NFC-e

`<pasta>/nfce/` tem o mesmo desenho de `<pasta>/nfe/` (um XML por nota em `AAAA/MM/`, `indice.json`, `estado.json`), com
**índice e numeração só do modelo 65**: nada do que a NF-e guardou é lido nem mudado. As regras de gasto do número são as da
NF-e: autorizada, denegada e cancelada **gastam** o número; a rejeitada **não gasta** (é a inferência já documentada para a
NF-e). Uma exceção da off-line: a NFC-e emitida off-line que a SEFAZ **rejeita** na transmissão **não sai da pasta** nem
libera o número (o cupom já está com o consumidor): ela fica pendente, com a recusa (`cStat` e motivo) anotada no índice e
mostrada por `pendentes`, e a MOC manda corrigir e transmitir de novo com a mesma chave. **O `rustnf` ainda não refaz essa
retransmissão corrigida sozinho.**

### O que a NFC-e do `rustnf` ainda não faz

* cancelamento por substituição (110112): a biblioteca não o tem;
* inutilização da NFC-e: o `inutNFe` da biblioteca é só do modelo 55;
* prazo de cancelamento de cada UF: o aviso usa o prazo nacional (MOC 7.00, Visao Geral, regra 2P12-14); a lista por UF não
  está nas fontes oficiais que o projeto usa;
* na interface de desktop, a via do estabelecimento do cupom off-line (a camada de aplicação já a gera).

## API HTTP

```
rustnf serve                                   # 127.0.0.1:8080, so' leitura
rustnf serve --porta 0                         # qualquer porta livre (o endereco sai no stderr)
```

Opções: `--endereco` (padrão `127.0.0.1`), `--porta`, `--limite-corpo` (bytes; padrão 1048576),
`--tempo-limite` (segundos; padrão 60), `--origem-permitida` (repetível), `--folga-relogio`. Ctrl+C para o servidor
limpo.

| rota | corpo | resposta |
|---|---|---|
| `GET /health` | | `{"status":"ok","versao":"...","emissao":false,"nfe":false,"nfce":false,"nfce_csc_guardado":false}` (`nfe` e `nfce`: a NF-e e a NFC-e podem falar com a SEFAZ neste servidor; `nfce_csc_guardado`: só um booleano, `true` quando o QR Code é o da versão 2 e o CSC foi lido do cofre ao subir) |
| `POST /calcular` | JSON de `calcular` | JSON da prévia |
| `POST /dps/conferir` | JSON da DPS | `{"ok":true}` ou 422 com o campo |
| `POST /dps/montar` | JSON da DPS | `{"xml":"..."}` |
| `POST /danfse` | XML da NFS-e (`?situacao=cancelada&canhoto=false`) | `application/pdf` |
| `POST /emitir` | JSON da DPS | JSON do resultado (só com emissão) |
| `POST /cancelar` | `{"chave","motivo","justificativa"}` | JSON do resultado (só com emissão) |
| `GET /nfse/{chave}` | | JSON da NFS-e (só com emissão) |
| `GET /dps/{id}` | (`?com_nota=true` busca a nota) | a chave que a DPS gerou (só com emissão) |
| `GET /nfse/{chave}/eventos/{tipo}/{sequencia}` | | o evento, conferido com a chave e o tipo (só com emissão) |
| `POST /eventos` | `{"chave","tipo","motivo","justificativa"}` | JSON do resultado (só com emissão) |
| `POST /notas/atualizar` | filtro: `{"de","ate","situacao","busca","intervalo_ms"}` (todos opcionais) | relatório: `conferidas`, `mudaram`, `iguais`, `falhas` (só com emissão) |
| `POST /mcp` | mensagem JSON-RPC | veja abaixo |

As rotas da NF-e (`/nfe/...`) estão em [NF-e pela API](#nf-e-pela-api) e as da NFC-e (`/nfce/...`), em [NFC-e pela API](#nfc-e-pela-api).

```
$ curl -s http://127.0.0.1:8080/health
{"emissao":false,"nfce":false,"nfce_csc_guardado":false,"nfe":false,"status":"ok","versao":"0.3.0"}

$ curl -s -X POST http://127.0.0.1:8080/calcular -H 'Content-Type: application/json' \
    --data-binary @crates/rustnf-cli/exemplos/calculo.json

$ curl -s -X POST http://127.0.0.1:8080/dps/montar -H 'Content-Type: application/json' \
    --data-binary @crates/rustnf-cli/exemplos/dps.json

$ curl -s -X POST 'http://127.0.0.1:8080/danfse?situacao=valida' --data-binary @crates/rustnf-cli/exemplos/nfse.xml \
    -o danfse.pdf
```

### Emissão pela API

As rotas `/emitir`, `/cancelar`, `/nfse/{chave}`, `/dps/{id}`, `/nfse/{chave}/eventos/{tipo}/{sequencia}`, `/eventos` e `/notas/atualizar` **nem existem** (404) sem `--permitir-emissao`. Com a opção, o
servidor exige certificado, ambiente e um token: sem token configurado, ele recusa subir.

```
$ export SENHA_A1='...'
$ export TOKEN_RUSTNF="$(head -c 48 /dev/urandom | base64 | tr -d '=+/\n')"      # 32 caracteres no minimo
$ rustnf serve --permitir-emissao --pfx certificado.pfx --senha-env SENHA_A1 \
    --ambiente restrita --token-env TOKEN_RUSTNF

$ curl -s -X POST http://127.0.0.1:8080/emitir -H "Authorization: Bearer $TOKEN_RUSTNF" \
    -H 'Content-Type: application/json' --data-binary @pedido.json
$ curl -s http://127.0.0.1:8080/nfse/<chave> -H "Authorization: Bearer $TOKEN_RUSTNF"
$ curl -s -X POST http://127.0.0.1:8080/cancelar -H "Authorization: Bearer $TOKEN_RUSTNF" \
    -H 'Content-Type: application/json' \
    -d '{"chave":"<chave>","motivo":"1","justificativa":"Valor digitado errado na nota"}'
```

O servidor lê o mesmo arquivo de configuração da linha de comando (`rustnf serve --config rustnf.toml`). No `/emitir`,
o `[emitente]` completa o que o pedido não traz; com `[notas].pasta`, o `numero` da DPS pode faltar (vale o próximo da
série), a nota autorizada é guardada e a resposta traz `guardada` e `numero_dps` (e `aviso` ou `recuperada_difere`,
nos casos descritos em [Pasta de notas](#pasta-de-notas)). No `/cancelar`, o cancelamento registrado marca a nota na
pasta. O servidor emite uma nota por vez, para duas emissões simultâneas não pegarem o mesmo número; o `/eventos` e o `/notas/atualizar` entram na mesma fila, e a atualização consulta uma nota e uma consulta por vez. `POST /eventos` recusa (422) o 101101 e os eventos do fisco antes de qualquer envio. `POST /notas/atualizar` responde 200 com o relatório mesmo havendo `falhas` (quem chama confere a lista); num lote grande ela passa do tempo limite da requisição, e o que foi conferido até lá já está gravado: use o filtro (período, situação) para dividir o trabalho. Sem `[emitente]`
e sem pasta de notas, o pedido precisa vir completo, com o `numero`.

Campo que a DPS não conhece: o `/emitir` recusa o pedido (422, com o campo) antes de qualquer envio; com
`?estrito=false` ele emite sem o campo e a resposta traz a lista em `campos_desconhecidos`. `/dps/conferir` e
`/dps/montar` só devolvem a lista, a menos que a rota leve `?estrito=true`.

Segurança: escuta em `127.0.0.1` por padrão (fora dele o tráfego é HTTP puro: ponha um proxy com TLS na frente); sem
CORS; cabeçalho `Origin` de fora do próprio computador recebe 403; teto de corpo e tempo limite por requisição; token
comparado em tempo constante; o PDF é gerado fora do laço assíncrono. Todo erro tem o envelope único:

```
{"erro": {"codigo": "recusa", "mensagem": "...", "campo": "infDPS/serie"}}
```

Códigos: `entrada_invalida` (400), `nao_autorizado` (401), `nao_encontrada` (404), `corpo_grande` (413),
`recusa` (422), `sefin` (502), `tempo_esgotado` (504), `erro_interno` (500). A mensagem nunca traz caminho de arquivo,
senha, token, chave de acesso nem o XML assinado, e nada disso vai para o registro do servidor (stderr).

### NF-e pela API

A NF-e (modelo 55) entra no MESMO servidor, com as mesmas regras de segurança e as mesmas funções da camada de aplicação
que `rustnf nfe` usa: nenhuma regra fiscal mora na API. O servidor lê a `[nfe]` e a `[notas].pasta` do arquivo de
configuração; o ambiente da NF-e é o de `[nfe].ambiente` (`homologacao` por padrão) e **não muda com o `--ambiente` da
NFS-e**. O certificado é o mesmo do `--pfx`.

| rota | corpo | resposta | função de `rustnf_app::nfe` |
|---|---|---|---|
| `POST /nfe/conferir` | pedido da NF-e ([docs/NFE_PEDIDO.md](docs/NFE_PEDIDO.md)) | `{"ok":true,"chave","serie","numero","totais","avisos"}` ou 422 com o campo e a rejeição | `montar_pedido` |
| `POST /nfe/montar` | o mesmo pedido | `{"xml","assinada":false,"chave",...}`: monta, **nunca assina**, não envia e não reserva o número | `montar_pedido` |
| `POST /nfe/danfe` | XML da NF-e autorizada, `nfeProc` (`?canhoto=false&cancelada=true`) | `application/pdf` | `danfe_do_xml` |
| `POST /nfe/emitir` | o pedido | JSON do resultado (só com emissão) | `emitir` |
| `POST /nfe/epec` | o pedido, com `x_just` ou `[nfe].motivo_contingencia`; e `"substitui": "<chave>"` se a venda tinha uma NF-e sem resposta (a nota fica ligada ao EPEC e a retomada só a consulta) | JSON do resultado, com `substitui` se houve vínculo (só com emissão) | `epec_substituindo` |
| `GET /nfe/{chave}` | | JSON da consulta; atualiza a pasta (só com emissão) | `consultar` |
| `POST /nfe/cancelar` | `{"chave","justificativa","protocolo"?}` | JSON do evento (só com emissão) | `cancelar` |
| `POST /nfe/corrigir` | `{"chave","texto","sequencia"?}` | JSON do evento (só com emissão) | `corrigir` |
| `POST /nfe/inutilizar` | `{"serie"?,"inicio","fim","justificativa"}` | JSON da inutilização (só com emissão) | `inutilizar` |
| `GET /nfe/status` | (`?svc=true` pergunta a SVC) | estado do serviço (só com emissão) | `status` |
| `GET /nfe/pendentes` | | `{"notas":[...]}` sem rede, com `substituida_por` e `a_fazer` na nota substituída (só com emissão) | `NotasNfe::a_resolver` |
| `POST /nfe/pendentes/retomar` | | `{"notas":[...]}`: consulta a MESMA chave de cada pendente; a substituída por um EPEC só é consultada e volta com `a_fazer` (`cancelar` ou `inutilizar`) (só com emissão) | `retomar` |
| `POST /nfe/recebidas/sincronizar` | `{"lotes"?}` (o corpo é opcional) | JSON da sincronização: `status`, `frase`, `documentos`, `tipos`, `cursor` (só com emissão; **rede**) | `sincronizar` |
| `GET /nfe/recebidas` | (`?situacao=`, `?busca=`) | `{"notas":[...]}` das NF-e recebidas, sem rede (só com emissão) | `listar` |
| `GET /nfe/recebidas/cursor` | | o cursor da distribuição, sem rede (só com emissão) | `cursor` |
| `POST /nfe/recebidas/baixar` | `{"chave","ciencia"?}` | JSON da baixa; com `ciencia` registra a ciência da operação, **evento fiscal** (só com emissão; **rede**) | `baixar` |
| `POST /nfe/recebidas/manifestar` | `{"chave","tipo","justificativa"?}` com `tipo` `ciencia`, `confirmacao`, `desconhecimento` ou `nao_realizada` | JSON do evento: a manifestação é **evento fiscal** (só com emissão; **rede**) | `manifestar` |

`conferir`, `montar` e `danfe` ficam sempre no ar e não usam a rede. As demais (inclusive `/nfe/recebidas`, que também lê a pasta dos documentos recebidos) **nem existem** (404) sem
`--permitir-emissao`, e com ela pedem o `Authorization: Bearer` (401 sem ele). O envelope de erro e os códigos HTTP são os
da NFS-e. O que a SEFAZ não autoriza (rejeição, denegação, sem resposta, serviço parado) volta com o JSON COMPLETO do
resultado (`chave`, `cstat`, `situacao` na pasta) mais o envelope `erro` com a frase e o `cStat`, no status do erro:
422 para recusa de regra e 502 para o que ficou sem resposta (a nota fica `pendente`: use `/nfe/pendentes/retomar`, nunca emita outra no lugar). Campo que o leiaute não conhece
recusa o pedido (422, com o campo) antes de qualquer envio; `?estrito=false` o ignora e a resposta o lista em
`campos_desconhecidos`. O servidor emite, reserva faixa e retoma uma operação por vez, para duas emissões
simultâneas na mesma série não gastarem o mesmo número. Sem `[nfe].uf`, as rotas de efeito fiscal respondem 400 dizendo o que configurar, e
`conferir`, `montar` e `danfe` seguem no ar.

```
$ curl -s http://127.0.0.1:8080/nfe/status -H "Authorization: Bearer $TOKEN_RUSTNF"
$ curl -s -X POST http://127.0.0.1:8080/nfe/emitir -H "Authorization: Bearer $TOKEN_RUSTNF" \
    -H 'Content-Type: application/json' --data-binary @pedido-nfe.json
$ curl -s -X POST http://127.0.0.1:8080/nfe/cancelar -H "Authorization: Bearer $TOKEN_RUSTNF" \
    -H 'Content-Type: application/json' \
    -d '{"chave":"<chave>","justificativa":"Cancelamento por erro de digitacao"}'
$ curl -s -X POST 'http://127.0.0.1:8080/nfe/danfe?canhoto=false' --data-binary @nfe-autorizada.xml -o danfe.pdf
```

O pedido (`pedido-nfe.json`) é o de `rustnf nfe emitir`, com dados fictícios, por exemplo: `{"nat_op":"VENDA","dest":{"documento":"52998224725",
"nome":"CLIENTE TESTE","ind_ie_dest":9,"endereco":{...}},"ind_final":1,"ind_pres":1,"itens":[{"codigo":"1",
"descricao":"Cafe torrado 500g","ncm":"09012100","cfop":"5102","unidade":"UN","quantidade":"2","valor_unitario":"15.90",
"icms":{"codigo":"102"},"pis":{"cst":"49"},"cofins":{"cst":"49"}}],"pagamentos":[{"t_pag":"01","valor":"31.80"}]}`.

### NFC-e pela API

A NFC-e (modelo 65) entra no MESMO servidor, com as mesmas regras de segurança da NF-e, e chama as MESMAS funções da camada de
aplicação que `rustnf nfce` chama: nenhuma regra fiscal mora na API. O servidor lê a `[nfce]` do mesmo arquivo de configuração
(o ambiente e a série dela são os da NFC-e e **não mudam com o `[nfe]` nem com o `--ambiente`**), o `[emitente]` e o `[nfe]`
compartilhados, o mesmo certificado do `--pfx` e a pasta `nfce/` dentro de `[notas].pasta` (numeração própria: uma NFC-e e uma NF-e
na mesma série numeram cada uma a sua, ao mesmo tempo, sem se atrapalhar).

| rota | corpo | resposta | função de `rustnf_app` |
|---|---|---|---|
| `POST /nfce/danfce` | XML da NFC-e, `nfeProc` ou NFe de contingência off-line (`?formato=pdf` ou `escpos`, `largura=80` ou `58`, `colunas=48`, `42` ou `32`, `resumido=true`, `via_estabelecimento=true`, `cancelada=true`) | `application/pdf`, ou `application/octet-stream` com `formato=escpos` | `nfce::danfce::{opcoes, pdf, escpos}` |
| `POST /nfce/emitir` | o pedido ([docs/NFE_PEDIDO.md](docs/NFE_PEDIDO.md); o modelo 65 vem sozinho), com `offline` ou `offline_se_necessario` (booleanos, opcionais) | JSON do resultado (só com emissão) | `nfce::emitir` |
| `GET /nfce/{chave}` | | JSON da consulta; atualiza a pasta (só com emissão) | `nfe::consultar` |
| `POST /nfce/cancelar` | `{"chave","justificativa","protocolo"?}` | JSON do evento, com `aviso_prazo` se as 24 horas já acabaram (só com emissão) | `nfce::aviso_de_prazo` e `nfe::cancelar` |
| `GET /nfce/status` | | estado do serviço da NFC-e da UF (só com emissão) | `status_nfce` |
| `GET /nfce/pendentes` | | `{"notas":[...],"pode_fechar_caixa","aviso"}`, sem rede (só com emissão) | `nfce::listar_pendentes` |
| `POST /nfce/pendentes/retomar` | | `{"retomadas":[...],"notas":[o que sobrou],"pode_fechar_caixa","aviso"}`: consulta a MESMA chave de cada pendente e transmite as off-line (só com emissão) | `nfe::retomar` |

`/nfce/danfce` fica sempre no ar e não usa a rede (ele não lê a pasta: o corpo é o XML). As demais **nem existem**
(404) sem `--permitir-emissao`, e com ela pedem o `Authorization: Bearer` (401 sem ele). `pode_fechar_caixa` é `true` quando não
há NFC-e pendente: faz, na API, o papel do código de saída 4 do `--fechar-caixa`. A NFC-e que a SEFAZ não autoriza volta com o JSON
COMPLETO e o envelope `erro`, como na NF-e (422 para recusa de regra, 502 para sem resposta: a nota fica `pendente` com a chave
no corpo, e se retoma, nunca se emite outra no lugar); a NFC-e off-line volta 200 com `"status":"offline_pendente"` e o `prazo` para
transmitir. O servidor emite uma NFC-e por vez (fila própria). Campo que o leiaute não conhece recusa o pedido (422) antes de qualquer
envio.

**O CSC só se guarda pela linha de comando**, na máquina do servidor (`rustnf nfce csc guardar`). Nenhuma rota nem ferramenta
grava, lê ou apaga o CSC; o servidor o lê do cofre UMA vez, ao subir, para o QR Code versão 2 (guardou depois? reinicie o servidor).
Pedido com um campo de CSC (`csc`, `id_csc`, `idCSC`...) é recusado (422) antes de qualquer envio, **mesmo com `?estrito=false`**, e
o valor nunca aparece em resposta nem em arquivo da pasta de notas (há teste para isso). Sem `[nfce]` utilizável (falta a `[nfe].uf`
compartilhada, ou o QR Code versão 2 sem CSC guardado) as rotas de efeito fiscal da NFC-e respondem 400 dizendo o que fazer; o
`/nfce/danfce`, a NF-e e a NFS-e seguem no ar. Não há `/nfce/conferir` nem `/nfce/montar`: o pedido da NFC-e se confere e se monta
dentro de `/nfce/emitir`, antes de qualquer envio (o `/nfe/conferir` e o `/nfe/montar` seguem sendo do modelo 55).

```
$ curl -s http://127.0.0.1:8080/nfce/status -H "Authorization: Bearer $TOKEN_RUSTNF"
$ curl -s -X POST http://127.0.0.1:8080/nfce/emitir -H "Authorization: Bearer $TOKEN_RUSTNF" \
    -H 'Content-Type: application/json' --data-binary @pedido-nfce.json
$ curl -s http://127.0.0.1:8080/nfce/pendentes -H "Authorization: Bearer $TOKEN_RUSTNF"
$ curl -s -X POST 'http://127.0.0.1:8080/nfce/danfce?formato=escpos&largura=58&colunas=32' \
    --data-binary @nfce-autorizada.xml -o cupom.bin
```

O pedido (`pedido-nfce.json`) é o de `rustnf nfce emitir`, com dados fictícios: `{"nat_op":"VENDA","ind_final":1,"ind_pres":1,
"itens":[{"codigo":"1","descricao":"Cafe torrado 500g","ncm":"09012100","cfop":"5102","unidade":"UN","quantidade":"2",
"valor_unitario":"15.90","icms":{"codigo":"102"},"pis":{"cst":"49"},"cofins":{"cst":"49"}}],"pagamentos":[{"t_pag":"01",
"valor":"31.80"}]}`; para emitir sem internet, acrescente `"offline":true`.

## MCP

`POST /mcp` no mesmo servidor, transporte **Streamable HTTP** com resposta JSON (sem SSE) e sem estado. Implementa a
revisão **2026-07-28** da especificação
(<https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http>): cada requisição leva
a versão e as capacidades em `_meta` e os cabeçalhos `MCP-Protocol-Version`, `Mcp-Method` e `Mcp-Name`; também atende
`initialize` da revisão 2025-11-25 para clientes mais antigos. Ferramentas:

| ferramenta | o que faz |
|---|---|
| `calcular_cbs_ibs` | prévia do IBS/CBS (mesmo JSON de `calcular`) |
| `conferir_dps` | confere a DPS; recusa volta com `isError: true` |
| `montar_dps` | XML da DPS |
| `gerar_danfse` | PDF em base64, num recurso embutido |
| `emitir_nfse` | emite como o `POST /emitir`; só com `--permitir-emissao` e o Bearer |
| `consultar_nfse` | consulta pela chave de acesso; só com `--permitir-emissao` e o Bearer |
| `cancelar_nfse` | cancela como o `POST /cancelar`; só com `--permitir-emissao` e o Bearer |
| `consultar_dps` | a chave que a DPS gerou, pelo Id ou pelo número (o emitente da configuração do servidor dá o resto); só com `--permitir-emissao` e o Bearer |
| `consultar_evento` | consulta um evento pela chave, o tipo e a sequência; só com `--permitir-emissao` e o Bearer |
| `registrar_evento` | registra um evento que o contribuinte pede, como o `POST /eventos` (**efeito fiscal**, como `cancelar_nfse`); só com `--permitir-emissao` e o Bearer |
| `atualizar_situacoes` | confere a situação das notas da pasta, como o `POST /notas/atualizar`; só com `--permitir-emissao` e o Bearer |
| `conferir_nfe` | confere o pedido da NF-e (regras do MOC e das NT), sem rede e sem gastar número; recusa volta com `isError: true` |
| `montar_nfe` | XML da NF-e, **nunca assinado** |
| `gerar_danfe` | PDF do DANFE em base64, num recurso embutido |
| `emitir_nfe` | emite como o `POST /nfe/emitir` (**efeito fiscal**); só com `--permitir-emissao` e o Bearer |
| `consultar_nfe` | consulta pela chave e atualiza a pasta; só com `--permitir-emissao` e o Bearer |
| `cancelar_nfe` | cancela como o `POST /nfe/cancelar` (**efeito fiscal**); só com `--permitir-emissao` e o Bearer |
| `corrigir_nfe` | carta de correção, como o `POST /nfe/corrigir` (**efeito fiscal**); só com `--permitir-emissao` e o Bearer |
| `inutilizar_nfe` | inutiliza uma faixa de números (**efeito fiscal**); só com `--permitir-emissao` e o Bearer |
| `status_sefaz` | estado do serviço da SEFAZ da UF (ou da SVC); só com `--permitir-emissao` e o Bearer |
| `registrar_epec` | emite em contingência EPEC (**efeito fiscal**); `substitui` (a chave da NF-e sem resposta cuja venda o EPEC registra) liga as duas notas; só com `--permitir-emissao` e o Bearer |
| `retomar_pendentes_nfe` | consulta a MESMA chave de cada NF-e pendente (**efeito fiscal**); só com `--permitir-emissao` e o Bearer |
| `sincronizar_recebidas` | baixa os documentos recebidos (distribuição de DF-e) e grava na pasta; respeita a espera de uma hora e o bloqueio 656; só com `--permitir-emissao` e o Bearer |
| `listar_recebidas` | lista as NF-e recebidas da pasta (`situacao`, `busca`); **não usa a rede**, mas lê os documentos da empresa e por isso também exige `--permitir-emissao` e o Bearer |
| `baixar_recebida` | baixa a NF-e de uma chave; com `ciencia: true` registra a ciência da operação (**efeito fiscal**); só com `--permitir-emissao` e o Bearer |
| `gerar_danfce` | o DANFC-e da NFC-e em PDF de bobina ou, com `formato: "escpos"`, em bytes ESC/POS, em base64 num recurso embutido; não usa a rede nem fala com impressora |
| `emitir_nfce` | emite como o `POST /nfce/emitir` (**efeito fiscal**), com `offline` e `offline_se_necessario`; só com `--permitir-emissao` e o Bearer |
| `consultar_nfce` | consulta pela chave e atualiza a pasta; só com `--permitir-emissao` e o Bearer |
| `cancelar_nfce` | cancela como o `POST /nfce/cancelar` (**efeito fiscal**), com `aviso_prazo`; só com `--permitir-emissao` e o Bearer |
| `status_sefaz_nfce` | estado do serviço da NFC-e da UF (a NFC-e não tem SVC, por isso não é o `status_sefaz`); só com `--permitir-emissao` e o Bearer |
| `pendentes_nfce` | lista, sem rede, as NFC-e pendentes com o prazo, `pode_fechar_caixa` e o aviso; só com `--permitir-emissao` e o Bearer |
| `retomar_pendentes_nfce` | consulta a MESMA chave de cada NFC-e pendente e transmite as off-line (**efeito fiscal**); só com `--permitir-emissao` e o Bearer |
| `manifestar_recebida` | manifestação do destinatário: ciência, confirmação, desconhecimento ou operação não realizada (**efeito fiscal**, assinado com o certificado; não há como desfazer); só com `--permitir-emissao` e o Bearer |

Recusa de regra fiscal volta como resultado da ferramenta com `isError: true`, não como erro de protocolo. Campo que
a DPS não conhece volta listado em `campos_desconhecidos` no `conferir_dps`, e no `emitir_nfse` recusa o pedido
(`isError: true`) antes de qualquer envio. Na NF-e, o pedido com campo desconhecido é sempre recusado (`isError: true`) antes de qualquer envio. A NFC-e também recusa o pedido com campo desconhecido ou de CSC (o CSC só se guarda pela linha de comando; nenhuma ferramenta o recebe). Sem o
Bearer, `tools/list` mostra só as oito de leitura (quatro da NFS-e, `conferir_nfe`, `montar_nfe`, `gerar_danfe` e `gerar_danfce`). Cada ferramenta tem `inputSchema` (JSON Schema). Exemplo:

```
$ curl -s -X POST http://127.0.0.1:8080/mcp \
    -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
    -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/call' -H 'Mcp-Name: conferir_dps' \
    -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
          "name":"conferir_dps","arguments":{...a DPS...},
          "_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28",
                   "io.modelcontextprotocol/clientCapabilities":{}}}}'
```

Fora do que foi implementado: respostas em fluxo (SSE) e `subscriptions/listen`, `GET`/`DELETE` no endpoint (405),
recursos, modelos de mensagem, logging, tarefas e autorização OAuth (as ferramentas de emissão usam o Bearer fixo do operador).

## Interface de desktop

`rustnf-gui` é uma janela para configurar o rustnf sem editar o arquivo TOML. **Só existe para Windows** nesta versão
(o GPUI usado só tem janela no Windows); em outro sistema o executável avisa e sai com código 2. Ela lê e grava o **mesmo
arquivo de configuração** da linha de comando (`--config <arquivo>`, depois `RUSTNF_CONFIG`, depois o local padrão) e
guarda a senha do certificado no Gerenciador de Credenciais do Windows, nunca no arquivo. Aceita só `--config`,
`--version` e `--help`; `RUSTNF_COFRE=desligado` desliga o cofre (a janela avisa que a senha não será guardada).

O que já faz: primeiro uso em três passos (emitente, certificado, pasta das notas); Início com o resumo; Configurações
(emitente, Sefin Nacional com ambiente, certificado, senha, assinatura, tempo limite e folga do relógio, notas, canhoto do
DANFSe, alíquotas por ano e tema); botões "Testar certificado" (abre o `.pfx`, sem rede) e "Testar conexão com a Sefin"
(chamada real à Sefin Nacional, só quando você clica); selo fixo do ambiente (Produção ou Produção restrita) e estado
do certificado no rodapé; a **Nova nota**, as **Notas** (com a conferência na Sefin, o histórico e os eventos), a **Consulta** e a **Prévia** (abaixo). O que ainda **não** faz:
os casos de emissão fora do caso comum (veja o fim da Nova nota), que seguem pela linha de comando. Nas Configurações,
Ctrl+S salva e Ctrl+Shift+D descarta o que foi alterado e ainda não foi salvo.

### Nova nota

Item "Nova nota" da barra lateral (Ctrl+N; o Início tem o botão "Emitir uma nota"). Com a configuração incompleta ela só
explica o que falta. O prestador, a série, o município emissor e o regime vêm do `[emitente]`; o número da DPS vem da pasta
de notas e a data e hora de emissão são automáticas. O formulário cobre o **caso comum**, em uma página:

* **tomador** (CNPJ ou CPF com conferência dos dígitos, endereço nacional, com "Buscar endereço pelo CEP": só no
  clique, o CEP vai ao ViaCEP ou à BrasilAPI; ou "nota sem tomador identificado");
* **serviço**: busca na Lista Nacional por texto ou por código, descrição, município da prestação, NBS e códigos opcionais;
* **valores**: valor do serviço, descontos e competência (dinheiro no formato `1.234,56`);
* **ISS**: tributável no município ou não incidência, retenção pelo tomador, alíquota (a padrão do `[emitente]` aparece
  como dica; o resumo mostra o ISS estimado) e os tributos aproximados que o regime do emitente pede (percentual do
  Simples Nacional, ou totais por esfera);
* **tributos federais** (opcional): IRRF, CSLL, contribuição previdenciária e PIS/COFINS;
* **IBS/CBS**: `cIndOp`, `cClassTrib` (o CST sai dele) e consumidor final, com a **sugestão da correlação do Anexo VIII**
  (NBS, `cIndOp` e `cClassTrib`): ela só propõe, só preenche campo vazio e não escolhe calada quando há mais de uma
  classificação. A biblioteca aceita a nota sem este grupo, então a tela o deixa desligável: a nota nasce com ele
  ligado para quem não é optante do Simples Nacional e desligado para o optante.

**Tomador já usado.** Ao digitar o CPF ou CNPJ completo de alguém que já recebeu nota, a tela procura na pasta de
notas a nota mais recente dele e completa os campos vazios do tomador (nome, inscrição municipal, e-mail, telefone) e
o endereço, com um aviso de qual nota vieram. O que você já digitou fica; com parte do endereço digitada, o endereço
não é mexido.

Um painel fixo ao lado (embaixo, em janela estreita) mostra, enquanto você digita, as **pendências** (clique leva ao campo),
o resumo de valores com o valor líquido estimado, a **prévia do IBS/CBS** (com as `[[aliquotas]]` da configuração; sem
alíquota para o ano, ela diz isso e aponta a aba Alíquotas) e o botão Emitir, desligado com o motivo ao lado enquanto
houver pendência. O erro aparece embaixo do campo, só depois de você passar por ele. O rascunho é salvo sozinho na pasta
de notas (`rascunho.json`) e volta ao reabrir; "Limpar" pergunta antes.

Emitir abre uma confirmação com o ambiente em destaque; **em produção ela só habilita depois de marcar "Entendo que esta
nota tem valor fiscal"**. O envio roda fora da tela e o pedido enviado fica congelado (mesmo número de DPS, mesma data e
hora): se a Sefin não responder, o formulário trava e "Tentar de novo" reenvia exatamente o mesmo pedido (a biblioteca
recupera a nota se ela já existir); "Desistir desta tentativa" avisa que a nota pode ter sido autorizada. O número da
DPS não avança na desistência: se a nota anterior existir, é ela que a Sefin devolve na emissão seguinte, e a tela
avisa quando os dados dela não são os do formulário (que fica como estava, para emitir de novo). Se a Sefin
rejeitar, os dados ficam e as mensagens aparecem no painel. Sem senha no cofre, a tela a pede (só para aquela emissão;
não é guardada). O que a tela não cobre (tomador no exterior, intermediário, comércio exterior, obra, evento, locação,
deduções, reembolsos, substituição, benefício municipal, suspensão, imunidade, destinatário diferente do tomador) se emite
pela linha de comando com o JSON completo. A interface foi conferida com um emissor de demonstração; a primeira nota
real por ela é a prova que falta (veja [Estado](#estado)).

### Notas

Item "Notas" da barra lateral (Ctrl+L). Lista as notas da pasta de notas, da mais recente para a mais antiga, lidas fora
da linha da tela e desenhadas só no que cabe na janela (a pasta pode ter milhares de notas): data, número da NFS-e, DPS
(série e número), tomador (o CPF aparece mascarado, `***.***.***-NN`; o CNPJ, inteiro), valor e situação (autorizada ou
cancelada). A busca (nome sem acento, CPF/CNPJ ou número; uma chave colada também acha a nota) e o filtro por situação
valem juntos. A lista se atualiza sozinha quando a Nova nota autoriza uma nota e quando um cancelamento termina; "Atualizar"
lê a pasta de novo e "Abrir a pasta" mostra a pasta de notas no gerenciador de arquivos do sistema. Na segunda linha,
o período ("Todo o período", "Este mês", "Mês passado", "Este ano", contados a partir de hoje) vale junto com a busca
e o filtro, e "Exportar a lista" pergunta uma pasta e grava nela o XML de cada nota que está na lista e a lista em
CSV (`notas.csv`, o mesmo formato de `rustnf notas listar --csv`): é o que se manda para a contabilidade no fim do
mês. Nada sai da pasta de notas.

**Conferir situação na Sefin** (botão da primeira linha): confere as notas que a lista mostra agora (a busca, o filtro e o
período da tela), uma de cada vez e uma consulta por vez, com pausa entre elas (200 ms), fora da linha da tela. Mostra o
andamento ("Conferindo 12 de 40...") e o botão **Parar**: o que já foi conferido fica gravado na pasta. Com mais de 50
notas, diz antes quanto tempo leva no mínimo (são pelo menos 5 consultas por nota) e pede confirmação. O resultado vem em
palavras: quantas notas mudaram de situação (de Autorizada para Cancelada, por exemplo), quantas já estavam iguais e, de
cada nota que não deu para conferir, o que aconteceu e o que fazer. A pasta nunca volta de cancelada para autorizada por
aqui: a divergência vai para as falhas, para você abrir a nota e conferir. A lista se atualiza sozinha. Pasta vazia, leitura com erro ("Tentar de novo") e busca sem resultado tem tela própria. Setas movem
a linha em foco, Enter abre o detalhe e Esc volta da página do detalhe.

O **detalhe** (ao lado da lista em janela larga; página em janela estreita) mostra os dados lidos do XML guardado: número,
chave de acesso em grupos de 5 com "Copiar a chave", datas, tomador (aqui o documento aparece inteiro), serviço, valores
e situação. XML ausente ou ilegível é dito sem derrubar a tela. Dele saem:

* **Abrir o DANFSe**: gera o PDF fora da tela (com a marca de cancelada, quando for o caso, e o canhoto da configuração),
  grava `<chave>.pdf` ao lado do XML (o nome sai só da chave conferida) e o abre no programa padrão do sistema. O PDF não
  entra no índice nem na numeração. "Mostrar na pasta" mostra o XML no explorador de arquivos e "Copiar o XML" põe o
  XML inteiro da nota na área de transferência (para colar num e-mail ou no sistema da contabilidade);
* **Cancelar a nota** (só nota autorizada): diálogo com o ambiente em destaque, o resumo da nota, o motivo (erro na
  emissão, serviço não prestado ou outros) e a justificativa, de 15 a 255 caracteres, com contador e erro embaixo do campo.
  Em produção o botão só habilita depois de marcar "Entendo que o cancelamento tem efeito fiscal". O envio é feito fora
  da tela, pelo mesmo fluxo de certificado e senha da emissão. Só o cancelamento **registrado** marca a nota como cancelada
  na pasta; rejeitado mostra as mensagens da Sefin e não marca. **Sem resposta**: o pedido tem sempre o mesmo identificador
  (a chave da nota mais o tipo do evento), então reenviar não cria um segundo cancelamento. Se o primeiro envio já tiver sido
  registrado, a Sefin recusa o reenvio por repetição: diante de uma recusa, o programa confere se a nota já tem o
  cancelamento e, tendo, mostra que ela já estava cancelada e marca a pasta. O diálogo oferece "Tentar de novo" (o mesmo
  pedido) e "Conferir na Sefin" (só consulta o evento de cancelamento da nota);
* **Pedir análise fiscal do cancelamento** (só nota autorizada do emitente): pede ao fisco que analise o cancelamento da
  nota; **não cancela a nota na hora**. Mesmo diálogo do cancelamento (ambiente em destaque, resumo, motivo, justificativa
  de 15 a 255 caracteres e, em produção, a marcação "Entendo que este pedido tem efeito fiscal e será analisado pelo
  fisco"). Registrado, a nota passa a "Em análise fiscal" na pasta e na lista. Sem resposta, o pedido **não** é reenviado
  às cegas (um segundo pedido poderia valer como outro): o diálogo oferece "Conferir na Sefin" (só consulta) e, se a Sefin
  disser que não há registro, "Tentar de novo". A ação fica sempre no detalhe e aparece também no desfecho do
  cancelamento recusado por prazo (E0822, Anexo II), já com o motivo e a justificativa que a pessoa escreveu;
* **Histórico na Sefin**: botão que consulta os eventos da nota (cancelamento, substituição, pedido de análise fiscal,
  decisão do fisco, manifestações) e os mostra do mais antigo para o mais novo, com data, quem registrou e o motivo.
  Só consulta; uma consulta por tipo de evento, com a mesma pausa do lote;
* **Usar como modelo**: leva tomador, serviço, descrição e valor da nota para a Nova nota (o que o XML permite ler; o resto
  fica vazio e a conferência da Nova nota aponta), com a competência de hoje. Se a Nova nota tiver algo digitado, pergunta
  antes de substituir; com uma tentativa de emissão pendente, não substitui.

### Nova NF-e (assistente de venda)

Na **Nova nota** (Ctrl+N) há, no alto, a escolha **NFS-e | NF-e (venda de mercadoria)**, a mesma da tela Notas, lembrada de
uma vez para outra. A NF-e é um assistente de cinco passos, em palavras de quem vende; o termo fiscal (CFOP, CST ou CSOSN,
`modFrete`, `indPres`...) é escolhido pelo programa a partir das respostas e **mostrado**, nunca escondido:

1. **Para quem**: CPF ou CNPJ (máscara e dígito verificador), nome, "Buscar endereço pelo CEP" (só no clique), se o cliente
   compra para uso próprio ou para revender, se tem inscrição estadual e como comprou (balcão, internet, telefone). Ao digitar o
   CPF ou o CNPJ de quem já recebeu uma NF-e, o cadastro é completado com a nota mais recente dele (só os campos vazios).
2. **O que vendeu**: busca no cadastro de produtos ou produto novo (NCM obrigatório: o programa não tem a tabela do NCM e diz
   isso), quantidade, valor, desconto, e "Detalhes fiscais" recolhidos.
3. **Como recebeu**: dinheiro, Pix, cartão, cheque, boleto ou a prazo em parcelas (fatura e duplicatas; o resto da divisão vai
   na última parcela, as datas andam de mês em mês).
4. **Como entrega**: balcão, o cliente busca, entrega própria ou transportadora (com os dados dela e os volumes).
5. **Conferir e enviar**: o resumo em palavras, os erros e avisos que a montagem da aplicação devolve traduzidos com o que fazer
   e um botão que leva ao passo, o ambiente em destaque e, **em produção, a marcação "Entendo que a nota tem efeito fiscal"**
   (sem ela o botão não habilita e nenhuma chamada sai). Sem senha no cofre, a tela a pede só para aquela operação.

**O que o programa escolhe, e de onde vem** (a tabela está em `crates/rustnf-gui/src/modelo/nfe_fiscal.rs`, com um teste que
confere cada CFOP na tabela da biblioteca e na vigência):

| pergunta | escolha | fonte |
|---|---|---|
| dentro do estado, outro estado, exterior | `idDest` 1, 2 ou 3 e CFOP 5.xxx, 6.xxx ou 7.xxx (o destino sai das UFs) | MOC 7.00, B11; tabela de CFOP |
| fabrica ou compra para revender | CFOP x.101 ou x.102; **para outro estado, a cliente que não tem inscrição, 6.107 ou 6.108** | tabela de CFOP do Portal da NF-e |
| venda para outro estado a consumidor final sem inscrição, no regime normal | a alíquota interna do ICMS do estado do cliente e o adicional do fundo de combate à pobreza (a pessoa digita; o programa não tem a tabela de alíquotas dos estados): a nota leva o DIFAL, que a biblioteca calcula (alíquota interestadual de 4, 7 ou 12% e partilha do ano) e a tela mostra em "ICMS para o estado do cliente (DIFAL)"; no Simples (CRT 1) e com CST de isenção ou não tributação a nota não leva | MOC 7.00, NA01 a NA17 e rejeição 694; Convênio ICMS 236/2021 |
| ICMS já cobrado antes (só quem revende) | CFOP 5.405 ou 6.404, CSOSN 500 ou CST 60 (base, alíquota e valor retidos a pessoa digita) | tabela de CFOP; MOC, N12 e N12a |
| regime (CRT) | CSOSN com o CRT 1 e 4 (Simples e MEI), CST com o 2 e o 3 | MOC, C21, N12 e N12a; a biblioteca recusa o resto |
| paga ICMS na venda (regime normal) | CST 00; a alíquota a pessoa digita (o programa não chuta a do seu estado) | MOC, N12 |
| PIS e COFINS | CST 49 no Simples (LC 123/2006, art. 13); CST 01 no regime normal, com 0,65% e 3% (cumulativo) ou 1,65% e 7,6% (não cumulativo) | Leis 9.715/1998, 9.718/1998, 10.637/2002 e 10.833/2003 |
| como entrega | `modFrete` 9 (balcão), 4 (o cliente busca), 3 (própria), 0, 1 ou 2 (transportadora) | MOC, X02 |
| como comprou | `indPres` 1, 2, 3 ou 9; nas não presenciais, `indIntermed` 0 | MOC, B25a |
| como pagou | `tPag` 01, 17, 03, 04, 02, 15 (a prazo: `indPag` 1 e `cobr`); cartão leva `tpIntegra` 2 | MOC, YA03 |

**Quando a pergunta simples não decide, a tela mostra as opções e a pessoa escolhe**: o regime do PIS e da COFINS no regime
normal (cumulativo ou não, se a configuração não diz), o ICMS (as opções do regime), a inscrição estadual do cliente CNPJ, o
frete por transportadora (quem paga), o CFOP quando o cliente é isento de inscrição ou compra mercadoria com ICMS já cobrado antes sem ser contribuinte (a tabela tem duas opções). **Detalhes fiscais**: CFOP, CST ou CSOSN e CST do PIS e da COFINS podem ser digitados;
o que a pessoa digita vale no lugar da tabela e **nunca é sobrescrito** (mudar a origem, o ICMS ou o cliente não o desfaz).

**Cadastro de produtos** (`produtos.json` na pasta de notas, lido com tolerância e gravado de forma atômica; arquivo ruim abre
vazio com aviso e não é trocado). Os produtos novos de uma nota **autorizada** entram nele (a pessoa pode desmarcar "Guardar
este produto"), com o NCM, a origem e o detalhe fiscal que ela fixou. A tela **Produtos** (barra lateral) lista, busca, edita e
apaga. **Nota anterior como modelo**: no detalhe da NF-e, "Usar como modelo" lê o XML guardado e preenche o assistente
(cliente, produtos, pagamento, entrega); número, data e protocolo não se copiam e o que não dá para ler fica vazio, para a
conferência apontar. Com o assistente preenchido, pergunta antes de substituir.

**Sem resposta e contingência.** A aplicação grava a nota como pendente **antes** de enviar. Se a SEFAZ não responde, a tela diz
que a nota PODE ter sido autorizada e oferece **Retomar** (consulta a MESMA chave e só reenvia a MESMA nota); não há "emitir de
novo". Rejeitada: o motivo em palavras, o passo onde se corrige e o número continua livre. **EPEC**: no passo final, "Verificar a
SEFAZ" só pergunta o status; se o serviço está parado ou não responde, a tela explica o que é a contingência, pede o motivo
(15 a 255 letras) e oferece "Emitir em contingência (EPEC)", com a marcação em produção. Depois diz que a **mesma nota** tem de
ser transmitida à SEFAZ da UF em até 168 horas (MOC 7.00, Visão Geral, 3.3.6): Notas, aba NF-e, Retomar. **A contingência não é
oferecida enquanto há nota sem resposta na pasta**: o EPEC é outra nota (outro número e outra chave) e, com uma pendente, seria
a gêmea da primeira, que a SEFAZ pode já ter recebido; a aplicação hoje não impede isso, então a tela impede. O rascunho (`rascunho_nfe.json`, arquivo próprio) volta ao reabrir a janela.

Na tela Notas, a NF-e mantém lista, detalhe, DANFE, "Conferir na SEFAZ", cancelar, carta de correção e inutilizar. Nas
Configurações, a seção **NF-e** tem o ambiente próprio, UF, IE, regime, série, endereço, contingência SVC, canhoto e tempos, e o
botão "Testar conexão com a SEFAZ"; o certificado é o da NFS-e. A janela não conta o prazo de cancelamento: o prazo vencido volta
da SEFAZ como recusa 501. Exemplo de demonstração (sem rede): `--nfe-evento registrado|rejeitado|sem-resposta` e
`--nfe-emissao autorizada|rejeitada|sem-resposta|fora`; a pasta de ensaio ganha NF-e de exemplo e um cadastro com três produtos.
A primeira NF-e real pela interface é a prova que falta (veja [Estado](#estado)). Fora do assistente (vão pela linha de comando): venda ao exterior, IPI,
venda por intermediador, valor unitário com mais de duas casas, ST com cálculo próprio, devolução e nota complementar.

### Caixa (NFC-e)

Item "Caixa" da barra lateral (também o seletor "NFC-e" da Nova nota). É a venda rápida do balcão, para quem está com o
cliente na frente: tudo cabe numa tela e o teclado faz o caminho inteiro.

* **Busca** pelo código ou pelo nome no cadastro de produtos da NF-e (o mesmo arquivo `produtos.json`); a quantidade é a do
  campo ao lado (vazio = 1; aceita `0,5`). Enter põe o primeiro produto achado (o código exato ganha). "Produto novo" (ou
  Enter numa busca sem resultado) cadastra nome, preço e o **NCM de 8 dígitos, obrigatório**, e já põe no cupom.
* **Cupom** com o total grande, mais e menos por linha, tirar o item, **desconto** (rateado entre os itens, porque a nota o
  declara por item) e **CPF na nota?** opcional (máscara e conferência do dígito, como na NFS-e).
* **Pagamento**: dinheiro, Pix, cartão de crédito e de débito, em mais de uma forma. O campo "valor recebido" vazio lança o que
  falta; Enter nele lança dinheiro. O **troco** é calculado (só o dinheiro devolve troco) e vai na nota. Ctrl+Enter emite.
* **Depois de autorizada**, o cupom (DANFC-e) sai como arquivo: "Abrir o cupom" (PDF de bobina, no programa do sistema) e "Salvar
  para a impressora (ESC/POS)". **O programa não fala com a impressora**: só gera o arquivo. O caixa já fica pronto para a
  próxima venda e o rascunho do cupom em andamento sobrevive a fechar a janela.
* **Rejeitada**: o motivo em palavras; o número volta livre e o cupom continua para a correção. **Sem resposta**: a nota fica
  pendente e só o Retomar a resolve (consulta a mesma chave; nunca uma segunda NFC-e para o mesmo cupom). **SEFAZ fora antes de
  enviar**: a venda sai sozinha em **contingência off-line**; o cupom vale e a tela diz até quando a nota tem de ser transmitida
  (o prazo que a aplicação calcula, MOC 7.00, Anexo IV).
* **Fechar o caixa** lista as NFC-e por transmitir (off-line, com o prazo, e as sem resposta) e tem "Transmitir agora" (a
  mesma retomada). O fechamento avisa, não impede.
* **Produção**: o ambiente do cupom é o de `[nfce].ambiente` (o selo da janela mostra o dele, que pode diferir do da NF-e). A
  primeira venda da sessão em produção pede "Entendo que a nota tem efeito fiscal" e a marcação vale até fechar a janela ou
  trocar o ambiente (por cupom o caixa ficaria lento; o selo vermelho fica sempre à vista). Sem a marcação, nenhuma chamada sai.

Em **Configurações, NFC-e**: ambiente e série do cupom, versão do QR Code, `idCSC`, largura da bobina, colunas do ESC/POS e o
motivo da contingência. O **CSC** é um campo de senha que grava **no cofre do Windows** (nunca no arquivo, em log, em erro, na
tela nem na nota): a tela só diz "guardado" ou "não guardado" e tem "Apagar". O QR Code versão 3 (padrão) não usa CSC; a
versão 2 usa, e com o cofre desligado (`RUSTNF_COFRE=desligado`) a tela explica que o CSC não pode ser guardado e o caixa explica
por que não emite. O CSC se pega no portal da SEFAZ da sua UF (área do contribuinte, NFC-e), junto com o número dele, o `idCSC`.

Em **Notas** o seletor passa a ser "NFS-e | NF-e | NFC-e": a lista da NFC-e reusa a da NF-e (a off-line aparece como "sem
internet"), com o detalhe, "Abrir o cupom", "Conferir na SEFAZ", "Cancelar a nota" (com o aviso do prazo nacional de 24 horas
ANTES de enviar, sem bloquear: quem decide é a SEFAZ da UF) e "Retomar". A NFC-e não tem carta de correção e a aplicação não
inutiliza números do modelo 65, por isso esses dois não aparecem.

### Consultar

Item "Consultar" (Ctrl+K). Dois modos: **pela chave de acesso** e **pela série e número da DPS**. Na DPS, o município e
o CNPJ (ou CPF) vêm do `[emitente]` da configuração e a série vazia vale a do `[emitente]` (ou 1); a biblioteca monta e
valida o Id de 45 posições, a Sefin diz que nota a DPS gerou e a tela consulta essa nota. A nota devolvida só vale se for a da
DPS pedida. O campo da chave aceita a chave colada com espaços e pontuação, mostra-a em grupos de 5 e diz
embaixo do campo o que falta ou por que não confere (tamanho, formato e dígito verificador, pela biblioteca). Enter no
campo consulta. A consulta é feita fora da tela, com o certificado, e **não altera nada** na Sefin. O resultado é o mesmo
detalhe das Notas, mais: se a nota tem cancelamento registrado (o XML da nota não diz; a tela consulta o evento e, se não
conseguir saber, avisa); se ela já está na pasta de notas, ou o botão "Guardar na pasta de notas" (só para nota do
emitente da configuração; nota de outro emitente não é guardada); "Abrir o DANFSe" (nota que não está na pasta pergunta
onde salvar); e Cancelar. A nota devolvida só vale como resultado se a chave dela for a que se pediu. Não encontrada, sem
resposta e erro têm cada um o seu texto, com "Tentar de novo" quando faz sentido.

**Nota recebida.** Quando a nota é de outro emitente e o tomador é o documento do emitente da configuração (o do
certificado a usar), o resultado oferece **Confirmar que o serviço foi prestado** e **Recusar esta nota** (com um dos
motivos da biblioteca; a justificativa só é obrigatória no motivo "Outros"). O texto do diálogo explica o efeito: a
manifestação fica registrada na nota e não cancela nada, porque o cancelamento é do emitente. Mesmos desfechos e a mesma
confirmação em produção do cancelamento. Nota de outro emitente sem ser o tomador não tem nenhuma ação de evento.

### Prévia

Item "Prévia" (Ctrl+P). Calculadora avulsa do IBS e da CBS, sem emitir nada e sem precisar de certificado nem de pasta de
notas: valor do serviço, desconto incondicionado, data do fato gerador (o ano decide as alíquotas), `cClassTrib` (com busca
por texto na tabela oficial e o nome ao lado), regime simulado (começa pelo do emitente) e, se houver, o ISSQN, o PIS e a
COFINS que saem da base. Usa as `[[aliquotas]]` da configuração; sem alíquota para o ano ela diz isso e aponta a aba
Alíquotas. Mostra a base de cálculo, a alíquota efetiva e o valor do IBS estadual, do IBS municipal e da CBS, e os totais,
com os avisos e as recusas da biblioteca. Fora da tela, porque não se digita ou não faz sentido numa calculadora avulsa:
desconto condicionado, documentos de ajuste da base, salão parceiro e locação (os mesmos casos de `rustnf calcular` com o
JSON completo).

Para clicar em tudo sem falar com a Sefin, há o exemplo `demonstracao` (feature `demonstracao`, desligada por padrão; o
executável normal não tem como entrar nesse modo). Ele abre a mesma janela com um emissor que não usa a rede, com selo e
título próprios. A nota "autorizada" entra de verdade na pasta de notas do arquivo de configuração: use uma pasta de
ensaio. A consulta (pela chave e pela DPS) devolve as notas que a demonstração autorizou e as que estão nessa pasta; o
cancelamento e os pedidos de evento seguem um roteiro, como a emissão (`--cancelamento`, `--evento`). O cancelamento
`rejeitado` reproduz a recusa por prazo (o E0822, com o texto do Anexo II). A demonstração também "recebeu" uma nota de
outra pessoa, com o emitente das Configurações como tomador: ao abrir, o exemplo escreve a chave dela, para você colar em
Consultar e ver Confirmar e Recusar. Os eventos que a demonstração "registrou" voltam no histórico da nota.

```
cargo run -p rustnf-gui                      # modo de depuracao
cargo run -p rustnf-gui -- --config rustnf.toml
cargo build --release -p rustnf-gui          # no Windows, precisa do fxc.exe do Windows SDK

# sem falar com a Sefin: --resultado autorizada (padrao), rejeitada ou sem-resposta (emissao);
# --cancelamento e --evento registrado (padrao), rejeitado ou sem-resposta; --atraso-ms 1200 e a demora de cada "envio";
# --nfce-emissao autorizada (padrao), rejeitada, sem-resposta ou fora (a venda do Caixa; fora = off-line)
cargo run -p rustnf-gui --features demonstracao --example demonstracao -- --config ensaio.toml --resultado autorizada --cancelamento registrado
```

A compilação **release** no Windows precisa do `fxc.exe` (compilador de shaders do Windows SDK): deixe-o no `PATH` ou
aponte a variável `GPUI_FXC_PATH` para ele. O modo de depuração não precisa. Para o executável não depender do
Visual C++ Redistributable, compile com `RUSTFLAGS="-C target-feature=+crt-static"` (é o que o fluxo de release
faz, e ele confere as dependências do executável antes de empacotar). A primeira compilação é demorada (centenas
de crates). O mapa da camada de janela: `crates/rustnf-gui/src/modelo` (estado dos formulários, máscaras, conferência
campo a campo, passos do primeiro uso, lista das notas, detalhe, cancelamento, consulta e prévia avulsa; sem janela e
testável em qualquer sistema), `src/servicos.rs` e `src/servicos` (arquivo, cofre, certificado, conexão, pasta de notas,
DANFSe, cancelar e consultar) e `src/telas` (só desenho).

## Ambientes

| ambiente | `--ambiente` | `tpAmb` | Sefin Nacional |
|---|---|---|---|
| produção | `producao` | 1 | `https://sefin.nfse.gov.br/sefinnacional` |
| produção restrita | `restrita` | 2 | `https://sefin.producaorestrita.nfse.gov.br/SefinNacional` |

A NF-e usa os ambientes da SEFAZ, escolhidos em `[nfe].ambiente`: `homologacao` (`tpAmb` 2, o padrão) e `producao`
(`tpAmb` 1); os endereços de cada UF vêm da tabela oficial do Portal Nacional da NF-e.

A produção restrita é o ambiente de testes da Sefin: use-a até o fluxo estar certo. A DPS e a nota ficam presas ao
ambiente em que foram feitas (regra E0006).

## Como testar sem certificado

* `dps montar`, `dps conferir`, `calcular` e `danfse` não precisam de certificado nem de rede.
* Os testes automáticos geram certificados A1 autoassinados em memória (feature `cert_a1_teste`; nunca valem no
  fisco), usam transporte falso no lugar da Sefin e comparam o XML gerado, texto a texto, com os contratos em
  `crates/rustnf/tests/casos/`. Nenhum teste usa rede nem porta fixa.

```
cargo test -p rustnf --test nfse_nacional
cargo test -p rustnf --test reforma_previa
cargo test -p rustnf --test nfse_sefin
cargo test -p rustnf --test danfse
cargo test -p rustnf-cli
cargo test -p rustnf-app
cargo test -p rustnf-gui
cargo test -p rustnf --test higiene
```

## Contribuir e segurança

Veja [CONTRIBUTING.md](CONTRIBUTING.md) (testes, contrato para mudar regra, teste de higiene) e
[SECURITY.md](SECURITY.md) (como relatar vulnerabilidade, o que é segredo). Plano em `docs/ROADMAP.md`; fontes
oficiais em `docs/FONTES.md`; histórico em `CHANGELOG.md`.

## Licença

[Mozilla Public License 2.0](LICENSE) (`MPL-2.0`). Em resumo: pode usar, inclusive dentro de software fechado; quem
DISTRIBUIR uma versão com arquivos do `rustnf` modificados tem de manter o aviso da licença e disponibilizar o código
desses arquivos sob a MPL 2.0. O texto que vale é o do arquivo `LICENSE`.
