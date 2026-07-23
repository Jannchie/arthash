//! V4 DCT decoder. SPEC §5.1.

use super::colorspace::oklab_channels_to_rgb_u8;

const COMPANDER_POWER_L: f32 = 0.6;
const COMPANDER_POWER_PQ: f32 = 0.5;
const COMPANDER_POWER_A: f32 = 0.6;
const DC_COMPANDER_POWER_PQ: f32 = 0.4;

fn count_ac(nx: usize, ny: usize) -> usize {
    let mut n = 0usize;
    for cy in 0..ny {
        let mut cx = if cy == 0 { 1 } else { 0 };
        while cx * ny < nx * (ny - cy) {
            n += 1;
            cx += 1;
        }
    }
    n
}

fn derive_lx_ly(aspect: f32, has_alpha: bool) -> (usize, usize) {
    let l_limit: f32 = if has_alpha { 5.0 } else { 7.0 };
    let (lx, ly) = if aspect >= 1.0 {
        (l_limit as usize, (l_limit / aspect).round().max(1.0) as usize)
    } else {
        ((l_limit * aspect).round().max(1.0) as usize, l_limit as usize)
    };
    (lx.max(3), ly.max(3))
}

fn pack_coeffs(nx: usize, ny: usize, dc: f32, ac: &[f32]) -> Vec<f32> {
    let mut c = vec![0.0f32; ny * nx];
    c[0] = dc;
    let mut idx = 0;
    for cy in 0..ny {
        let mut cx = if cy == 0 { 1 } else { 0 };
        while cx * ny < nx * (ny - cy) {
            c[cy * nx + cx] = ac[idx];
            idx += 1;
            cx += 1;
        }
    }
    c
}

fn idct(w_out: usize, h_out: usize, nx: usize, ny: usize, coeffs: &[f32]) -> Vec<f32> {
    // cos_x[x, cx] = cos(π/w_out · (x+0.5) · cx) * α_x[cx]
    // α[0]=1, α[k>0]=2.
    let mut ax = vec![2.0f32; nx];
    ax[0] = 1.0;
    let mut ay = vec![2.0f32; ny];
    ay[0] = 1.0;
    let pi_w = std::f32::consts::PI / (w_out as f32);
    let pi_h = std::f32::consts::PI / (h_out as f32);
    let mut cos_x = vec![0.0f32; w_out * nx]; // (w_out, nx) row-major
    for x in 0..w_out {
        for cx in 0..nx {
            cos_x[x * nx + cx] = (pi_w * ((x as f32) + 0.5) * (cx as f32)).cos() * ax[cx];
        }
    }
    let mut cos_y = vec![0.0f32; h_out * ny]; // (h_out, ny) row-major
    for y in 0..h_out {
        for cy in 0..ny {
            cos_y[y * ny + cy] = (pi_h * ((y as f32) + 0.5) * (cy as f32)).cos() * ay[cy];
        }
    }
    // tmp = cos_y(h_out × ny) · coeffs(ny × nx)
    let mut tmp = vec![0.0f32; h_out * nx];
    unsafe {
        matrixmultiply::sgemm(
            h_out, ny, nx,
            1.0,
            cos_y.as_ptr(),  ny as isize, 1,
            coeffs.as_ptr(), nx as isize, 1,
            0.0,
            tmp.as_mut_ptr(), nx as isize, 1,
        );
    }
    // out = tmp(h_out × nx) · cos_xᵀ(nx × w_out).
    // cos_x is (w_out, nx) row-major; viewed as cos_xᵀ it's (nx, w_out) with
    // row-stride 1, col-stride nx. The right matrix is (nx × w_out), so:
    //   rsb = 1, csb = nx (since cos_x[x][cx] = cos_x.as_ptr()[x*nx+cx]).
    let mut out = vec![0.0f32; h_out * w_out];
    unsafe {
        matrixmultiply::sgemm(
            h_out, nx, w_out,
            1.0,
            tmp.as_ptr(),     nx as isize, 1,
            cos_x.as_ptr(),   1, nx as isize,
            0.0,
            out.as_mut_ptr(), w_out as isize, 1,
        );
    }
    out
}

/// Decode DCT hash bytes to (w, h, RGBA u8).
///
/// `dither` enables ordered (Bayer 8×8) dithering at the f32→u8
/// quantization, breaking up the banding that plain rounding produces in
/// DCT's smooth gradients. `false` keeps the historical byte-exact output.
pub fn decode_dct(
    hash: &[u8],
    base_size: u32,
    override_aspect: Option<f32>,
    dither: bool,
) -> (u32, u32, Vec<u8>) {
    // Truncated / empty hashes degrade gracefully (missing bytes read as 0)
    // instead of panicking, matching the zero-fill policy that `BitReader`
    // already gives shape decoders. For a well-formed hash (len ≥ 5, or ≥ 6
    // with alpha) `hdr_byte` returns exactly `hash[i]`, so decoded output is
    // byte-for-byte identical to the previous direct-indexing path.
    let hdr_byte = |i: usize| hash.get(i).copied().unwrap_or(0);

    let header24: u32 =
        (hdr_byte(0) as u32) | ((hdr_byte(1) as u32) << 8) | ((hdr_byte(2) as u32) << 16);
    let header16: u32 = (hdr_byte(3) as u32) | ((hdr_byte(4) as u32) << 8);

    let l_dc = (header24 & 63) as f32 / 63.0;
    let p_dc_companded = ((header24 >> 6) & 63) as f32 / 31.5 - 1.0;
    let q_dc_companded = ((header24 >> 12) & 63) as f32 / 31.5 - 1.0;
    let inv_dc_p = 1.0 / DC_COMPANDER_POWER_PQ;
    let p_dc = p_dc_companded.signum() * p_dc_companded.abs().powf(inv_dc_p);
    let q_dc = q_dc_companded.signum() * q_dc_companded.abs().powf(inv_dc_p);
    let l_scale = ((header24 >> 18) & 31) as f32 / 31.0;
    let has_alpha = ((header24 >> 23) & 1) != 0;

    let aspect_code = header16 & 0xff;
    let p_scale = ((header16 >> 8) & 0xf) as f32 / 15.0;
    let q_scale = ((header16 >> 12) & 0xf) as f32 / 15.0;
    let aspect = 2.0f32.powf((aspect_code as f32) / 254.0 * 6.0 - 3.0);

    let (lx, ly) = derive_lx_ly(aspect, has_alpha);

    let (a_dc, a_scale, ac_start) = if has_alpha {
        let b5 = hdr_byte(5);
        let a_dc = (b5 & 0x0f) as f32 / 15.0;
        let a_scale = ((b5 >> 4) & 0x0f) as f32 / 15.0;
        (a_dc, a_scale, 6usize)
    } else {
        (1.0, 1.0, 5usize)
    };

    let total_nibbles =
        count_ac(lx, ly) + count_ac(3, 3) + count_ac(3, 3) + if has_alpha { count_ac(5, 5) } else { 0 };

    let mut nibble_idx = 0usize;
    let take_nibble = |nibble_idx: &mut usize| -> u8 {
        if *nibble_idx >= total_nibbles {
            return 8;
        }
        let byte_off = ac_start + (*nibble_idx >> 1);
        if byte_off >= hash.len() {
            return 8;
        }
        let byte = hash[byte_off];
        let nibble = if *nibble_idx & 1 != 0 {
            (byte >> 4) & 0x0f
        } else {
            byte & 0x0f
        };
        *nibble_idx += 1;
        nibble
    };

    fn decode_ac_inner(
        nx: usize,
        ny: usize,
        scale: f32,
        power: f32,
        nibble_idx: &mut usize,
        total: usize,
        hash: &[u8],
        ac_start: usize,
    ) -> Vec<f32> {
        if scale <= 0.0 {
            return vec![0.0; count_ac(nx, ny)];
        }
        let scale_c = scale.powf(power);
        let inv_p = 1.0 / power;
        let mut out = Vec::new();
        for cy in 0..ny {
            let mut cx = if cy == 0 { 1 } else { 0 };
            while cx * ny < nx * (ny - cy) {
                let nibble = if *nibble_idx >= total {
                    8u8
                } else {
                    let byte_off = ac_start + (*nibble_idx >> 1);
                    if byte_off >= hash.len() {
                        8u8
                    } else {
                        let b = hash[byte_off];
                        if *nibble_idx & 1 != 0 {
                            (b >> 4) & 0x0f
                        } else {
                            b & 0x0f
                        }
                    }
                };
                *nibble_idx += 1;
                let c_companded = ((nibble as f32) / 7.5 - 1.0) * scale_c;
                let s = c_companded.signum();
                out.push(s * c_companded.abs().powf(inv_p));
                cx += 1;
            }
        }
        out
    }

    let _ = take_nibble; // silence unused

    let l_ac = decode_ac_inner(
        lx,
        ly,
        l_scale,
        COMPANDER_POWER_L,
        &mut nibble_idx,
        total_nibbles,
        hash,
        ac_start,
    );
    let p_ac = decode_ac_inner(
        3,
        3,
        p_scale,
        COMPANDER_POWER_PQ,
        &mut nibble_idx,
        total_nibbles,
        hash,
        ac_start,
    );
    let q_ac = decode_ac_inner(
        3,
        3,
        q_scale,
        COMPANDER_POWER_PQ,
        &mut nibble_idx,
        total_nibbles,
        hash,
        ac_start,
    );
    let a_ac = if has_alpha {
        decode_ac_inner(
            5,
            5,
            a_scale,
            COMPANDER_POWER_A,
            &mut nibble_idx,
            total_nibbles,
            hash,
            ac_start,
        )
    } else {
        Vec::new()
    };

    let out_aspect = override_aspect.unwrap_or(aspect);
    let (w_out, h_out) = if out_aspect > 1.0 {
        (base_size as usize, (((base_size as f32) / out_aspect).round().max(1.0)) as usize)
    } else {
        ((((base_size as f32) * out_aspect).round().max(1.0)) as usize, base_size as usize)
    };

    let l_plane = idct(w_out, h_out, lx, ly, &pack_coeffs(lx, ly, l_dc, &l_ac));
    let p_plane = idct(w_out, h_out, 3, 3, &pack_coeffs(3, 3, p_dc, &p_ac));
    let q_plane = idct(w_out, h_out, 3, 3, &pack_coeffs(3, 3, q_dc, &q_ac));

    let rgb_u8 = oklab_channels_to_rgb_u8(&l_plane, &p_plane, &q_plane, w_out, dither);

    let mut rgba = vec![0u8; w_out * h_out * 4];
    for i in 0..(w_out * h_out) {
        rgba[i * 4] = rgb_u8[i * 3];
        rgba[i * 4 + 1] = rgb_u8[i * 3 + 1];
        rgba[i * 4 + 2] = rgb_u8[i * 3 + 2];
        rgba[i * 4 + 3] = 255;
    }
    if has_alpha {
        let a_plane = idct(w_out, h_out, 5, 5, &pack_coeffs(5, 5, a_dc, &a_ac));
        for y in 0..h_out {
            for x in 0..w_out {
                let i = y * w_out + x;
                rgba[i * 4 + 3] = crate::render::quant_u8(a_plane[i] * 255.0, x, y, dither);
            }
        }
    }

    (w_out as u32, h_out as u32, rgba)
}
