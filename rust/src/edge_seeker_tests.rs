use super::super::{p3_compat::oklch_to_clipped_p3, SINGLE};
use super::*;

// Independent bisection of the scaled circle residual. This is monotone in y
// for |k| < 1, covering all three LUTs; no production arc formula is used.
fn arc_oracle(x: f64, k: f64) -> f64 {
    let t = (2.0 - k * k).sqrt();
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..60 {
        let y = (lo + hi) / 2.0;
        if y == lo || y == hi {
            break;
        }
        let residual = k * (x * x + y * y - x - y) + t * (y - x);
        if residual < 0.0 {
            lo = y;
        } else {
            hi = y;
        }
    }
    (lo + hi) / 2.0
}

#[test]
fn arc_endpoints_and_small_curvatures_match_oracle() {
    for k in [
        -0.54, -0.2, -1e-6, -1e-12, -1e-15, 0.0, 1e-15, 1e-12, 1e-6, 0.14,
    ] {
        for x in [
            0.0,
            Float::from_bits(1),
            Float::EPSILON,
            0.1,
            0.5,
            0.9,
            (1.0 as Float).next_down(),
            1.0,
        ] {
            let actual = intersection_with_arc(x, k);
            let expected = arc_oracle(f64::from(x), f64::from(k));
            assert!(
                actual.is_finite() && (0.0..=1.0).contains(&actual),
                "{x}, {k}: {actual}"
            );
            assert!(
                (f64::from(actual) - expected).abs() <= 8.0 * f64::from(Float::EPSILON),
                "{x}, {k}: {actual}, {expected}"
            );
            if k == 0.0 {
                assert_eq!(actual.to_bits(), x.to_bits());
            }
        }
    }
    assert_eq!(
        intersection_with_arc(-0.0, 0.0).to_bits(),
        (-0.0 as Float).to_bits()
    );
}

#[test]
fn near_cusp_yellow_keeps_its_hue_in_both_mappers_and_modes() {
    let h = 96.03;
    let item = get_lut_item::<DisplayP3>(h);
    let input = [item[0].next_up(), 0.4, h];
    let x = (1.0 - input[0]) / (1.0 - item[0]);
    let expected_chroma = item[1] * arc_oracle(f64::from(x), f64::from(item[3])) as Float;
    let mut expected = [0.0; 3];
    oklch_to_clipped_p3(input[0], expected_chroma, h, &mut expected);
    assert!(expected[0] > 0.99 && expected[1] > 0.8 && expected[2] < 0.01);
    let (mut plain, mut indexed) = (
        EdgeSeeker::<DisplayP3>::new(),
        EdgeSeekerIndexed::<DisplayP3>::new(),
    );
    for checked in [false, true] {
        let (mut a, mut b) = ([0.0; 3], [0.0; 3]);
        if checked {
            plain.map_with_in_gamut_check(&input, &mut a);
            indexed.map_with_in_gamut_check(&input, &mut b);
        } else {
            plain.map(&input, &mut a);
            indexed.map(&input, &mut b);
        }
        assert_eq!(a, b);
        let tolerance = if SINGLE { 1e-5 } else { 5e-13 };
        assert!(
            (0..3).all(|i| (a[i] - expected[i]).abs() <= tolerance),
            "{input:?}: {a:?}, expected {expected:?}"
        );
    }
}

fn cusp_and_white_neighbours<G: EdgeSeekerData>() {
    let (mut plain, mut indexed) = (EdgeSeeker::<G>::new(), EdgeSeekerIndexed::<G>::new());
    let mut max_error = 0.0f64;
    for n in 0..=36000 {
        let h = n as Float / 100.0;
        let item = conditioned_item::<G>(h).unwrap_or_else(|| get_lut_item::<G>(normalized_hue(h)));
        assert!(item[3].abs() < 1.0);
        let (mut cusp, mut white) = (item[0], 1.0 as Float);
        for _ in 0..8 {
            cusp = cusp.next_up();
            white = white.next_down();
            for l in [cusp, white] {
                let x = (1.0 - l) / (1.0 - item[0]);
                let expected = f64::from(item[1]) * arc_oracle(f64::from(x), f64::from(item[3]));
                let a = plain.max_chroma(l, h);
                let b = indexed.max_chroma(l, h);
                assert!(a.is_finite() && a >= 0.0 && a <= item[1], "{l}, {h}: {a}");
                assert_eq!(a, b, "{l}, {h}");
                let error = (f64::from(a) - expected).abs();
                max_error = max_error.max(error);
                assert!(
                    error <= 8.0 * f64::from(Float::EPSILON),
                    "{l}, {h}: {a}, {expected}"
                );
            }
        }
    }
    eprintln!(
        "{} {}-bit Edge Seeker: 576016 endpoint samples, max chroma error {max_error:e}",
        G::DEFINITION.name,
        Float::MANTISSA_DIGITS
    );
}

#[test]
fn all_gamut_cusp_and_white_neighbours_match_oracle() {
    cusp_and_white_neighbours::<Srgb>();
    cusp_and_white_neighbours::<DisplayP3>();
    cusp_and_white_neighbours::<Rec2020>();
}

// A separate lookup using the original binary64 knots, linear scanning and
// circle-residual bisection. It does not use either production lookup, its
// narrow-interval correction, or its rationalized arc formula.
fn reference_chroma<G: crate::float64::edge_seeker::EdgeSeekerData>(l: f64, h: f64) -> f64 {
    if l <= 0.0 || l >= 1.0 {
        return 0.0;
    }
    let rows = G::LUT;
    let h = h.rem_euclid(360.0);
    let [a, b] = rows.windows(2).find(|pair| pair[1][2] >= h).unwrap() else {
        unreachable!()
    };
    let t = (h - a[2]) / (b[2] - a[2]);
    let [il, ic, _, k] = std::array::from_fn(|i| a[i] + t * (b[i] - a[i]));
    if l <= il {
        l / il * ic
    } else {
        ic * arc_oracle((1.0 - l) / (1.0 - il), k)
    }
}

fn mapping_policy<G: EdgeSeekerData + crate::float64::edge_seeker::EdgeSeekerData>() {
    use crate::rgb_reference::{distance, Reference};
    let reference = Reference::new(G::ID);
    let rows = <G as EdgeSeekerData>::LUT;
    assert_eq!(rows[0][2], 0.0);
    assert_eq!(rows.last().unwrap()[2], 360.0);
    assert!(rows.windows(2).all(|p| p[0][2] < p[1][2]));
    for row in rows {
        assert!(row.iter().all(|v| v.is_finite()));
        assert!(row[0] > 0.0 && row[0] < 1.0 && row[1] > 0.0 && row[3].abs() < 1.0);
    }
    let mut inputs = super::super::rgb_tests::boundary_inputs::<G>();
    inputs.extend(
        crate::build_grid()
            .into_iter()
            .map(|v| v.map(|x| x as Float)),
    );
    inputs.extend(
        crate::build_random(35640)
            .into_iter()
            .map(|v| v.map(|x| x as Float)),
    );
    // Knot neighbours, wrapped hues, low chroma, gray, and authored negative C.
    for &[l, c, h, _] in rows {
        for h in [h.next_down(), h, h.next_up()] {
            for h in [h - 720.0, h - 360.0, h, h + 360.0, h + 720.0] {
                for l in [0.0001, l.next_down(), l, l.next_up(), 0.9999] {
                    for c in [-0.01, 0.0, c.next_down(), c.next_up(), 0.5] {
                        inputs.push([l, c, h]);
                    }
                }
            }
        }
    }
    // Exercise every representable f32 hue around the repaired fold, including
    // wrapping that loses low bits when 360 is added. f64 uses the same points.
    for &(i, _, _) in <G as EdgeSeekerData>::SHARP_INTERVALS {
        let center = rows[i][2] as f32;
        for bits in center.to_bits() - 96..=center.to_bits() + 96 {
            let h = f32::from_bits(bits) as Float;
            for offset in [-360.0, 0.0, 360.0] {
                for l in [0.17938176, 0.414, rows[i][0], rows[i + 1][0], 0.8] {
                    inputs.push([l, 0.5, h + offset]);
                }
            }
        }
    }
    for l in [
        -1.0,
        0.0,
        Float::from_bits(1),
        (1.0 as Float).next_down(),
        1.0,
        2.0,
    ] {
        for h in [-1e21, -720.0, -0.0, 30.0, 360.0, 1e21] {
            inputs.push([l, 0.4, h]);
        }
    }
    let (mut binary, mut indexed) = (EdgeSeeker::<G>::new(), EdgeSeekerIndexed::<G>::new());
    let (mut max_c, mut max_linear, mut max_delta) = (0.0f64, 0.0f64, 0.0f64);
    let mut accepted = 0;
    for input in &inputs {
        let [l, c, h] = input.map(f64::from);
        let expected_c = reference_chroma::<G>(l, h);
        let actual_c = binary.max_chroma(input[0], input[2]);
        assert_eq!(
            actual_c.to_bits(),
            indexed.max_chroma(input[0], input[2]).to_bits()
        );
        let c_error = (f64::from(actual_c) - expected_c).abs();
        max_c = max_c.max(c_error);
        let c_limit = if SINGLE { 2e-6 } else { 2e-12 };
        assert!(
            c_error <= c_limit,
            "{} chroma at {input:?}: {actual_c}, {expected_c}, error {c_error:e}",
            G::DEFINITION.name
        );
        let expected = if l <= 0.0 {
            [0.0; 3]
        } else if l >= 1.0 {
            [1.0; 3]
        } else {
            reference
                .linear_rgb([l, c.min(expected_c), h])
                .map(|v| v.clamp(0.0, 1.0))
        };
        let canonical = Oklch::from(*input).to_oklab().to_linear_rgb::<G>();
        for checked in [false, true] {
            let (mut a, mut b) = ([0.0; 3], [0.0; 3]);
            if checked {
                binary.map_with_in_gamut_check(input, &mut a);
                indexed.map_with_in_gamut_check(input, &mut b);
            } else {
                binary.map(input, &mut a);
                indexed.map(input, &mut b);
            }
            assert_eq!(a.map(Float::to_bits), b.map(Float::to_bits), "{input:?}");
            assert!(a.iter().all(|x| x.is_finite() && (0.0..=1.0).contains(x)));
            if checked && canonical.in_gamut() {
                accepted += 1;
                assert_eq!(
                    a.map(Float::to_bits),
                    canonical.encode_clamped().channels.map(Float::to_bits)
                );
                continue;
            }
            // Huge hues follow each lane's incumbent trigonometry/reduction
            // policy, so their conversion is checked for finiteness only.
            if h.abs() > 1e6 {
                continue;
            }
            let actual = a.map(|v| reference.decode(f64::from(v)));
            let error = (0..3)
                .map(|i| (actual[i] - expected[i]).abs())
                .fold(0.0, f64::max);
            let delta = distance(
                reference.linear_to_lab(actual),
                reference.linear_to_lab(expected),
            );
            max_linear = max_linear.max(error);
            max_delta = max_delta.max(delta);
            let (linear_limit, delta_limit) = if SINGLE { (1e-5, 3e-6) } else { (3e-12, 3e-12) };
            assert!(
                error <= linear_limit && delta <= delta_limit,
                "{} {input:?}, checked {checked}: linear {error:e}, delta {delta:e}",
                G::DEFINITION.name
            );
        }
    }
    assert!(accepted > 10000);
    eprintln!("{} {}-bit Edge policy: {} inputs, {accepted} canonical, chroma {max_c:e}, linear {max_linear:e}, delta {max_delta:e}", G::DEFINITION.name, Float::MANTISSA_DIGITS, inputs.len());
}

#[test]
fn all_gamut_mapping_matches_independent_table_policy_and_preserves_canonical() {
    mapping_policy::<Srgb>();
    mapping_policy::<DisplayP3>();
    mapping_policy::<Rec2020>();
}

// Approximation quality is separate from arithmetic accuracy. Edge Seeker's
// LUT/arc policy is not an exact first-exit solve, especially at a blue fold.
trait ApproximationLimits: EdgeSeekerData {
    const CHROMA: f64;
    const DELTA: f64;
    const CLIPPING: f64;
}
impl ApproximationLimits for Srgb {
    const CHROMA: f64 = 0.049;
    const DELTA: f64 = 0.049;
    const CLIPPING: f64 = 0.0034;
}
impl ApproximationLimits for DisplayP3 {
    const CHROMA: f64 = 0.026;
    const DELTA: f64 = 0.025;
    const CLIPPING: f64 = 0.004;
}
impl ApproximationLimits for Rec2020 {
    const CHROMA: f64 = 0.054;
    const DELTA: f64 = 0.054;
    const CLIPPING: f64 = 0.0049;
}
fn approximation_quality<G: ApproximationLimits>() {
    use crate::rgb_reference::{distance, lab, Reference};
    let reference = Reference::new(G::ID);
    let boundary = crate::test_oracle::BoundaryOracle::new(G::ID);
    let mut mapper = EdgeSeeker::<G>::new();
    let mut samples = crate::build_grid();
    samples.extend(crate::build_random(samples.len()));
    // Fractional hue/lightness at cusps, top arcs and both repaired folds.
    for row in G::LUT {
        for step in 1..20 {
            samples.push([
                f64::from(row[0]) * step as f64 / 20.0,
                0.5,
                f64::from(row[2]),
            ]);
            samples.push([
                f64::from(row[0]) + (1.0 - f64::from(row[0])) * step as f64 / 20.0,
                0.5,
                f64::from(row[2]),
            ]);
        }
    }
    if let Some([lo, hi]) = crate::rgb_spaces::blue_fold_window(G::ID) {
        for n in 0..=1000 {
            let h = f64::from(lo) + f64::from(hi - lo) * n as f64 / 1000.0;
            for li in 1..100 {
                samples.push([li as f64 / 100.0, 0.5, h]);
            }
        }
    }
    let (mut max_c, mut max_delta, mut max_clip) = (0.0f64, 0.0f64, 0.0f64);
    let mut worst_delta = [0.0; 3];
    for sample in &samples {
        let input = sample.map(|v| v as Float);
        let [l, _, h] = input.map(f64::from);
        let c = f64::from(mapper.max_chroma(input[0], input[2]));
        let exact = boundary.boundary(l, h);
        let rgb = reference.linear_rgb([l, c, h]).map(|v| v.clamp(0.0, 1.0));
        let mapped_lab = reference.linear_to_lab(rgb);
        max_c = max_c.max((c - exact).abs());
        let delta = distance(mapped_lab, lab([l, exact, h]));
        if delta > max_delta {
            max_delta = delta;
            worst_delta = [l, c, h];
        }
        max_clip = max_clip.max(distance(mapped_lab, lab([l, c, h])));
    }
    eprintln!("{} {}-bit Edge approximation: {} samples, first-exit chroma {max_c:e}, first-exit delta {max_delta:e}, clipping delta {max_clip:e}, worst delta at {worst_delta:?}", G::DEFINITION.name, Float::MANTISSA_DIGITS, samples.len());
    // Sampled approximation envelopes, independent of the arithmetic limits.
    assert!(max_c <= G::CHROMA && max_delta <= G::DELTA && max_clip <= G::CLIPPING);
}

#[test]
fn all_gamut_approximation_quality_against_geometric_boundary() {
    approximation_quality::<Srgb>();
    approximation_quality::<DisplayP3>();
    approximation_quality::<Rec2020>();
}

#[test]
fn sharp_residuals_use_the_actual_native_table_knots() {
    fn check<G: EdgeSeekerData + crate::float64::edge_seeker::EdgeSeekerData>() {
        let wide = <G as crate::float64::edge_seeker::EdgeSeekerData>::LUT;
        let narrow = <G as EdgeSeekerData>::LUT;
        for &(i, residuals, band) in <G as EdgeSeekerData>::SHARP_INTERVALS {
            for j in 0..4 {
                let expected = (wide[i + j - 1][2] - f64::from(narrow[i + j - 1][2])) as Float;
                assert_eq!(expected.to_bits(), residuals[j].to_bits());
            }
            assert_eq!(
                band,
                [
                    (narrow[i - 1][2] + narrow[i][2]) * 0.5,
                    (narrow[i + 1][2] + narrow[i + 2][2]) * 0.5
                ]
            );
        }
    }
    check::<Srgb>();
    check::<DisplayP3>();
    check::<Rec2020>();
}
