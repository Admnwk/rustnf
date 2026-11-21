# Fontes oficiais

Tudo que o rustnf implementa vem de documento público. Data de leitura: 2026-11-20.

| fonte | onde | usada em |
|---|---|---|
| Esquemas XSD da NFS-e Nacional v1.01 (20260209) e da produção restrita (20261020) | <https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica> | `nfse::dps`, `nfse::base` (padrões, ordem das tags) |
| Anexo I (Sefin ADN, DPS/NFS-e): regras de validação e códigos de rejeição (E0008, E0037, E0312, E0710, E0712, E0713...) | mesma página | `nfse::dps`, `nfse::leitura` |
| Nota Técnica 008 (DANFSe v2.0): leiaute da página, blocos, fonte (métrica da Arial), logotipo, marca d'água de NFS-e cancelada e substituída, QR de consulta | <https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica> | `danfse` |
| Anexo B: Lista Nacional de serviços v1.01-20260122 | <https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/documentacao-atual/anexo_b-nbs2-lista_servico_nacional-snnfse-v1-01-20260122.xlsx> | `dados/ctribnac.json` |
| Anexo VII (cIndOp) v1.03.00 | <https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/anexovii-indop_ibscbs_v1-03-00-nt009.xlsx> | `dados/indop.json` |
| Anexo VIII (correlação item x NBS x cIndOp x cClassTrib) v1.02.00 | <https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/anexo_viii-tabela-correlacao-v01-02-00-20261026.xlsx> | `dados/nfse_correlacao.json` |
| Tabela de CST e cClassTrib (NT 2025.002) | <https://dfe-portal.svrs.rs.gov.br/Cff/ClassificacaoTributaria> | `dados/cclasstrib.json` |
| Calculadora RTC da Receita Federal (regime regular e Simples Nacional), licença MIT | <https://github.com/ReceitaFederal/calculadora-rtc-regime-regular> e <https://github.com/ReceitaFederal/calculadora-rtc-simples-nacional> | `dados/atividades_sn.json`, `tests/casos/reforma_atividades_sn.json` |
| Calculadora RTC da Receita Federal, versão 1.7.2: serviço `base-calculo-unificada-nfse`, cálculo do regime geral e o registro de decisão ADR003 (precisão numérica) | <https://github.com/ReceitaFederal/calculadora-rtc-regime-regular> | `reforma::previa`, `tests/casos/reforma_previa.json` |
| Anexo VI (leiaute e regras do IBS/CBS na NFS-e) v1.04.01 | <https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica> | `reforma::previa` (de onde vem cada valor do grupo IBSCBS da NFS-e) |
| Lei Complementar 214/2025 | <https://www.planalto.gov.br/ccivil_03/leis/lcp/lcp214.htm> | `reforma::aliquotas`, `reforma::tabela` |
| CNPJ alfanumérico (Receita Federal) | <https://www.gov.br/receitafederal/pt-br/acesso-a-informacao/acoes-e-programas/programas-e-atividades/cnpj-alfanumerico> | `chave_dfe` (dígito verificador) |
| Plano Geral de Códigos Nacionais (PGCN), dados abertos da ANATEL: código IBGE, UF e nome dos municípios | <https://www.anatel.gov.br/dadosabertos/PDA/Codigo_Nacional/PGCN.csv> | `dados/municipios.csv` |
| Model Context Protocol, especificação 2026-07-28 (transporte Streamable HTTP, ciclo de versão por requisição, `server/discover`, `tools/list`, `tools/call`, erros `-32020` e `-32022`, cache `ttlMs`/`cacheScope`) e, para a compatibilidade com `initialize`, a 2025-11-25 (ciclo de vida e transporte) | <https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http> e <https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle> | `rustnf-cli`, módulo `mcp` |
| XML Signature e Canonical XML 1.0 (W3C), RFC 7468 (PEM) | <https://www.w3.org/TR/xmldsig-core1/>, <https://www.w3.org/TR/2001/REC-xml-c14n-20010315>, <https://www.rfc-editor.org/rfc/rfc7468> | `assinatura` |

## Regras do Anexo I conferidas

* **E0710 / E0712 / E0713** (`totTrib` por regime do Simples Nacional): MEI não informa `pTotTribSN`; ME/EPP não informa
  `indTotTrib`; não optante não informa `indTotTrib` nem `pTotTribSN`. Casos em `tests/casos/nfse_regras_novas.json`.
* **E0008** (`dhEmi` posterior ao processamento): a biblioteca recebe o `dhEmi` explícito;
  `nfse::base::agora_com_folga` gera "agora menos N segundos" (o binário `rustnf` usa 60 s por padrão, `--folga-relogio`).
* **E0037** e **E0312**: não são conferidas antes do envio (dependem do cadastro da Sefin); a leitura do retorno
  acrescenta uma dica (`nfse::leitura::Mensagem::dica`).
