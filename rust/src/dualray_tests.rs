use super::*;
use crate::test_oracle::{boundary, encoded};

fn verify_direction(l: Float, h: Float, max: &mut f64) {
    // The oracle builds physical-chroma cubics from LMS matrices, independently
    // of Dualray's normalized basis, sector classification, and seed fits.
    let edge = boundary(f64::from(l), f64::from(h));
    let c = edge as Float;
    let mut solver = Dualray::<DisplayP3>::new();
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
        let slope = (<DisplayP3 as DualrayData>::BASIS[0][7] * a
            + <DisplayP3 as DualrayData>::BASIS[0][8] * b)
            .max(
                <DisplayP3 as DualrayData>::BASIS[1][7] * a
                    + <DisplayP3 as DualrayData>::BASIS[1][8] * b,
            )
            .max(
                <DisplayP3 as DualrayData>::BASIS[2][7] * a
                    + <DisplayP3 as DualrayData>::BASIS[2][8] * b,
            );
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
fn first_exit_keeps_the_first_crossing() {
    // One channel varies; the others stay inside. Rows are [d, b, a, constant].
    let inside = [0.0, 0.0, 0.0, 0.5];
    let exit = |row: [Float; 4], target: Float, limit: Float| {
        super::first_exit(&[row, inside, inside], [0.0; 3], target, limit, 0.0)
    };
    // -(x - 0.25)(x - 0.5)(x - 1): endpoints at 0 and 0.75 have the same
    // sign, so a single endpoint bracket would miss both early roots.
    let (u, beyond) = exit([-1.0, 1.75, -0.875, 0.125], 1.0, 0.75);
    assert!((u - 0.25).abs() <= Float::EPSILON && beyond.is_some());
    let (u, beyond) = exit([0.0, 1.0, -0.75, 0.125], 1.0, 1.0);
    assert!((u - 0.25).abs() <= Float::EPSILON && beyond.is_some());
    // Touching either face at x = 0.5 remains inside on both sides.
    assert_eq!(exit([0.0, 1.0, -1.0, 0.25], 1.0, 1.0), (1.0, None));
    assert_eq!(exit([0.0, -1.0, 1.0, 1.75], 2.0, 1.0), (1.0, None));
}

#[test]
fn endpoints_gray_and_extreme_hues_follow_boundary_policy() {
    let mut solver = Dualray::<DisplayP3>::new();
    let (mut out, mut expected) = ([0.0; 3], [0.0; 3]);
    for l in [-0.1, 0.0, 1.0, 1.1] {
        solver.map(&[l, 0.4, 90.0], &mut out);
        assert_eq!(out, [if l <= 0.0 { 0.0 } else { 1.0 }; 3]);
    }
    for c in [-0.1, 0.0] {
        solver.map(&[0.5, c, 90.0], &mut out);
        assert_eq!(
            out,
            [<DisplayP3 as RgbGamut>::Transfer::encode_clamped(0.125); 3]
        );
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

fn target_outputs<G: DualrayData>() -> [f64; 2] {
    use crate::rgb_reference::{distance, lab};
    use crate::test_oracle::BoundaryOracle;
    let oracle = BoundaryOracle::new(G::ID);
    let mut solver = Dualray::<G>::new();
    let mut rays = Vec::new();
    for li in 1..100 {
        for hi in 0..720 {
            rays.push([li as Float / 100.0, hi as Float / 2.0]);
        }
    }
    for n in 10..=Float::MANTISSA_DIGITS {
        let l = 1.0 - (2.0 as Float).powi(-(n as i32));
        for hi in 0..720 {
            rays.push([l, hi as Float / 2.0]);
        }
    }
    for l in [Float::from_bits(1), 1e-20, 1e-10, 0.000001, 0.001] {
        for hi in 0..720 {
            rays.push([l, hi as Float / 2.0]);
        }
    }
    if let Some([lo, hi]) = crate::dualray_config::config(G::ID).fold {
        // Retain coverage outside the narrowed isolation window as well.
        for j in 0..=3000 {
            let h = (lo.floor() - 1.0 + j as f64 / 1000.0) as Float;
            for l in [0.1, 0.414, 0.45, 0.49, 0.9, 0.99] {
                rays.push([l, h]);
            }
        }
        for j in 0..=8000 {
            let h = (lo + (hi - lo) * j as f64 / 8000.0) as Float;
            for l in [
                0.01, 0.1, 0.2, 0.3, 0.4, 0.414, 0.45, 0.49, 0.5, 0.6, 0.7, 0.9, 0.99,
            ] {
                rays.push([l, h]);
            }
        }
    }
    for rgb in [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [1.0, 0.0, 1.0],
    ] {
        let [l, _, h] = crate::rgb_reference::lch(oracle.reference.linear_to_lab(rgb));
        let h = h.rem_euclid(360.0) as Float;
        for li in -10..=10 {
            let l = (l + li as f64 * 0.0001) as Float;
            for hi in -100..=100 {
                rays.push([l, h + hi as Float * 0.00001]);
            }
        }
    }
    let mut max = [0.0f64; 2];
    let mut worst = [[0.0; 3]; 2];
    let mut count = 0;
    for [l, h] in rays {
        let edge = oracle.boundary(f64::from(l), f64::from(h));
        let c = edge as Float;
        for chroma in [c * 0.5, c.next_down().max(0.0), c, c.next_up(), 0.4, 0.6] {
            let input = [l, chroma, h];
            let expected_lch = [f64::from(l), f64::from(chroma).min(edge), f64::from(h)];
            let expected = oracle
                .reference
                .linear_rgb(expected_lch)
                .map(|v| v.clamp(0.0, 1.0));
            let mut actual = [0.0; 3];
            solver.map(&input, &mut actual);
            let mut checked = [0.0; 3];
            solver.map_with_in_gamut_check(&input, &mut checked);
            assert_eq!(actual.map(Float::to_bits), checked.map(Float::to_bits));
            assert!(
                actual
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
                "{} {input:?}: {actual:?}",
                G::DEFINITION.name
            );
            let actual = actual.map(f64::from);
            let linear = actual.map(|v| oracle.reference.decode(v));
            let errors = [
                (0..3)
                    .map(|i| (linear[i] - expected[i]).abs())
                    .fold(0.0, f64::max),
                distance(oracle.reference.encoded_to_lab(actual), lab(expected_lch)),
            ];
            for i in 0..2 {
                if errors[i] > max[i] {
                    max[i] = errors[i];
                    worst[i] = input.map(f64::from);
                }
            }
            count += 2;
        }
    }
    eprintln!(
        "{} {}-bit Dualray: {count} mappings, errors {max:?}, worst {worst:?}",
        G::DEFINITION.name,
        Float::MANTISSA_DIGITS
    );
    max
}

#[test]
fn fold_exits_snap_the_selected_face_but_interior_inputs_do_not() {
    fn check<G: DualrayData>() {
        let [lo, hi] = crate::dualray_config::config(G::ID).fold.unwrap();
        let oracle = crate::test_oracle::BoundaryOracle::new(G::ID);
        let mut solver = Dualray::<G>::new();
        let mut out = [0.0; 3];
        for j in 0..=2000 {
            let h = (lo + (hi - lo) * j as f64 / 2000.0) as Float;
            for h in [h, h - 360.0, h + 360.0] {
                for l in [0.1, 0.414, 0.45, 0.49, 0.8, 0.99] {
                    solver.map(&[l, 0.6, h], &mut out);
                    let edge = oracle.boundary(f64::from(l), f64::from(h));
                    let expected = oracle
                        .reference
                        .linear_rgb([f64::from(l), edge, f64::from(h)]);
                    // At a corner, either incident face may win after rounding.
                    // Everywhere else this requires the oracle's unique face.
                    let tolerance = if SINGLE { 2e-6 } else { 2e-11 };
                    assert!(
                        (0..3).any(|i| [0.0, 1.0]
                            .into_iter()
                            .any(|face| (expected[i] - face).abs() <= tolerance
                                && out[i] == face as Float)),
                        "wrong or unsnapped face: {} {l} {h}: {out:?}, oracle {expected:?}",
                        G::DEFINITION.name
                    );
                    for (i, actual) in out.into_iter().enumerate() {
                        if actual == 0.0 || actual == 1.0 {
                            assert!(
                                (expected[i] - f64::from(actual)).abs() <= tolerance,
                                "wrong extra face: {} {l} {h}: {out:?}, oracle {expected:?}",
                                G::DEFINITION.name
                            );
                        }
                    }
                }
                solver.map(&[0.45, 0.001, h], &mut out);
                assert!(
                    out.iter().all(|v| *v > 0.0 && *v < 1.0),
                    "interior input snapped: {h}: {out:?}"
                );
            }
        }
    }
    check::<Srgb>();
    check::<Rec2020>();
    let mut solver = Dualray::<Rec2020>::new();
    let mut out = [0.0; 3];
    let input = [0.45, 0.4, 245.1];
    assert!(
        in_blue_fold::<Rec2020>(input[2]),
        "regression must exercise fold isolation"
    );
    let oracle = crate::test_oracle::BoundaryOracle::new(crate::rgb_spaces::SpaceId::Rec2020);
    let edge = oracle.boundary(f64::from(input[0]), f64::from(input[2]));
    let expected = oracle
        .reference
        .linear_rgb([f64::from(input[0]), edge, f64::from(input[2])]);
    let face = (0..3)
        .min_by(|&i, &j| expected[i].abs().total_cmp(&expected[j].abs()))
        .unwrap();
    assert!(expected[face].abs() < 1e-12);
    solver.map(&input, &mut out);
    assert_eq!(
        out[face], 0.0,
        "in-window review reproducer: {out:?}, oracle {expected:?}"
    );
}

#[test]
fn all_targets_match_independent_first_exit() {
    let maxima = [
        target_outputs::<Srgb>(),
        target_outputs::<DisplayP3>(),
        target_outputs::<Rec2020>(),
    ];
    for [linear, delta] in maxima {
        assert!(
            linear <= if SINGLE { 2e-5 } else { 2e-11 },
            "linear {linear:e}"
        );
        assert!(
            delta <= if SINGLE { 5e-6 } else { 2e-11 },
            "DeltaEOK {delta:e}"
        );
    }
}

fn target_seed_data<G: DualrayData>() {
    let oracle = crate::test_oracle::BoundaryOracle::new(G::ID);
    let mut max_seed = 0.0f64;
    let mut max_basis = 0.0f64;
    let mut max_root = 0.0f64;
    for i in 0..36000 {
        let h = (i as Float + 0.37) / 100.0;
        let r = h * (PI / 180.0);
        let (a, b) = (r.cos(), r.sin());
        let coefficients = G::BASIS.map(|k| {
            [
                k[0] * a * a * a + k[1] * a * a * b + k[2] * a + k[3] * b,
                k[4] + k[5] * a * a + k[6] * a * b,
                k[7] * a + k[8] * b,
            ]
        });
        for c in [0.1, 0.5, 0.9] {
            let expected = oracle.reference.linear_rgb([1.0, c, f64::from(h)]);
            for j in 0..3 {
                let k = coefficients[j];
                let actual = cubic([k[0], k[1], k[2], 1.0], c as Float);
                let error = (f64::from(actual) - expected[j]).abs() / expected[j].abs().max(1.0);
                max_basis = max_basis.max(error);
            }
        }
        let saturation = oracle.boundary(0.001, f64::from(h)) / 0.001;
        assert!(saturation > 0.0 && saturation < f64::from(G::ROOT_LIMIT));
        max_root = max_root.max(saturation);
        if in_blue_fold::<G>(h) {
            continue;
        }
        let face = if a * G::SECTORS[0][0] + b * G::SECTORS[0][1] > 1.0 {
            0
        } else if a * G::SECTORS[1][0] + b * G::SECTORS[1][1] > 1.0 {
            1
        } else {
            2
        };
        let k = coefficients[face];
        let seed = G::seed(a, b, face as u8);
        let refined = polish(seed, [k[0], k[1], k[2], 1.0]);
        assert!(refined.is_finite() && refined > 0.0);
        max_seed = max_seed.max((f64::from(refined) - saturation).abs());
    }
    eprintln!("{} {}-bit Dualray data: relative basis {max_basis:e}, refined seed {max_seed:e}, lower root {max_root:e}",G::DEFINITION.name,Float::MANTISSA_DIGITS);
    assert!(max_basis <= if SINGLE { 5e-6 } else { 1e-13 });
    assert!(max_seed <= if SINGLE { 2e-5 } else { 1e-11 });
}
#[test]
fn basis_seeds_and_root_limits_match_independent_geometry() {
    target_seed_data::<Srgb>();
    target_seed_data::<DisplayP3>();
    target_seed_data::<Rec2020>();
}

#[test]
fn fold_tangent_regressions_use_native_conditioning() {
    fn verify<G: DualrayData>(h: Float, l: Float) {
        let oracle = crate::test_oracle::BoundaryOracle::new(G::ID);
        let mut solver = Dualray::<G>::new();
        for offset in -32..=32 {
            let h = Float::from_bits(h.to_bits().wrapping_add_signed(offset));
            for h in [h, h - 360.0, h + 360.0] {
                let edge = oracle.boundary(f64::from(l), f64::from(h));
                let expected = oracle
                    .reference
                    .linear_rgb([f64::from(l), edge, f64::from(h)])
                    .map(|v| v.clamp(0.0, 1.0));
                let mut out = [0.0; 3];
                solver.map(&[l, 0.4, h], &mut out);
                let actual = out.map(|v| oracle.reference.decode(f64::from(v)));
                let error = (0..3)
                    .map(|i| (actual[i] - expected[i]).abs())
                    .fold(0.0, f64::max);
                assert!(
                    error <= if SINGLE { 2e-6 } else { 2e-11 },
                    "{} {l}, {h}: {error:e}",
                    G::DEFINITION.name
                );
            }
        }
    }
    verify::<Srgb>(264.207763671875, 0.45);
    verify::<Rec2020>(245.28399658203125, 0.414);
}
#[test]
fn bernstein_controls_reject_inside_endpoints_with_outside_interiors() {
    // 1 - 6x + 6x² dips below zero; 1 + 6x - 6x² rises above two.
    // Both have endpoints equal to one on [0, 1].
    assert!(!super::interior_within([0.0, 6.0, -6.0, 1.0], 1.0, 2.0));
    assert!(!super::interior_within([0.0, -6.0, 6.0, 1.0], 1.0, 2.0));
    assert!(super::interior_within([0.0, 6.0, -6.0, 1.0], 0.01, 2.0));
    assert!(super::interior_within([0.0, 0.0, 0.0, 1.0], 1.0, 1.0));
}
