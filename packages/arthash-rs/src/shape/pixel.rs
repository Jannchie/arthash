//! PIXEL mode — fixed-grid mosaic. SPEC §5.7.
//!
//! Grid `(gw, gh)` is derived from the QUANTIZED aspect (the same value the
//! decoder reconstructs from the header), so both sides agree on cell count.
//! Each cell stores one color; no per-cell alpha.

use super::quant::{aspect_code, aspect_from_code, read_color, write_color};
use crate::bitio::{BitReader, BitWriter};
use crate::codec::Codec;
use crate::colorspace::linear_to_srgb_u8;

#[derive(Clone, Debug)]
pub struct Cell {
    pub pidx: u32,
    pub color: [f32; 3],
}

/// Pick `(grid_w, grid_h)` with `grid_w * grid_h == n_cells`, minimizing
/// `|log(grid_w / grid_h) - log(target)|`. Ties break to smaller grid_w.
pub fn pixel_grid(n_cells: u32, aspect: f32, grid_aspect_hint: Option<f32>) -> (u32, u32) {
    let target = grid_aspect_hint.unwrap_or(aspect);
    let log_target = target.max(1e-9).ln();
    let mut best = (1u32, n_cells);
    let mut best_err = f32::INFINITY;
    for gw in 1..=n_cells {
        if n_cells.is_multiple_of(gw) {
            let gh = n_cells / gw;
            let ratio = (gw as f32 / gh as f32).max(1e-9);
            let err = (ratio.ln() - log_target).abs();
            if err < best_err {
                best_err = err;
                best = (gw, gh);
            }
        }
    }
    best
}

/// Mean-color downsample each grid cell of the (h, w, 3) linear-RGB target.
/// If `palette` is set, snap each cell to its nearest palette entry.
pub fn fit_pixels(
    target: &[f32],
    th: u32,
    tw: u32,
    codec: &Codec,
    w_orig: u32,
    h_orig: u32,
) -> Vec<Cell> {
    let a_code = aspect_code(w_orig, h_orig);
    let quant_aspect = aspect_from_code(a_code);
    let (gw, gh) = pixel_grid(codec.n_shapes, quant_aspect, codec.grid_aspect);
    let palette = codec.palette_linear();
    let mut out = Vec::with_capacity((gw * gh) as usize);
    for gy in 0..gh {
        let y0 = ((gy as f32 * th as f32) / (gh as f32)).round().max(0.0) as u32;
        let y1 = (((gy + 1) as f32 * th as f32) / (gh as f32)).round() as u32;
        let y1 = y1.max(y0 + 1).min(th);
        for gx in 0..gw {
            let x0 = ((gx as f32 * tw as f32) / (gw as f32)).round().max(0.0) as u32;
            let x1 = (((gx + 1) as f32 * tw as f32) / (gw as f32)).round() as u32;
            let x1 = x1.max(x0 + 1).min(tw);
            let mut acc = [0.0f64; 3];
            let mut cnt = 0u64;
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = ((y * tw + x) * 3) as usize;
                    acc[0] += target[p] as f64;
                    acc[1] += target[p + 1] as f64;
                    acc[2] += target[p + 2] as f64;
                    cnt += 1;
                }
            }
            let cell = if cnt > 0 {
                [
                    (acc[0] / cnt as f64) as f32,
                    (acc[1] / cnt as f64) as f32,
                    (acc[2] / cnt as f64) as f32,
                ]
            } else {
                [0.0; 3]
            };
            let (pidx, color) = match &palette {
                Some(pal) => {
                    let k = pal.len() / 3;
                    let mut best_k = 0usize;
                    let mut best_d = f32::INFINITY;
                    for ki in 0..k {
                        let d = (0..3)
                            .map(|i| (pal[ki * 3 + i] - cell[i]).powi(2))
                            .sum::<f32>();
                        if d < best_d {
                            best_d = d;
                            best_k = ki;
                        }
                    }
                    (best_k as u32, [
                        pal[best_k * 3],
                        pal[best_k * 3 + 1],
                        pal[best_k * 3 + 2],
                    ])
                }
                None => (0u32, cell),
            };
            out.push(Cell { pidx, color });
        }
    }
    out
}

pub fn encode_body(bw: &mut BitWriter, cells: &[Cell], codec: &Codec) {
    for c in cells {
        write_color(bw, &c.color, c.pidx, codec);
    }
}

/// Encode PIXEL: aspect_code(8) + per-cell color × (gw·gh).
pub fn encode_pixel(
    target: &[f32],
    th: u32,
    tw: u32,
    w_orig: u32,
    h_orig: u32,
    codec: &Codec,
) -> Vec<u8> {
    let cells = fit_pixels(target, th, tw, codec, w_orig, h_orig);
    let mut bw = BitWriter::new();
    bw.write(aspect_code(w_orig, h_orig), 8);
    encode_body(&mut bw, &cells, codec);
    bw.finish()
}

/// Decode PIXEL → flat sRGB u8 RGB at (w_out, h_out). `smooth = "nearest"`
/// produces hard color blocks; `"bilinear"` softens cell boundaries.
pub fn decode_render(
    br: &mut BitReader,
    codec: &Codec,
    w: u32,
    h: u32,
    quant_aspect: f32,
    smooth: PixelSmooth,
) -> Vec<u8> {
    let (gw, gh) = pixel_grid(codec.n_shapes, quant_aspect, codec.grid_aspect);
    let mut cells_lin = vec![0.0f32; (gw * gh * 3) as usize];
    for i in 0..(gw * gh) as usize {
        let c = read_color(br, codec);
        cells_lin[i * 3] = c[0];
        cells_lin[i * 3 + 1] = c[1];
        cells_lin[i * 3 + 2] = c[2];
    }

    match smooth {
        PixelSmooth::Nearest => {
            let mut out = vec![0u8; (w * h * 3) as usize];
            for gy in 0..gh {
                let y0 = ((gy as f32 * h as f32) / (gh as f32)).round().max(0.0) as u32;
                let y1 = (((gy + 1) as f32 * h as f32) / (gh as f32)).round() as u32;
                let y1 = y1.max(y0 + 1).min(h);
                for gx in 0..gw {
                    let x0 = ((gx as f32 * w as f32) / (gw as f32)).round().max(0.0) as u32;
                    let x1 = (((gx + 1) as f32 * w as f32) / (gw as f32)).round() as u32;
                    let x1 = x1.max(x0 + 1).min(w);
                    let idx = (gy * gw + gx) as usize;
                    let r = linear_to_srgb_u8(cells_lin[idx * 3]);
                    let g = linear_to_srgb_u8(cells_lin[idx * 3 + 1]);
                    let b = linear_to_srgb_u8(cells_lin[idx * 3 + 2]);
                    for y in y0..y1 {
                        for x in x0..x1 {
                            let p = ((y * w + x) * 3) as usize;
                            out[p] = r;
                            out[p + 1] = g;
                            out[p + 2] = b;
                        }
                    }
                }
            }
            out
        }
        PixelSmooth::Bilinear => {
            // Simple bilinear in sRGB space (matches CSS / SQIP behaviour).
            let mut cells_u8 = vec![0u8; (gw * gh * 3) as usize];
            for i in 0..(gw * gh) as usize {
                cells_u8[i * 3] = linear_to_srgb_u8(cells_lin[i * 3]);
                cells_u8[i * 3 + 1] = linear_to_srgb_u8(cells_lin[i * 3 + 1]);
                cells_u8[i * 3 + 2] = linear_to_srgb_u8(cells_lin[i * 3 + 2]);
            }
            let mut out = vec![0u8; (w * h * 3) as usize];
            for y in 0..h {
                let fy = (y as f32 + 0.5) * (gh as f32) / (h as f32) - 0.5;
                let y0 = (fy.floor().max(0.0) as u32).min(gh - 1);
                let y1 = (y0 + 1).min(gh - 1);
                let dy = (fy - y0 as f32).clamp(0.0, 1.0);
                for x in 0..w {
                    let fx = (x as f32 + 0.5) * (gw as f32) / (w as f32) - 0.5;
                    let x0 = (fx.floor().max(0.0) as u32).min(gw - 1);
                    let x1 = (x0 + 1).min(gw - 1);
                    let dx = (fx - x0 as f32).clamp(0.0, 1.0);
                    let p00 = ((y0 * gw + x0) * 3) as usize;
                    let p01 = ((y0 * gw + x1) * 3) as usize;
                    let p10 = ((y1 * gw + x0) * 3) as usize;
                    let p11 = ((y1 * gw + x1) * 3) as usize;
                    let p = ((y * w + x) * 3) as usize;
                    for c in 0..3 {
                        let v00 = cells_u8[p00 + c] as f32;
                        let v01 = cells_u8[p01 + c] as f32;
                        let v10 = cells_u8[p10 + c] as f32;
                        let v11 = cells_u8[p11 + c] as f32;
                        let v = (1.0 - dy) * ((1.0 - dx) * v00 + dx * v01)
                            + dy * ((1.0 - dx) * v10 + dx * v11);
                        out[p + c] = v.clamp(0.0, 255.0) as u8;
                    }
                }
            }
            out
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum PixelSmooth {
    #[default]
    Nearest,
    Bilinear,
}
