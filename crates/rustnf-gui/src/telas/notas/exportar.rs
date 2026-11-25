// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As acoes da tela Notas que mexem com a pasta de notas no sistema de arquivos: "Exportar a lista" (copia os XML e
//! grava o CSV na pasta que a pessoa escolher) e "Abrir a pasta". Tudo o que grava roda fora da linha da tela.

use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, PathPromptOptions, Window};

use super::TelaNotas;
use crate::modelo::certificado::Gravidade;
use crate::modelo::lista::contagem_de_notas;
use crate::servicos::Destino;

impl TelaNotas {
    /// "Exportar a lista": pergunta a pasta de destino e grava nela o que esta na lista.
    pub(super) fn exportar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.exportando || self.lista.quantas() == 0 {
            return;
        }
        let recebe = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Exportar para esta pasta".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(caminhos))) = recebe.await {
                if let Some(destino) = caminhos.into_iter().next() {
                    this.update(cx, |this, cx| this.exportar_para(destino, cx))
                        .ok();
                }
            }
        })
        .detach();
    }

    /// Grava em `destino`, fora da linha da tela, o XML de cada nota da lista e a lista em CSV (`notas.csv`); depois
    /// abre a pasta. Nada sai da pasta de notas.
    pub(in crate::telas) fn exportar_para(&mut self, destino: PathBuf, cx: &mut Context<Self>) {
        let registros = self.lista.registros_visiveis();
        if self.exportando || registros.is_empty() {
            return;
        }
        self.exportando = true;
        self.exportacao = None;
        cx.notify();
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        let pasta = destino.clone();
        cx.spawn(async move |this, cx| {
            let quantas = registros.len();
            let r = cx
                .background_spawn(async move { servicos.exportar_notas(&config, &registros, &pasta) })
                .await;
            this.update(cx, |this, cx| {
                this.exportando = false;
                let nome = destino
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                this.exportacao = Some(match r {
                    Ok(feito) if feito.sem_xml.is_empty() => {
                        if this.servicos.abridor().pedir(Destino::Abrir, &destino) {
                            cx.open_with_system(&destino);
                        }
                        (
                            Gravidade::Ok,
                            format!("{} exportada(s) para a pasta {nome}: os XML e a lista notas.csv.", contagem_de_notas(quantas, quantas)),
                        )
                    }
                    Ok(feito) => (
                        Gravidade::Aviso,
                        format!(
                            "{} exportada(s) para a pasta {nome}, mas {} sem o XML na pasta de notas: elas estão na lista notas.csv, sem o arquivo.",
                            contagem_de_notas(quantas, quantas),
                            contagem_de_notas(feito.sem_xml.len(), feito.sem_xml.len())
                        ),
                    ),
                    Err(e) => (Gravidade::Erro, e.texto()),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// "Abrir a pasta": pede ao sistema que abra a pasta de notas inteira no gerenciador de arquivos.
    pub(super) fn abrir_a_pasta(&mut self, cx: &mut Context<Self>) {
        let pasta = PathBuf::from(self.config.notas.pasta.trim());
        if pasta.as_os_str().is_empty() {
            return;
        }
        if self.servicos.abridor().pedir(Destino::Abrir, &pasta) {
            cx.open_with_system(&pasta);
        }
    }
}
