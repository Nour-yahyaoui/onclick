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

SIDE_W = 262


def render(theme, out, modal=False):
    c = LIGHT if theme == "light" else DARK
    img = Image.new("RGB", (W, H), c["canvas"])
    d = ImageDraw.Draw(img)

    # ================= sidebar =================
    d.rectangle((0, 0, SIDE_W, H), fill=c["surface"])
    d.line((SIDE_W, 0, SIDE_W, H), fill=c["hair"], width=1)

    pad = 14
    rr(d, (pad, 16, pad + 38, 54), 11, fill=c["accent"])
    text(d, (pad + 19, 35), "1C", 13, True, c["accent_ink"], anchor="mm")
    text(d, (pad + 50, 21), "Écran Bleu", 14, True, c["ink"])
    text(d, (pad + 50, 38), "Patron", 10, False, c["muted"])

    # quick sell
    rr(d, (pad, 66, SIDE_W - pad, 116), 16, fill=c["accent"])
    rr(d, (pad + 14, 80, pad + 36, 102), 7, fill=None, outline=c["accent_ink"])
    text(d, (pad + 25, 91), "+", 14, True, c["accent_ink"], anchor="mm")
    text(d, (pad + 46, 91), "Vente rapide", 14, True, c["accent_ink"], anchor="lm")

    # search
    rr(d, (pad, 128, SIDE_W - pad, 172), 12, fill=c["surface2"], outline=c["hair"])
    d.ellipse((pad + 13, 143, pad + 25, 155), outline=c["muted"], width=2)
    d.line((pad + 23, 153, pad + 28, 158), fill=c["muted"], width=2)
    text(d, (pad + 38, 150), "Chercher ou scanner…", 11, False, c["muted"], anchor="lm")

    # nav
    def group(title, y, items):
        text(d, (pad + 10, y), title, 9, True, c["muted"])
        y += 16
        for label, active, badge in items:
            if active:
                rr(d, (pad, y, SIDE_W - pad, y + 42), 11, fill=c["accent_soft"], outline=c["accent_line"])
            text(d, (pad + 12, y + 21), label, 13, active, c["accent"] if active else c["ink2"], anchor="lm")
            if badge:
                bx = SIDE_W - pad - 30
                rr(d, (bx, y + 11, bx + 20, y + 31), 10, fill=c["accent"])
                text(d, (bx + 10, y + 21), badge, 9, True, c["accent_ink"], anchor="mm")
            y += 46
        return y

    y = group("CAISSE", 190, [("Vendre", True, "3"), ("Ventes", False, None),
                              ("Carnet", False, None), ("Produits", False, None)])
    group("GESTION", y + 4, [("Caisse / Z", False, None), ("Journal", False, None),
                             ("Réglages", False, None)])

    # footer
    fy = H - 56
    d.line((pad, fy - 14, SIDE_W - pad, fy - 14), fill=c["hair"], width=1)
    for i, kind in enumerate(("FR", "moon", "menu")):
        x0 = pad + i * 46
        rr(d, (x0, fy, x0 + 38, fy + 38), 11, fill=c["surface"], outline=c["hair"])
        cx, cy = x0 + 19, fy + 19
        if kind == "FR":
            text(d, (cx, cy), "FR", 10, True, c["ink"], anchor="mm")
        elif kind == "moon":
            d.ellipse((cx - 7, cy - 7, cx + 7, cy + 7), outline=c["ink"], width=2)
            d.pieslice((cx - 7, cy - 7, cx + 7, cy + 7), 90, 270, fill=c["ink"])
        else:
            for k in (-6, 0, 6):
                d.ellipse((cx + k - 1.5, cy - 1.5, cx + k + 1.5, cy + 1.5), fill=c["ink"])

    # ================= main column =================
    mx = SIDE_W
    d.line((mx, 46, W, 46), fill=c["hair"], width=1)
    text(d, ((mx + W) // 2, 23), "بِسْمِ اللّٰهِ الرَّحْمٰنِ الرَّحِيمِ", 23, False, c["ink"], anchor="mm")

    pad2 = 20
    # page head
    text(d, (mx + pad2, 74), "Vendre", 20, True, c["ink"])
    text(d, (mx + pad2 + tw("Vendre", 20, True) + 12, 79), "Aujourd'hui 1,250.000 DT · 14 tickets", 11, False, c["muted"])
    bw = tw("Caisse ouverte depuis 08:12", 11, True) + 26
    rr(d, (W - pad2 - bw, 62, W - pad2, 94), 11, fill=c["surface"], outline=c["hair"])
    text(d, (W - pad2 - bw // 2, 78), "Caisse ouverte depuis 08:12", 11, True, c["ink"], anchor="mm")

    # ---------- product grid ----------
    cart_w = 384
    gx = mx + pad2
    gw = W - pad2 - cart_w - 18 - gx
    gy = 112
    cols, gap, ch = 3, 12, 158
    cw = (gw - gap * (cols - 1)) // cols
    for i, (name, price, stock, mono) in enumerate(PRODUCTS):
        r_, col = divmod(i, cols)
        x0 = gx + col * (cw + gap)
        y0 = gy + r_ * (ch + gap)
        if y0 + ch > H - pad2:
            break
        rr(d, (x0, y0, x0 + cw, y0 + ch), 16, fill=c["surface"], outline=c["hair"])
        th = (cw - 20) * 9 / 18
        rr(d, (x0 + 10, y0 + 8, x0 + cw - 10, y0 + 8 + th), 12, fill=c["surface3"])
        text(d, (x0 + cw // 2, y0 + 8 + th / 2), mono, 17, True, c["accent"], anchor="mm")
        nm = name if tw(name, 12, True) < cw - 24 else name[:16] + "…"
        text(d, (x0 + 12, y0 + 96), nm, 12, True, c["ink"])
        text(d, (x0 + 12, y0 + 114), price, 16, True, c["ink"])
        if stock == 0:
            rr(d, (x0 + 12, y0 + 136, x0 + 74, y0 + 152), 8, fill=c["warn_bg"])
            text(d, (x0 + 43, y0 + 144), "Rupture", 9, True, c["warn_ink"], anchor="mm")
        else:
            text(d, (x0 + 12, y0 + 144), f"{stock} en stock", 9, False, c["muted"], anchor="lm")

    # ---------- cart panel ----------
    cx0, cx1 = W - pad2 - cart_w, W - pad2
    cy0, cy1 = 62, H - pad2
    rr(d, (cx0, cy0, cx1, cy1), 20, fill=c["surface"], outline=c["hair"])
    inner = 16
    text(d, (cx0 + inner, cy0 + 22), "Panier", 14, True, c["ink"])
    bx = cx0 + inner + tw("Panier", 14, True) + 8
    rr(d, (bx, cy0 + 12, bx + 24, cy0 + 32), 10, fill=c["accent_soft"])
    text(d, (bx + 12, cy0 + 22), "3", 10, True, c["accent"], anchor="mm")
    d.line((cx0 + inner, cy0 + 44, cx1 - inner, cy0 + 44), fill=c["hair"], width=1)

    ly = cy0 + 56
    for name, qty, total, unit in CART:
        nm = name if tw(name, 12, True) < 240 else name[:24] + "…"
        text(d, (cx0 + inner, ly), nm, 12, True, c["ink"])
        text(d, (cx1 - inner, ly), total, 12, True, c["ink"], anchor="ra")
        sx = cx0 + inner
        rr(d, (sx, ly + 20, sx + 96, ly + 48), 10, fill=c["surface3"])
        d.line((sx + 12, ly + 34, sx + 24, ly + 34), fill=c["ink"], width=2)
        text(d, (sx + 48, ly + 34), qty, 12, True, c["ink"], anchor="mm")
        d.line((sx + 72, ly + 34, sx + 84, ly + 34), fill=c["ink"], width=2)
        d.line((sx + 78, ly + 28, sx + 78, ly + 40), fill=c["ink"], width=2)
        rr(d, (sx + 104, ly + 20, sx + 200, ly + 48), 10, fill=c["surface3"])
        text(d, (sx + 128, ly + 34), unit, 11, True, c["ink"], anchor="lm")
        ly += 62
        d.line((cx0 + inner, ly - 8, cx1 - inner, ly - 8), fill=c["hair"], width=1)

    fy = cy1 - 190
    d.line((cx0 + inner, fy - 12, cx1 - inner, fy - 12), fill=c["hair"], width=1)
    text(d, (cx0 + inner, fy + 6), "3 articles", 11, False, c["muted"])
    text(d, (cx0 + inner, fy + 34), "Total", 11, False, c["muted"])
    text(d, (cx1 - inner, fy + 40), "2,538.000", 24, True, c["ink"], anchor="ra")
    text(d, (cx1 - inner, fy + 60), "DT", 10, False, c["muted"], anchor="ra")

    rr(d, (cx0 + inner, cy1 - 106, cx1 - inner, cy1 - 50), 14, fill=c["accent"])
    text(d, ((cx0 + cx1) // 2, cy1 - 78), "Encaisser", 18, True, c["accent_ink"], anchor="mm")
    half = (cx1 - inner - (cx0 + inner) - 8) // 2
    rr(d, (cx0 + inner, cy1 - 42, cx0 + inner + half, cy1 - 10), 10, fill=c["surface"], outline=c["hair"])
    text(d, (cx0 + inner + half // 2, cy1 - 26), "Carte", 12, True, c["ink"], anchor="mm")
    rr(d, (cx1 - inner - half, cy1 - 42, cx1 - inner, cy1 - 10), 10, fill=c["surface"], outline=c["hair"])
    text(d, (cx1 - inner - half // 2, cy1 - 26), "Crédit", 12, True, c["ink"], anchor="mm")

    if modal:
        img = Image.alpha_composite(
            img.convert("RGBA"), Image.new("RGBA", (W, H), (12, 16, 14, 118))
        ).convert("RGB")
        d = ImageDraw.Draw(img)

        mw, mh = 720, 560
        mx0, my0 = (W - mw) // 2, (H - mh) // 2
        mx1, my1 = mx0 + mw, my0 + mh
        rr(d, (mx0, my0, mx1, my1), 26, fill=c["surface"], outline=c["hair"])
        text(d, (mx0 + 20, my0 + 26), "Réglages", 17, True, c["ink"])
        rr(d, (mx1 - 56, my0 + 16, mx1 - 20, my0 + 52), 10, fill=c["surface"], outline=c["hair"])
        text(d, (mx1 - 38, my0 + 33), "×", 15, True, c["ink2"], anchor="mm")
        d.line((mx0, my0 + 68, mx1, my0 + 68), fill=c["hair"], width=1)

        # language segment
        text(d, (mx0 + 20, my0 + 90), "Langue", 12, True, c["ink"])
        rr(d, (mx0 + 20, my0 + 110, mx0 + 340, my0 + 154), 11, fill=c["surface3"])
        rr(d, (mx0 + 23, my0 + 113, mx0 + 130, my0 + 151), 9, fill=c["surface"])
        text(d, (mx0 + 76, my0 + 132), "Français", 11, True, c["ink"], anchor="mm")
        text(d, (mx0 + 190, my0 + 132), "العربية", 11, False, c["muted"], anchor="mm")
        text(d, (mx0 + 285, my0 + 132), "English", 11, False, c["muted"], anchor="mm")

        d.line((mx0 + 20, my0 + 176, mx1 - 20, my0 + 176), fill=c["hair"], width=1)
        text(d, (mx0 + 20, my0 + 196), "Nom du magasin", 12, True, c["ink"])
        rr(d, (mx0 + 20, my0 + 216, mx1 - 20, my0 + 264), 12, fill=c["surface"], outline=c["hair"])
        text(d, (mx0 + 34, my0 + 240), "Écran Bleu", 13, False, c["ink"], anchor="lm")
        rr(d, (mx0 + 20, my0 + 274, mx0 + 132, my0 + 316), 11, fill=c["accent"])
        text(d, (mx0 + 76, my0 + 295), "Enregistrer", 11, True, c["accent_ink"], anchor="mm")

        d.line((mx0 + 20, my0 + 340, mx1 - 20, my0 + 340), fill=c["hair"], width=1)
        text(d, (mx0 + 20, my0 + 360), "Historique", 12, True, c["ink"])
        rr(d, (mx0 + 20, my0 + 382, mx0 + 178, my0 + 424), 11, fill=c["surface"], outline=c["hair"])
        text(d, (mx0 + 99, my0 + 403), "Qui a fait quoi", 11, True, c["ink"], anchor="mm")
        rr(d, (mx0 + 190, my0 + 382, mx0 + 296, my0 + 424), 11, fill=c["surface"], outline=c["hair"])
        text(d, (mx0 + 243, my0 + 403), "Caisse / Z", 11, True, c["ink"], anchor="mm")

        # footer
        d.line((mx0, my1 - 64, mx1, my1 - 64), fill=c["hair"], width=1)
        d.rectangle((mx0, my1 - 64, mx1, my1), fill=c["surface2"])
        rr(d, (mx1 - 210, my1 - 50, mx1 - 112, my1 - 14), 11, fill=c["surface"], outline=c["hair"])
        text(d, (mx1 - 161, my1 - 32), "Fermer", 11, True, c["ink"], anchor="mm")
        rr(d, (mx1 - 104, my1 - 50, mx1 - 20, my1 - 14), 11, fill=c["accent"])
        text(d, (mx1 - 62, my1 - 32), "Terminé", 11, True, c["accent_ink"], anchor="mm")

        # toast
        tw_ = 232
        rr(d, ((W - tw_) // 2, H - 70, (W + tw_) // 2, H - 26), 22, fill=c["ink"])
        d.ellipse(((W - tw_) // 2 + 20, H - 52, (W - tw_) // 2 + 29, H - 43), fill="#4ADE80")
        text(d, ((W - tw_) // 2 + 40, H - 48), "Réglages enregistrés", 11, True, c["canvas"], anchor="lm")

    img.save(out)
    print("wrote", out)


def render_login(theme, out):
    c = LIGHT if theme == "light" else DARK
    img = Image.new("RGB", (W, H), c["canvas"])
    d = ImageDraw.Draw(img)

    cw, ch = 460, 470
    x0, y0 = (W - cw) // 2, (H - ch) // 2 - 10
    x1, y1 = x0 + cw, y0 + ch
    rr(d, (x0 + 2, y0 + 6, x1 + 2, y1 + 6), 26, fill=c["hair"])
    rr(d, (x0, y0, x1, y1), 26, fill=c["surface"], outline=c["hair"])

    px = x0 + 30
    rr(d, (px, y0 + 28, px + 38, y0 + 66), 11, fill=c["accent"])
    text(d, (px + 19, y0 + 47), "1C", 13, True, c["accent_ink"], anchor="mm")
    text(d, (px + 50, y0 + 38), "Écran Bleu", 14, True, c["ink"])
    text(d, (px + 50, y0 + 55), "OneClick", 10, False, c["muted"])

    text(d, (px, y0 + 94), "بِسْمِ اللّٰهِ الرَّحْمٰنِ الرَّحِيمِ", 20, False, c["muted"])
    d.line((x0, y0 + 116, x1, y0 + 116), fill=c["hair"], width=1)

    text(d, (px, y0 + 158), "Bonjour", 26, True, c["ink"])
    text(d, (px, y0 + 194), "Entrez votre mot de passe pour ouvrir la caisse.", 12, False, c["muted"])

    text(d, (px, y0 + 240), "Mot de passe", 11, True, c["ink2"])
    rr(d, (px, y0 + 258, x1 - 30, y0 + 310), 12, fill=c["surface"], outline=c["hair"])
    text(d, (px + 16, y0 + 284), "••••••••", 13, False, c["muted"], anchor="lm")

    rr(d, (px, y0 + 330, x1 - 30, y0 + 386), 14, fill=c["accent"])
    text(d, ((px + x1 - 30) // 2, y0 + 358), "Entrer", 16, True, c["accent_ink"], anchor="mm")

    d.line((x0 + 30, y0 + 412, x1 - 30, y0 + 412), fill=c["hair"], width=1)
    rr(d, (px, y0 + 426, px + 168, y0 + 456), 10, fill=c["surface3"])
    for i, code in enumerate(["FR", "ع", "EN"]):
        sx = px + 6 + i * 54
        if i == 0:
            rr(d, (sx, y0 + 429, sx + 46, y0 + 453), 8, fill=c["surface"])
        text(d, (sx + 23, y0 + 441), code, 10, True, c["ink"] if i == 0 else c["muted"], anchor="mm")
    rr(d, (x1 - 30 - 74, y0 + 426, x1 - 30, y0 + 456), 10, fill=c["surface"], outline=c["hair"])
    text(d, (x1 - 30 - 37, y0 + 441), "Jour", 10, True, c["ink"], anchor="mm")

    img.save(out)
    print("wrote", out)


if __name__ == "__main__":
    outdir = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "docs/assets")
    outdir.mkdir(parents=True, exist_ok=True)
    render("light", outdir / "design-pos-light.png")
    render("dark", outdir / "design-pos-dark.png")
    render("light", outdir / "design-settings.png", modal=True)
    render_login("light", outdir / "design-login.png")
