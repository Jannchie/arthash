# arthash format specification — v1

**Status:** v1 draft, frozen for the v1 development cycle. Implementations:
`@arthash/py`, `@arthash/ts`, `arthash` (Rust). Reference implementation:
`@arthash/py`. When they disagree on a corner case the SPEC wins; please open a
PR amending this file rather than mutating any single implementation.

## 0. Why a SPEC

`arthash` is a family of compact placeholder-image hashes. Multiple
implementations (Python / TypeScript / Rust) target the same byte format so
hashes produced by one can be decoded by another. The byte format is **purely
defined by the SPEC, not by the reference implementation** — if Python's
encoder produces output the SPEC doesn't define, that's a bug in Python.

## 1. Scope and design philosophy

arthash hashes are tiny (typically 6–32 bytes) and intended only as **lossy
placeholders** while the full image loads. They are NOT intended for:

- content addressing (use BLAKE3/SHA-256)
- perceptual deduplication (use pHash/dHash)
- thumbnail generation at fidelity (use real image resize)

A arthash carries roughly the same information as a 12×12 image and is
specifically tuned to look plausible when blown up to display size.

### 1.1 Two-sided consensus, not self-describing bytes

A arthash hash is **not self-describing**. The bytes alone are meaningless;
they must be decoded with the same **codec** that produced them. The codec
carries: which mode (DCT / CIRCLE / TRIANGLE / PIXEL), how many shapes, all
bit widths, and the palette if any.

In exchange for losing self-description, the byte stream carries no header
overhead — every bit is image-dependent. Storage systems that need different
codecs per content type (e.g. avatars use one codec, hero images another)
choose the codec at the application layer.

> **Implication for SDKs:** a hash and a codec are a single logical unit.
> SDKs must accept a Codec value as input to both `encode` and `decode`.

### 1.2 Codec defaults

The fields below have defaults; when a field is "ignored" for a mode, its
value MUST still parse but does not affect bytes.

| Field | Default | DCT | CIRCLE | TRIANGLE | PIXEL |
|---|---|---|---|---|---|
| `shape` | `"circle"` | — | — | — | — |
| `n_shapes` | `12` | ignored | required | required | required (= grid_w × grid_h) |
| `cx_bits` | `5` | ignored | required | required | ignored |
| `cy_bits` | `5` | ignored | required | required | ignored |
| `r_bits` | `4` | ignored | required | ignored | ignored |
| `alpha_bits` | `3` | ignored | required | required | ignored |
| `color_bits` | `16` | ignored | when palette null | when palette null | when palette null |
| `palette` | `null` | ignored | optional | optional | optional |
| `palette_k` | `null` | ignored | when palette set | when palette set | when palette set |
| `alpha_levels` | linspace(0.20, 0.90, 2^alpha_bits) | ignored | optional | optional | ignored |
| `grid_aspect` | `null` | ignored | ignored | ignored | optional |

A field marked "ignored" MAY be omitted by serialization formats but MUST
not break parsing if present.

### 1.3 What "version 1" means

v1 freezes:

- The set of modes: `DCT`, `CIRCLE`, `TRIANGLE`, `PIXEL` (mode tags are
  enum names; integer encodings only exist within a Codec).
- The bit layout for each mode given a Codec.
- The semantics of every field (aspect, color encoding, palette indexing,
  alpha quantization, geometry quantization).
- Decoder reconstruction rules — which fields are normative and which are
  renderer freedom (e.g. anti-aliasing is renderer freedom).

v1 does NOT freeze:

- Encoder hyperparameters (search size, hill-climb steps, palette training).
- Decoder rendering quality (AA modes, smoothing, upsample interpolators).
- The set of bundled palettes — palettes can be added/refined.
- Numerical tolerance for color conversion — see §4.1.

Adding a new mode (e.g. ellipses) requires v2. Adding a new bundled palette
or improving the encoder does NOT.

## 2. Codec contract

```ts
type ShapeType = "dct" | "circle" | "triangle" | "pixel"

interface Codec {
  shape: ShapeType
  n_shapes: number      // DCT: ignored. PIXEL: grid_w*grid_h.

  // Quantization grids (CIRCLE/TRIANGLE/PIXEL).
  cx_bits: number       // default 5
  cy_bits: number       // default 5
  r_bits: number        // default 4 (CIRCLE only)
  alpha_bits: number    // default 3 (CIRCLE/TRIANGLE only)

  // Color storage (continuous mode only).
  color_bits: 16 | 24   // 16 = RGB-565, 24 = RGB-888

  // Optional external palette. Triggers palette mode.
  palette: Uint8Array | null  // (K, 3) uint8 sRGB, row-major
  palette_k: number | null    // effective K (default = palette.length / 3)

  // Optional discrete alpha set. Length must equal 2^alpha_bits.
  alpha_levels: Float32Array | null  // default = linspace(0.20, 0.90, 2^alpha_bits)

  // PIXEL only: aspect ratio of the cell grid. null → derive from image aspect.
  grid_aspect: number | null
}
```

**Two codecs are byte-compatible iff:**

- `shape` matches.
- `n_shapes` matches.
- For shape modes: all bit widths (`cx_bits`, `cy_bits`, `r_bits`, `alpha_bits`,
  `color_bits`) match.
- Palette mode matches (both `palette == null` or both `palette != null`).
- If palette mode: `palette_k` is the same and `palette[0:palette_k]` is
  element-wise identical (uint8). Entries beyond `palette_k` in either codec
  are ignored.
- If discrete alpha levels are provided: `alpha_levels` matches element-wise
  within 1e-5 (allow for floating-point reconstruction differences across
  languages).

The codec is opaque to the byte stream — none of it is stored in the hash.

## 3. Bit stream conventions

### 3.1 Bit packing

The byte stream is read and written **LSB-first within each byte**. When a
field of `n` bits is written:

```
acc       |= (value & ((1 << n) - 1)) << cursor_bits
cursor_bits += n
while cursor_bits >= 8:
  emit acc & 0xff
  acc >>= 8
  cursor_bits -= 8
```

Equivalently: the first written bit ends up in bit 0 of byte 0, the second
written bit in bit 1 of byte 0, and so on.

### 3.2 Hash length and end-of-stream padding

The hash byte length is **deterministic given the codec**:

```
hash_bytes = ceil((header_bits + n_shapes * per_shape_bits) / 8)
```

where `header_bits` and `per_shape_bits` are mode-specific (see §5). The
encoder MUST emit exactly this many bytes; the decoder MAY validate against
this count and reject mismatches.

The final partial byte is zero-padded in the high bits (i.e. the unused
top bits of the last byte are 0). The decoder MUST tolerate up to 7 zero
pad bits after the last semantic field; they have no meaning.

### 3.3 Field ordering

Within one logical record (a circle, a triangle, an AC nibble run, etc.),
fields are written in the order described in this SPEC. Reordering is a
backwards-incompatible change.

### 3.4 Endianness

The byte stream itself has no endianness — bytes are emitted in order. All
multi-byte semantic values are reconstructed bit-by-bit per §3.1, so host
endianness is irrelevant.

## 4. Common encodings

### 4.1 sRGB ↔ linear RGB

Encoders and decoders SHOULD perform alpha-blending and color fitting in
**linear** RGB, NOT sRGB. Pixel-grid color averaging is mathematically wrong
in sRGB and visibly so on saturated areas.

The transform is the standard IEC 61966-2-1 sRGB transfer function. Let
`s = srgb_u8 / 255`:

```
linear = ((s + 0.055) / 1.055) ^ 2.4   if s > 0.04045
linear = s / 12.92                     otherwise
```

The inverse:

```
srgb = 1.055 * (linear ^ (1/2.4)) - 0.055           if linear > 0.0031308
srgb = linear * 12.92                                otherwise
```

Implementations MAY use a 256-entry lookup table for the forward transform.
Implementations MUST agree on rounded sRGB output to within 1 unit per
channel on round-trip, given the same linear inputs.

### 4.2 Aspect code (8 bits)

The 8-bit aspect code is a quantized log2-aspect ratio:

```
aspect_code = clamp(round((log2(w/h) + 3.0) / 6.0 * 254.0), 0, 254)
aspect_quant = 2 ^ (aspect_code / 254.0 * 6.0 - 3.0)
```

This represents aspect ratios in [1/8, 8] with 255 levels evenly spaced in
log space. Aspect ratios outside this range clip to the endpoints (the
encoder MUST NOT error — extreme banner images are valid input).

**Aspect code 255 is reserved.** The encoder MUST clamp to 254. A decoder
encountering code 255 MAY treat it as 254 (lenient mode) or reject the hash
(strict mode); v1 SDKs are free to choose either.

### 4.3 RGB-565 (16 bits) and RGB-888 (24 bits)

When `color_bits == 16`:

```
bits 0..4   = blue5   = b_u8 >> 3
bits 5..10  = green6  = g_u8 >> 2
bits 11..15 = red5    = r_u8 >> 3
```

Decoder reconstructs 8-bit channels by replicating the top bits into the
dropped bottom bits (standard RGB-565 reconstruction):

```
r_u8 = (red5 << 3)   | (red5 >> 2)
g_u8 = (green6 << 2) | (green6 >> 4)
b_u8 = (blue5 << 3)  | (blue5 >> 2)
```

When `color_bits == 24`: three consecutive 8-bit fields, R then G then B.

### 4.4 Palette index

When `codec.palette != null`, color fields are replaced by a `palette_bits`-bit
index, where `palette_bits = log2(codec.palette_k)`. `palette_k` MUST be a
power of two in {2, 4, 8, 16, 32, 64, 128, 256, 512, 1024}.

Decoder looks up `codec.palette[index]` and applies §4.1 sRGB→linear before
compositing.

### 4.5 Discrete alpha levels

When `codec.alpha_bits > 0`, the alpha field is a `alpha_bits`-bit index into
`codec.alpha_levels`, which is an array of `1 << alpha_bits` float32 values
in [0, 1]. Default:

```
alpha_levels = linspace(0.20, 0.90, 1 << alpha_bits)
```

The 0.20 lower bound is intentional — alphas below ~0.15 contribute too little
visual delta per shape to be worth the codec budget; the 0.90 upper bound
preserves background visibility under overlapping shapes.

### 4.6 Oklab color space (used by DCT mode only)

Oklab (Björn Ottosson, 2020) is a perceptually-uniform color space.
Implementations MUST use the following transform from **linear RGB**
(D65) — NOT sRGB — to Oklab.

Forward: linear-RGB → LMS → Oklab

```
[ l ]   [ 0.4122214708   0.5363325363   0.0514459929 ]   [ r ]
[ m ] = [ 0.2119034982   0.6806995451   0.1073969566 ] · [ g ]
[ s ]   [ 0.0883024619   0.2817188376   0.6299787005 ]   [ b ]

l' = l ^ (1/3)
m' = m ^ (1/3)
s' = s ^ (1/3)

[ L ]   [ 0.2104542553   0.7936177850  -0.0040720468 ]   [ l' ]
[ a ] = [ 1.9779984951  -2.4285922050   0.4505937099 ] · [ m' ]
[ b ]   [ 0.0259040371   0.7827717662  -0.8086757660 ]   [ s' ]
```

Reverse: Oklab → linear-RGB

```
[ l' ]   [ 1   0.3963377774   0.2158037573 ]   [ L ]
[ m' ] = [ 1  -0.1055613458  -0.0638541728 ] · [ a ]
[ s' ]   [ 1  -0.0894841775  -1.2914855480 ]   [ b ]

l = l' ^ 3
m = m' ^ 3
s = s' ^ 3

[ r ]   [  4.0767416621  -3.3077115913   0.2309699292 ]   [ l ]
[ g ] = [ -1.2684380046   2.6097574011  -0.3413193965 ] · [ m ]
[ b ]   [ -0.0041960863  -0.7034186147   1.7076147010 ]   [ s ]
```

The cube root MUST be the **real** (signed) cube root, NOT
`pow(x, 1/3)` which only works for non-negative inputs. Implementations
must handle negative `l, m, s` if they occur during round-trip
quantization noise.

The L channel is in `[0, 1]`; the a, b channels are unbounded in theory
but typically in `[-0.5, 0.5]` for in-gamut sRGB inputs.

### 4.7 AB_SCALE (DCT mode only)

The Oklab a, b channels are scaled by `AB_SCALE = 5` before quantization
and divided out on decode:

```
a_storage = AB_SCALE * a_oklab
b_storage = AB_SCALE * b_oklab
```

This scale matches the dynamic range of a, b to the L channel so the
shared 4-bit AC quantizer wastes no codes. Out-of-range scaled values
(`|a_storage| > 1`) are companded into range before quantization (§5.1).

### 4.8 Circle radius quantization

Circle radius uses a log scale anchored to the rendered image size:

```
r_min = max(1.0, min(w, h) / 24.0)
r_max = max(r_min + 1, max(w, h))
t = log2(max(r, r_min) / r_min) / log2(r_max / r_min)
r_q = clamp(round(t * ((1 << r_bits) - 1)), 0, (1 << r_bits) - 1)
```

Reverse:

```
t = r_q / ((1 << r_bits) - 1)
r = r_min * (r_max / r_min) ^ t
```

NOTE: `r_min` and `r_max` depend on the thumbnail size used during encoding
(48 in the reference impl) but ALSO on the decode `(w, h)`. Encoder MUST use
thumbnail size; decoder MUST use the rendered output size. This means an
encoded radius will scale proportionally when decoded at a larger size,
which is the desired behavior.

## 5. Per-mode byte layouts

### 5.1 DCT mode (V4 arthash, ~21 bytes)

#### 5.1.1 Byte layout

Per-image byte stream is:

```
[ header24: 3 bytes ]
  bits  0..  5 : l_dc       (6 bits)   — Oklab L DC, unsigned [0, 63] → [0, 1]
  bits  6.. 11 : p_dc       (6 bits)   — Oklab a DC, signed (companded)
  bits 12.. 17 : q_dc       (6 bits)   — Oklab b DC, signed (companded)
  bits 18.. 22 : l_scale    (5 bits)   — L AC scale, [0, 31] → [0, 1]
  bits 23      : has_alpha  (1 bit)

[ header16: 2 bytes ]
  bits  0.. 7  : aspect_code (8 bits, see §4.2)
  bits  8..11  : p_scale    (4 bits)   — a AC scale
  bits 12..15  : q_scale    (4 bits)   — b AC scale

[ if has_alpha: 1 byte ]
  bits  0.. 3  : a_dc       (4 bits)   — alpha DC
  bits  4.. 7  : a_scale    (4 bits)   — alpha AC scale

[ AC nibble stream, 4 bits per coefficient, packed LSB-first ]
  L AC: N_L coefficients (see §5.1.4)
  P AC: N_P = 8 coefficients (3×3 triangular minus DC)
  Q AC: N_Q = 8 coefficients
  A AC (if has_alpha): N_A = 24 coefficients (5×5 minus DC)
```

`header_bits = 3*8 + 2*8 + (has_alpha ? 8 : 0) = 40 or 48 bits`.

`per_shape_bits` for §3.2 is not applicable; DCT length is:

```
hash_bytes = (header_bits + 4 * (N_L + 16 + (has_alpha ? 24 : 0))) / 8
```

#### 5.1.2 DCT-II basis

The basis is DCT-II with offset sampling. For a 1D channel of length `n`,
the basis function for frequency `k` evaluated at integer pixel index
`x ∈ {0, …, n-1}` is:

```
basis(n, k, x) = cos((π/n) * k * (x + 0.5))
```

For a 2D channel `f[y, x]` of shape `(h, w)`, the unnormalized
coefficient at frequency `(cy, cx)` is:

```
F[cy, cx] = Σ_{y,x} f[y, x] * basis(h, cy, y) * basis(w, cx, x) / (w * h)
```

Equivalently in matrix form, with `Cx[k, x] = basis(w, k, x)` (shape
`nx × w`) and `Cy[k, y] = basis(h, k, y)` (shape `ny × h`):

```
F = (Cy · f · Cxᵀ) / (w · h)
```

The inverse:

```
f̂[y, x] = Σ_{cy, cx} F[cy, cx] * basis(h, cy, y) * basis(w, cx, x)
```

i.e. `f̂ = Cyᵀ · F · Cx` with the appropriate scaling absorbed in `F`
(the `1/(w*h)` is applied at projection time, not inverse time, so the
inverse uses bare matrix multiplications).

NOTE: this is the same convention as ThumbHash and most "lossless"
DCT-II implementations — but DIFFERENT from JPEG's DCT-II, which uses
orthonormal normalization. Implementations MUST use the convention
above to remain compatible.

#### 5.1.3 Triangular mask

For a `(nx, ny)` DCT grid, only coefficients `(cx, cy)` with

```
cx * ny < nx * (ny - cy)
```

are stored. This forms a triangle in the upper-left of the grid. The
coefficient at `(0, 0)` is the DC and is stored separately; the
remaining `count(mask) - 1` coefficients form the AC stream.

The count of `True` entries can be computed by iterating once over the
grid; SDKs MAY hard-code the counts for the standard sizes:

| (nx, ny) | count(mask) | AC count |
|---|---|---|
| (3, 3) | 9 | 8 |
| (5, 5) | 24 | 23 |
| (3, 7), (7, 3) | varies | varies |
| (4, 7), (7, 4) | varies | varies |
| (5, 7), (7, 5) | varies | varies |
| (6, 7), (7, 6) | varies | varies |
| (7, 7) | varies | varies |

The "varies" entries depend on integer arithmetic — implementations
SHOULD compute the mask freshly. The N_P, N_Q, N_A constants in
§5.1.1 correspond to fixed 3×3 and 5×5 grids.

#### 5.1.4 (lx, ly) derivation

For luma, the grid dimensions depend on `aspect_quant` (§4.2) and `has_alpha`:

```
l_limit = 5 if has_alpha else 7
if aspect_quant >= 1:
  lx = l_limit
  ly = max(1, round(l_limit / aspect_quant))
else:
  lx = max(1, round(l_limit * aspect_quant))
  ly = l_limit
```

Then `N_L = count(triangular_mask(max(3, lx), max(3, ly))) - 1`. The
`max(3, ·)` floor mirrors the reference encoder, which always
guarantees a 3×3 minimum so that DC + at least 8 AC coefficients fit.

#### 5.1.5 DC quantization

The luma DC `L ∈ [0, 1]` is quantized linearly to 6 bits:

```
l_dc_q = clamp(round(63 * L), 0, 63)
L̂     = l_dc_q / 63
```

The chroma a, b channels are first scaled by AB_SCALE (§4.7), then
companded with power `0.4`, then quantized signed to 6 bits:

```
a_storage   = AB_SCALE * a                 # AB_SCALE = 5
a_companded = sign(a_storage) * |a_storage| ^ 0.4
p_dc_q      = clamp(round(31.5 + 31.5 * a_companded), 0, 63)

# Decode:
a_companded_hat = (p_dc_q - 31.5) / 31.5
a_storage_hat   = sign(a_companded_hat) * |a_companded_hat| ^ (1/0.4)
a_hat           = a_storage_hat / AB_SCALE
```

Same for b. Note the offset `31.5` (not `32` or `31`) — this centers the
6-bit signed range exactly on zero, with codes 0..63 representing values
roughly in `[-1.016, +1.016]` in companded space.

Alpha DC `α ∈ [0, 1]` is quantized linearly to 4 bits:

```
a_dc_q = clamp(round(15 * α), 0, 15)
α̂     = a_dc_q / 15
```

#### 5.1.6 AC quantization and companders

Each AC coefficient `c` is companded with the channel-specific power
before quantization:

| Channel | Compander power `p` | Scale bits |
|---|---|---|
| L (luma) | `0.6` | 5 |
| a, b (chroma) | `0.5` | 4 |
| α (alpha) | `0.6` | 4 |

Encoding:

```
c_companded = sign(c) * |c| ^ p
q           = clamp(round(c_companded / scale^p * 7.5 + 7.5), 0, 15)
```

The "declared scale" is quantized to `scale_bits`:

```
scale_q ∈ [0, (1 << scale_bits) - 1]
scale   = scale_q / ((1 << scale_bits) - 1)
```

Decoding:

```
scale       = scale_q / ((1 << scale_bits) - 1)
c_companded = (q / 7.5 - 1.0) * scale^p
c_hat       = sign(c_companded) * |c_companded| ^ (1/p)
```

#### 5.1.7 DCT decode procedure

Decoder reconstructs each channel at the rendered output resolution
`(w, h)`, NOT the encode resolution:

1. Parse header → DC values, scales, has_alpha, aspect_code.
2. Derive `(lx, ly)` from quantized aspect and has_alpha (§5.1.4).
3. Parse AC nibble stream → `(N_L, 8, 8, [24])` coefficients.
4. Dequantize each AC coefficient using `scale^p` and the inverse
   compander (§5.1.6).
5. Place AC coefficients into the upper-left triangle of empty
   `(ly, lx)`, `(3, 3)`, `(3, 3)`, `(5, 5)` coefficient grids. Place DC
   at `[0, 0]`.
6. Inverse-DCT each grid (§5.1.2) at the output `(w, h)` size to get
   `L, a, b, [α]` channel maps.
7. Convert `(L, a, b)` Oklab → linear RGB (§4.6).
8. Composite over white if no alpha, else use `α` as transparency.
9. Linear → sRGB (§4.1) for output.

### 5.2 CIRCLE mode

Per-image byte stream:

```
[ aspect_code: 8 bits, see §4.2 ]
[ background color: color_field_bits, see §4.3 / §4.4 ]
[ for i in 0..n_shapes: ]
  [ cx_q:    cx_bits     ]
  [ cy_q:    cy_bits     ]
  [ r_q:     r_bits, see §4.8     ]
  [ color:   color_field_bits ]
  [ alpha_q: alpha_bits, see §4.5  ]
```

Positions are quantized linearly:

```
cx_q = clamp(round(cx_px / (tw - 1) * ((1 << cx_bits) - 1)), 0, max)
cy_q = clamp(round(cy_px / (th - 1) * ((1 << cy_bits) - 1)), 0, max)
```

where `tw`, `th` are the **thumbnail dimensions used by the encoder**.
Decoder un-quantizes against its own `(w, h)` rendered output dimensions.

### 5.3 TRIANGLE mode

```
[ aspect_code: 8 bits ]
[ background color: color_field_bits ]
[ for i in 0..n_shapes: ]
  [ for j in 0..3: ]
    [ vx_q: cx_bits ]
    [ vy_q: cy_bits ]
  [ color:   color_field_bits ]
  [ alpha_q: alpha_bits ]
```

Vertices are quantized identically to circle centers. Vertex order within a
triangle is NOT semantic — winding order is whatever the encoder produces;
the renderer must rasterize regardless of orientation.

### 5.4 PIXEL mode

```
[ aspect_code: 8 bits ]
[ for i in 0..n_shapes: ]
  [ color: color_field_bits ]
```

`n_shapes = grid_w * grid_h`. Grid dimensions are derived deterministically
from `aspect_code` and `codec.grid_aspect`:

```
target_aspect = grid_aspect ?? aspect_quant
choose (grid_w, grid_h) such that grid_w * grid_h == n_shapes
and |log(grid_w / grid_h) - log(target_aspect)| is minimized.
ties break to smaller grid_w.
```

Both encoder and decoder MUST use the same algorithm so they agree on
`(grid_w, grid_h)`. Cells are written in row-major order: row 0 left-to-right,
then row 1, etc.

PIXEL mode has no per-cell alpha field — cells are opaque.

**Degenerate `n_shapes`:** when `n_shapes` is prime the only factorizations
are `(1, n_shapes)` and `(n_shapes, 1)`, giving a strip rather than a grid.
This is legal but rarely useful — callers should prefer composite
`n_shapes` (4, 6, 8, 12, 16, 24, 32, 48, 64 …). The SPEC does NOT reserve
"nice" values; it's a usability concern.

## 6. Decoder rendering contract

The decoder converts a byte stream + codec into an RGBA raster image at a
caller-specified `base_size` (long-edge pixels). Several decisions are
**renderer freedom**, not SPEC:

| Decision | SPEC says | Notes |
|---|---|---|
| Anti-aliasing | implementation-defined | Renderer may use distance-field AA (circles), supersample AA (triangles), or no AA. |
| Pixel-cell upsample filter | implementation-defined | "nearest" (default), "bilinear", or "bicubic". |
| Working color space | linear RGB | Compositing MUST happen in linear; final output converted back to sRGB. |
| Output dtype | 8-bit RGBA, premultiplied | Alpha = 1 throughout — arthash placeholders are fully opaque. |
| Background fill | exact color from header | No dithering. |

Renderers MAY make any rendering decision NOT listed above — but the
resulting output must be visually consistent with the input image (i.e. the
renderer can't add artistic flourishes).

### 6.1 Alpha compositing formula (shape modes)

For CIRCLE and TRIANGLE, each shape is composited onto the running canvas
in linear RGB. Given canvas pixel `C` (linear RGB), shape color `K`
(linear RGB), shape alpha `α ∈ [0, 1]`, and shape coverage `m ∈ [0, 1]`
(1.0 inside the shape, 0.0 outside; fractional for anti-aliased edges):

```
contrib  = α * m
C_next   = (1 - contrib) * C + contrib * K
```

After all shapes are composited, the final canvas is converted linear →
sRGB (§4.1) for output. The canvas is initialized to the background color
from the header.

For PIXEL mode there is no compositing — each cell directly outputs its
color (after sRGB → linear if needed for upsample filtering, then linear
→ sRGB for output).

For DCT mode the IDCT reconstructs each channel directly; compositing is
not used.

## 7. Encoder freedom

The SPEC defines the byte format, not the algorithm that produces those
bytes. An encoder may use any technique to choose shape positions / colors /
alphas, as long as the output decodes correctly under the same codec.

Reference implementations use:

- **DCT:** grid-search per-channel scale that minimizes SSE under the
  4-bit quantizer.
- **CIRCLE / TRIANGLE:** Fogleman-style hill climbing — N random
  candidates, refine top-K with steepest-descent mutations.
- **PIXEL:** mean-color of each cell, snapped to nearest palette entry
  (in linear-RGB Euclidean distance).

Future encoders that beat these on SSE / SSIM / human preference are
welcome. The byte format is the only contract.

## 8. Reference test vectors

Cross-language conformance vectors live at `docs/test-vectors/vectors.json`.
Every SDK (Python, TypeScript, Rust) MUST produce `expected_hex` exactly
for each `(input, codec, encode_kwargs)` triple.

### Format

```json
{
  "version": "1.0",
  "input_kinds": { ... documentation of each input generator ... },
  "vectors": [
    {
      "name": "dct-solid-red-100x100",
      "input": {"kind": "solid", "h": 100, "w": 100, "rgb": [255, 0, 0]},
      "codec": {
        "shape": "dct",
        "n_shapes": 12,
        "cx_bits": 5, "cy_bits": 5, "r_bits": 4, "alpha_bits": 3,
        "color_bits": 16
      },
      "encode_kwargs": {"target_size": 100},
      "expected_hex": "e8af037f0088888888888888888888888888888888888808",
      "expected_bytes": 24
    },
    ...
  ]
}
```

### Input kinds

Test inputs are **deterministic in-memory uint8 RGB arrays**, not PNG files
— this keeps vectors binary-stable across platforms (PNG encoders differ
on metadata, compression, and color profiles).

| `kind`     | Generator |
|------------|-----------|
| `solid`    | `arr[..., :] = rgb` — uniform `H × W × 3` fill |
| `gradient` | `arr[y, x] = (linspace(0,255,W)[x], linspace(0,255,H)[y], 64)` |
| `random`   | `(np.random.default_rng(seed).random((H, W, 3)) * 255).astype(uint8)` — non-Python SDKs may need to replicate numpy's PCG64 sequence, or skip random vectors |

### Palette serialization

When a codec has a palette, it's emitted as `palette_hex` — a list of
`"RRGGBB"` strings. SDKs reconstruct a `(K, 3) uint8` palette by parsing
those bytes. `palette_k` is emitted only when smaller than `len(palette)`.
`alpha_levels` is emitted only when non-default (default = `linspace(0.20,
0.90, 1<<alpha_bits)`).

### Regeneration

```sh
cd packages/arthash-py
uv run python -m tests.generate_vectors
```

The generator lives at `packages/arthash-py/tests/generate_vectors.py`. Any
intentional byte-format change must bump the SPEC version (§9) and
regenerate vectors in the same commit.

## 9. Versioning and compatibility

Within v1:

- Mode set is fixed: DCT, CIRCLE, TRIANGLE, PIXEL.
- Byte layouts are fixed.
- Default codecs may be added but not changed.

Breaking changes (new mode, layout change, new field) → v2 SPEC, with both
v1 and v2 supported by SDKs for at least one minor version.

## 10. Open questions / not yet specified

- **Test vector format.** Likely: a JSON file with `{image_sha256, codec_json,
  expected_bytes_hex}` triples, plus the source PNGs in `docs/test-vectors/`.
- **Endianness of palette serialization.** The Codec is in-memory; if
  palettes are persisted to disk separately, the encoding (`.npy` vs JSON
  hex vs PNG strip) is application-level. SPEC defines the in-memory shape.
- **Reserved aspect_code 255.** Currently unused; reserved for a future
  "out of range" sentinel that lets `override_aspect` skip the quantization
  step entirely. Pending v1.1.
