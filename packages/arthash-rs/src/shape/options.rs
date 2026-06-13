//! Search-quality knobs for CIRCLE / TRIANGLE.
//!
//! These affect encoder cost and output fidelity, NOT the byte format.
//! Same Codec + same bytes decode identically regardless of these settings.
//!
//! Two strategies (matches Python):
//!
//! * "primitive" — fogleman/primitive style. Tiny random init scaled to
//!   canvas, Gaussian perturbations, α decoupled from hill-climb, m
//!   independent attempts. Default. Produces bigger, more "summarizing"
//!   shapes.
//! * "topk_uniform" — arthash's historical strategy. Uniform random pool,
//!   top-K hill-climb with uniform [-step, step] perturbation + step
//!   decay. Smaller / more numerous shapes.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    Primitive,
    TopkUniform,
}

#[derive(Clone, Copy, Debug)]
pub struct SearchOptions {
    pub strategy: Strategy,
    pub n_random: u32,
    /// `topk_uniform` only — how many of the top random candidates to climb.
    /// Ignored by `primitive` (always picks 1 best per attempt).
    pub n_topk: u32,
    /// Fixed hill-climb step budget. Used when `hill_climb_max_age` is None.
    pub hill_climb_steps: u32,
    /// Stop hill-climb after this many consecutive non-improving steps.
    /// When set, supersedes `hill_climb_steps`.
    pub hill_climb_max_age: Option<u32>,
    /// Repeat (random + climb) pipeline this many times per shape.
    pub n_attempts: u32,
    /// Joint-refinement (backfitting) passes after the greedy fit: each pass
    /// revisits every shape once — remove, re-search against the remaining
    /// canvas, keep the better of old/new by exact total SSE. 0 (default)
    /// preserves the historical greedy output bit-for-bit.
    pub refine_passes: u32,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            strategy: Strategy::Primitive,
            // Lowered from the original 200: residual-weighted init (see
            // shape::residual) hits the useful candidate space ~3× faster
            // per draw, so fewer draws reach equal-or-better quality.
            n_random: 64,
            n_topk: 1,
            hill_climb_steps: 40,
            hill_climb_max_age: Some(30),
            n_attempts: 4,
            refine_passes: 0,
        }
    }
}

impl SearchOptions {
    /// TRIANGLE's tuned default. Lighter budget than circle since triangle
    /// per-eval rasterization is cheap (small bbox).
    pub fn triangle_default() -> Self {
        Self {
            strategy: Strategy::Primitive,
            n_random: 50,
            n_topk: 1,
            hill_climb_steps: 40,
            hill_climb_max_age: Some(50),
            n_attempts: 3,
            refine_passes: 0,
        }
    }
}
