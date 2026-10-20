#!/usr/bin/env python3
"""Draw the OneClick icon (assets/oneclick.ico + assets/oneclick.png).

Reproducible on purpose: the icon is the only binary asset in the repo, so it
must be regenerable with one command. Run: python3 scripts/make_icon.py
"""
from PIL import Image, ImageDraw, ImageFont
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"

ACCENT = (15, 123, 95, 255)      # --accent, the brand green
ACCENT_DARK = (9, 84, 65, 255)   # a touch darker at the bottom, for depth
WHITE = (255, 255, 255, 255)

def draw(size: int) -> Image.Image:
    # Draw at 4x then downscale: keeps the letters crisp at 16 px.
    big = size * 4
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))

    # Vertical gradient, masked by a rounded square (one shape, no seams).
    grad = Image.new("RGBA", (big, big))
    gd = ImageDraw.Draw(grad)
    for y in range(big):
        t = y / max(big - 1, 1)
        color = tuple(
            round(ACCENT[i] + (ACCENT_DARK[i] - ACCENT[i]) * t) for i in range(4)
        )
        gd.line([(0, y), (big, y)], fill=color)

    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [0, 0, big - 1, big - 1], radius=int(big * 0.23), fill=255
    )
    img.paste(grad, (0, 0), mask)

    # "1C", optically centred (anchor handles the real ink box).
    d = ImageDraw.Draw(img)
    font = ImageFont.truetype(FONT, int(big * 0.44))
    d.text((big / 2, big / 2 + big * 0.01), "1C", font=font, fill=WHITE, anchor="mm")

    return img.resize((size, size), Image.LANCZOS)

def main() -> None:
    ASSETS.mkdir(exist_ok=True)
    sizes = [16, 24, 32, 48, 64, 128, 256]
    frames = [draw(s) for s in sizes]
    ico = ASSETS / "oneclick.ico"
    frames[-1].save(ico, format="ICO", sizes=[(s, s) for s in sizes])
    frames[-1].save(ASSETS / "oneclick.png", format="PNG")
    print(f"{ico} ({ico.stat().st_size} bytes) + oneclick.png")

if __name__ == "__main__":
    main()
