"""Generate icon.svg / banner.svg / og.png for the arthash docs site.

Design: monospaced "arthash" wordmark + a single row of pixel-style blocks
as accent — visually echoing the PIXEL / shape codec modes.

Font: Berkeley Mono Bold (locally installed). The wordmark is converted to
SVG <path> elements via fonttools so the assets don't depend on the viewer
having Berkeley Mono.
"""

from __future__ import annotations

import os
from io import BytesIO
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.ttLib import TTFont
from PIL import Image, ImageDraw, ImageFont

# ---------------------------------------------------------------------------
# Paths & constants
# ---------------------------------------------------------------------------

ROOT = Path(__file__).resolve().parent.parent
PUBLIC = ROOT / "docs" / "site" / "public"
PUBLIC.mkdir(parents=True, exist_ok=True)

FONT_REG = Path(os.environ["LOCALAPPDATA"]) / "Microsoft/Windows/Fonts/BerkeleyMono-Regular.ttf"
FONT_BOLD = Path(os.environ["LOCALAPPDATA"]) / "Microsoft/Windows/Fonts/BerkeleyMono-Bold.ttf"

# Brand colors (re-used across all three assets)
BG = "#0c4a6e"          # sky-900
INK = "#f8fafc"         # slate-50
MUTED = "#7dd3fc"       # sky-300
ACCENT_SKY = "#0ea5e9"  # sky-500
ACCENT_AMBER = "#f59e0b"
ACCENT_EMERALD = "#22c55e"
ACCENT_ROSE = "#f43f5e"

WORD = "arthash"

# ---------------------------------------------------------------------------
# Text → SVG path
# ---------------------------------------------------------------------------


def text_to_svg_group(font_path: Path, text: str, font_size_px: float, fill: str) -> tuple[str, float, float]:
    """Render `text` as an SVG `<g>` containing one transformed glyph path
    per character. Returns (svg_fragment, advance_width_px, ascent_px).

    The fragment is positioned so that the line baseline sits at y=0 and the
    first glyph starts at x=0. Caller wraps it in another `<g transform>` to
    place the baseline on the canvas.
    """
    font = TTFont(str(font_path))
    units_per_em = font["head"].unitsPerEm
    scale = font_size_px / units_per_em
    cmap = font.getBestCmap()
    glyph_set = font.getGlyphSet()
    hmtx = font["hmtx"]

    parts: list[str] = []
    x_cursor = 0.0
    for ch in text:
        gname = cmap.get(ord(ch))
        if gname is None:
            gname = ".notdef"
        glyph = glyph_set[gname]
        pen = SVGPathPen(glyph_set)
        glyph.draw(pen)
        d = pen.getCommands()
        if d:
            # SVGPathPen emits font-unit coords with +y-up. We invert y and
            # scale via a per-glyph matrix; translate to the cursor x.
            parts.append(
                f'<g transform="matrix({scale:.4f} 0 0 {-scale:.4f} {x_cursor:.2f} 0)">'
                f'<path d="{d}"/></g>'
            )
        x_cursor += hmtx[gname][0] * scale

    ascent = font["OS/2"].sTypoAscender * scale
    return f'<g fill="{fill}">{"".join(parts)}</g>', x_cursor, ascent


# ---------------------------------------------------------------------------
# Asset 1: icon.svg (square, 256x256 — also serves as favicon)
# ---------------------------------------------------------------------------


def make_icon() -> str:
    """Square icon: 'arth' / 'ash' stacked on two lines, with a 7-block
    pixel row underneath as the codec/PIXEL nod.

    Reads as 'arthash' top-to-bottom; the block row signposts the brand.
    """
    SIZE = 256
    PAD = 22
    GRID = 8  # 8-px grid the design snaps to

    # Two lines: "arth" / "ash"
    LINE1 = "arth"
    LINE2 = "ash."
    FONT_SIZE = 78

    g1, w1, asc1 = text_to_svg_group(FONT_BOLD, LINE1, FONT_SIZE, INK)
    g2, w2, asc2 = text_to_svg_group(FONT_BOLD, LINE2, FONT_SIZE, INK)

    line_h = asc1 * 1.05
    text_block_h = line_h * 2
    block_row_h = 18
    gap = 18

    total_h = text_block_h + gap + block_row_h
    y_start = (SIZE - total_h) / 2 + asc1
    line1_y = y_start
    line2_y = y_start + line_h

    line_w = max(w1, w2)
    x_text = (SIZE - line_w) / 2

    # Block row underneath: 7 cells (one per letter of "arthash"), 4 sky + 3 accent
    block_y = line2_y + 14
    cell = (line_w) / 7
    cells = []
    palette = [
        ACCENT_SKY, ACCENT_SKY, ACCENT_SKY, ACCENT_SKY,
        ACCENT_AMBER, ACCENT_EMERALD, ACCENT_ROSE,
    ]
    for i, color in enumerate(palette):
        cx = x_text + i * cell
        cells.append(
            f'<rect x="{cx:.2f}" y="{block_y:.2f}" width="{cell - 3:.2f}" '
            f'height="{block_row_h}" fill="{color}" rx="2"/>'
        )

    svg = f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIZE} {SIZE}" role="img" aria-label="arthash">
  <rect width="{SIZE}" height="{SIZE}" rx="36" fill="{BG}"/>
  <g transform="translate({x_text:.2f} {line1_y:.2f})">{g1}</g>
  <g transform="translate({x_text:.2f} {line2_y:.2f})">{g2}</g>
  {''.join(cells)}
</svg>
"""
    return svg


# ---------------------------------------------------------------------------
# Asset 2: banner.svg (wide, 1280x320)
# ---------------------------------------------------------------------------


def make_banner() -> str:
    W, H = 1280, 320

    FONT_SIZE = 168
    g_title, text_w, asc = text_to_svg_group(FONT_BOLD, WORD, FONT_SIZE, INK)

    # Tagline below
    TAGLINE = "compact placeholder-image hash"
    TAG_SIZE = 34
    g_tag, tag_w, asc_tag = text_to_svg_group(FONT_REG, TAGLINE, TAG_SIZE, MUTED)

    # Vertical layout
    line_gap = 14
    total_h = asc * 0.90 + line_gap + asc_tag
    y0 = (H - total_h) / 2 + asc * 0.85
    x_text = (W - text_w) / 2

    # Pixel block strip on the left and right edges — vertical bars of 7 cells
    bars = []
    cell = 28
    bar_h = cell * 7
    bar_y = (H - bar_h) / 2
    colors = [
        ACCENT_SKY, MUTED, ACCENT_SKY, ACCENT_AMBER,
        ACCENT_EMERALD, MUTED, ACCENT_ROSE,
    ]

    # Left bar
    for i, color in enumerate(colors):
        bars.append(
            f'<rect x="56" y="{bar_y + i * cell:.2f}" width="{cell}" height="{cell - 4}" '
            f'fill="{color}" rx="3"/>'
        )
    # Right bar (mirrored color order)
    for i, color in enumerate(reversed(colors)):
        bars.append(
            f'<rect x="{W - 56 - cell}" y="{bar_y + i * cell:.2f}" width="{cell}" height="{cell - 4}" '
            f'fill="{color}" rx="3"/>'
        )

    svg = f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" role="img" aria-label="arthash — compact placeholder-image hash">
  <rect width="{W}" height="{H}" fill="{BG}"/>
  {''.join(bars)}
  <g transform="translate({x_text:.2f} {y0:.2f})">{g_title}</g>
  <g transform="translate({(W - tag_w) / 2:.2f} {y0 + line_gap + asc_tag:.2f})">{g_tag}</g>
</svg>
"""
    return svg


# ---------------------------------------------------------------------------
# Asset 3: og.png (1200x630, GitHub / Twitter social card)
# ---------------------------------------------------------------------------


def make_og() -> Image.Image:
    W, H = 1200, 630
    img = Image.new("RGB", (W, H), BG)
    draw = ImageDraw.Draw(img, "RGBA")

    title_font = ImageFont.truetype(str(FONT_BOLD), 184)
    tag_font = ImageFont.truetype(str(FONT_REG), 36)
    sub_font = ImageFont.truetype(str(FONT_REG), 26)

    title = WORD
    title_bbox = draw.textbbox((0, 0), title, font=title_font)
    title_w = title_bbox[2] - title_bbox[0]
    title_h = title_bbox[3] - title_bbox[1]
    title_x = (W - title_w) / 2
    title_y = (H - title_h) / 2 - 64
    draw.text((title_x - title_bbox[0], title_y - title_bbox[1]), title, fill=INK, font=title_font)

    tag = "A compact placeholder-image hash"
    tag_bbox = draw.textbbox((0, 0), tag, font=tag_font)
    tag_w = tag_bbox[2] - tag_bbox[0]
    tag_y = title_y + title_h + 32
    draw.text(((W - tag_w) / 2 - tag_bbox[0], tag_y - tag_bbox[1]), tag, fill=MUTED, font=tag_font)

    sub = "17 B – 400 B per image  ·  Rust core  ·  TS / Python / Rust SDKs"
    sub_bbox = draw.textbbox((0, 0), sub, font=sub_font)
    sub_w = sub_bbox[2] - sub_bbox[0]
    sub_y = tag_y + (tag_bbox[3] - tag_bbox[1]) + 22
    draw.text(((W - sub_w) / 2 - sub_bbox[0], sub_y - sub_bbox[1]), sub, fill="#94a3b8", font=sub_font)

    # Pixel accent row across the bottom — full-width strip, 60 cells of 20 px
    cell = 20
    bar_y = H - 56
    bar_h = 18
    # Generate a deterministic-ish pattern: most BG, scattered accents
    pattern_colors = [
        ACCENT_SKY, ACCENT_SKY, MUTED, ACCENT_SKY, BG, BG,
        ACCENT_AMBER, BG, BG, MUTED, ACCENT_SKY, ACCENT_SKY,
        BG, ACCENT_EMERALD, BG, ACCENT_SKY, MUTED, BG,
        ACCENT_SKY, BG, ACCENT_ROSE, BG, ACCENT_SKY, ACCENT_SKY,
        MUTED, BG, ACCENT_SKY, BG, ACCENT_AMBER, BG,
        BG, ACCENT_SKY, ACCENT_SKY, MUTED, BG, ACCENT_EMERALD,
        BG, ACCENT_SKY, BG, BG, ACCENT_SKY, MUTED,
        ACCENT_SKY, BG, ACCENT_ROSE, BG, ACCENT_SKY, ACCENT_AMBER,
        BG, MUTED, ACCENT_SKY, BG, ACCENT_SKY, BG,
        ACCENT_EMERALD, BG, ACCENT_SKY, MUTED, ACCENT_SKY, ACCENT_SKY,
    ]
    n_cells = W // cell
    for i in range(n_cells):
        color = pattern_colors[i % len(pattern_colors)]
        if color == BG:
            continue  # skip BG cells for cleaner look
        draw.rectangle(
            [(i * cell + 1, bar_y), (i * cell + cell - 1, bar_y + bar_h)],
            fill=color,
        )

    # Top-left badge
    badge_text = "arthash.dev"
    badge_font = ImageFont.truetype(str(FONT_REG), 22)
    badge_bbox = draw.textbbox((0, 0), badge_text, font=badge_font)
    bw, bh = badge_bbox[2] - badge_bbox[0], badge_bbox[3] - badge_bbox[1]
    pad = 14
    draw.rounded_rectangle(
        [(40, 40), (40 + bw + 2 * pad, 40 + bh + 2 * pad)],
        radius=8, outline=MUTED, width=2,
    )
    draw.text((40 + pad - badge_bbox[0], 40 + pad - badge_bbox[1]), badge_text, fill=MUTED, font=badge_font)

    return img


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    assert FONT_BOLD.exists(), f"Berkeley Mono Bold not found at {FONT_BOLD}"
    assert FONT_REG.exists(), f"Berkeley Mono Regular not found at {FONT_REG}"

    icon = make_icon()
    (PUBLIC / "logo.svg").write_text(icon, encoding="utf-8")
    print(f"wrote {PUBLIC / 'logo.svg'}  ({len(icon)} B)")

    banner = make_banner()
    (PUBLIC / "banner.svg").write_text(banner, encoding="utf-8")
    print(f"wrote {PUBLIC / 'banner.svg'}  ({len(banner)} B)")

    og = make_og()
    og_path = PUBLIC / "og.png"
    og.save(og_path, "PNG", optimize=True)
    print(f"wrote {og_path}  ({og_path.stat().st_size} B)")


if __name__ == "__main__":
    main()
