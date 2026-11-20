// DANFSe A4 retrato (NFS-e Padrao Nacional, NT 008). O modelo nao decide nada: executa as operacoes de
// `dados()["pagina"]`, em cm a partir do canto superior esquerdo da folha; texto com `y` = linha de base.
// Fonte: Liberation Sans (metrica da Arial, NT 008 item 2.4); logotipo: arquivo `logo.png`.
#let d = json(bytes(sys.inputs.at("dados", default: "{}")))
#set document(title: "DANFSe " + d.at("acesso", default: (:)).at("numero", default: ""))
#set page(paper: "a4", margin: 0pt)
#set text(font: "Liberation Sans", lang: "pt", hyphenate: false, top-edge: "baseline", bottom-edge: "baseline")
#set par(spacing: 0pt)

#let CINZA = rgb(242, 242, 242)
#let em(x, y, c) = place(top + left, dx: x * 1cm, dy: y * 1cm, c)

#for op in d.at("pagina", default: ()) {
  let t = op.t
  if t == "ret" {
    em(op.x, op.y, rect(width: op.w * 1cm, height: op.h * 1cm,
      fill: if op.cinza { CINZA } else { none },
      stroke: if op.borda > 0 { op.borda * 1pt + black } else { none }))
  } else if t == "lin" {
    place(top + left, line(start: (op.x1 * 1cm, op.y1 * 1cm), end: (op.x2 * 1cm, op.y2 * 1cm),
      stroke: op.esp * 1pt + black))
  } else if t == "txt" {
    em(op.x, op.y, box(text(size: op.s * 1pt, weight: if op.b { "bold" } else { "regular" },
      fill: if op.cor == "vermelho" { rgb(255, 0, 0) } else { black }, op.txt)))
  } else if t == "img" {
    em(op.x, op.y, image("logo.png", width: op.w * 1cm, height: op.h * 1cm, fit: "stretch"))
  } else if t == "qr" {
    let q = d.qr
    let m = op.lado / q.n
    for c in q.corridas {
      em(op.x + c.at(1) * m, op.y + c.at(0) * m, rect(width: c.at(2) * m * 1cm, height: m * 1cm, fill: black,
        stroke: none))
    }
  } else if t == "marca" {
    let g = int(255 * op.cinza)
    place(center + horizon, rotate(-op.ang * 1deg, text(size: op.s * 1pt,
      fill: rgb(g, g, g, int(255 * op.opacidade)), top-edge: "cap-height", bottom-edge: "baseline", op.txt)))
  }
}
