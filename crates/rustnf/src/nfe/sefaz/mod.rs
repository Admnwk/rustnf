// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Cliente dos webservices da SEFAZ para a NF-e modelo 55, leiaute 4.00: tabela oficial de enderecos, pedidos e
//! envelope SOAP 1.2, leitura das respostas e o cliente que envia a nota, consulta o recibo e a chave, recupera a
//! nota cuja resposta se perdeu, registra eventos (cancelamento, carta de correcao, EPEC, manifestacao do destinatario e
//! eventos da apuracao do IBS/CBS), inutiliza numeracao, envia a SEFAZ Virtual de Contingencia (SVC) e baixa os documentos
//! de interesse de um CNPJ ou CPF (distribuicao de DF-e do Ambiente Nacional).
//!
//! * [`tabela`]: os enderecos por autorizador e ambiente, lidos das paginas "Relacao de Servicos Web" do Portal
//!   Nacional da NF-e, e a UF -> autorizador;
//! * [`pedidos`]: `consStatServ`, `enviNFe`, `consReciNFe`, `consSitNFe` e o envelope SOAP 1.2 (puro, sem rede);
//! * [`evento`]: o `envEvento` de cada evento, assinado com o A1 (puro);
//! * [`dfe`]: a distribuicao de DF-e (`NFeDistribuicaoDFe`): o pedido, o cursor de NSU, os documentos abertos e lidos e
//!   o cliente (`distribuir`, `sincronizar`);
//! * [`inutilizacao`]: o `inutNFe`, assinado, e a data em que AC, BA e SP perderam o servico (puro);
//! * [`svc`]: qual SVC atende cada UF (Ato COTEPE/ICMS 39/12) e o `tpEmis` de cada uma;
//! * [`resposta`] e [`resposta_evento`]: o SOAP de volta, o retorno recortado do jeito que a SEFAZ mandou, a leitura
//!   dos retornos, o `nfeProc`, o `procEventoNFe` e o `ProcInutNFe` (puro);
//! * [`ClienteSefaz`]: `status`, `autorizar`, `retorno`, `consultar`, `cancelar`, `corrigir`, `registrar_epec`,
//!   `registrar_evento_rtc`, `manifestar` e `inutilizar`, sobre o trait [`Transporte`] (falso nos testes, `reqwest` com
//!   mTLS no real);
//! * [`limbo`]: a varredura das notas enviadas sem resultado (quem guarda a lista e' a aplicacao).
//!
//! # A regra que o cliente nao quebra
//!
//! Resposta perdida (tempo esgotado, conexao caida, corpo ilegivel) NAO quer dizer que a nota nao saiu. Antes de
//! qualquer reenvio o cliente consulta a chave: achou autorizada, devolve a nota (`recuperada`); a SEFAZ diz que nao
//! consta (217), ai o reenvio da MESMA NF-e e' seguro; sem resposta nem da consulta, o resultado manda consultar de
//! novo e nunca emitir outra. A duplicidade (204) tambem se resolve pela consulta. Nenhum caminho gera nota gemea com
//! outra chave: o cliente so' envia a NF-e que recebeu, assinada, sem mexer em um byte. A regra vale para o evento: se
//! a resposta se perde, o cliente consulta a nota (que devolve os eventos) antes de qualquer reenvio, e nunca cria um
//! segundo cancelamento ou uma carta de sequencia nova por causa disso.
//!
//! # Fontes
//!
//! MOC 7.00, Visao Geral: 4.2.2 (SOAP 1.2, TLS 1.2 ou superior com autenticacao mutua, mensagem em `nfeDadosMsg`),
//! 4.4.1 (o 4.00 nao tem o cabecalho `nfeCabecMsg`), 4.3.1 e 4.3.2 (servicos sincronos e assincronos) e o capitulo 5
//! (`NfeAutorizacao`, `NfeRetAutorizacao`, `NfeConsultaProtocolo`, `NfeStatusServico`); Anexo I, 4.1.3 (grupo D,
//! regras GAP03a) e 4.2.1 (regras 204 e 539); NT 2023.002 v1.02 (fim da autorizacao assincrona, rejeicao 129, e fim
//! da inutilizacao em AC, BA e SP); MOC 7.00, Visao Geral, 3.3 e 5.3 a 5.12 (EPEC, inutilizacao e eventos); NT
//! 2025.002-RTC v1.53, item 8 (eventos da apuracao do IBS/CBS); Ato COTEPE/ICMS 39/12, art. 1o, na redacao do Ato
//! 110/22 (SVC). A lista completa esta em `docs/FONTES.md`.

mod cliente;
mod cliente_eventos;
pub mod dfe;
mod erro;
pub mod evento;
mod fluxo;
pub mod inutilizacao;
pub mod limbo;
pub mod pedidos;
pub mod resposta;
pub mod resposta_evento;
mod resultado;
mod resultado_evento;
pub mod svc;
pub mod tabela;
mod transporte;

pub use cliente::{ClienteSefaz, Config};
pub use cliente_eventos::protocolo_epec_danfe;
pub use erro::ErroSefaz;
pub use inutilizacao::PedidoInutilizar;
pub use resultado::{Desfecho, Resultado, SituacaoServico, StatusServico};
pub use resultado_evento::{
    DesfechoEvento, DesfechoInutilizacao, ResultadoEvento, ResultadoInutilizacao,
};
pub use svc::Svc;
pub use tabela::{Endereco, Servico};
pub use transporte::{FalhaRede, Transporte, TransporteReqwest};
