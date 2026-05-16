//! Nearest-neighbor index over a linear-RGB palette.
//!
//! Built once per `fit_*` / `encode_*` call and passed into `ShapeSums::finalize`.
//! Replaces the per-eval O(K) SSE scan with a constant-cost lookup, which is
//! the dominant cost difference between palette mode and continuous color
//! (RGB-565 / RGB-888) on shape hill-climb.
//!
//! ## Strategy
//!
//! `finalize` reduces palette selection to "find p ∈ palette closest to p*"
//! where p* is the continuous-optimal color (see `raster::ShapeSums::finalize`
//! derivation). So this module only needs a fast Euclidean NN query in the
//! unit RGB cube.
//!
//! * **Small palettes (K ≤ 32)** — linear scan. Themed palettes (PICO-8,
//!   GB-DMG, …) and the auto K ≤ 16 modes all fall here. K is tiny so the
//!   scan is already cheaper than a LUT lookup + memory traffic.
//! * **Large palettes (K ≥ 64)** — precomputed 16³ voxel LUT storing a
//!   *candidate set* per voxel: all palette entries within
//!   `r_min(center) + voxel_diag` of the voxel center, which provably covers
//!   the true NN for any query point inside the voxel. The query then does
//!   an exact NN scan over that small candidate set (typically ≤ ~8 entries
//!   for K = 256 RGB-cube), keeping per-query cost ≈ O(1) without losing
//!   precision.
//!
//! ## Correctness of the candidate-set radius
//!
//! Let `c` be a voxel center, `r = r_min(c)` the distance to its NN, and
//! `D = voxel_diag = √3 / side` (the full corner-to-corner distance). For
//! any `q` inside the voxel and any palette entry `p_far` with
//! `|c − p_far| > r + D`:
//!
//! ```text
//! |q − p_far| ≥ |c − p_far| − |q − c| > r + D − D/2 = r + D/2 ≥ |q − p_NN(q)|
//! ```
//!
//! (Using `|q − p_NN(q)| ≤ |q − c| + |c − p_NN(c)| ≤ D/2 + r`.) So `p_far`
//! cannot be `q`'s NN — every entry within `r + D` of `c` suffices.

/// K threshold above which we precompute a voxel LUT. Below this, linear scan
/// over the palette is cheaper than the LUT lookup overhead.
const LUT_THRESHOLD: usize = 64;

/// Voxel resolution per axis. 8³ = 512 cells; build cost is `side³ × K`
/// distance evaluations, ~130 K FLOP at K=256 (sub-ms). Larger sides shrink
/// the per-voxel candidate count but blow up build cost cubically — at the
/// shape hill-climb's typical query count (~5 k per encode) the build/query
/// trade favors a coarse LUT.
const LUT_SIDE: u32 = 8;

/// LUT-bearing palette index. The LUT is built lazily on the first
/// `nearest` call for large K — small-K palettes and short-lived users
/// (PIXEL mode's per-cell scan) never pay the build cost.
pub struct PaletteIndex {
    palette: Vec<f32>,
    k: usize,
    /// `None` for small K (always linear scan).
    /// `Some(OnceCell::new())` for large K, populated on first `nearest`.
    lut_cell: Option<std::cell::OnceCell<VoxelLut>>,
}

struct VoxelLut {
    side: u32,
    /// Prefix-sum offsets into `candidates`, length = `side³ + 1`. The
    /// candidate slice for voxel `v` is `candidates[offsets[v]..offsets[v+1]]`.
    offsets: Vec<u32>,
    /// Flat candidate-set: palette indices that could be the NN for some
    /// query point inside the corresponding voxel (see module docs for the
    /// covering-radius proof).
    candidates: Vec<u32>,
}

impl PaletteIndex {
    /// Build with lazy LUT for large K — appropriate for hill-climb modes
    /// that issue thousands of `nearest` queries per encode.
    pub fn build(palette: Vec<f32>) -> Self {
        let k = palette.len() / 3;
        let lut_cell = if k >= LUT_THRESHOLD {
            Some(std::cell::OnceCell::new())
        } else {
            None
        };
        Self { palette, k, lut_cell }
    }

    /// Build without any LUT. Use when only a handful of queries are
    /// expected (e.g. PIXEL mode with ≤ 64 cells), so the O(side³·K) LUT
    /// build cost wouldn't be amortized.
    pub fn build_linear(palette: Vec<f32>) -> Self {
        let k = palette.len() / 3;
        Self { palette, k, lut_cell: None }
    }

    #[inline]
    pub fn k(&self) -> usize {
        self.k
    }

    /// Look up palette entry by index (returns a 3-element linear-RGB tuple).
    #[inline]
    pub fn entry(&self, idx: u32) -> [f32; 3] {
        let base = (idx as usize) * 3;
        [self.palette[base], self.palette[base + 1], self.palette[base + 2]]
    }

    /// Return `(idx, palette_color)` for the palette entry closest to `color`.
    /// `color` is linear RGB; may lie outside [0, 1] (the LUT clamps).
    #[inline]
    pub fn nearest(&self, color: [f32; 3]) -> (u32, [f32; 3]) {
        match &self.lut_cell {
            Some(cell) => {
                let lut = cell.get_or_init(|| build_lut(&self.palette, self.k, LUT_SIDE));
                let idx = self.nearest_via_lut(lut, color);
                (idx, self.entry(idx))
            }
            None => self.nearest_linear(color),
        }
    }

    fn nearest_via_lut(&self, lut: &VoxelLut, color: [f32; 3]) -> u32 {
        let v = lut.voxel_of(color);
        let start = lut.offsets[v] as usize;
        let end = lut.offsets[v + 1] as usize;
        // Per-voxel candidate set is non-empty by construction (always
        // includes the voxel's own NN), so unwrap is safe.
        let first = lut.candidates[start];
        let mut best_k = first;
        let base = (first as usize) * 3;
        let dr = self.palette[base] - color[0];
        let dg = self.palette[base + 1] - color[1];
        let db = self.palette[base + 2] - color[2];
        let mut best_d = dr * dr + dg * dg + db * db;
        for &ki in &lut.candidates[start + 1..end] {
            let base = (ki as usize) * 3;
            let dr = self.palette[base] - color[0];
            let dg = self.palette[base + 1] - color[1];
            let db = self.palette[base + 2] - color[2];
            let d = dr * dr + dg * dg + db * db;
            if d < best_d {
                best_d = d;
                best_k = ki;
            }
        }
        best_k
    }

    #[inline]
    fn nearest_linear(&self, color: [f32; 3]) -> (u32, [f32; 3]) {
        let mut best_k = 0usize;
        let mut best_d = f32::INFINITY;
        for ki in 0..self.k {
            let base = ki * 3;
            let dr = self.palette[base] - color[0];
            let dg = self.palette[base + 1] - color[1];
            let db = self.palette[base + 2] - color[2];
            let d = dr * dr + dg * dg + db * db;
            if d < best_d {
                best_d = d;
                best_k = ki;
            }
        }
        let base = best_k * 3;
        (
            best_k as u32,
            [self.palette[base], self.palette[base + 1], self.palette[base + 2]],
        )
    }
}

impl VoxelLut {
    #[inline]
    fn voxel_of(&self, color: [f32; 3]) -> usize {
        let s = self.side;
        let sf = s as f32;
        let xi = ((color[0].clamp(0.0, 1.0) * sf) as u32).min(s - 1) as usize;
        let yi = ((color[1].clamp(0.0, 1.0) * sf) as u32).min(s - 1) as usize;
        let zi = ((color[2].clamp(0.0, 1.0) * sf) as u32).min(s - 1) as usize;
        let s_us = s as usize;
        zi * s_us * s_us + yi * s_us + xi
    }
}

fn build_lut(palette: &[f32], k: usize, side: u32) -> VoxelLut {
    let s = side as usize;
    let inv = 1.0f32 / side as f32;
    // Corner-to-corner distance of a voxel. Candidate radius is
    // `r_min(center) + voxel_diag` — see module docs for the proof.
    let voxel_diag = (3.0f32).sqrt() * inv;

    let n_voxels = s * s * s;
    let mut offsets = Vec::with_capacity(n_voxels + 1);
    offsets.push(0u32);
    let mut candidates: Vec<u32> = Vec::with_capacity(n_voxels * 4);

    for zi in 0..s {
        let z = (zi as f32 + 0.5) * inv;
        for yi in 0..s {
            let y = (yi as f32 + 0.5) * inv;
            for xi in 0..s {
                let x = (xi as f32 + 0.5) * inv;
                // Pass 1: find r_min² = distance² to the NN of this voxel center.
                let mut min_d2 = f32::INFINITY;
                for ki in 0..k {
                    let base = ki * 3;
                    let dr = palette[base] - x;
                    let dg = palette[base + 1] - y;
                    let db = palette[base + 2] - z;
                    let d2 = dr * dr + dg * dg + db * db;
                    if d2 < min_d2 {
                        min_d2 = d2;
                    }
                }
                // Pass 2: keep every entry within `r_min + voxel_diag` of
                // the center. Comparing squared distances against
                // (r_min + voxel_diag)² avoids any sqrt on the hot path.
                let r_min = min_d2.sqrt();
                let threshold = r_min + voxel_diag;
                let threshold2 = threshold * threshold;
                for ki in 0..k {
                    let base = ki * 3;
                    let dr = palette[base] - x;
                    let dg = palette[base + 1] - y;
                    let db = palette[base + 2] - z;
                    let d2 = dr * dr + dg * dg + db * db;
                    if d2 <= threshold2 {
                        candidates.push(ki as u32);
                    }
                }
                offsets.push(candidates.len() as u32);
            }
        }
    }
    candidates.shrink_to_fit();
    VoxelLut { side, offsets, candidates }
}

/// Convenience: build a `PaletteIndex` from `codec.palette_linear()` if any.
pub fn from_codec(codec: &crate::codec::Codec) -> Option<PaletteIndex> {
    codec.palette_linear().map(PaletteIndex::build)
}

/// One-shot nearest-neighbor lookup over a codec's palette without building
/// (and never amortizing) a LUT. Use when a single `nearest` query is all
/// that's needed — e.g. picking the bg palette index in `encode_*`.
pub fn nearest_in_codec(codec: &crate::codec::Codec, color: [f32; 3]) -> Option<u32> {
    let pal = codec.palette_linear()?;
    let k = pal.len() / 3;
    let mut best_k = 0u32;
    let mut best_d = f32::INFINITY;
    for ki in 0..k {
        let base = ki * 3;
        let dr = pal[base] - color[0];
        let dg = pal[base + 1] - color[1];
        let db = pal[base + 2] - color[2];
        let d = dr * dr + dg * dg + db * db;
        if d < best_d {
            best_d = d;
            best_k = ki as u32;
        }
    }
    Some(best_k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb_cube_palette(rb: u32, gb: u32, bb: u32) -> Vec<f32> {
        let (rn, gn, bn) = (1u32 << rb, 1u32 << gb, 1u32 << bb);
        let mut out = Vec::with_capacity((rn * gn * bn * 3) as usize);
        let lv = |i: u32, n: u32| -> f32 {
            if n == 1 { 0.5 } else { i as f32 / (n - 1) as f32 }
        };
        for r in 0..rn {
            for g in 0..gn {
                for b in 0..bn {
                    out.push(lv(r, rn));
                    out.push(lv(g, gn));
                    out.push(lv(b, bn));
                }
            }
        }
        out
    }

    fn dist_sq(palette: &[f32], idx: u32, color: [f32; 3]) -> f32 {
        let base = (idx as usize) * 3;
        let dr = palette[base] - color[0];
        let dg = palette[base + 1] - color[1];
        let db = palette[base + 2] - color[2];
        dr * dr + dg * dg + db * db
    }

    fn min_dist_sq(palette: &[f32], color: [f32; 3]) -> f32 {
        let k = palette.len() / 3;
        (0..k as u32)
            .map(|i| dist_sq(palette, i, color))
            .fold(f32::INFINITY, f32::min)
    }

    #[test]
    fn small_palette_uses_linear_scan() {
        let pal = vec![
            0.0, 0.0, 0.0,
            1.0, 1.0, 1.0,
            1.0, 0.0, 0.0,
            0.0, 1.0, 0.0,
        ];
        let idx = PaletteIndex::build(pal);
        assert!(idx.lut_cell.is_none());
        let (i, c) = idx.nearest([0.9, 0.1, 0.05]);
        assert_eq!(i, 2);
        assert_eq!(c, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn large_palette_uses_lut() {
        // K=256 RGB-cube (8×8×4) — the "auto-8" preset.
        let pal = rgb_cube_palette(3, 3, 2);
        assert_eq!(pal.len() / 3, 256);
        let idx = PaletteIndex::build(pal.clone());
        assert!(idx.lut_cell.is_some());
        // LUT may pick a different ties-breaking entry than linear scan at
        // points equidistant from multiple palette cells. What matters is
        // that the chosen entry sits at the same (minimal) Euclidean
        // distance from the query — voxel-center LUT is exact for cube
        // palettes whose axis spacing exceeds the voxel side (1/16 here).
        for &c in &[
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.5, 0.5, 0.5],
            [0.2, 0.8, 0.4],
            [0.43, 0.43, 0.34],
        ] {
            let (i_lut, _) = idx.nearest(c);
            let d_lut = dist_sq(&pal, i_lut, c);
            let d_min = min_dist_sq(&pal, c);
            assert!(
                (d_lut - d_min).abs() < 1e-6,
                "lut at {:?} picked d={}, true min d={}",
                c, d_lut, d_min,
            );
        }
    }

    #[test]
    fn nearest_clamps_out_of_range() {
        let pal = rgb_cube_palette(3, 3, 2);
        let idx = PaletteIndex::build(pal);
        // Should not panic on out-of-range inputs.
        let _ = idx.nearest([-0.5, 0.5, 1.5]);
        let _ = idx.nearest([2.0, 2.0, 2.0]);
    }
}
