//! Oklab + sRGB↔linear for DCT mode. SPEC §4.6 / §4.7.
//!
//! Two stages each direction:
//!     sRGB  --gamma-->  linear RGB  --LMS+cbrt-->  Oklab
//!
//! a/b pre-scaled by AB_SCALE = 5 so they fill the same DC/AC grid as L.

pub const AB_SCALE: f32 = 5.0;

// Forward LMS-like matrix (linear-sRGB → cone responses).
const M1: [[f32; 3]; 3] = [
    [0.412_221_46, 0.536_332_55, 0.051_445_995],
    [0.211_903_5, 0.680_699_5, 0.107_396_96],
    [0.088_302_46, 0.281_718_85, 0.629_978_7],
];
const M2: [[f32; 3]; 3] = [
    [0.210_454_26, 0.793_617_8, -0.004_072_047],
    [1.977_998_5, -2.428_592_2, 0.450_593_7],
    [0.025_904_037, 0.782_771_77, -0.808_675_77],
];

// Inverses precomputed (matches Python's numpy.linalg.inv at f32 precision).
const M1_INV: [[f32; 3]; 3] = [
    [4.076_741_7, -3.307_711_6, 0.230_969_94],
    [-1.268_438, 2.609_757_4, -0.341_319_38],
    [-0.0041960863, -0.703_418_6, 1.707_614_7],
];
const M2_INV: [[f32; 3]; 3] = [
    [1.0, 0.396_337_78, 0.215_803_76],
    [1.0, -0.105_561_346, -0.063_854_17],
    [1.0, -0.089_484_18, -1.291_485_5],
];

#[inline]
fn srgb_to_linear_f(s: f32) -> f32 {
    let s = s.clamp(0.0, 1.0);
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// 256-entry LUT mapping sRGB u8 (0..255) → linear-RGB f32 (0..1).
/// Built once at first use; the cost is amortised across every encode call.
fn srgb_to_linear_u8_lut() -> &'static [f32; 256] {
    use std::sync::OnceLock;
    static LUT: OnceLock<[f32; 256]> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut t = [0.0f32; 256];
        for (i, slot) in t.iter_mut().enumerate() {
            *slot = srgb_to_linear_f(i as f32 / 255.0);
        }
        t
    })
}

/// sRGB-u8 interleaved `(R,G,B)` → Oklab `(L, a·AB_SCALE, b·AB_SCALE)` planes
/// in a single fused pass. Combines the 256-entry sRGB→linear LUT with the
/// LMS+cbrt Oklab projection so the intermediate linear-RGB never lands in
/// heap Vecs (3 output allocations instead of 6). The per-pixel arithmetic
/// order is identical to the old `rgb_u8_to_linear_planes` +
/// `linear_rgb_to_oklab_channels` pair it replaced, so output is byte-for-byte
/// unchanged. Used by the opaque DCT fast path.
pub fn rgb_u8_to_oklab_channels(rgb: &[u8], n: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let lut = srgb_to_linear_u8_lut();
    let mut l_out = vec![0.0f32; n];
    let mut a_out = vec![0.0f32; n];
    let mut b_out = vec![0.0f32; n];
    for i in 0..n {
        let rl = lut[rgb[i * 3] as usize];
        let gl = lut[rgb[i * 3 + 1] as usize];
        let bl = lut[rgb[i * 3 + 2] as usize];
        let lm = M1[0][0] * rl + M1[0][1] * gl + M1[0][2] * bl;
        let mm = M1[1][0] * rl + M1[1][1] * gl + M1[1][2] * bl;
        let sm = M1[2][0] * rl + M1[2][1] * gl + M1[2][2] * bl;
        let lc = lm.cbrt();
        let mc = mm.cbrt();
        let sc = sm.cbrt();
        l_out[i] = M2[0][0] * lc + M2[0][1] * mc + M2[0][2] * sc;
        a_out[i] = (M2[1][0] * lc + M2[1][1] * mc + M2[1][2] * sc) * AB_SCALE;
        b_out[i] = (M2[2][0] * lc + M2[2][1] * mc + M2[2][2] * sc) * AB_SCALE;
    }
    (l_out, a_out, b_out)
}

#[inline]
fn linear_to_srgb_f(l: f32) -> f32 {
    let c = l.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB (0..1) `(R, G, B)` planes → Oklab `(L, a*AB_SCALE, b*AB_SCALE)`.
///
/// Inputs and outputs are flat row-major buffers of length `h*w`.
pub fn rgb_to_oklab_channels(
    r: &[f32],
    g: &[f32],
    b: &[f32],
) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let n = r.len();
    let mut l_out = Vec::with_capacity(n);
    let mut a_out = Vec::with_capacity(n);
    let mut b_out = Vec::with_capacity(n);
    for i in 0..n {
        let rl = srgb_to_linear_f(r[i]);
        let gl = srgb_to_linear_f(g[i]);
        let bl = srgb_to_linear_f(b[i]);
        let lm = M1[0][0] * rl + M1[0][1] * gl + M1[0][2] * bl;
        let mm = M1[1][0] * rl + M1[1][1] * gl + M1[1][2] * bl;
        let sm = M1[2][0] * rl + M1[2][1] * gl + M1[2][2] * bl;
        let lc = lm.cbrt();
        let mc = mm.cbrt();
        let sc = sm.cbrt();
        let ll = M2[0][0] * lc + M2[0][1] * mc + M2[0][2] * sc;
        let aa = M2[1][0] * lc + M2[1][1] * mc + M2[1][2] * sc;
        let bb = M2[2][0] * lc + M2[2][1] * mc + M2[2][2] * sc;
        l_out.push(ll);
        a_out.push(aa * AB_SCALE);
        b_out.push(bb * AB_SCALE);
    }
    (l_out, a_out, b_out)
}

/// Oklab planes (L, a*AB_SCALE, b*AB_SCALE) → packed RGB u8 (length 3*n),
/// with optional ordered dithering at the sRGB f32→u8 quantization.
/// `width` is the row length used to tile the Bayer matrix; `dither = false`
/// is byte-identical to plain rounding. The same threshold is applied to
/// all three channels of a pixel so the noise is luminance-only — no
/// per-channel color speckle.
pub fn oklab_channels_to_rgb_u8(
    l_ok: &[f32],
    a_x2: &[f32],
    b_x2: &[f32],
    width: usize,
    dither: bool,
) -> Vec<u8> {
    let n = l_ok.len();
    let inv_ab = 1.0 / AB_SCALE;
    let mut out = Vec::with_capacity(n * 3);
    let (mut x, mut y) = (0usize, 0usize);
    for i in 0..n {
        let ll = l_ok[i];
        let aa = a_x2[i] * inv_ab;
        let bb = b_x2[i] * inv_ab;
        // Oklab → LMS_cbrt via M2_INV (note: M2_INV[*][0] = 1 in Oklab inverse).
        let lc = M2_INV[0][0] * ll + M2_INV[0][1] * aa + M2_INV[0][2] * bb;
        let mc = M2_INV[1][0] * ll + M2_INV[1][1] * aa + M2_INV[1][2] * bb;
        let sc = M2_INV[2][0] * ll + M2_INV[2][1] * aa + M2_INV[2][2] * bb;
        // LMS_cbrt → LMS (cube)
        let lm = lc * lc * lc;
        let mm = mc * mc * mc;
        let sm = sc * sc * sc;
        // LMS → linear RGB via M1_INV
        let rl = M1_INV[0][0] * lm + M1_INV[0][1] * mm + M1_INV[0][2] * sm;
        let gl = M1_INV[1][0] * lm + M1_INV[1][1] * mm + M1_INV[1][2] * sm;
        let bl = M1_INV[2][0] * lm + M1_INV[2][1] * mm + M1_INV[2][2] * sm;
        let rs = linear_to_srgb_f(rl);
        let gs = linear_to_srgb_f(gl);
        let bs = linear_to_srgb_f(bl);
        out.push(crate::render::quant_u8(rs * 255.0, x, y, dither));
        out.push(crate::render::quant_u8(gs * 255.0, x, y, dither));
        out.push(crate::render::quant_u8(bs * 255.0, x, y, dither));
        x += 1;
        if x == width {
            x = 0;
            y += 1;
        }
    }
    out
}
