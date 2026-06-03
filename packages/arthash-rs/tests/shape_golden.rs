//! Shape/PIXEL byte-format regression lock.
//!
//! `tests/vectors.rs` only byte-asserts DCT; shape/PIXEL modes were left
//! unlocked because their *reference* vectors come from PIL LANCZOS resize +
//! numpy PCG64, which aren't byte-portable. But that left the shape search
//! TRAJECTORY with no byte-level guard at all — every "hash-preserving"
//! optimization in `docs/OPTIMIZATIONS.md` had nothing proving it actually
//! preserved bytes.
//!
//! This file closes that gap with a *self-referential* golden: inputs are
//! pure-arithmetic (solid / gradient — no LANCZOS), the codec + seed are
//! fixed, and the expected hex is whatever this crate currently emits. Any
//! change that alters the hill-climb trajectory, integral evaluator, α-sweep,
//! or residual init will flip a byte here and fail the test — which is exactly
//! the signal we want before declaring an optimization "hash-preserving".
//!
//! PLATFORM NOTE: shape search consumes `Rng::normal()` (Box-Muller, uses
//! `f64::ln`). `sqrt` is IEEE-correct hardware, but `ln` is a libm routine
//! that *may* differ by 1 ulp across targets; combined with the strict `<`
//! ΔSSE compare on near-ties this golden is a SAME-TOOLCHAIN lock. If CI on a
//! different target ever fails here, first determine whether shape bytes are
//! simply not bit-portable across libm before assuming a code regression.

mod common;
use common::{cases, encode_case, hex};

/// Golden table: (case name, expected hex). Generated on the dev machine via
/// the `emit_golden` test below. See the PLATFORM NOTE at the top of the file.
const GOLDEN: &[(&str, &str)] = &[
    ("circle12_solid", "7f26cb10a2c93220449365408826cb80104d9601219a2c03423459068468b20c08d1641910a2c93220449365408826cb80104d9601"),
    ("circle12_gradient", "7fc89ceb2fea0fff5ae478faaee8ff43881100560b22e0ae54442f18a58854abf20ff980fe1ffa3d7f39047c1aa368088cff902f09"),
    ("circle24_gradient", "91c89cff2d1a3ddb5ff4032db30838d43fd1dfb7492260f37c453d109d884c2bf90ffd7344221ac1be3c64fe9591a810fde510f0dbbfa1df539244c054f98778c0670db950eb13fa578735d4027d5c48fed3b7909ba5492140ef3e43dd95fe84fe3caa09d174"),
    ("triangle12_gradient", "9ec89ce003e83ff20bffff2002e47cfeff21fce8ff031c000e1000782d070d20e00e30c0ad452dd8b71f6080803f60e3071ffd60ed72783d9ab1f0ffff67f4ffa59f3fc5e88f02004c0a100006"),
    ("triangle12_solid", "7f4f1e000400c2930700080084270f001000084f1e002000109e3c004000203c790080004078f200000180f0e401000200e1c903000400c2930700080084270f001000084f1e002000109e3c00"),
    ("square12_gradient", "7fc89ceb37ea097d62c470f3cc28ff5cab114148e522b3aefe45be1d2d8a80ca0f17817f662e52c1cc5fc4037a8fc8fffc4591e10d"),
    ("rect12_gradient", "7fc89c1cea23d3f7f557f43b26d58d94526479d10ba6b726d2fd30624680def17de807a45415a9553e5d23e7d39e6304d4796f88fa5ffa15d15f07"),
    ("rotrect12_gradient", "8ac89cfeed7f64fa3efe1ad0cfb88ef746c0a1e6ba0fed7efa300304f09d9afd500b96febd443f58ad9a1da954c66c33a4887ea39ad0e0eb3d9e45fe2734aa100171"),
    ("pixel16_gradient", "9e0821086108a108e10823086308a308e32825286528a528e52827286728a728e7"),
    ("circle8_palette", "7f5c7fb1bff64c8a69fd0d2a5b11bfebf245bf58b004"),
    ("triangle12_rgb888", "7f9898401fc0ff3f7f32100104ff7f807fa07df0c0cfe90040dc006c00000080f8a7fdfffbfb03cd7ef08105f807da871f50800f00b48c01031c4017480b60bc032c0090ae5c9af7546e2001098004000040dcf6ebbffe498106"),
    ("circle12_search_override", "7fc89ceb2fea0fbf5ba4fd29b50800e33e91dff98922a08f5a442e9d31899ef3c015e540b32002e6fe3c74fef96fe8ff84ffd01f07"),
];

#[test]
fn shape_bytes_golden() {
    if GOLDEN.is_empty() {
        // Not yet generated — skip rather than fail, so the first commit that
        // adds this file (before running emit_golden) stays green.
        eprintln!("shape_golden: GOLDEN table empty; run `cargo test emit_golden -- --nocapture` and fill it");
        return;
    }
    for c in cases() {
        let got = hex(&encode_case(&c));
        let want = GOLDEN
            .iter()
            .find(|(n, _)| *n == c.name)
            .unwrap_or_else(|| panic!("no golden entry for case {}: actual={}", c.name, got));
        assert_eq!(
            got, want.1,
            "shape byte golden mismatch for {}\n  expected: {}\n  actual:   {}",
            c.name, want.1, got
        );
    }
}

/// Run with `cargo test emit_golden -- --nocapture` to (re)generate the
/// GOLDEN table, then paste the output into `GOLDEN` above. Intentionally
/// always passes; it only prints.
#[test]
fn emit_golden() {
    println!("\n----- BEGIN GOLDEN -----");
    for c in cases() {
        let got = hex(&encode_case(&c));
        println!("    (\"{}\", \"{}\"),", c.name, got);
    }
    println!("----- END GOLDEN -----\n");
}
