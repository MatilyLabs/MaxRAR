"""Render MaxRAR's README hero. Requires Pillow; uses the canonical bundle icon.

Run from any directory: python3 scripts/generate-readme-hero.py
The GIF is an illustrative workflow, not a recording or a performance claim.
"""

from pathlib import Path
import math

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs" / "assets"
W, H = 1200, 440
INK = "#172334"
MUTED = "#617184"
BLUE = "#3569ad"
LINE = "#dce3eb"


def font(size, bold=False):
    # Supply equivalent fonts here when regenerating on a non-macOS host.
    name = "Arial Bold.ttf" if bold else "Arial.ttf"
    return ImageFont.truetype(f"/System/Library/Fonts/Supplemental/{name}", size)


def label(draw, xy, text, size, fill=INK, bold=False):
    draw.text(xy, text, font=font(size, bold), fill=fill)


def folder(draw, x, y):
    draw.rounded_rectangle((x, y, x + 31, y + 18), radius=5, fill="#eeb53c")
    draw.rounded_rectangle((x, y + 10, x + 67, y + 56), radius=7, fill="#f4c353")


def render(t):
    im = Image.new("RGB", (W, H), "#f7f9fc")
    d = ImageDraw.Draw(im)
    d.rounded_rectangle((1, 1, W - 2, H - 2), radius=22, outline=LINE, width=2)
    logo = Image.open(ROOT / "src-tauri/icons/icon.png").convert("RGBA")
    logo.thumbnail((82, 82), Image.Resampling.LANCZOS)
    im.paste(logo, (44, 45), logo)
    label(d, (45, 148), "MaxRAR", 51, bold=True)
    label(d, (47, 215), "Know what you unpack.", 25)
    label(d, (47, 258), "Create. Inspect. Extract.", 19, MUTED)
    label(d, (47, 362), "WINDOWS   /   macOS   /   LINUX", 12, MUTED, True)
    d.line((410, 48, 410, 390), fill=LINE, width=1)

    stages = [(452, "01", "Browse", "See your files"),
              (690, "02", "Inspect", "Understand contents"),
              (928, "03", "Extract", "Choose a destination")]
    active = min(2, int(t * 3))
    for i, (x, number, title, subtitle) in enumerate(stages):
        selected = i == active
        d.rounded_rectangle((x, 109, x + 198, 302), radius=14,
                            fill="white", outline=BLUE if selected else LINE,
                            width=2 if selected else 1)
        label(d, (x + 17, 124), number, 12, BLUE if selected else MUTED, True)
        if i == 0:
            small = logo.resize((62, 62), Image.Resampling.LANCZOS)
            im.paste(small, (x + 68, 160), small)
        elif i == 1:
            for j, length in enumerate((71, 55, 63)):
                yy = 170 + 17 * j
                d.rounded_rectangle((x + 48, yy, x + 56, yy + 8), radius=2, fill=BLUE)
                d.rounded_rectangle((x + 65, yy, x + 65 + length, yy + 7), radius=3,
                                    fill="#c6d4e6")
        else:
            folder(d, x + 65, 163)
        label(d, (x + 18, 241), title, 21, bold=True)
        label(d, (x + 18, 274), subtitle, 13, MUTED)
        if i < 2:
            xx = x + 216
            d.line((xx - 8, 204, xx + 7, 204), fill=MUTED, width=2)
            d.line((xx + 2, 199, xx + 7, 204, xx + 2, 209), fill=MUTED, width=2)

    d.rounded_rectangle((452, 330, 1126, 334), radius=2, fill="#e3e9f1")
    progress = 452 + round(674 * t)
    d.rounded_rectangle((452, 330, max(456, progress), 334), radius=2, fill=BLUE)
    label(d, (452, 358), "Your archives. One clear workspace.", 15, MUTED)
    label(d, (1051, 358), "MATILY", 12, MUTED, True)
    return im


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    render(0.55).save(OUT / "maxrar-hero.png", optimize=True)
    # A shared palette prevents color flicker between frames.
    palette = render(0.55).quantize(colors=128)
    frames = []
    for i in range(60):
        t = (1 - math.cos(math.pi * i / 59)) / 2
        frames.append(render(t).quantize(palette=palette, dither=Image.Dither.NONE))
    durations = [80] * len(frames)
    durations[0], durations[-1] = 700, 1500
    frames[0].save(OUT / "maxrar-hero.gif", save_all=True, append_images=frames[1:],
                   duration=durations, loop=0, optimize=True, disposal=1)


if __name__ == "__main__":
    main()
