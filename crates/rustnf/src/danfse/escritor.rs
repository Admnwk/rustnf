// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Escrita do PDF: executa, uma a uma, as operacoes de desenho de `dados()["pagina"]` (cm a partir do canto superior
//! esquerdo; texto com `y` na linha de base) sobre uma pagina A4, dentro do processo e sem fonte do sistema. As
//! fontes entram no binario por `include_bytes!`. O texto e' desenhado por glifo: um caractere, um glifo, com o
//! avanco da propria fonte e sem kerning, a mesma regra da tabela de larguras que decide a quebra de linha.

use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point, Rect, Size as Tamanho, Transform};
use krilla::image::Image;
use krilla::metadata::Metadata;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, Stroke};
use krilla::surface::Surface;
use krilla::text::{Font, GlyphId, KrillaGlyph};
use krilla::Document;
use serde_json::Value;
use skrifa::charmap::Charmap;
use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::GlyphMetrics;
use skrifa::{FontRef, MetadataProvider};

use super::medidas::{f, s, CM};
use super::ErroDanfse;

/// Liberation Sans (metrica da Arial, NT 008 item 2.4): regular e negrito, licenca SIL OFL 1.1.
const REGULAR: &[u8] = include_bytes!("../../dados/fontes/LiberationSans-Regular.ttf");
const NEGRITO: &[u8] = include_bytes!("../../dados/fontes/LiberationSans-Bold.ttf");
/// Logotipo da NFS-e (NT 008 item 2.4.3).
const LOGO: &[u8] = include_bytes!("../../dados/danfse_logo_nfse.png");
/// A4 em pontos (210 x 297 mm).
const LARGURA_A4: f32 = 595.276;
const ALTURA_A4: f32 = 841.89;

fn erro(motivo: impl std::fmt::Display) -> ErroDanfse {
    ErroDanfse::Pdf(motivo.to_string())
}

/// Uma fonte pronta para desenhar: a do PDF, o mapa caractere -> glifo e os avancos, tudo lido do proprio arquivo.
struct Fonte {
    pdf: Font,
    mapa: Charmap<'static>,
    avancos: GlyphMetrics<'static>,
    upem: f32,
    altura_maiuscula: f32,
}

impl Fonte {
    fn nova(bytes: &'static [u8]) -> Result<Self, ErroDanfse> {
        let ref_ = FontRef::new(bytes).map_err(|_| erro("fonte embutida ilegivel"))?;
        let metricas = ref_.metrics(Size::unscaled(), LocationRef::default());
        let upem = f32::from(metricas.units_per_em);
        Ok(Fonte {
            pdf: Font::new(bytes.into(), 0).ok_or_else(|| erro("fonte embutida recusada"))?,
            mapa: ref_.charmap(),
            avancos: ref_.glyph_metrics(Size::unscaled(), LocationRef::default()),
            upem,
            altura_maiuscula: metricas.cap_height.unwrap_or(0.7 * upem) / upem,
        })
    }

    /// Os glifos do texto (caractere sem glifo vira o `.notdef`) e a largura total, em unidades de 1 pt de corpo.
    fn glifos(&self, texto: &str) -> (Vec<KrillaGlyph>, f32) {
        let mut largura = 0.0;
        let glifos = texto
            .char_indices()
            .map(|(i, c)| {
                let id = self.mapa.map(c).unwrap_or_default();
                let avanco = self.avancos.advance_width(id).unwrap_or(0.0) / self.upem;
                largura += avanco;
                let fim = i + c.len_utf8();
                KrillaGlyph::new(
                    GlyphId::new(id.to_u32()),
                    avanco,
                    0.0,
                    0.0,
                    0.0,
                    i..fim,
                    None,
                )
            })
            .collect();
        (glifos, largura)
    }
}

struct Fontes {
    regular: Fonte,
    negrito: Fonte,
}

impl Fontes {
    fn escolher(&self, negrito: bool) -> &Fonte {
        if negrito {
            &self.negrito
        } else {
            &self.regular
        }
    }
}

fn pt(cm: &Value) -> f32 {
    (f(cm) * CM) as f32
}

fn cor(nome: &str) -> rgb::Color {
    match nome {
        "vermelho" => rgb::Color::new(255, 0, 0),
        _ => rgb::Color::new(0, 0, 0),
    }
}

fn preencher(c: rgb::Color, opacidade: f32) -> Option<Fill> {
    Some(Fill {
        paint: c.into(),
        opacity: NormalizedF32::new(opacidade).unwrap_or(NormalizedF32::ONE),
        rule: Default::default(),
    })
}

fn tracar(largura: f32) -> Option<Stroke> {
    Some(Stroke {
        paint: rgb::Color::new(0, 0, 0).into(),
        width: largura,
        ..Default::default()
    })
}

fn retangulo(sup: &mut Surface, op: &Value) -> Result<(), ErroDanfse> {
    let (x, y, w, h) = (pt(&op["x"]), pt(&op["y"]), pt(&op["w"]), pt(&op["h"]));
    let borda = f(&op["borda"]) as f32;
    let area = Rect::from_xywh(x, y, w, h).ok_or_else(|| erro("retangulo sem area"))?;
    let mut caminho = PathBuilder::new();
    caminho.push_rect(area);
    let caminho = caminho.finish().ok_or_else(|| erro("retangulo invalido"))?;
    let cinza = op["cinza"].as_bool().unwrap_or(false);
    sup.set_fill(
        cinza
            .then(|| preencher(rgb::Color::new(242, 242, 242), 1.0))
            .flatten(),
    );
    sup.set_stroke((borda > 0.0).then(|| tracar(borda)).flatten());
    sup.draw_path(&caminho);
    Ok(())
}

fn linha(sup: &mut Surface, op: &Value) -> Result<(), ErroDanfse> {
    let mut caminho = PathBuilder::new();
    caminho.move_to(pt(&op["x1"]), pt(&op["y1"]));
    caminho.line_to(pt(&op["x2"]), pt(&op["y2"]));
    let caminho = caminho.finish().ok_or_else(|| erro("linha invalida"))?;
    sup.set_fill(None);
    sup.set_stroke(tracar(f(&op["esp"]) as f32));
    sup.draw_path(&caminho);
    Ok(())
}

/// O logotipo, esticado para `w` x `h` cm com o canto superior esquerdo em (`x`, `y`).
fn imagem(sup: &mut Surface, op: &Value) -> Result<(), ErroDanfse> {
    let figura = Image::from_png(LOGO.into(), true).map_err(erro)?;
    let tamanho =
        Tamanho::from_wh(pt(&op["w"]), pt(&op["h"])).ok_or_else(|| erro("logotipo sem area"))?;
    sup.push_transform(&Transform::from_translate(pt(&op["x"]), pt(&op["y"])));
    sup.draw_image(figura, tamanho);
    sup.pop();
    Ok(())
}

fn texto(sup: &mut Surface, fontes: &Fontes, op: &Value) {
    let fonte = fontes.escolher(op["b"].as_bool().unwrap_or(false));
    let corpo = f(&op["s"]) as f32;
    let t = s(&op["txt"]);
    let (glifos, _) = fonte.glifos(t);
    sup.set_fill(preencher(cor(s(&op["cor"])), 1.0));
    sup.set_stroke(None);
    let inicio = Point::from_xy(pt(&op["x"]), pt(&op["y"]));
    sup.draw_glyphs(inicio, &glifos, fonte.pdf.clone(), t, corpo, false);
}

/// Corridas de modulos pretos do QR (linha, coluna, comprimento), todas num unico caminho: sem fresta entre vizinhas.
fn qr(sup: &mut Surface, dados: &Value, op: &Value) -> Result<(), ErroDanfse> {
    let n = f(&dados["qr"]["n"]);
    if n <= 0.0 {
        return Err(erro("QR sem modulos"));
    }
    let modulo = f(&op["lado"]) / n;
    let mut caminho = PathBuilder::new();
    for c in dados["qr"]["corridas"].as_array().into_iter().flatten() {
        let v = |i: usize| c.get(i).map_or(0.0, f);
        let x = (f(&op["x"]) + v(1) * modulo) * CM;
        let y = (f(&op["y"]) + v(0) * modulo) * CM;
        let area = Rect::from_xywh(
            x as f32,
            y as f32,
            (v(2) * modulo * CM) as f32,
            (modulo * CM) as f32,
        )
        .ok_or_else(|| erro("corrida de QR sem area"))?;
        caminho.push_rect(area);
    }
    let caminho = caminho.finish().ok_or_else(|| erro("QR vazio"))?;
    sup.set_fill(preencher(rgb::Color::new(0, 0, 0), 1.0));
    sup.set_stroke(None);
    sup.draw_path(&caminho);
    Ok(())
}

/// Marca d'agua: texto centrado na pagina, girado, cinza e translucido. A caixa vai da linha de base a altura das
/// maiusculas, e a rotacao e' em torno do centro dela.
fn marca(sup: &mut Surface, fontes: &Fontes, op: &Value) {
    let fonte = &fontes.regular;
    let corpo = f(&op["s"]) as f32;
    let t = s(&op["txt"]);
    let (glifos, largura) = fonte.glifos(t);
    let (cx, cy) = (LARGURA_A4 / 2.0, ALTURA_A4 / 2.0);
    let nivel = (255.0 * f(&op["cinza"])).floor() as u8;
    let opacidade = (255.0 * f(&op["opacidade"])).floor() as f32 / 255.0;
    sup.set_fill(preencher(rgb::Color::new(nivel, nivel, nivel), opacidade));
    sup.set_stroke(None);
    sup.push_transform(&Transform::from_rotate_at(-(f(&op["ang"]) as f32), cx, cy));
    let inicio = Point::from_xy(
        cx - largura * corpo / 2.0,
        cy + fonte.altura_maiuscula * corpo / 2.0,
    );
    sup.draw_glyphs(inicio, &glifos, fonte.pdf.clone(), t, corpo, false);
    sup.pop();
}

fn executar(sup: &mut Surface, fontes: &Fontes, dados: &Value) -> Result<(), ErroDanfse> {
    for op in dados["pagina"].as_array().into_iter().flatten() {
        match s(&op["t"]) {
            "ret" => retangulo(sup, op)?,
            "lin" => linha(sup, op)?,
            "img" => imagem(sup, op)?,
            "txt" => texto(sup, fontes, op),
            "qr" => qr(sup, dados, op)?,
            "marca" => marca(sup, fontes, op),
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn pdf(dados: &Value) -> Result<Vec<u8>, ErroDanfse> {
    let fontes = Fontes {
        regular: Fonte::nova(REGULAR)?,
        negrito: Fonte::nova(NEGRITO)?,
    };
    let mut documento = Document::new();
    let titulo = format!("DANFSe {}", s(&dados["acesso"]["numero"]));
    documento.set_metadata(Metadata::new().title(titulo).language("pt".to_string()));
    let tamanho =
        PageSettings::from_wh(LARGURA_A4, ALTURA_A4).ok_or_else(|| erro("pagina invalida"))?;
    let mut pagina = documento.start_page_with(tamanho);
    let mut sup = pagina.surface();
    executar(&mut sup, &fontes, dados)?;
    sup.finish();
    pagina.finish();
    documento.finish().map_err(|e| erro(format!("{e:?}")))
}
