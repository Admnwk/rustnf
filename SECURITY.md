# Segurança

## Como relatar uma vulnerabilidade

Use o relato privado de vulnerabilidade do GitHub: na página do repositório, aba **Security**, botão
**Report a vulnerability** (em português, "Relatar uma vulnerabilidade"). Só quem mantém o repositório lê o relato.
Não abra uma issue pública nem um pedido de mudança para um problema de segurança, e não mande o relato por e-mail:
este projeto não usa e-mail para isso.

Descreva o que acontece, como reproduzir (de preferência com os documentos fictícios de
`crates/rustnf/tests/higiene_documentos.txt`) e qual o efeito. Nunca anexe certificado, senha, token nem nota fiscal
de verdade: se o problema só aparece com dado real, descreva-o sem enviar o dado.

Vale o que está no código da versão mais recente do ramo principal; não há versões antigas mantidas.

## O que o projeto trata como segredo

* a **senha do certificado A1** (`.pfx`/`.p12`) e a **chave privada** dentro dele;
* o **token Bearer** das rotas de emissão da API (`--token-env`);
* o **XML assinado** da DPS e dos eventos antes de a Sefin Nacional recebê-los.

Como o código os trata:

* a senha do certificado nunca vem por argumento de linha de comando (apareceria na lista de processos e no histórico
  do shell): só por variável de ambiente (`--senha-env`), pelo stdin (`--senha-stdin`) ou pelo **cofre do sistema**;
* no Windows a senha pode ficar no Gerenciador de Credenciais (`rustnf senha guardar`, serviço `rustnf`): o sistema a
  protege por usuário e ela nunca é gravada em arquivo do projeto. Antes de guardar, o programa confere que a senha abre
  o certificado. `rustnf senha apagar` a remove; `RUSTNF_COFRE=desligado` desliga o cofre. Nos outros sistemas o cofre
  não é usado. Quem tem acesso à sessão do usuário no Windows tem acesso ao item: o cofre protege a senha em repouso,
  não de um processo rodando como o próprio usuário;
* o **arquivo de configuração** (`rustnf.toml`) não tem campo de senha nem de token: `senha` ou `token` nele é recusado
  como campo desconhecido, e a mensagem de erro não repete o valor lido. `rustnf config mostrar` nunca mostra segredo;
* a **pasta de notas** guarda só o XML das notas autorizadas e o índice (sem certificado, senha nem token); o nome de
  cada arquivo é montado só com a chave de acesso conferida (50 posições, `0-9A-Z`), então nada vindo de fora vira
  pedaço de caminho (`..` ou separador); toda gravação é atômica;
* senha e token ficam em um tipo com `Debug` redigido e são zerados da memória quando soltos;
* o token é comparado em tempo constante e precisa ter ao menos 32 caracteres; sem ele, `--permitir-emissao` recusa
  subir;
* mensagem de erro, resposta da API e registro do servidor não levam caminho de arquivo, senha, token, chave de acesso
  nem XML assinado;
* a API escuta só em `127.0.0.1` por padrão, não envia cabeçalhos CORS, valida o cabeçalho `Origin` e limita o
  tamanho do corpo e o tempo de cada requisição. As rotas de emissão, consulta e cancelamento nem existem sem
  `--permitir-emissao`.

O que **não** é protegido pelo programa: o tráfego da API é HTTP puro. Fora de `127.0.0.1`, ponha na frente um proxy
com TLS. Quem tem o token emite e cancela notas com o certificado do servidor: trate o token como a própria senha do
certificado.

## Material de teste

As chaves privadas em `crates/rustnf/tests/casos/pem.json` e os certificados gerados pela feature `cert_a1_teste` são
**material de teste**: foram feitos só para verificar a leitura de PEM e a assinatura, não valem em nenhum ambiente
do fisco e não protegem nada. Achar uma chave privada ali não é vazamento. O arquivo `.gitignore` bloqueia `*.pfx`,
`*.p12`, `*.pem`, `*.key` e `.env` justamente para que material real não entre por engano.

Para varrer segredo no repositório, use o `gitleaks` com a configuração da raiz (`.gitleaks.toml`): ela usa as regras
padrão e só deixa de fora o arquivo `pem.json` acima. Uma chave em qualquer outro arquivo é acusada.

## Dependências

O CI roda `cargo deny check licenses advisories`. Os avisos conhecidos e aceitos estão em `deny.toml`, cada um com o
motivo; mudar essa lista exige justificativa no pedido de mudança.
