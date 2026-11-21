// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O modelo da interface: estado dos formularios, conversao de e para a configuracao, mascaras, conferencia campo a
//! campo, estado do certificado, passos do primeiro uso e a Nova nota (estado, pedido, conferencia, previa, sugestao,
//! rascunho e desfecho da emissao). Nenhuma dependencia de janela: compila e testa em qualquer sistema.

pub mod ambiente;
pub mod argumentos;
pub mod campo;
pub mod certificado;
pub mod conferencia;
pub mod emissao;
pub mod formulario;
pub mod mascara;
pub mod nota;
pub mod passos;
pub mod pedido;
pub mod previa;
pub mod rascunho;
pub mod sugestao;
