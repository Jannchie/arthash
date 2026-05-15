"""Hand-curated retro / pixel-game palettes for the placeholder hash.

These are externally-designed color sets, NOT trained on any image corpus.
They're meant to give placeholders a deliberate stylistic feel — choose the
palette to choose the "look" of every placeholder.

When the encoder snaps colors to a palette, alpha-blended overlap of two
palette colors produces colors NOT in the palette — so the rendered output
has many more distinct colors than the K entries below. The palette
constrains the *flavor*, not literally every pixel.

References / origins:
    gb_dmg          Game Boy DMG (1989) — the iconic green-on-green LCD
    gb_pocket       Game Boy Pocket (1996) — grayscale LCD
    cga_mode4_p1    IBM CGA mode 4 palette 1 — DOS-era 4-color
    pico8           PICO-8 fantasy console (Lexaloffle, 2014)
    sweetie_16      GrafxKid's modern 16-color indie pixel-art palette
    endesga_32      Endesga's modern 32-color indie palette (very popular)
    nes_min         15-color subset commonly used by NES games

See https://lospec.com for many more palettes in the same flat-list format.
"""

from __future__ import annotations

import numpy as np


def _hex(*hexes: str) -> np.ndarray:
    """Build (K, 3) uint8 sRGB palette from a sequence of hex strings."""
    out = np.zeros((len(hexes), 3), dtype=np.uint8)
    for i, h in enumerate(hexes):
        h = h.lstrip("#")
        out[i] = [int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)]
    return out


# K=4 ---------------------------------------------------------------------

GB_DMG = _hex("0F380F", "306230", "8BAC0F", "9BBC0F")
GB_POCKET = _hex("000000", "545454", "A9A9A9", "FFFFFF")
CGA_MODE4_P1 = _hex("000000", "55FFFF", "FF55FF", "FFFFFF")


# K=8 ---------------------------------------------------------------------

C64_BASIC8 = _hex(
    "000000", "FFFFFF", "880000", "AAFFEE",
    "CC44CC", "00CC55", "0000AA", "EEEE77",
)


# K=16 --------------------------------------------------------------------

PICO8 = _hex(
    "000000", "1D2B53", "7E2553", "008751",
    "AB5236", "5F574F", "C2C3C7", "FFF1E8",
    "FF004D", "FFA300", "FFEC27", "00E436",
    "29ADFF", "83769C", "FF77A8", "FFCCAA",
)

SWEETIE_16 = _hex(
    "1A1C2C", "5D275D", "B13E53", "EF7D57",
    "FFCD75", "A7F070", "38B764", "257179",
    "29366F", "3B5DC9", "41A6F6", "73EFF7",
    "F4F4F4", "94B0C2", "566C86", "333C57",
)

NES_16 = _hex(
    "000000", "FCFCFC", "F8F8F8", "BCBCBC",
    "7C7C7C", "A4E4FC", "3CBCFC", "0078F8",
    "0000FC", "B8B8F8", "6888FC", "0058F8",
    "0000BC", "D8B8F8", "9878F8", "6844FC",
)

C64 = _hex(
    "000000", "FFFFFF", "880000", "AAFFEE",
    "CC44CC", "00CC55", "0000AA", "EEEE77",
    "DD8855", "664400", "FF7777", "333333",
    "777777", "AAFF66", "0088FF", "BBBBBB",
)


# K=32 --------------------------------------------------------------------

ENDESGA_32 = _hex(
    "BE4A2F", "D77643", "EAD4AA", "E4A672",
    "B86F50", "733E39", "3E2731", "A22633",
    "E43B44", "F77622", "FEAE34", "FEE761",
    "63C74D", "3E8948", "265C42", "193C3E",
    "124E89", "0099DB", "2CE8F5", "FFFFFF",
    "C0CBDC", "8B9BB4", "5A6988", "3A4466",
    "262B44", "181425", "FF0044", "68386C",
    "B55088", "F6757A", "E8B796", "C28569",
)

AAP_SPLENDOR_32 = _hex(
    "050403", "0E0C0C", "2D1B1E", "612721",
    "B9451D", "F1641F", "FCA570", "FFE0B7",
    "FFFFFF", "FFF089", "F8C53A", "E88A36",
    "B05B2C", "672D1F", "452326", "5D3B61",
    "AD5B91", "DF8BAD", "C2B0D1", "8E7DA5",
    "65517A", "39314C", "1A1932", "1E3E5E",
    "1A6291", "30A7D7", "53D2FF", "C3F4FF",
    "55B57F", "1E7541", "0E3E2C", "192418",
)


PRESETS: dict[str, np.ndarray] = {
    "gb_dmg":         GB_DMG,
    "gb_pocket":      GB_POCKET,
    "cga_mode4_p1":   CGA_MODE4_P1,
    "c64_basic8":     C64_BASIC8,
    "pico8":          PICO8,
    "sweetie_16":     SWEETIE_16,
    "nes_16":         NES_16,
    "c64":            C64,
    "endesga_32":     ENDESGA_32,
    "aap_splendor_32": AAP_SPLENDOR_32,
}


def list_presets() -> list[str]:
    return list(PRESETS.keys())


def get(name: str) -> np.ndarray:
    """Return a copy of the named preset palette."""
    if name not in PRESETS:
        raise KeyError(f"unknown preset {name!r}; known: {sorted(PRESETS)}")
    return PRESETS[name].copy()
