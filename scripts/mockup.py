"""Render the OneClick till mockups (PIL) so the design can be judged without a build.

Mirrors crates/ui/assets/store.css: app bar -> Bismillah strip -> POS grid + cart panel.
"""
from PIL import Image, ImageDraw, ImageFont
import pathlib, sys

W, H = 1366, 768
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
FONT_B = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"

def font(size, bold=False):
    return ImageFont.truetype(FONT_B if bold else FONT, size)

def rr(d, box, r, fill=None, outline=None, width=1):
    d.rounded_rectangle(tuple(int(v) for v in box), radius=r, fill=fill, outline=outline, width=width)

def text(d, xy, s, size=16, bold=False, fill="#000", anchor="la"):
    d.text(xy, s, font=font(size, bold), fill=fill, anchor=anchor)

def tw(s, size=16, bold=False):
    return font(size, bold).getlength(s)

LIGHT = dict(canvas="#F7F5F0", surface="#FFFFFF", surface2="#FBFAF6", surface3="#F2EFE8",
             ink="#121614", ink2="#3D4642", muted="#6C766F", hair="#E8E3D9", hair2="#D9D2C4",
             accent="#0F7B5F", accent_soft="#E7F2EC", accent_line="#CBE3D8", accent_ink="#FFFFFF",
             warn_bg="#FBF0D8", warn_ink="#7D5511", danger="#B8402F")
DARK = dict(canvas="#101519", surface="#171E23", surface2="#1C252B", surface3="#212C33",
            ink="#E9EEF1", ink2="#C2CCD2", muted="#93A1AA", hair="#26323A", hair2="#33424C",
            accent="#56D6A0", accent_soft="#14251F", accent_line="#22453A", accent_ink="#07130E",
            warn_bg="#2A2418", warn_ink="#E4C07C", danger="#F08B7E")

PRODUCTS = [
    ("iPhone 13 128 Go", "2,450.000", 3, "IP"),
    ("Samsung A54", "1,290.000", 7, "SA"),
    ("Chargeur 20W USB-C", "39.500", 24, "CH"),
    ("Coque iPhone 13", "18.000", 41, "CO"),
    ("Écouteurs BT Pro", "89.000", 12, "ÉC"),
    ("Câble Lightning 1m", "15.000", 0, "CÂ"),
    ("Batterie externe 10k", "69.000", 9, "BA"),
    ("Verre trempé A54", "9.000", 60, "VE"),
    ("Carte mémoire 64 Go", "32.000", 18, "CM"),
    ("Adaptateur USB-C", "12.500", 5, "AD"),
    ("Support voiture", "25.000", 11, "SV"),
    ("Powerbank 20k", "119.000", 4, "PB"),
]
CART = [("iPhone 13 128 Go", "1", "2,450.000", "2,450.000"),
        ("Chargeur 20W USB-C", "2", "79.000", "39.500"),
        ("Verre trempé A54", "1", "9.000", "9.000")]

BAR_H = 70


def render(theme, out):
    c = LIGHT if theme == "light" else DARK
    img = Image.new("RGB", (W, H), c["canvas"])
    d = ImageDraw.Draw(img)

    # ---------------- app bar ----------------
    d.line((0, BAR_H, W, BAR_H), fill=c["hair"], width=1)
    pad = 20
    rr(d, (pad, 16, pad + 38, 54), 11, fill=c["accent"])
    text(d, (pad + 19, 35), "1C", 14, True, c["accent_ink"], anchor="mm")
    text(d, (pad + 50, 25), "Écran Bleu", 15, True, c["ink"])
    text(d, (pad + 50, 43), "Patron", 11, False, c["muted"])

    # nav tabs
    tabs = ["Vendre", "Produits", "Carnet", "Réglages"]
    wsum = sum(tw(t, 13, True) + 30 + (24 if i == 0 else 0) for i, t in enumerate(tabs)) + 8
    x1 = W - pad - 38 - 8 - 38 - 8            # room for theme + menu buttons
    x0 = x1 - wsum
    rr(d, (x0, 15, x1, 55), 13, fill=c["surface3"], outline=c["hair"])
    tx = x0 + 8
    for i, t in enumerate(tabs):
        tw_ = tw(t, 13, True) + 22 + (26 if i == 0 else 0)
        if i == 0:
            rr(d, (tx, 19, tx + tw_, 51), 10, fill=c["surface"])
            badge = "3"
            text(d, (tx + 12, 35), t, 13, True, c["ink"], anchor="lm")
            bx = tx + 12 + tw(t, 13, True) + 6
            rr(d, (bx, 27, bx + 18, 43), 8, fill=c["accent"])
            text(d, (bx + 9, 35), badge, 10, True, c["accent_ink"], anchor="mm")
        else:
            text(d, (tx + 11, 35), t, 13, True, c["muted"], anchor="lm")
        tx += tw_ + 4

    # theme + menu
    rr(d, (x1 + 8, 15, x1 + 46, 55), 11, fill=c["surface"], outline=c["hair"])
    cxm, cym = x1 + 27, 35
    d.ellipse((cxm - 7, cym - 7, cxm + 7, cym + 7), outline=c["ink"], width=2)
    d.pieslice((cxm - 7, cym - 7, cxm + 7, cym + 7), 90, 270, fill=c["ink"])
    rr(d, (x1 + 54, 15, x1 + 92, 55), 11, fill=c["surface"], outline=c["hair"])
    for i in (-6, 0, 6):
        d.ellipse((x1 + 73 + i - 1.5, 33.5, x1 + 73 + i + 1.5, 36.5), fill=c["ink"])

    # ---------------- bismillah strip (under the header) ----------------
    by = BAR_H
    d.line((0, by + 46, W, by + 46), fill=c["hair"], width=1)
    text(d, (W // 2, by + 23), "بِسْمِ اللّٰهِ الرَّحْمٰنِ الرَّحِيمِ", 24, False, c["ink"], anchor="mm")

    # ---------------- content ----------------
    top = by + 46 + 20
    px, pw = pad, W - pad * 2 - 384 - 18
    panel_x0, panel_x1 = W - pad - 384, W - pad

    # search + new
    rr(d, (px, top, px + pw - 128, top + 52), 16, fill=c["surface"], outline=c["hair"])
    d.ellipse((px + 18, top + 19, px + 32, top + 33), outline=c["muted"], width=2)
    d.line((px + 30, top + 31, px + 36, top + 37), fill=c["muted"], width=2)
    text(d, (px + 44, top + 26), "Scannez un code-barres ou cherchez un article…", 14, False, c["muted"], anchor="lm")
    rr(d, (px + pw - 118, top, px + pw, top + 52), 12, fill=c["surface"], outline=c["hair"])
    text(d, (px + pw - 59, top + 26), "+ Nouveau", 13, True, c["ink"], anchor="mm")

    # grid
    gy = top + 68
    cols, gap = 4, 12
    cw = (pw - gap * (cols - 1)) // cols
    ch = 158
    for i, (name, price, stock, mono) in enumerate(PRODUCTS):
        r_, col = divmod(i, cols)
        x0 = px + col * (cw + gap)
        y0 = gy + r_ * (ch + gap)
        if y0 + ch > H - pad:
            break
        rr(d, (x0, y0, x0 + cw, y0 + ch), 16, fill=c["surface"], outline=c["hair"])
        rr(d, (x0 + 10, y0 + 10, x0 + cw - 10, y0 + 8 + (cw - 20) * 9 / 18), 12, fill=c["surface3"])
        text(d, ((x0 + x0 + cw) // 2, y0 + 8 + (cw - 20) * 9 / 36), mono, 18, True, c["accent"], anchor="mm")
        nm = name if tw(name, 13, True) < cw - 24 else name[:16] + "…"
        text(d, (x0 + 12, y0 + 96), nm, 12, True, c["ink"])
        text(d, (x0 + 12, y0 + 114), price, 16, True, c["ink"])
        if stock == 0:
            rr(d, (x0 + 12, y0 + 136, x0 + 74, y0 + 152), 8, fill=c["warn_bg"])
            text(d, (x0 + 43, y0 + 144), "Rupture", 9, True, c["warn_ink"], anchor="mm")
        else:
            text(d, (x0 + 12, y0 + 144), f"{stock} en stock", 9, False, c["muted"], anchor="lm")

    # ---------------- cart panel ----------------
    py0, py1 = top, H - pad
    rr(d, (panel_x0, py0, panel_x1, py1), 20, fill=c["surface"], outline=c["hair"])
    inner = 16
    text(d, (panel_x0 + inner, py0 + 22), "Vendre", 14, True, c["ink"])
    bx = panel_x0 + inner + tw("Vendre", 14, True) + 8
    rr(d, (bx, py0 + 12, bx + 24, py0 + 32), 10, fill=c["accent_soft"])
    text(d, (bx + 12, py0 + 22), "3", 10, True, c["accent"], anchor="mm")
    d.line((panel_x0 + inner, py0 + 44, panel_x1 - inner, py0 + 44), fill=c["hair"], width=1)

    ly = py0 + 56
    for name, qty, total, unit in CART:
        nm = name if tw(name, 12, True) < 240 else name[:24] + "…"
        text(d, (panel_x0 + inner, ly), nm, 12, True, c["ink"])
        text(d, (panel_x1 - inner, ly), total, 12, True, c["ink"], anchor="ra")
        sx = panel_x0 + inner
        rr(d, (sx, ly + 20, sx + 96, ly + 48), 10, fill=c["surface3"])
        d.line((sx + 12, ly + 34, sx + 24, ly + 34), fill=c["ink"], width=2)
        text(d, (sx + 48, ly + 34), qty, 12, True, c["ink"], anchor="mm")
        d.line((sx + 72, ly + 34, sx + 84, ly + 34), fill=c["ink"], width=2)
        d.line((sx + 78, ly + 28, sx + 78, ly + 40), fill=c["ink"], width=2)
        rr(d, (sx + 104, ly + 20, sx + 200, ly + 48), 10, fill=c["surface3"])
        text(d, (sx + 128, ly + 34), unit, 11, True, c["ink"], anchor="lm")
        ly += 62
        d.line((panel_x0 + inner, ly - 8, panel_x1 - inner, ly - 8), fill=c["hair"], width=1)

    # footer
    fy = py1 - 190
    d.line((panel_x0 + inner, fy - 12, panel_x1 - inner, fy - 12), fill=c["hair"], width=1)
    text(d, (panel_x0 + inner, fy + 6), "3 articles", 11, False, c["muted"])
    text(d, (panel_x0 + inner, fy + 34), "Total", 11, False, c["muted"])
    text(d, (panel_x1 - inner, fy + 40), "2,538.000", 26, True, c["ink"], anchor="ra")
    text(d, (panel_x1 - inner, fy + 62), "DT", 11, False, c["muted"], anchor="ra")

    rr(d, (panel_x0 + inner, py1 - 106, panel_x1 - inner, py1 - 50), 14, fill=c["accent"])
    text(d, ((panel_x0 + panel_x1) // 2, py1 - 78), "Encaisser", 18, True, c["accent_ink"], anchor="mm")
    half = (panel_x1 - inner - (panel_x0 + inner) - 8) // 2
    rr(d, (panel_x0 + inner, py1 - 42, panel_x0 + inner + half, py1 - 10), 10, fill=c["surface"], outline=c["hair"])
    text(d, (panel_x0 + inner + half // 2, py1 - 26), "Carte", 12, True, c["ink"], anchor="mm")
    rr(d, (panel_x1 - inner - half, py1 - 42, panel_x1 - inner, py1 - 10), 10, fill=c["surface"], outline=c["hair"])
    text(d, (panel_x1 - inner - half // 2, py1 - 26), "Crédit", 12, True, c["ink"], anchor="mm")

    img.save(out)
    print("wrote", out)


if __name__ == "__main__":
    outdir = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "docs/assets")
    outdir.mkdir(parents=True, exist_ok=True)
    render("light", outdir / "design-pos-light.png")
    render("dark", outdir / "design-pos-dark.png")
