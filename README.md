# rustnf

Emissor aberto de NFS-e do padrao nacional (Sefin Nacional / ADN), escrito em Rust: uma biblioteca e um binario
(`rustnf`) que oferece **linha de comando**, **API HTTP** e **servidor MCP** no mesmo executavel.

> **Aviso: este projeto NAO e' software oficial** da Receita Federal, do Comite Gestor da NFS-e nem de qualquer
> prefeitura. Ele segue a documentacao publica (fontes em `docs/FONTES.md`); a responsabilidade pelo uso em producao e'
> de quem o opera. Emitir ou cancelar nota em producao tem efeito fiscal. Teste antes na producao restrita.

## O que faz

| comando | o que faz |
|---|---|
| `rustnf dps montar` | le o JSON de um pedido e escreve o XML da DPS (nao assinado) |
| `rustnf dps conferir` | confere as regras do Anexo I e diz "ok" ou a recusa (campo e motivo) |
| `rustnf calcular` | previa da base de calculo e do IBS/CBS (Reforma Tributaria, LC 214/2025), como a calculadora da Receita Federal |
| `rustnf danfse` | gera o DANFSe em PDF (NT 008) a partir do XML da NFS-e autorizada |
| `rustnf emitir` | monta, assina com o certificado A1 e envia a DPS a Sefin Nacional (idempotente) |
| `rustnf consultar` | consulta uma NFS-e pela chave de acesso |
| `rustnf cancelar` | cancela uma NFS-e (evento 101101) |
| `rustnf serve` | sobe a API HTTP e o servidor MCP |

Nenhuma regra fiscal mora no binario: ele le a entrada, chama a biblioteca (`crates/rustnf`) e escreve a saida. A
biblioteca monta a DPS (leiaute 1.01 de producao e NT 009 de producao restrita), confere as regras antes do envio,
le o retorno, calcula a previa do IBS/CBS, gera o DANFSe e fala com a Sefin Nacional por mTLS com o A1 em memoria.
Valores e percentuais usam inteiros de ponto fixo, nunca ponto flutuante. A DPS nao leva IBS/CBS calculado (quem
calcula e' a Sefin Nacional): `calcular` e' so' uma previa; vale o que voltar na NFS-e.

## Instalacao

Precisa do Rust (edicao 2021, versao estavel recente).

```
git clone https://github.com/Admnwk/rustnf
cd rustnf
cargo build --release -p rustnf-cli
```

O executavel fica em `target/release/rustnf` (`rustnf.exe` no Windows). Nao e' preciso nenhum programa externo: o
Typst, as fontes e o logotipo do DANFSe vao dentro do binario. A primeira compilacao e' demorada por causa do Typst.

## Linha de comando

Convencoes: dado no stdout, diagnostico no stderr; `--json` onde a saida padrao e' texto; onde se espera um arquivo,
`-` le do stdin. Codigos de saida:

| codigo | significado |
|---|---|
| 0 | ok |
| 1 | falha interna |
| 2 | erro de uso ou de entrada (opcao errada, JSON invalido, arquivo ausente, certificado que nao abre) |
| 3 | recusa de regra (a DPS nao passa no Anexo I, a previa nao pode calcular, a Sefin rejeitou) |
| 4 | erro de rede ou resposta de erro da Sefin Nacional |

Os exemplos abaixo usam os arquivos de `crates/rustnf-cli/exemplos/` (so' documentos ficticios).

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
de casos). Valores aceitam numero ou texto. Atencao: campo desconhecido no pedido e' ignorado em silencio; confira o
nome do campo.

### Previa do IBS/CBS

```
$ rustnf calcular crates/rustnf-cli/exemplos/calculo.json
{
  "bases": { "regime_regular": "80.00", "simples": null, "avisos": [], "erros": [] },
  "recusado": false,
  "tributos": { "base": "80.00", "ibs": "0.08", "cbs": { "aliquota": "0.90", "devido": "0.72", ... }, ... }
}
```

O JSON tem `{"base": {...}}` e/ou `{"tributos": {...}}`. Se vierem os dois e `tributos.base` faltar, vale a base do
regime regular calculada. Se a base registrar recusas de regra (`bases.erros`), a conta sai mesmo assim, `recusado` e'
`true` e o codigo de saida e' 3.

### DANFSe

```
$ rustnf danfse crates/rustnf-cli/exemplos/nfse.xml -o danfse.pdf
DANFSe gravado: 116902 bytes
$ rustnf danfse nfse.xml -o danfse.pdf --situacao cancelada --sem-canhoto
```

`--situacao` aceita `valida` (padrao), `cancelada` e `substituida` (marca d'agua da NT 008). `-o -` escreve o PDF no
stdout.

### Emitir, consultar e cancelar

Precisam do certificado A1 (`.pfx`/`.p12`), do ambiente e da senha. **A senha nunca vem por argumento** (apareceria na
lista de processos e no historico do shell): so' por variavel de ambiente (`--senha-env`) ou pela primeira linha do
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

* `--ambiente producao|restrita` e' obrigatorio e prevalece sobre o `ambiente` do pedido.
* Sem `emissao` (`dhEmi`) no pedido, vale "agora menos 60 s" (`--folga-relogio`), para o relogio local adiantado nao
  cair na regra E0008.
* A emissao e' idempotente: resposta perdida, erro 5xx ou E0014 fazem a biblioteca consultar a DPS antes de reenviar o
  MESMO XML. Nunca renumere a DPS (`numero`) para "tentar de novo".
* Motivos de cancelamento: `1` erro na emissao, `2` servico nao prestado, `9` outros; a justificativa e' obrigatoria.
* `--json` imprime o resultado completo (inclui a NFS-e lida e os alertas); sem ele, um resumo.

## API HTTP

```
rustnf serve                                   # 127.0.0.1:8080, so' leitura
rustnf serve --porta 0                         # qualquer porta livre (o endereco sai no stderr)
```

Opcoes: `--endereco` (padrao `127.0.0.1`), `--porta`, `--limite-corpo` (bytes; padrao 1048576),
`--tempo-limite` (segundos; padrao 60), `--origem-permitida` (repetivel), `--folga-relogio`. Ctrl+C para o servidor
limpo.

| rota | corpo | resposta |
|---|---|---|
| `GET /health` | | `{"status":"ok","versao":"...","emissao":false}` |
| `POST /calcular` | JSON de `calcular` | JSON da previa |
| `POST /dps/conferir` | JSON da DPS | `{"ok":true}` ou 422 com o campo |
| `POST /dps/montar` | JSON da DPS | `{"xml":"..."}` |
| `POST /danfse` | XML da NFS-e (`?situacao=cancelada&canhoto=false`) | `application/pdf` |
| `POST /emitir` | JSON da DPS | JSON do resultado (so' com emissao) |
| `POST /cancelar` | `{"chave","motivo","justificativa"}` | JSON do resultado (so' com emissao) |
| `GET /nfse/{chave}` | | JSON da NFS-e (so' com emissao) |
| `POST /mcp` | mensagem JSON-RPC | veja abaixo |

```
$ curl -s http://127.0.0.1:8080/health
{"emissao":false,"status":"ok","versao":"0.1.0"}

$ curl -s -X POST http://127.0.0.1:8080/calcular -H 'Content-Type: application/json' \
    --data-binary @crates/rustnf-cli/exemplos/calculo.json

$ curl -s -X POST http://127.0.0.1:8080/dps/montar -H 'Content-Type: application/json' \
    --data-binary @crates/rustnf-cli/exemplos/dps.json

$ curl -s -X POST 'http://127.0.0.1:8080/danfse?situacao=valida' --data-binary @crates/rustnf-cli/exemplos/nfse.xml \
    -o danfse.pdf
```

### Emissao pela API

As rotas `/emitir`, `/cancelar` e `/nfse/{chave}` **nem existem** (404) sem `--permitir-emissao`. Com a opcao, o
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

Seguranca: escuta em `127.0.0.1` por padrao (fora dele o trafego e' HTTP puro: ponha um proxy com TLS na frente); sem
CORS; cabecalho `Origin` de fora do proprio computador recebe 403; teto de corpo e tempo limite por requisicao; token
comparado em tempo constante; o PDF e' gerado fora do laco assincrono. Todo erro tem o envelope unico:

```
{"erro": {"codigo": "recusa", "mensagem": "...", "campo": "infDPS/serie"}}
```

Codigos: `entrada_invalida` (400), `nao_autorizado` (401), `nao_encontrada` (404), `corpo_grande` (413),
`recusa` (422), `sefin` (502), `tempo_esgotado` (504), `erro_interno` (500). A mensagem nunca traz caminho de arquivo,
senha, token, chave de acesso nem o XML assinado, e nada disso vai para o registro do servidor (stderr).

## MCP

`POST /mcp` no mesmo servidor, transporte **Streamable HTTP** com resposta JSON (sem SSE) e sem estado. Implementa a
revisao **2026-07-28** da especificacao
(<https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http>): cada requisicao leva
a versao e as capacidades em `_meta` e os cabecalhos `MCP-Protocol-Version`, `Mcp-Method` e `Mcp-Name`; tambem atende
`initialize` da revisao 2025-11-25 para clientes mais antigos. Ferramentas:

| ferramenta | o que faz |
|---|---|
| `calcular_cbs_ibs` | previa do IBS/CBS (mesmo JSON de `calcular`) |
| `conferir_dps` | confere a DPS; recusa volta com `isError: true` |
| `montar_dps` | XML da DPS |
| `gerar_danfse` | PDF em base64, num recurso embutido |
| `emitir_nfse` | so' com `--permitir-emissao` e o Bearer |
| `cancelar_nfse` | so' com `--permitir-emissao` e o Bearer |

Recusa de regra fiscal volta como resultado da ferramenta com `isError: true`, nao como erro de protocolo. Sem o
Bearer, `tools/list` mostra so' as quatro de leitura. Cada ferramenta tem `inputSchema` (JSON Schema). Exemplo:

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
recursos, modelos de mensagem, logging, tarefas e autorizacao OAuth (as ferramentas de emissao usam o Bearer fixo do operador).

## Ambientes

| ambiente | `--ambiente` | `tpAmb` | Sefin Nacional |
|---|---|---|---|
| producao | `producao` | 1 | `https://sefin.nfse.gov.br/sefinnacional` |
| producao restrita | `restrita` | 2 | `https://sefin.producaorestrita.nfse.gov.br/SefinNacional` |

A producao restrita e' o ambiente de testes da Sefin: use-a ate o fluxo estar certo. A DPS e a nota ficam presas ao
ambiente em que foram feitas (regra E0006).

## Como testar sem certificado

* `dps montar`, `dps conferir`, `calcular` e `danfse` nao precisam de certificado nem de rede.
* Os testes automaticos geram certificados A1 autoassinados em memoria (feature `cert_a1_teste`; nunca valem no
  fisco), usam transporte falso no lugar da Sefin e comparam o XML gerado, texto a texto, com os contratos em
  `crates/rustnf/tests/casos/`. Nenhum teste usa rede nem porta fixa.

```
cargo test -p rustnf --test nfse_nacional
cargo test -p rustnf --test reforma_previa
cargo test -p rustnf --test nfse_sefin
cargo test -p rustnf --test danfse
cargo test -p rustnf-cli
cargo test -p rustnf --test higiene
```

## Contribuir e seguranca

Veja [CONTRIBUTING.md](CONTRIBUTING.md) (testes, contrato para mudar regra, teste de higiene) e
[SECURITY.md](SECURITY.md) (como relatar vulnerabilidade, o que e' segredo). Plano em `docs/ROADMAP.md`; fontes
oficiais em `docs/FONTES.md`; historico em `CHANGELOG.md`.

## Licenca

[Mozilla Public License 2.0](LICENSE) (`MPL-2.0`). Em resumo: pode usar, inclusive dentro de software fechado; quem
DISTRIBUIR uma versao com arquivos do `rustnf` modificados tem de manter o aviso da licenca e disponibilizar o codigo
desses arquivos sob a MPL 2.0. O texto que vale e' o do arquivo `LICENSE`.
