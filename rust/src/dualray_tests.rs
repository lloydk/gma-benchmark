use super::*;
use crate::test_oracle::{boundary, encoded};

fn verify_direction(l: Float, h: Float, max: &mut f64) {
    // The oracle builds physical-chroma cubics from LMS matrices, independently
    // of Dualray's normalized basis, sector classification, and seed fits.
    let edge = boundary(f64::from(l), f64::from(h));
    let c = edge as Float;
    let mut solver = Dualray::new();
    for chroma in [c * 0.5, c.next_down(), c, c.next_up(), 0.4, 0.6] {
        let input = [l, chroma, h];
        let expected = encoded(f64::from(l), f64::from(chroma).min(edge), f64::from(h));
        let (mut actual, mut checked) = ([0.0; 3], [0.0; 3]);
        solver.map(&input, &mut actual);
        solver.map_with_in_gamut_check(&input, &mut checked);
        assert_eq!(actual, checked, "{input:?}");
        for i in 0..3 {
            assert!(
                actual[i].is_finite() && (0.0..=1.0).contains(&actual[i]),
                "{input:?}: {actual:?}"
            );
            let error = (f64::from(actual[i]) - expected[i]).abs();
            *max = max.max(error);
            let tolerance = if SINGLE { 1e-4 } else { 1e-8 };
            assert!(
                error <= tolerance,
                "{input:?}: {actual:?}, expected {expected:?}, error {error:e}"
            );
        }
    }
}

#[test]
fn boundary_neighbours_and_mixed_chroma_match_independent_oracle() {
    let mut max = 0.0;
    for li in 1..100 {
        let l = li as Float / 100.0;
        for hi in 0..720 {
            verify_direction(l, hi as Float / 2.0, &mut max);
        }
    }
    for l in [0.001, 0.9999, (1.0 as Float).next_down()] {
        for hi in 0..720 {
            verify_direction(l, hi as Float / 2.0, &mut max);
        }
    }
    eprintln!(
        "dualray {}-bit oracle max channel error: {max:e}",
        Float::MANTISSA_DIGITS
    );
}

#[test]
fn upper_face_handoffs_and_gate_neighbours_match_oracle() {
    let mut max = 0.0;
    for (min_l, min_h, max_h) in [
        (700, 32800, 33300),
        (850, 19200, 19700),
        (950, 10600, 11200),
    ] {
        for li in (min_l..1000).step_by(5) {
            for hi in (min_h..=max_h).step_by(2) {
                verify_direction(li as Float / 1000.0, hi as Float / 100.0, &mut max);
            }
        }
    }
    for hi in 0..720 {
        let h = hi as Float / 2.0;
        let angle = h * (PI / 180.0);
        let (a, b) = (angle.cos(), angle.sin());
        let slope = (R_A1 * a + R_A0 * b)
            .max(G_A1 * a + G_A0 * b)
            .max(B_A1 * a + B_A0 * b);
        let gate = 1.0 / (1.0 + 0.15 * slope).cbrt();
        for l in [gate.next_down(), gate, gate.next_up()] {
            verify_direction(l, h, &mut max);
        }
    }
    verify_direction(0.25669940977808137, -122.0080463960767, &mut max);
    eprintln!(
        "dualray {}-bit handoff oracle max channel error: {max:e}",
        Float::MANTISSA_DIGITS
    );
}

#[test]
fn first_root_recovery_keeps_the_first_crossing() {
    // (x - 0.25)(x - 0.5)(x - 1): endpoints at 0 and 0.75 have the
    // same sign, so a single endpoint bracket would miss both early roots.
    // Polynomial evaluation can round to zero a few ulps from the root.
    assert!((first_root(1.0, -1.75, 0.875, -0.125, 0.75) - 0.25).abs() <= Float::EPSILON);
    assert!((first_root(0.0, 1.0, -0.75, 0.125, 1.0) - 0.25).abs() <= Float::EPSILON);
}

#[test]
fn endpoints_gray_and_extreme_hues_follow_boundary_policy() {
    let mut solver = Dualray::new();
    let (mut out, mut expected) = ([0.0; 3], [0.0; 3]);
    for l in [-0.1, 0.0, 1.0, 1.1] {
        solver.map(&[l, 0.4, 90.0], &mut out);
        assert_eq!(out, [if l <= 0.0 { 0.0 } else { 1.0 }; 3]);
    }
    for c in [-0.1, 0.0] {
        solver.map(&[0.5, c, 90.0], &mut out);
        assert_eq!(out, [clamped_gamma(0.125); 3]);
    }
    for h in [
        -Float::MAX,
        -1e21,
        -1e9,
        -HUE_FAST_LIMIT,
        HUE_FAST_LIMIT,
        1e9,
        1e21,
        Float::MAX,
    ] {
        for l in [0.01, 0.5, 0.99] {
            for c in [0.001, 0.1, 0.4] {
                solver.map(&[l, c, h], &mut out);
                solver.map(&[l, c, h % 360.0], &mut expected);
                assert_eq!(out, expected, "{l}, {c}, {h}");
                assert!(out.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
            }
        }
    }
}
