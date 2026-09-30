use super::*;
use crate::rgb_reference::{distance, lab, lch, Reference};
use crate::test_oracle::BoundaryOracle;

// Derive the sectors independently from RGB primary colors via XYZ/Oklab.
fn primary_hues(reference: &Reference) -> [f64; 3] {
    std::array::from_fn(|i| {
        let mut rgb = [0.0; 3];
        rgb[i] = 1.0;
        lch(reference.linear_to_lab(rgb))[2].rem_euclid(360.0)
    })
}
fn face(h: f64, hues: [f64; 3]) -> usize {
    let h = h.rem_euclid(360.0);
    if h >= hues[1] && h < hues[2] {
        0
    } else if h < hues[0] || h >= hues[2] {
        1
    } else {
        2
    }
}

// Empirical regression envelopes for the explicit corpora below. The P3 fit
// is the incumbent fit, whose blue-sector error is larger than the new fits.
trait Limits: BottossonData {
    const SATURATION: f64;
    const CUSP: f64;
    const FIRST_EXIT: f64;
    const CLIPPING: f64;
}
impl Limits for Srgb {
    const SATURATION: f64 = 0.00041;
    const CUSP: f64 = 0.00024;
    const FIRST_EXIT: f64 = 0.049;
    const CLIPPING: f64 = 0.001;
}
impl Limits for DisplayP3 {
    const SATURATION: f64 = 0.029;
    const CUSP: f64 = 0.01;
    const FIRST_EXIT: f64 = 0.013;
    const CLIPPING: f64 = 0.00009;
}
impl Limits for Rec2020 {
    const SATURATION: f64 = 0.00019;
    const CUSP: f64 = 0.00015;
    const FIRST_EXIT: f64 = 0.054;
    const CLIPPING: f64 = 0.0018;
}

fn cusp_accuracy<G: Limits>() {
    let oracle = BoundaryOracle::new(G::ID);
    let hues = primary_hues(&oracle.reference);
    for i in 0..3 {
        assert!((hues[i] - G::PRIMARY_HUES[i]).abs() < 1e-10);
    }
    let mut samples: Vec<Float> = (0..36000).map(|i| (i as Float + 0.37) / 100.0).collect();
    for hue in hues {
        let h = hue as Float;
        let (mut lo, mut hi) = (h, h);
        for _ in 0..96 {
            for offset in [-720.0, -360.0, 0.0, 360.0, 720.0] {
                samples.extend([lo + offset, hi + offset]);
            }
            lo = lo.next_down();
            hi = hi.next_up();
        }
    }
    let (mut max_s, mut max_cusp) = (0.0f64, 0.0f64);
    for h in &samples {
        let hr = hue_radians(*h);
        let (a, b) = (hr.cos(), hr.sin());
        let channel = face(f64::from(*h), hues);
        // f64 points within roundoff of a primary have no stable side; skip
        // only this sector assertion there, not the ordinary fit samples.
        let ambiguous = hues
            .iter()
            .any(|x| (f64::from(*h).rem_euclid(360.0) - x).abs() < 1e-10);
        if ambiguous && !SINGLE {
            continue;
        }
        assert_eq!(sector::<G>(a, b, *h), channel, "{} {h}", G::DEFINITION.name);
        let expected = oracle.face_saturation(f64::from(*h), channel);
        let cusp = find_cusp::<G>(a, b, *h).map(f64::from);
        let actual = cusp[1] / cusp[0];
        assert!(actual.is_finite() && expected.is_finite());
        max_s = max_s.max((actual - expected).abs());
        let rgb = oracle.reference.linear_rgb([1.0, expected, f64::from(*h)]);
        let l = rgb.into_iter().fold(0.0, f64::max).cbrt().recip();
        max_cusp = max_cusp
            .max((cusp[0] - l).abs())
            .max((cusp[1] - l * expected).abs());
    }
    eprintln!(
        "{} {}-bit Bottosson cusp: {} samples, saturation {max_s:e}, cusp LC {max_cusp:e}",
        G::DEFINITION.name,
        Float::MANTISSA_DIGITS,
        samples.len()
    );
    assert!(max_s <= G::SATURATION && max_cusp <= G::CUSP);
}

#[test]
fn generated_sectors_and_cusps_match_independent_geometry() {
    cusp_accuracy::<Srgb>();
    cusp_accuracy::<DisplayP3>();
    cusp_accuracy::<Rec2020>();
}

// Independent policy evaluation: expand cubics using XYZ-derived matrix rows,
// then differentiate the polynomial, rather than differentiating LMS cubes as
// the production mapper does. This checks arithmetic, not approximation quality.
fn policy<G: crate::float64::bottosson::BottossonData>(
    reference: &Reference,
    l: f64,
    h: f64,
    channel: usize,
) -> f64 {
    let angle = h.to_radians();
    let (a, b) = (angle.cos(), angle.sin());
    let [k0, k1, k2, k3, k4] = G::SATURATION_FITS[channel];
    let seed = k0 + k1 * a + k2 * b + k3 * a * a + k4 * a * b;
    // Recover the cubic coefficients from values at C=0,+1,-1,+2, via an
    // independent conversion path. No production matrix or derivative helper.
    let polynomial = |l: f64, channel: usize, face: f64| {
        let v = [0.0, 1.0, -1.0, 2.0].map(|c| reference.linear_rgb([l, c, h])[channel]);
        let d = v[0] - face;
        let b = (v[1] + v[2]) / 2.0 - v[0];
        let odd = (v[1] - v[2]) / 2.0;
        let a = (v[3] - v[0] - 4.0 * b - 2.0 * odd) / 6.0;
        [a, b, odd - a, d]
    };
    let correction = |p: [f64; 4], x: f64| {
        let [a, b, c, d] = p;
        let f = ((a * x + b) * x + c) * x + d;
        let f1 = (3.0 * a * x + 2.0 * b) * x + c;
        let f2 = 6.0 * a * x + 2.0 * b;
        let u = f1 / (f1 * f1 - 0.5 * f * f2);
        (u, -f * u)
    };
    let saturation = seed + correction(polynomial(1.0, channel, 0.0), seed).1;
    let rgb = reference.linear_rgb([1.0, saturation, h]);
    let cusp_l = rgb.into_iter().fold(0.0, f64::max).cbrt().recip();
    if l <= cusp_l {
        return l * saturation;
    }
    let c = cusp_l * saturation * (1.0 - l) / (1.0 - cusp_l);
    let step = (0..3)
        .map(|i| correction(polynomial(l, i, 1.0), c))
        .filter(|(u, _)| *u >= 0.0)
        .map(|(_, step)| step)
        .fold(f64::INFINITY, f64::min);
    c + step
}

fn mapping<G: BottossonData + crate::float64::bottosson::BottossonData>() {
    let reference = Reference::new(G::ID);
    let mut samples = super::super::rgb_tests::boundary_inputs::<G>();
    samples.extend(
        crate::build_grid()
            .into_iter()
            .chain(crate::build_random(35640))
            .map(|x| x.map(|x| x as Float)),
    );
    for h in [0.0, 30.0123, 96.03, 150.05, 264.05202, 341.1062, 359.95] {
        for offset in [-720.0, -360.0, 0.0, 360.0, 720.0] {
            for l in [
                -1.0, 0.0, 1e-9, 0.17938176, 0.43879423, 0.5, 0.9999, 1.0, 2.0,
            ] {
                for c in [-0.01, 0.0, 1e-14, 1e-11, 3.8166054e-5, 0.2, 0.4] {
                    samples.push([l, c, h + offset]);
                }
            }
        }
    }
    for h in <G as BottossonData>::PRIMARY_HUES {
        let h = h as Float;
        for h in [h.next_down(), h, h.next_up(), h - 360.0] {
            for l in [0.17938176, 0.414, 0.5, 0.8] {
                samples.push([l, 0.5, h]);
            }
        }
    }
    for n in 1..=52 {
        let near_black = (2.0f64.powi(-n)) as Float;
        let near_white = (1.0 - 2.0f64.powi(-n)) as Float;
        for h in [30.0, 150.0, 245.067, 264.052, 301.75] {
            for l in [near_black, near_white] {
                samples.push([l, 0.4, h]);
            }
        }
    }
    let (mut direct, mut cached) = (
        BottossonLightness::<G>::new(),
        BottossonLightnessCached::<G>::new(),
    );
    let (mut max_linear, mut max_delta) = (0.0f64, 0.0f64);
    let mut accepted = 0;
    let mut sector_contacts = 0;
    let hues = primary_hues(&reference);
    for input in &samples {
        let canonical = Oklch::from(*input).to_oklab().to_linear_rgb::<G>();
        for checked in [false, true] {
            for cache in [false, true] {
                let mut out = [0.0; 3];
                match (cache, checked) {
                    (false, false) => direct.map(input, &mut out),
                    (false, true) => direct.map_with_in_gamut_check(input, &mut out),
                    (true, false) => cached.map(input, &mut out),
                    (true, true) => cached.map_with_in_gamut_check(input, &mut out),
                }
                assert!(
                    out.iter().all(|x| x.is_finite() && (0.0..=1.0).contains(x)),
                    "{input:?}: {out:?}"
                );
                if checked && canonical.in_gamut() {
                    assert_eq!(
                        out.map(Float::to_bits),
                        canonical.encode_clamped().channels.map(Float::to_bits),
                        "canonical {input:?}"
                    );
                    accepted += 1;
                    continue;
                }
                let [l, c, mut h] = input.map(f64::from);
                if cache {
                    let mut hh = input[2] % 360.0;
                    if hh < 0.0 {
                        hh += 360.0;
                    }
                    h = f64::from((hh * 10.0).round() / 10.0);
                }
                let actual = out.map(|x| reference.decode(f64::from(x)));
                let measure = |channel| {
                    let expected = if c <= 1e-12 || l <= 0.0 || l >= 1.0 {
                        [l.clamp(0.0, 1.0).powi(3); 3]
                    } else {
                        reference
                            .linear_rgb([l, policy::<G>(&reference, l, h, channel), h])
                            .map(|x| x.clamp(0.0, 1.0))
                    };
                    let error = (0..3)
                        .map(|i| (actual[i] - expected[i]).abs())
                        .fold(0.0, f64::max);
                    let delta = distance(
                        reference.linear_to_lab(actual),
                        reference.linear_to_lab(expected),
                    );
                    (error, delta)
                };
                let (mut error, mut delta) = measure(face(h, hues));
                // Independently derived matrices can place an exact primary
                // on either side by a few binary64 ULPs. Require one of the
                // two adjacent face policies, with the ordinary error limits.
                if !SINGLE {
                    for (i, hue) in hues.iter().enumerate() {
                        if (h.rem_euclid(360.0) - hue).abs() <= 1e-10 {
                            sector_contacts += 1;
                            for channel in [[1, 2], [2, 0], [0, 1]][i] {
                                let candidate = measure(channel);
                                if candidate.0 < error {
                                    (error, delta) = candidate;
                                }
                            }
                        }
                    }
                }
                max_linear = max_linear.max(error);
                max_delta = max_delta.max(delta);
                let (linear_limit, delta_limit) = if SINGLE {
                    (1.2e-5, 5e-6)
                } else {
                    (2e-11, 2e-11)
                };
                assert!(error <= linear_limit && delta <= delta_limit, "{} {input:?}, checked {checked}, cached {cache}: linear {error:e}, delta {delta:e}",G::DEFINITION.name);
            }
        }
    }
    assert!(accepted > 1000);
    eprintln!("{} {}-bit Bottosson policy: {} inputs, {accepted} canonical, {sector_contacts} sector contacts, linear {max_linear:e}, delta {max_delta:e}",G::DEFINITION.name,Float::MANTISSA_DIGITS,samples.len());
}

#[test]
fn all_gamut_mapping_matches_independent_policy_and_preserves_canonical() {
    mapping::<Srgb>();
    mapping::<DisplayP3>();
    mapping::<Rec2020>();
}

fn quality<G: Limits>() {
    let oracle = BoundaryOracle::new(G::ID);
    let mut samples = crate::build_grid();
    samples.extend(crate::build_random(35640));
    for n in 0..3600 {
        let h = (n as f64 + 0.37) / 10.0;
        let hf = h as Float;
        let rad = hue_radians(hf);
        let [l, _] = find_cusp::<G>(rad.cos(), rad.sin(), hf).map(f64::from);
        for scale in [0.001, 0.1, 0.5, 0.99, 1.0] {
            samples.push([l * scale, 0.5, h]);
        }
        for scale in [0.01, 0.5, 0.99, 0.99999] {
            samples.push([l + (1.0 - l) * scale, 0.5, h]);
        }
    }
    if let Some([lo, hi]) = crate::rgb_spaces::blue_fold_window(G::ID) {
        for n in 0..=1000 {
            for li in 1..100 {
                samples.push([
                    li as f64 / 100.0,
                    0.5,
                    f64::from(lo) + f64::from(hi - lo) * n as f64 / 1000.0,
                ]);
            }
        }
    }
    let (mut max_c, mut max_delta, mut max_clip) = (0.0f64, 0.0f64, 0.0f64);
    let mut worst = [0.0; 3];
    for input in &samples {
        let [l, c, h] = input.map(|x| x as Float);
        let hr = hue_radians(h);
        let (a, b) = (hr.cos(), hr.sin());
        let cusp = find_cusp::<G>(a, b, h);
        let mapped_c = f64::from(find_gamut_intersection::<G>(a, b, l, c, l, cusp) * c);
        let (l, h) = (f64::from(l), f64::from(h));
        let exact = oracle.boundary(l, h);
        let unclipped = lab([l, mapped_c, h]);
        let mapped = oracle.reference.linear_to_lab(
            oracle
                .reference
                .linear_rgb([l, mapped_c, h])
                .map(|x| x.clamp(0.0, 1.0)),
        );
        let delta = distance(mapped, lab([l, exact, h]));
        assert!(mapped_c.is_finite() && delta.is_finite());
        if delta > max_delta {
            max_delta = delta;
            worst = [l, mapped_c, h];
        }
        max_c = max_c.max((mapped_c - exact).abs());
        max_clip = max_clip.max(distance(mapped, unclipped));
    }
    eprintln!("{} {}-bit Bottosson approximation: {} samples, first-exit chroma {max_c:e}, delta {max_delta:e}, clipping delta {max_clip:e}, worst {worst:?}",G::DEFINITION.name,Float::MANTISSA_DIGITS,samples.len());
    assert!(max_c <= G::FIRST_EXIT && max_delta <= G::FIRST_EXIT && max_clip <= G::CLIPPING);
}
#[test]
fn approximation_quality_against_geometric_first_exit() {
    quality::<Srgb>();
    quality::<DisplayP3>();
    quality::<Rec2020>();
}

fn dense_blue_primary<G: BottossonData + crate::float64::bottosson::BottossonData>() {
    let reference = Reference::new(G::ID);
    let hues = primary_hues(&reference);
    let (lo, hi) = (
        ((hues[2] - 0.1) as f32).to_bits(),
        ((hues[2] + 0.1) as f32).to_bits(),
    );
    let (mut direct, mut cached) = (
        BottossonLightness::<G>::new(),
        BottossonLightnessCached::<G>::new(),
    );
    let (mut max_linear, mut max_delta) = (0.0f64, 0.0f64);
    let mut worst = [0.0; 3];
    let mut count = 0;
    let mut samples: Vec<_> = (lo..=hi).map(f32::from_bits).collect();
    // Offset grid includes both sides of the conditioning cutover, without
    // relying on integer hues or landing on the same representable values.
    samples.extend((0..3001).map(|i| (hues[2] - 1.1 + 2.2 * (i as f64 + 0.37) / 3001.0) as f32));
    for hue in samples {
        for offset in [-360.0, 0.0, 360.0] {
            let h = hue as Float + offset;
            for l in [
                0.01, 0.17938176, 0.3, 0.414, 0.45, 0.49, 0.5, 0.6, 0.8, 0.99,
            ] {
                let input = [l, 0.4, h];
                let canonical = Oklch::from(input).to_oklab().to_linear_rgb::<G>();
                for cache in [false, true] {
                    let mut hh = h;
                    if cache {
                        hh %= 360.0;
                        if hh < 0.0 {
                            hh += 360.0;
                        }
                        hh = (hh * 10.0).round() / 10.0;
                    }
                    let hh = f64::from(hh);
                    let c = policy::<G>(&reference, f64::from(l), hh, face(hh, hues));
                    let expected = reference
                        .linear_rgb([f64::from(l), c, hh])
                        .map(|x| x.clamp(0.0, 1.0));
                    for checked in [false, true] {
                        let mut out = [0.0; 3];
                        match (cache, checked) {
                            (false, false) => direct.map(&input, &mut out),
                            (false, true) => direct.map_with_in_gamut_check(&input, &mut out),
                            (true, false) => cached.map(&input, &mut out),
                            (true, true) => cached.map_with_in_gamut_check(&input, &mut out),
                        }
                        count += 1;
                        assert!(out.iter().all(|x| x.is_finite() && (0.0..=1.0).contains(x)));
                        if checked && canonical.in_gamut() {
                            assert_eq!(
                                out.map(Float::to_bits),
                                canonical.encode_clamped().channels.map(Float::to_bits)
                            );
                            continue;
                        }
                        let actual = out.map(|x| reference.decode(f64::from(x)));
                        let error = (0..3)
                            .map(|i| (actual[i] - expected[i]).abs())
                            .fold(0.0, f64::max);
                        let delta = distance(
                            reference.linear_to_lab(actual),
                            reference.linear_to_lab(expected),
                        );
                        assert!(error.is_finite() && delta.is_finite());
                        if error > max_linear {
                            max_linear = error;
                            worst = input.map(f64::from);
                        }
                        max_delta = max_delta.max(delta);
                    }
                }
            }
        }
    }
    eprintln!("{} {}-bit dense blue: {count} mappings, linear {max_linear:e}, delta {max_delta:e}, worst {worst:?}",G::DEFINITION.name,Float::MANTISSA_DIGITS);
    let (linear_limit, delta_limit) = if SINGLE {
        (1.2e-5, 5e-6)
    } else {
        (2e-11, 2e-11)
    };
    assert!(max_linear <= linear_limit && max_delta <= delta_limit);
}
#[test]
fn every_f32_hue_near_blue_primary_meets_policy_limits() {
    dense_blue_primary::<Srgb>();
    dense_blue_primary::<DisplayP3>();
    dense_blue_primary::<Rec2020>();
}
