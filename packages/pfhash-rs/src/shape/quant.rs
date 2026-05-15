//! Field-level encode/decode helpers. SPEC §4.2 / §4.3 / §4.5 / §4.8.

use crate::bitio::{BitReader, BitWriter};
use crate::codec::Codec;
use crate::colorspace::{linear_to_srgb_u8, srgb_u8_to_linear};

// --- aspect (SPEC §4.2) ---

pub fn aspect_code(w: u32, h: u32) -> u32 {
    let aspect = (w as f32) / (h as f32);
    let code = ((aspect.log2() + 3.0) / 6.0 * 254.0).round();
    code.clamp(0.0, 254.0) as u32
}

pub fn aspect_from_code(code: u32) -> f32 {
    2.0f32.powf((code as f32) / 254.0 * 6.0 - 3.0)
}

// --- radius (CIRCLE only; SPEC §4.8) ---

pub fn r_to_q(r: f32, w: u32, h: u32, r_bits: u32) -> u32 {
    let levels = (1u32 << r_bits) - 1;
    let r_min = (1.0f32).max((w.min(h) as f32) / 24.0);
    let r_max = (r_min + 1.0).max(w.max(h) as f32);
    let r = r.max(r_min);
    let t = (r / r_min).log2() / (r_max / r_min).log2();
    (t * (levels as f32)).round().clamp(0.0, levels as f32) as u32
}

pub fn q_to_r(q: u32, w: u32, h: u32, r_bits: u32) -> f32 {
    let levels = (1u32 << r_bits) - 1;
    let r_min = (1.0f32).max((w.min(h) as f32) / 24.0);
    let r_max = (r_min + 1.0).max(w.max(h) as f32);
    if levels == 0 {
        return r_min;
    }
    let t = (q as f32) / (levels as f32);
    r_min * (r_max / r_min).powf(t)
}

// --- alpha (SPEC §4.5) ---

pub fn alpha_to_q(alpha: f32, levels: &[f32]) -> u32 {
    let mut best_i = 0u32;
    let mut best_d = f32::INFINITY;
    for (i, &lv) in levels.iter().enumerate() {
        let d = (lv - alpha).abs();
        if d < best_d {
            best_d = d;
            best_i = i as u32;
        }
    }
    best_i
}

pub fn q_to_alpha(q: u32, levels: &[f32]) -> f32 {
    let i = (q as usize).min(levels.len() - 1);
    levels[i]
}

// --- RGB-565 (SPEC §4.3) ---

pub fn rgb565_pack(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16 >> 3) << 11) | ((g as u16 >> 2) << 5) | (b as u16 >> 3)
}

pub fn rgb565_unpack(p: u16) -> (u8, u8, u8) {
    let r5 = ((p >> 11) & 0x1f) as u8;
    let g6 = ((p >> 5) & 0x3f) as u8;
    let b5 = (p & 0x1f) as u8;
    let r8 = (r5 << 3) | (r5 >> 2);
    let g8 = (g6 << 2) | (g6 >> 4);
    let b8 = (b5 << 3) | (b5 >> 2);
    (r8, g8, b8)
}

// --- color field (palette index OR raw RGB565/RGB888) ---

/// Write a color field. `color_linear` is the float32 linear-RGB triplet
/// the encoder chose; `pidx` is the palette index (ignored when not in
/// palette mode).
pub fn write_color(bw: &mut BitWriter, color_linear: &[f32; 3], pidx: u32, codec: &Codec) {
    if codec.is_palette_mode() {
        let bits = codec.palette_bits();
        let mask = (1u32 << bits) - 1;
        bw.write(pidx & mask, bits);
        return;
    }
    let r8 = linear_to_srgb_u8(color_linear[0]);
    let g8 = linear_to_srgb_u8(color_linear[1]);
    let b8 = linear_to_srgb_u8(color_linear[2]);
    if codec.color_bits == 16 {
        bw.write(rgb565_pack(r8, g8, b8) as u32, 16);
    } else {
        bw.write(r8 as u32, 8);
        bw.write(g8 as u32, 8);
        bw.write(b8 as u32, 8);
    }
}

/// Read a color field; returns linear-RGB triplet.
pub fn read_color(br: &mut BitReader, codec: &Codec) -> [f32; 3] {
    if codec.is_palette_mode() {
        let bits = codec.palette_bits();
        let pidx = br.read(bits) as usize;
        let pal = codec.palette_linear().unwrap();
        let base = pidx * 3;
        return [pal[base], pal[base + 1], pal[base + 2]];
    }
    let (r, g, b) = if codec.color_bits == 16 {
        rgb565_unpack(br.read(16) as u16)
    } else {
        (br.read(8) as u8, br.read(8) as u8, br.read(8) as u8)
    };
    [
        srgb_u8_to_linear(r),
        srgb_u8_to_linear(g),
        srgb_u8_to_linear(b),
    ]
}

/// Quantize a (cx_px, cy_px) coord to its byte-format integer grid.
pub fn quant_xy(cx: f32, cy: f32, tw: u32, th: u32, cx_bits: u32, cy_bits: u32) -> (u32, u32) {
    let x_max = (1u32 << cx_bits) - 1;
    let y_max = (1u32 << cy_bits) - 1;
    let tw_m1 = (tw as i32 - 1).max(1) as f32;
    let th_m1 = (th as i32 - 1).max(1) as f32;
    let x_q = (cx / tw_m1 * (x_max as f32)).round().clamp(0.0, x_max as f32) as u32;
    let y_q = (cy / th_m1 * (y_max as f32)).round().clamp(0.0, y_max as f32) as u32;
    (x_q, y_q)
}
