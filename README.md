# rustnf

Emissor aberto de NFS-e do padrão nacional (Sefin Nacional / ADN), escrito em Rust: uma biblioteca, um binário
(`rustnf`) que oferece **linha de comando**, **API HTTP** e **servidor MCP** no mesmo executável, e uma
**interface de desktop** para Windows (`rustnf-gui`).

> **Aviso: este projeto NÃO é software oficial** da Receita Federal, do Comitê Gestor da NFS-e nem de qualquer
> prefeitura. Ele segue a documentação pública (fontes em `docs/FONTES.md`); a responsabilidade pelo uso em produção é
> de quem o opera. Emitir ou cancelar nota em produção tem efeito fiscal. Teste antes na produção restrita.

![Nova nota: o formulário, a conferência e a prévia do IBS/CBS](docs/imagens/nova-nota.png)

![Notas: a pasta de notas, com busca e filtro](docs/imagens/notas.png)

As telas acima são da interface de desktop, com dados fictícios.

## Emitir uma nota: passo a passo

Pela interface de desktop (Windows):

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

**Nenhuma nota real foi emitida com este código até agora**, nem em produção restrita. O que existe hoje:

| parte | como foi conferida | o que falta |
|---|---|---|
| XML da DPS e regras antes do envio | contratos em `crates/rustnf/tests/casos/`, comparados texto a texto; regras lidas no Anexo I oficial | a Sefin Nacional aceitar uma DPS gerada aqui |
| assinatura | conferida pelo próprio verificador e por um XML assinado fora da biblioteca | a Sefin Nacional validar uma assinatura feita aqui, em SHA-256 (o padrão) ou em SHA-1 (`--assinatura sha1`) |
| cliente da Sefin Nacional | transporte falso (resposta perdida, 5xx, E0014, consulta da nota, da DPS e de evento, cancelamento, pedido de análise fiscal, conferência da situação das notas em lote) | uma chamada real, com TLS mútuo e certificado A1; **nenhuma consulta de DPS, consulta ou registro de evento e nenhuma atualização de situação foi feita contra a Sefin**, e o que ela responde aos eventos de manifestação (202201 e seguintes) e às sequências de um mesmo tipo de evento não foi conferido |
| prévia do IBS/CBS | casos de teste da calculadora oficial da Receita Federal | comparar com os valores de uma NFS-e autorizada |
| montagem e assinatura da NF-e (feature `nfe`) | 188 XML de contrato validados no XSD oficial 4.00 (pacote PL_010h v1.03), regras com o código da rejeição, assinatura conferida pelo verificador da própria biblioteca | **nenhum envio a SEFAZ e nenhum XML conferido por uma SEFAZ real**; o XML não foi comparado com uma nota autorizada |
| cliente da SEFAZ para a NF-e (feature `nfe_sefaz`) | transporte falso (autorizada, denegada, rejeitada, lote em processamento, resposta perdida, duplicidade 204 e 539, Fault, HTTP de erro, corte do assíncrono), esquemas XSD oficiais para os pedidos e o `nfeProc` e um simulador local (fora do repositório) | **nenhuma chamada a uma SEFAZ real e nenhum certificado A1 real**; o espaço de nomes e a ação SOAP de cada serviço não foram conferidos no WSDL publicado; as raízes da ICP-Brasil não foram testadas contra o servidor de uma SEFAZ |
| DANFSe | contrato de conteúdo e leiaute | comparar com um DANFSe emitido pelo portal |
| interface de desktop (`rustnf-gui`, só Windows) | testes do modelo (máscaras, conferência, pedido, prévia, sugestão, rascunho, lista das notas, detalhe, cancelamento, conferência em lote, histórico de eventos, pedido de análise fiscal, confirmação e recusa de nota recebida, consulta pela chave e pela DPS e prévia avulsa), dos fluxos com emissor de demonstração e pasta temporária e da janela no ambiente de teste do GPUI; uso manual da janela em modo de depuração e pelo exemplo de demonstração | **emitir, consultar ou cancelar uma nota real pela janela: nenhuma nota real foi emitida, consultada nem cancelada por esta interface, e nenhuma consulta de DPS ou de evento, nenhuma conferência de situação e nenhum evento (análise fiscal, confirmação, recusa) foi feito por ela contra a Sefin; só o emissor de demonstração respondeu**; o DANFSe aberto no leitor de PDF de outra máquina; uma chamada real à Sefin pelo botão "Testar conexão"; exame da janela em outras escalas de tela; o executável de release aberto numa máquina sem as ferramentas de desenvolvimento |

Se você testar, conte como foi: abra um relato pelo modelo **"Relato de emissão"** (aba Issues), mesmo quando der
certo. Correção é bem-vinda por pedido de mudança, com a fonte oficial citada e um caso no contrato (veja
`CONTRIBUTING.md`). Comece sempre pela produção restrita. Se a assinatura for recusada, repita com
`--assinatura sha1` e diga no relato qual das duas passou.

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
| `rustnf certificado mostrar` | mostra o titular, o documento, a validade e os avisos do certificado A1 |
| `rustnf sefin testar` | testa a conexão com a Sefin Nacional e a aceitação do certificado |

Nenhuma regra fiscal mora no binário: ele lê a entrada, chama a biblioteca (`crates/rustnf`) e escreve a saída. A
biblioteca monta a DPS (leiaute 1.01 de produção e NT 009 de produção restrita), confere as regras antes do envio,
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
rejeitada ou ficou sem resposta (veja `crates/rustnf/examples/nfe_sefaz.rs`). **Ainda não há eventos (cancelamento,
CC-e, EPEC), inutilização, contingência SVC, DANFE, linha de comando, API nem interface para a NF-e, e o modelo 65
(NFC-e) é recusado.** A leitura não confere a assinatura XML.

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
`RUSTNF_TEMPO_LIMITE` e `RUSTNF_ORIGENS_PERMITIDAS` (separadas por vírgula). Campo desconhecido no arquivo é recusado,
e os valores são conferidos (ambiente, código do município, CNPJ/CPF, números, anos de alíquota repetidos...) com a
seção e o campo no erro.

| seção | campos |
|---|---|
| `[emitente]` | `cnpj` ou `cpf`, `nome`, `inscricao_municipal`, `municipio` (IBGE, 7 dígitos), `op_simples` (1, 2 ou 3), `regime_apuracao` (0 = não informar), `regime_especial`, `serie`, `aliquota_iss` (vazio = não informar) |
| `[sefin]` | `ambiente` (`producao` ou `restrita`), `pfx`, `assinatura` (`sha256` ou `sha1`), `tempo_limite_s`, `folga_relogio_s` |
| `[notas]` | `pasta` (vazio = não guardar) |
| `[danfse]` | `canhoto` |
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
Nenhum destes comandos foi chamado contra a Sefin Nacional: veja [Estado](#estado).

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
| `GET /health` | | `{"status":"ok","versao":"...","emissao":false}` |
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

```
$ curl -s http://127.0.0.1:8080/health
{"emissao":false,"status":"ok","versao":"0.3.0"}

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

Recusa de regra fiscal volta como resultado da ferramenta com `isError: true`, não como erro de protocolo. Campo que
a DPS não conhece volta listado em `campos_desconhecidos` no `conferir_dps`, e no `emitir_nfse` recusa o pedido
(`isError: true`) antes de qualquer envio. Sem o
Bearer, `tools/list` mostra só as quatro de leitura. Cada ferramenta tem `inputSchema` (JSON Schema). Exemplo:

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
pela linha de comando com o JSON completo. **Nenhuma nota real foi emitida por esta interface.**

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
# --cancelamento e --evento registrado (padrao), rejeitado ou sem-resposta; --atraso-ms 1200 e a demora de cada "envio"
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
