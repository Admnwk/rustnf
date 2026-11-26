// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Cliente dos webservices da SEFAZ para a NF-e modelo 55, leiaute 4.00: tabela oficial de enderecos, pedidos e
//! envelope SOAP 1.2, leitura das respostas e o cliente que envia a nota, consulta o recibo e a chave e recupera a
//! nota cuja resposta se perdeu. **Eventos (cancelamento, CC-e, EPEC), inutilizacao e contingencia SVC nao estao
//! aqui.**
//!
//! * [`tabela`]: os enderecos por autorizador e ambiente, lidos das paginas "Relacao de Servicos Web" do Portal
//!   Nacional da NF-e, e a UF -> autorizador;
//! * [`pedidos`]: `consStatServ`, `enviNFe`, `consReciNFe`, `consSitNFe` e o envelope SOAP 1.2 (puro, sem rede);
//! * [`resposta`]: o SOAP de volta, o retorno recortado do jeito que a SEFAZ mandou e a leitura de
//!   `retConsStatServ`, `retEnviNFe`, `retConsReciNFe` e `retConsSitNFe` (puro);
//! * [`ClienteSefaz`]: `status`, `autorizar`, `retorno` e `consultar`, sobre o trait [`Transporte`] (falso nos testes,
//!   `reqwest` com mTLS no real);
//! * [`limbo`]: a varredura das notas enviadas sem resultado (quem guarda a lista e' a aplicacao).
//!
//! # A regra que o cliente nao quebra
//!
//! Resposta perdida (tempo esgotado, conexao caida, corpo ilegivel) NAO quer dizer que a nota nao saiu. Antes de
//! qualquer reenvio o cliente consulta a chave: achou autorizada, devolve a nota (`recuperada`); a SEFAZ diz que nao
//! consta (217), ai o reenvio da MESMA NF-e e' seguro; sem resposta nem da consulta, o resultado manda consultar de
//! novo e nunca emitir outra. A duplicidade (204) tambem se resolve pela consulta. Nenhum caminho gera nota gemea com
//! outra chave: o cliente so' envia a NF-e que recebeu, assinada, sem mexer em um byte.
//!
//! # Fontes
//!
//! MOC 7.00, Visao Geral: 4.2.2 (SOAP 1.2, TLS 1.2 ou superior com autenticacao mutua, mensagem em `nfeDadosMsg`),
//! 4.4.1 (o 4.00 nao tem o cabecalho `nfeCabecMsg`), 4.3.1 e 4.3.2 (servicos sincronos e assincronos) e o capitulo 5
//! (`NfeAutorizacao`, `NfeRetAutorizacao`, `NfeConsultaProtocolo`, `NfeStatusServico`); Anexo I, 4.1.3 (grupo D,
//! regras GAP03a) e 4.2.1 (regras 204 e 539); NT 2023.002 v1.02 (fim da autorizacao assincrona, rejeicao 129). A lista
//! completa esta em `docs/FONTES.md`.

mod cliente;
mod erro;
mod fluxo;
pub mod limbo;
pub mod pedidos;
pub mod resposta;
mod resultado;
pub mod tabela;
mod transporte;

pub use cliente::{ClienteSefaz, Config};
pub use erro::ErroSefaz;
pub use resultado::{Desfecho, Resultado, SituacaoServico, StatusServico};
pub use tabela::{Endereco, Servico};
pub use transporte::{FalhaRede, Transporte, TransporteReqwest};
