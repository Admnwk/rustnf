// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O modelo da interface: estado dos formularios, conversao de e para a configuracao, mascaras, conferencia campo a
//! campo, estado do certificado, passos do primeiro uso e a Nova nota (estado, pedido, conferencia, previa, sugestao,
//! rascunho e desfecho da emissao) e as telas de notas: lista, detalhe, "usar como modelo", cancelamento, consulta
//! pela chave e previa avulsa. Nenhuma dependencia de janela: compila e testa em qualquer sistema.

pub mod ambiente;
pub mod argumentos;
pub mod campo;
pub mod cancelamento;
pub mod certificado;
pub mod chave;
pub mod conferencia;
pub mod conferir;
pub mod consulta;
pub mod consulta_dps;
pub mod detalhe;
pub mod emissao;
pub mod evento;
pub mod formulario;
pub mod historico;
pub mod lista;
pub mod mascara;
pub mod nfce_caixa;
pub mod nfce_config;
pub mod nfce_desfecho;
pub mod nfce_rascunho;
pub mod nfe_campos;
pub mod nfe_config;
pub mod nfe_detalhe;
pub mod nfe_difal;
pub mod nfe_emissao;
pub mod nfe_eventos;
pub mod nfe_fiscal;
pub mod nfe_lista;
pub mod nfe_modelo;
pub mod nfe_nova;
pub mod nfe_pedido;
pub mod nfe_pendencias;
pub mod nfe_produtos;
pub mod nfe_rascunho;
pub mod nfe_recebidas;
pub mod nfe_recebidas_manifestar;
pub mod nfe_recebidas_resultados;
pub mod nfe_resultados;
pub mod nota;
pub mod passos;
pub mod pedido;
pub mod previa;
pub mod previa_avulsa;
pub mod rascunho;
pub mod reaproveitar;
pub mod sugestao;
pub mod tomador;
