"""Visual comparison: pfhash 4 modes vs thumbhash (multiple impls) on the
same source image. Outputs:
  * docs/benchmarks/visual_<name>.png — labeled grid of decoded placeholders
  * Per-cell PSNR vs the 256-px-long-edge ground truth, printed to stdout

Usage:
    uv run python scripts/visual_compare.py <image-path>

For thumbhash cross-impl decoded outputs we call:
  * Rust crate `thumbhash` via the pre-built bench binary's stdin? No — we
    use subprocess + a small helper. Currently we only run the npm
    thumbhash port (most maintained, official JS), and the pfhash bindings
    for our own modes. Adding Go/Rust thumbhash via subprocess is mechanical.
"""
from __future__ import annotations

import io
import json
import math
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

from pfhash import Codec, ShapeType, decode, encode

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "docs" / "benchmarks"
OUT_DIR.mkdir(parents=True, exist_ok=True)


def load_and_resize(path: Path, long_edge: int) -> Image.Image:
    im = Image.open(path).convert("RGB")
    w, h = im.size
    s = long_edge / max(w, h)
    return im.resize((max(1, round(w * s)), max(1, round(h * s))), Image.LANCZOS)


def psnr(a: np.ndarray, b: np.ndarray) -> float:
    a = a.astype(np.float64)
    b = b.astype(np.float64)
    mse = ((a - b) ** 2).mean()
    if mse <= 1e-12:
        return float("inf")
    return 20 * math.log10(255.0 / math.sqrt(mse))


def pfhash_decode_to_image(img_100: Image.Image, shape: ShapeType, n_shapes: int,
                            target_w: int, target_h: int) -> tuple[Image.Image, int]:
    """Encode then decode at base_size matched to thumbnail's long edge target_size."""
    codec = Codec(shape=shape, n_shapes=n_shapes) if shape != ShapeType.DCT else Codec()
    arr = np.array(img_100, dtype=np.uint8)
    hash_bytes = encode(arr, codec) if shape == ShapeType.DCT else encode(arr, codec, seed=0)
    base = max(target_w, target_h)
    w, h, pixels = decode(hash_bytes, codec, base_size=base, aa=(shape != ShapeType.DCT))
    if shape == ShapeType.DCT:
        # DCT returns raw RGBA bytes; reshape.
        rgba = np.frombuffer(pixels, dtype=np.uint8).reshape(h, w, 4)
        img = Image.fromarray(rgba[..., :3], "RGB")
    else:
        # Shape modes return ndarray (h, w, 3) RGB.
        img = Image.fromarray(pixels, "RGB")
    if img.size != (target_w, target_h):
        img = img.resize((target_w, target_h), Image.LANCZOS)
    return img, len(hash_bytes)


def thumbhash_js_decode(img_100: Image.Image, target_w: int, target_h: int) -> tuple[Image.Image, int]:
    """Call the JS thumbhash via Node, return decoded PNG resized to target."""
    rgba_100 = np.array(img_100.convert("RGBA"), dtype=np.uint8).flatten().tolist()
    js_dir = ROOT / "bench" / "thumbhash-js"
    payload = json.dumps({"w": img_100.width, "h": img_100.height, "rgba": rgba_100})
    script = (
        "import {rgbaToThumbHash, thumbHashToRGBA} from 'thumbhash';"
        "let buf=''; process.stdin.on('data',c=>buf+=c);"
        "process.stdin.on('end',()=>{const o=JSON.parse(buf);"
        "const h=rgbaToThumbHash(o.w,o.h,o.rgba);"
        "const d=thumbHashToRGBA(h);"
        "process.stdout.write(JSON.stringify({w:d.w,h:d.h,rgba:Array.from(d.rgba),hash_bytes:h.length}));});"
    )
    proc = subprocess.run(
        ["node", "--input-type=module", "-e", script],
        input=payload, cwd=str(js_dir), capture_output=True, text=True, check=True
    )
    out = json.loads(proc.stdout)
    arr = np.array(out["rgba"], dtype=np.uint8).reshape(out["h"], out["w"], 4)
    im = Image.fromarray(arr[..., :3], "RGB").resize((target_w, target_h), Image.LANCZOS)
    return im, out["hash_bytes"]


def sqip_render(src_path: Path, target_w: int, target_h: int, n_shapes: int = 12) -> tuple[Image.Image, int]:
    """Run sqip on the original image; return the rendered PNG resized to (target_w, target_h)
    and the SVG byte length (sqip's hash-equivalent)."""
    sqip_dir = ROOT / "bench" / "sqip"
    out_svg = sqip_dir / "tmp.svg"
    out_png = sqip_dir / "tmp.png"
    proc = subprocess.run(
        ["node", "sqip_run.mjs", str(src_path.resolve()), str(out_svg), str(out_png),
         str(max(target_w, target_h)), str(n_shapes)],
        cwd=str(sqip_dir), capture_output=True, text=True
    )
    if proc.returncode != 0:
        raise RuntimeError(f"sqip stderr:\n{proc.stderr}\nstdout:\n{proc.stdout}")
    info = json.loads(proc.stdout)
    im = Image.open(out_png).convert("RGB")
    if im.size != (target_w, target_h):
        im = im.resize((target_w, target_h), Image.LANCZOS)
    return im, info["bytes"]


def thumbhash_go_decode(img_100: Image.Image, target_w: int, target_h: int) -> tuple[Image.Image, int]:
    """Call thumbhash Go (galdor/go-thumbhash) via the encode_decode.go helper."""
    go_dir = ROOT / "bench" / "thumbhash"
    helper = go_dir / "encode_decode.go"
    bin_path = go_dir / "encode_decode.exe"
    if not bin_path.exists() or bin_path.stat().st_mtime < helper.stat().st_mtime:
        subprocess.run(
            ["go", "build", "-o", str(bin_path), str(helper)],
            cwd=str(go_dir), check=True
        )
    rgba_100 = np.array(img_100.convert("RGBA"), dtype=np.uint8).flatten().tolist()
    payload = json.dumps({"W": img_100.width, "H": img_100.height, "RGBA": rgba_100})
    proc = subprocess.run(
        [str(bin_path)], input=payload, capture_output=True, text=True, check=True
    )
    out = json.loads(proc.stdout)
    arr = np.array(out["RGBA"], dtype=np.uint8).reshape(out["H"], out["W"], 4)
    im = Image.fromarray(arr[..., :3], "RGB").resize((target_w, target_h), Image.LANCZOS)
    return im, out["HashBytes"]


def stack_grid(cells: list[tuple[str, Image.Image, str]], cell_w: int, cell_h: int,
                cols: int = 4, pad: int = 12, label_h: int = 32) -> Image.Image:
    # Ensure each cell has enough horizontal room for the label.
    cell_w = max(cell_w, 280)
    rows = (len(cells) + cols - 1) // cols
    W = cols * cell_w + (cols + 1) * pad
    H = rows * (cell_h + label_h) + (rows + 1) * pad
    canvas = Image.new("RGB", (W, H), (245, 245, 245))
    draw = ImageDraw.Draw(canvas)
    try:
        font = ImageFont.truetype("arial.ttf", 14)
    except OSError:
        font = ImageFont.load_default()
    for i, (label, im, sub) in enumerate(cells):
        r = i // cols
        c = i % cols
        x = pad + c * (cell_w + pad)
        y = pad + r * (cell_h + label_h + pad)
        cell_im = im.resize((cell_w, cell_h), Image.LANCZOS)
        canvas.paste(cell_im, (x, y))
        text = f"{label}  ·  {sub}"
        draw.text((x + 4, y + cell_h + 4), text, fill=(20, 20, 20), font=font)
    return canvas


def main():
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass
    if len(sys.argv) < 2:
        print("usage: visual_compare.py <image-path>", file=sys.stderr)
        sys.exit(2)
    src_path = Path(sys.argv[1])
    name = src_path.stem

    # Encoder input: 100-px long edge (matches DCT target_size and thumbhash).
    img_100 = load_and_resize(src_path, 100)
    # Ground truth for comparison: same source resized to 256 long edge.
    img_256 = load_and_resize(src_path, 256)
    tw, th = img_256.size
    gt_arr = np.array(img_256)

    print(f"# Source: {src_path.name}  encoder-input {img_100.size}  GT {img_256.size}")

    cells: list[tuple[str, Image.Image, str]] = []
    cells.append(("ground truth (256 long-edge)", img_256, f"PSNR=∞"))

    # pfhash 4 modes
    for shape, n in [
        (ShapeType.DCT, 0),
        (ShapeType.CIRCLE, 12),
        (ShapeType.TRIANGLE, 12),
        (ShapeType.PIXEL, 12),
    ]:
        im, hb = pfhash_decode_to_image(img_100, shape, n, tw, th)
        p = psnr(np.array(im), gt_arr)
        cells.append((f"pfhash · {shape.value.upper()}",
                       im, f"{hb} B · PSNR {p:.1f} dB"))

    # thumbhash JS
    try:
        im, hb = thumbhash_js_decode(img_100, tw, th)
        p = psnr(np.array(im), gt_arr)
        cells.append(("thumbhash JS (npm)", im, f"{hb} B · PSNR {p:.1f} dB"))
    except Exception as e:
        print(f"thumbhash-js skipped: {e}", file=sys.stderr)

    # thumbhash Go
    try:
        im, hb = thumbhash_go_decode(img_100, tw, th)
        p = psnr(np.array(im), gt_arr)
        cells.append(("thumbhash Go (galdor)", im, f"{hb} B · PSNR {p:.1f} dB"))
    except Exception as e:
        print(f"thumbhash-go skipped: {e}", file=sys.stderr)

    # sqip (Node) — operates on the original-resolution file, outputs SVG.
    try:
        im, hb = sqip_render(src_path, tw, th, n_shapes=12)
        p = psnr(np.array(im), gt_arr)
        cells.append(("sqip 12 primitives", im, f"{hb} B (SVG) · PSNR {p:.1f} dB"))
    except Exception as e:
        print(f"sqip skipped: {e}", file=sys.stderr)

    grid = stack_grid(cells, cell_w=tw, cell_h=th, cols=4)
    out_path = OUT_DIR / f"visual_{name}.png"
    grid.save(out_path)
    print(f"wrote {out_path}")
    for label, _, sub in cells:
        print(f"  - {label}: {sub}")


if __name__ == "__main__":
    main()
