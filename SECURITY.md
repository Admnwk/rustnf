# Seguranca

## Como relatar uma vulnerabilidade

Use o relato privado de vulnerabilidade do GitHub: na pagina do repositorio, aba **Security**, botao
**Report a vulnerability** (em portugues, "Relatar uma vulnerabilidade"). So' quem mantem o repositorio le o relato.
Nao abra uma issue publica nem um pedido de mudanca para um problema de seguranca, e nao mande o relato por e-mail:
este projeto nao usa e-mail para isso.

Descreva o que acontece, como reproduzir (de preferencia com os documentos ficticios de
`crates/rustnf/tests/higiene_documentos.txt`) e qual o efeito. Nunca anexe certificado, senha, token nem nota fiscal
de verdade: se o problema so' aparece com dado real, descreva-o sem enviar o dado.

Vale o que esta no codigo da versao mais recente do ramo principal; nao ha versoes antigas mantidas.

## O que o projeto trata como segredo

* a **senha do certificado A1** (`.pfx`/`.p12`) e a **chave privada** dentro dele;
* o **token Bearer** das rotas de emissao da API (`--token-env`);
* o **XML assinado** da DPS e dos eventos antes de a Sefin Nacional recebe-los.

Como o codigo os trata:

* a senha do certificado nunca vem por argumento de linha de comando (apareceria na lista de processos e no historico
  do shell): so' por variavel de ambiente (`--senha-env`), pelo stdin (`--senha-stdin`) ou pelo **cofre do sistema**;
* no Windows a senha pode ficar no Gerenciador de Credenciais (`rustnf senha guardar`, servico `rustnf`): o sistema a
  protege por usuario e ela nunca e' gravada em arquivo do projeto. Antes de guardar, o programa confere que a senha abre
  o certificado. `rustnf senha apagar` a remove; `RUSTNF_COFRE=desligado` desliga o cofre. Nos outros sistemas o cofre
  nao e' usado. Quem tem acesso a sessao do usuario no Windows tem acesso ao item: o cofre protege a senha em repouso,
  nao de um processo rodando como o proprio usuario;
* o **arquivo de configuracao** (`rustnf.toml`) nao tem campo de senha nem de token: `senha` ou `token` nele e' recusado
  como campo desconhecido, e a mensagem de erro nao repete o valor lido. `rustnf config mostrar` nunca mostra segredo;
* a **pasta de notas** guarda so' o XML das notas autorizadas e o indice (sem certificado, senha nem token); o nome de
  cada arquivo e' montado so' com a chave de acesso conferida (50 posicoes, `0-9A-Z`), entao nada vindo de fora vira
  pedaco de caminho (`..` ou separador); toda gravacao e' atomica;
* senha e token ficam em um tipo com `Debug` redigido e sao zerados da memoria quando soltos;
* o token e' comparado em tempo constante e precisa ter ao menos 32 caracteres; sem ele, `--permitir-emissao` recusa
  subir;
* mensagem de erro, resposta da API e registro do servidor nao levam caminho de arquivo, senha, token, chave de acesso
  nem XML assinado;
* a API escuta so' em `127.0.0.1` por padrao, nao envia cabecalhos CORS, valida o cabecalho `Origin` e limita o
  tamanho do corpo e o tempo de cada requisicao. As rotas de emissao, consulta e cancelamento nem existem sem
  `--permitir-emissao`.

O que **nao** e' protegido pelo programa: o trafego da API e' HTTP puro. Fora de `127.0.0.1`, ponha na frente um proxy
com TLS. Quem tem o token emite e cancela notas com o certificado do servidor: trate o token como a propria senha do
certificado.

## Material de teste

As chaves privadas em `crates/rustnf/tests/casos/pem.json` e os certificados gerados pela feature `cert_a1_teste` sao
**material de teste**: foram feitos so' para verificar a leitura de PEM e a assinatura, nao valem em nenhum ambiente
do fisco e nao protegem nada. Achar uma chave privada ali nao e' vazamento. O arquivo `.gitignore` bloqueia `*.pfx`,
`*.p12`, `*.pem`, `*.key` e `.env` justamente para que material real nao entre por engano.

Para varrer segredo no repositorio, use o `gitleaks` com a configuracao da raiz (`.gitleaks.toml`): ela usa as regras
padrao e so' deixa de fora o arquivo `pem.json` acima. Uma chave em qualquer outro arquivo e' acusada.

## Dependencias

O CI roda `cargo deny check licenses advisories`. Os avisos conhecidos e aceitos estao em `deny.toml`, cada um com o
motivo; mudar essa lista exige justificativa no pedido de mudanca.
