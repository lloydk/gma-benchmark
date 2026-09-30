use super::*;
use crate::rgb_spaces::{DisplayP3, Rec2020, Srgb};
use crate::test_oracle::BoundaryOracle;

fn rays<G: RgbGamut>() -> Vec<[Float; 2]> {
    let oracle = BoundaryOracle::new(G::ID);
    let mut rays = Vec::new();
    for l in [
        0.000001,
        0.001,
        0.01,
        0.1,
        0.3,
        0.5,
        0.7,
        0.9,
        0.99,
        0.9999,
        1.0 - Float::EPSILON,
        (1.0 as Float).next_down(),
        1.0 - 4.0 * Float::EPSILON,
        1.0 - 16.0 * Float::EPSILON,
    ] {
        for hi in 0..1440 {
            rays.push([l, hi as Float / 4.0]);
        }
    }
    // Fill the previously untested interval between 0.9999 and 1-2^-27.
    for n in 14..=28 {
        let l = 1.0 - (2.0 as Float).powi(-n);
        if l < 1.0 {
            for hi in 0..1440 {
                rays.push([l, hi as Float / 4.0]);
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
        let (l, h) = (l as Float, h.rem_euclid(360.0) as Float);
        for l in [
            l.next_down(),
            l,
            l.next_up(),
            l * 0.5,
            l * 0.99,
            (l + 1.0) * 0.5,
        ] {
            for dh in [
                -0.01, -0.001, -0.0001, -0.00001, -0.000001, 0.0, 0.000001, 0.00001, 0.0001, 0.001,
                0.01,
            ] {
                rays.push([l, h + dh]);
            }
            for i in -32..=32 {
                rays.push([l, Float::from_bits(h.to_bits().wrapping_add_signed(i))]);
            }
        }
    }
    let mut rand = crate::mulberry32(0x6d733232);
    for _ in 0..8192 {
        rays.push([
            (0.0001 + 0.9998 * rand()) as Float,
            (360.0 * rand()) as Float,
        ]);
    }
    rays
}

fn exact_boundaries<G: RgbGamut>() -> [f64; 3] {
    let oracle = BoundaryOracle::new(G::ID);
    let mut errors = [0.0f64; 3];
    let mut worst = [[0.0; 4]; 3];
    for [l, h] in rays::<G>() {
        let expected = [
            oracle.boundary(f64::from(l), f64::from(h)),
            oracle.iterative_boundary(f64::from(l), f64::from(h), false),
            oracle.iterative_boundary(f64::from(l), f64::from(h), true),
        ];
        let angle = hue_radians(h);
        let (cos, sin) = (angle.cos(), angle.sin());
        let q = [
            KA0 * cos + KB0 * sin,
            KA1 * cos + KB1 * sin,
            KA2 * cos + KB2 * sin,
        ];
        let actual = [
            max_chroma_cubic_direct::<G>(l, q[0], q[1], q[2]),
            solve_halley::<G>(l, h, q[0], q[1], q[2]),
            solve_ostrowski::<G>(l, h, q[0], q[1], q[2]),
        ];
        let alternatives = oracle.fold_boundaries(
            f64::from(l),
            f64::from(h),
            if SINGLE { 2e-6 } else { 2e-14 },
            true,
        );
        for i in 0..3 {
            // Iterative stopping near black is assessed in final RGB/DeltaE,
            // not relative chroma. The direct cubic retains its raw-C check.
            if i > 0 && l < 0.001 {
                continue;
            }
            let mut selected = expected[i];
            let mut error = (f64::from(actual[i]) - selected).abs();
            if i > 0 {
                for &edge in &alternatives {
                    let other = (f64::from(actual[i]) - edge).abs();
                    if other < error {
                        error = other;
                        selected = edge;
                    }
                }
            }
            if !actual[i].is_finite() {
                errors[i] = f64::INFINITY;
                worst[i] = [f64::from(l), f64::from(h), f64::from(actual[i]), selected];
            }
            if error > errors[i] {
                errors[i] = error;
                worst[i] = [f64::from(l), f64::from(h), f64::from(actual[i]), selected];
            }
        }
    }
    eprintln!(
        "{} {}-bit exact boundaries: {errors:?}; worst [L,h,actual,expected] {worst:?}",
        G::DEFINITION.name,
        Float::MANTISSA_DIGITS
    );
    errors
}

#[test]
fn solver_boundaries_match_their_independent_policies() {
    let errors = [
        exact_boundaries::<Srgb>(),
        exact_boundaries::<DisplayP3>(),
        exact_boundaries::<Rec2020>(),
    ];
    for error in errors.into_iter().flatten() {
        assert!(
            error <= if SINGLE { 2e-5 } else { 1e-8 },
            "boundary error {error:e}"
        );
    }
}

fn mapped_outputs<G: RgbGamut + crate::validation::ValidationProfile>() {
    let oracle = BoundaryOracle::new(G::ID);
    let (mut cached, mut uncached, mut direct, mut halley, mut ostrowski, mut ray) = (
        OklchCubic::<G>::new(),
        OklchCubicNoCache::<G>::new(),
        OklchCubicDirect::<G>::new(),
        OklchHalley::<G>::new(),
        OklchOstrowski::<G>::new(),
        Raytrace::<G>::new(),
    );
    let mut samples = super::super::rgb_tests::boundary_inputs::<G>();
    for [l, h] in rays::<G>() {
        let boundary = oracle.boundary(f64::from(l), f64::from(h)) as Float;
        for c in [boundary * (1.0 - 1e-5), boundary * (1.0 + 1e-5), 0.4] {
            samples.push([l, c, h]);
        }
    }
    let grid = crate::build_grid();
    samples.extend(
        crate::build_random(grid.len())
            .into_iter()
            .chain(grid)
            .map(|input| input.map(|v| v as Float)),
    );
    let mut errors = [0.0f64; 6];
    let mut worst = [[0.0; 3]; 6];
    let mut preserved = 0;
    let mut fold_branches = [0usize; 2];
    let mut fold_differences = [0.0f64; 2];
    for input in &samples {
        let [l, c, h] = *input;
        let canonical = Oklch::from(*input).to_oklab().to_linear_rgb::<G>();
        let inside = l > 0.0 && l < 1.0 && c >= 0.0 && canonical.in_gamut();
        if inside {
            preserved += 1;
        }
        let physical = input.map(f64::from);
        let mut expected = [oracle
            .reference
            .linear_rgb([
                physical[0],
                physical[1].min(oracle.boundary(physical[0], physical[2])),
                physical[2],
            ])
            .map(|v| v.clamp(0.0, 1.0)); 6];
        let hh = h.rem_euclid(360.0);
        let bucket = ((hh * 10.0).round() as usize) as Float / 10.0;
        expected[0] = oracle
            .reference
            .linear_rgb([
                physical[0],
                physical[1].min(oracle.boundary(physical[0], f64::from(bucket))),
                f64::from(bucket),
            ])
            .map(|v| v.clamp(0.0, 1.0));
        expected[1] = expected[0];
        for i in [3, 4] {
            expected[i] = oracle
                .reference
                .linear_rgb([
                    physical[0],
                    physical[1].min(oracle.iterative_boundary(physical[0], physical[2], i == 4)),
                    physical[2],
                ])
                .map(|v| v.clamp(0.0, 1.0));
        }

        expected[5] = crate::test_oracle::raytrace(&oracle.reference, physical, SINGLE);
        let mut outputs = [[0.0; 3]; 6];
        macro_rules! check {
            ($index:expr,$mapper:ident) => {{
                let mut checked = [0.0; 3];
                $mapper.map(input, &mut outputs[$index]);
                $mapper.map_with_in_gamut_check(input, &mut checked);
                if inside {
                    assert_eq!(
                        checked.map(Float::to_bits),
                        canonical.encode_clamped().channels.map(Float::to_bits),
                        "{} {} canonical {input:?}",
                        G::DEFINITION.name,
                        $index
                    );
                } else {
                    assert_eq!(
                        checked.map(Float::to_bits),
                        outputs[$index].map(Float::to_bits),
                        "{} {} checked/plain {input:?}",
                        G::DEFINITION.name,
                        $index
                    );
                }
                assert!(outputs[$index]
                    .iter()
                    .all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0));
            }};
        }
        check!(0, cached);
        check!(1, uncached);
        check!(2, direct);
        check!(3, halley);
        check!(4, ostrowski);
        check!(5, ray);
        assert_eq!(
            outputs[0].map(Float::to_bits),
            outputs[1].map(Float::to_bits),
            "{} {input:?}",
            G::DEFINITION.name
        );
        let alternatives = oracle.fold_boundaries(
            physical[0],
            physical[2],
            if SINGLE { 2e-6 } else { 2e-14 },
            true,
        );
        for i in 0..6 {
            let mut error = (0..3)
                .map(|j| (oracle.reference.decode(f64::from(outputs[i][j])) - expected[i][j]).abs())
                .fold(0.0, f64::max);
            let original_error = error;
            if i == 3 || i == 4 {
                for &edge in &alternatives {
                    let other = oracle
                        .reference
                        .linear_rgb([physical[0], physical[1].min(edge), physical[2]])
                        .map(|v| v.clamp(0.0, 1.0));
                    let alternative_error = (0..3)
                        .map(|j| {
                            (oracle.reference.decode(f64::from(outputs[i][j])) - other[j]).abs()
                        })
                        .fold(0.0, f64::max);
                    error = error.min(alternative_error);
                }
                if original_error > 5e-5 && error <= 5e-5 {
                    fold_branches[i - 3] += 1;
                    fold_differences[i - 3] = fold_differences[i - 3].max(original_error);
                }
            }
            if error > errors[i] {
                errors[i] = error;
                worst[i] = physical;
            }
        }
    }
    eprintln!(
        "{} {}-bit mapped {} samples, {preserved} canonical: {errors:?}; worst {worst:?}",
        G::DEFINITION.name,
        Float::MANTISSA_DIGITS,
        samples.len()
    );
    eprintln!("{} {}-bit fold branch differences [Halley,Ostrowski]: {fold_branches:?}; max original linear differences {fold_differences:?}", G::DEFINITION.name, Float::MANTISSA_DIGITS);
    for (i, error) in errors.into_iter().enumerate() {
        let limit = if SINGLE {
            if i == 5 {
                G::RAYTRACE_REFERENCE_LINEAR_LIMIT
            } else {
                5e-5
            }
        } else {
            if i < 2 {
                1e-6
            } else if i >= 3 {
                // Geometric accuracy includes the incumbent 1e-9 chroma
                // stopping rule; this oracle no longer copies that iteration.
                2e-8
            } else {
                1e-8
            }
        };
        assert!(
            error <= limit,
            "{} method {i} linear error {error:e} at {:?}",
            G::DEFINITION.name,
            worst[i]
        );
    }
}

#[test]
fn all_matrix_solvers_match_independent_mapping_policies_and_preserve_canonical() {
    mapped_outputs::<Srgb>();
    mapped_outputs::<DisplayP3>();
    mapped_outputs::<Rec2020>();
}

fn extremes<G: RgbGamut>() {
    let (mut cached, mut uncached, mut direct, mut halley, mut ostrowski, mut ray) = (
        OklchCubic::<G>::new(),
        OklchCubicNoCache::<G>::new(),
        OklchCubicDirect::<G>::new(),
        OklchHalley::<G>::new(),
        OklchOstrowski::<G>::new(),
        Raytrace::<G>::new(),
    );
    for l in [
        -1.0,
        0.0,
        1e-30,
        1e-12,
        0.01,
        0.5,
        0.99,
        (1.0 as Float).next_down(),
        1.0,
        2.0,
    ] {
        for c in [0.0, 0.001, 0.4] {
            // The legacy f64 degree conversion multiplies by PI before
            // dividing; retain its finite-angle domain. f32 reduces hue first.
            let extreme = if SINGLE { Float::MAX } else { Float::MAX / 4.0 };
            for h in [-extreme, -1e21, -720.25, -0.0, 90.0, 360.0, 1e21, extreme] {
                let input = [l, c, h];
                macro_rules! check {
                    ($mapper:ident) => {{
                        let mut out = [0.0; 3];
                        $mapper.map(&input, &mut out);
                        assert!(
                            out.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
                            "{} {input:?} {out:?}",
                            G::DEFINITION.name
                        );
                        $mapper.map_with_in_gamut_check(&input, &mut out);
                        assert!(out.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
                    }};
                }
                check!(cached);
                check!(uncached);
                check!(direct);
                check!(halley);
                check!(ostrowski);
                check!(ray);
            }
        }
    }
}

#[test]
fn matrix_solvers_keep_endpoints_and_extreme_hues_finite() {
    extremes::<Srgb>();
    extremes::<DisplayP3>();
    extremes::<Rec2020>();
}

#[test]
fn direct_cubic_polish_keeps_the_limiting_face_near_a_double_root() {
    let oracle = BoundaryOracle::new(crate::rgb_spaces::SpaceId::DisplayP3);
    let mut mapper = OklchCubicDirect::<DisplayP3>::new();
    for l in [0.01, 0.1, 0.4, 0.5, 0.9] {
        for dh in [-1e-6, -1e-10, -1e-12, 0.0, 1e-12, 1e-10, 1e-6] {
            let h = 264.53667577034344 + dh;
            let input = [l, 0.5, h];
            let edge = oracle.boundary(f64::from(l), f64::from(h));
            let expected = oracle.reference.linear_to_lab(
                oracle
                    .reference
                    .linear_rgb([f64::from(l), edge, f64::from(h)])
                    .map(|v| v.clamp(0.0, 1.0)),
            );
            let mut actual = [0.0; 3];
            mapper.map(&input, &mut actual);
            let delta = crate::rgb_reference::distance(
                oracle.reference.encoded_to_lab(actual.map(f64::from)),
                expected,
            );
            assert!(
                delta < if SINGLE { 2e-6 } else { 1e-8 },
                "{input:?}: {delta}"
            );
        }
    }
}

fn stable_outer_branch<G: RgbGamut>(h: Float) {
    let oracle = BoundaryOracle::new(G::ID);
    for h in [h, h - 360.0, h + 360.0] {
        let l = 0.4;
        let angle = hue_radians(h);
        let (cos, sin) = (angle.cos(), angle.sin());
        let q = [
            KA0 * cos + KB0 * sin,
            KA1 * cos + KB1 * sin,
            KA2 * cos + KB2 * sin,
        ];
        let first = oracle.boundary(f64::from(l), f64::from(h));
        let outer = oracle
            .fold_boundaries(f64::from(l), f64::from(h), 2e-14, false)
            .into_iter()
            .fold(first, f64::max);
        assert!(
            outer > first + 0.005,
            "test must contain a disconnected island"
        );
        for actual in [
            solve_halley::<G>(l, h, q[0], q[1], q[2]),
            solve_ostrowski::<G>(l, h, q[0], q[1], q[2]),
        ] {
            assert!(
                (f64::from(actual) - outer).abs() < if SINGLE { 2e-5 } else { 1e-8 },
                "{} {h}: {actual} vs outer {outer}, inner {first}",
                G::DEFINITION.name
            );
        }
    }
}

#[test]
fn iterative_solvers_retain_the_vivid_branch_in_stable_blue_folds() {
    stable_outer_branch::<Srgb>(264.13);
    stable_outer_branch::<Rec2020>(245.2);
}

#[test]
fn near_white_cubics_match_geometric_boundary() {
    fn check<G: RgbGamut>() {
        let oracle = BoundaryOracle::new(G::ID);
        let mut cached = OklchCubic::<G>::new();
        let mut uncached = OklchCubicNoCache::<G>::new();
        let mut direct = OklchCubicDirect::<G>::new();
        for n in 14..=Float::MANTISSA_DIGITS {
            let l = 1.0 - (2.0 as Float).powi(-(n as i32));
            for h in [18.5, 30.0, 119.0, 150.0, 264.1, 266.0, 301.75] {
                let input = [l, 0.4, h];
                let bucket = (h * 10.0).round() / 10.0;
                for checked in [false, true] {
                    let mut outputs = [[0.0; 3]; 3];
                    if checked {
                        cached.map_with_in_gamut_check(&input, &mut outputs[0]);
                        uncached.map_with_in_gamut_check(&input, &mut outputs[1]);
                        direct.map_with_in_gamut_check(&input, &mut outputs[2]);
                    } else {
                        cached.map(&input, &mut outputs[0]);
                        uncached.map(&input, &mut outputs[1]);
                        direct.map(&input, &mut outputs[2]);
                    }
                    for (i, output) in outputs.into_iter().enumerate() {
                        let hh = f64::from(if i == 2 { h } else { bucket });
                        let expected = oracle.reference.linear_rgb([
                            f64::from(l),
                            oracle.boundary(f64::from(l), hh),
                            hh,
                        ]);
                        for (actual, expected) in output.into_iter().zip(expected) {
                            let error = (oracle.reference.decode(f64::from(actual))
                                - expected.clamp(0.0, 1.0))
                            .abs();
                            assert!(
                                error <= if SINGLE { 3e-6 } else { 2e-12 },
                                "{} method {i} {input:?}: {error:e}",
                                G::DEFINITION.name
                            );
                        }
                    }
                    assert_eq!(outputs[0], outputs[1]);
                }
            }
        }
    }
    check::<Srgb>();
    check::<DisplayP3>();
    check::<Rec2020>();
}

#[test]
fn fold_sweeps_require_feasible_geometric_intersections() {
    fn check<G: RgbGamut>() {
        let oracle = BoundaryOracle::new(G::ID);
        let [lo, hi] = crate::rgb_spaces::blue_fold_window(G::ID).unwrap();
        for li in 0..=90 {
            let l = if li == 0 {
                0.414
            } else {
                0.05 + li as Float * 0.01
            };
            for i in 0..=600 {
                let h = if i == 0 {
                    if G::ID == crate::rgb_spaces::SpaceId::Srgb {
                        264.0425
                    } else {
                        245.2
                    }
                } else {
                    lo as Float + (hi - lo) as Float * i as Float / 600.0
                };
                let angle = hue_radians(h);
                let q = [
                    KA0 * angle.cos() + KB0 * angle.sin(),
                    KA1 * angle.cos() + KB1 * angle.sin(),
                    KA2 * angle.cos() + KB2 * angle.sin(),
                ];
                let expected = oracle.iterative_boundary(f64::from(l), f64::from(h), false);
                let alternatives = oracle.fold_boundaries(
                    f64::from(l),
                    f64::from(h),
                    if SINGLE { 2e-6 } else { 2e-14 },
                    true,
                );
                for actual in [
                    solve_halley::<G>(l, h, q[0], q[1], q[2]),
                    solve_ostrowski::<G>(l, h, q[0], q[1], q[2]),
                ] {
                    let error = alternatives
                        .iter()
                        .fold((f64::from(actual) - expected).abs(), |e, &v| {
                            e.min((f64::from(actual) - v).abs())
                        });
                    assert!(
                        error <= if SINGLE { 2e-5 } else { 1e-8 },
                        "{} {l} {h}: {actual}, expected {expected}, error {error:e}",
                        G::DEFINITION.name
                    );
                    let rgb = oracle.reference.linear_rgb([
                        f64::from(l),
                        f64::from(actual),
                        f64::from(h),
                    ]);
                    let slack = if SINGLE { 2e-6 } else { 2e-12 };
                    assert!(
                        rgb.iter().all(|v| *v >= -slack && *v <= 1.0 + slack),
                        "{} {l} {h} {actual}: {rgb:?}",
                        G::DEFINITION.name
                    );
                }
            }
        }
    }
    check::<Srgb>();
    check::<Rec2020>();
}

#[test]
fn achromatic_raytrace_uses_the_canonical_conversion() {
    fn check<G: RgbGamut>() {
        let mut mapper = Raytrace::<G>::new();
        for l in [0.001, 0.1, 0.5, 0.9, (1.0 as Float).next_down()] {
            for h in [-360.0, 0.0, 18.5, 264.1, 720.0] {
                let expected = Oklch::from([l, 0.0, h])
                    .to_oklab()
                    .to_linear_rgb::<G>()
                    .encode_clamped()
                    .channels
                    .map(Float::to_bits);
                for c in [-0.4, -0.0, 0.0] {
                    let mut output = [0.0; 3];
                    mapper.map(&[l, c, h], &mut output);
                    assert_eq!(output.map(Float::to_bits), expected);
                    mapper.map_with_in_gamut_check(&[l, c, h], &mut output);
                    assert_eq!(output.map(Float::to_bits), expected);
                }
            }
        }
    }
    check::<Srgb>();
    check::<DisplayP3>();
    check::<Rec2020>();
}

#[test]
fn a_face_contact_exits_only_in_the_outward_direction() {
    assert_eq!(face_exit(0.0, 1.0, -1.0, true, 2.0), 1.0); // upper: inward then outward
    assert_eq!(face_exit(0.0, -1.0, 1.0, false, 2.0), 1.0); // lower: inward then outward
    assert_eq!(face_exit(0.0, 1.0, 1.0, true, 2.0), 0.0);
    assert_eq!(face_exit(0.0, -1.0, -1.0, false, 2.0), 0.0);
    assert!(face_exit(0.0, 0.0, 0.0, true, 2.0).is_infinite());
}

#[test]
fn raytrace_and_reference_retain_a_converged_near_white_hit() {
    let input = [
        0.99993896484375 as Float,
        0.0002425732985673837 as Float,
        98.75,
    ];
    let oracle = BoundaryOracle::new(crate::rgb_spaces::SpaceId::Rec2020);
    let mut out = [0.0; 3];
    Raytrace::<Rec2020>::new().map(&input, &mut out);
    let expected = crate::test_oracle::raytrace(&oracle.reference, input.map(f64::from), SINGLE);
    // A reference-only roundoff failure used to turn this almost-white point
    // yellow. Check the physical outcome independently of either iteration.
    for v in out
        .map(|v| oracle.reference.decode(f64::from(v)))
        .into_iter()
        .chain(expected)
    {
        assert!(
            v >= 0.998 && v <= 1.0,
            "{input:?}: {out:?}, reference {expected:?}"
        );
    }
}

#[test]
fn zero_root_face_direction_is_explicit() {
    // t*(t-1) immediately leaves the lower face, but moves inward from the
    // upper face. A zero lower bound alone cannot tell those cases apart.
    assert_eq!(first_face_root(0.0, 1.0, -1.0, 0.0, 2.0, false), 0.0);
    assert_eq!(first_face_root(0.0, 1.0, -1.0, 0.0, 2.0, true), 1.0);
    assert_eq!(first_face_root(0.0, -1.0, 1.0, 0.0, 2.0, true), 0.0);
    assert_eq!(first_face_root(0.0, -1.0, 1.0, 0.0, 2.0, false), 1.0);
}
