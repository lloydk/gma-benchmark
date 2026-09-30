type BottossonLightness = bottosson::BottossonLightness<crate::rgb_spaces::DisplayP3>;
type BottossonLightnessCached = bottosson::BottossonLightnessCached<crate::rgb_spaces::DisplayP3>;
type EdgeSeeker = edge_seeker::EdgeSeeker<gamut::DisplayP3>;
type EdgeSeekerIndexed = edge_seeker::EdgeSeekerIndexed<gamut::DisplayP3>;
type OklchCubic = rgb_solvers::OklchCubic<gamut::DisplayP3>;
type OklchCubicNoCache = rgb_solvers::OklchCubicNoCache<gamut::DisplayP3>;
type OklchCubicDirect = rgb_solvers::OklchCubicDirect<gamut::DisplayP3>;
type OklchHalley = rgb_solvers::OklchHalley<gamut::DisplayP3>;
type OklchOstrowski = rgb_solvers::OklchOstrowski<gamut::DisplayP3>;
type Raytrace = rgb_solvers::Raytrace<gamut::DisplayP3>;
use super::*;
type CssMinde = css_minde::CssMinde<gamut::DisplayP3>;
type Clip = clip::Clip<gamut::DisplayP3>;

use crate::test_oracle::{boundary, encoded};

#[test]
fn rec2020_minde_jnd_neighbour_has_bounded_perceptual_error() {
    use crate::rgb_reference::{distance, lab, Reference};
    let reference = Reference::new(crate::rgb_spaces::SpaceId::Rec2020);
    let input = [0.98f32, 0.4, 104.0];
    // The third native-f32 midpoint is exactly 0.25. The independent real-
    // valued conversion puts its clipping error within 1e-6 of the JND.
    let candidate = [input[0], 0.25, input[2]];
    let candidate64 = candidate.map(f64::from);
    let rgb = reference
        .linear_rgb(candidate64)
        .map(|x| reference.encode(x.clamp(0.0, 1.0)));
    let error = distance(reference.encoded_to_lab(rgb), lab(candidate64));
    assert!((error - 0.02).abs() < 1e-6, "{error:e}");
    // Either stopping decision is valid at native precision. Do not require
    // a particular libm's rounding or a nonzero disagreement with f64.
    let mut actual = [0.0; 3];
    css_minde::CssMinde::<gamut::Rec2020>::new().map(&input, &mut actual);
    assert!(actual
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
    let (wide, _) = reference.css_minde(input.map(f64::from));
    assert!(
        distance(
            reference.encoded_to_lab(actual.map(f64::from)),
            reference.encoded_to_lab(wide)
        ) < 0.0015
    );
}

#[test]
fn exact_hue_methods_match_independent_boundary_oracle() {
    let mut hues: Vec<f32> = (0..1440).map(|i| i as f32 / 4.0).collect();
    for hue in [28.958133f32, 145.64496, 264.05203, 264.53668, 359.99997] {
        for offset in -16..=16 {
            hues.push(f32::from_bits(hue.to_bits().wrapping_add_signed(offset)));
        }
    }
    let (mut direct, mut halley, mut ostrowski) = (
        OklchCubicDirect::new(),
        OklchHalley::new(),
        OklchOstrowski::new(),
    );
    let mut max = [0.0f64; 3];
    let mut worst = [[0.0; 3]; 3];
    for l in [
        0.001f32,
        0.01,
        0.1,
        0.3,
        0.5,
        0.7,
        0.9,
        0.99,
        0.9999,
        1.0f32.next_down(),
    ] {
        for &h in &hues {
            let edge = boundary(l as f64, h as f64);
            let c = edge as f32;
            for chroma in [c.next_down(), c, c.next_up(), c * 1.001, 0.4] {
                let input = [l, chroma, h];
                let expected = encoded(l as f64, (chroma as f64).min(edge), h as f64);
                for checked in [false, true] {
                    let mut outputs = [[0.0; 3]; 3];
                    if checked {
                        direct.map_with_in_gamut_check(&input, &mut outputs[0]);
                        halley.map_with_in_gamut_check(&input, &mut outputs[1]);
                        ostrowski.map_with_in_gamut_check(&input, &mut outputs[2]);
                    } else {
                        direct.map(&input, &mut outputs[0]);
                        halley.map(&input, &mut outputs[1]);
                        ostrowski.map(&input, &mut outputs[2]);
                    }
                    for (index, out) in outputs.into_iter().enumerate() {
                        assert!(out.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
                        for channel in 0..3 {
                            let error = (out[channel] as f64 - expected[channel]).abs();
                            if error > max[index] {
                                max[index] = error;
                                worst[index] = input;
                            }
                        }
                    }
                }
            }
        }
    }
    for (i, name) in ["direct", "halley", "ostrowski"].into_iter().enumerate() {
        eprintln!("{name}: oracle max {:.9e} at {:?}", max[i], worst[i]);
        assert!(max[i] <= 1e-4, "{name}: {} at {:?}", max[i], worst[i]);
    }
}

#[test]
fn edge_seeker_arc_is_stable_near_zero_curvature() {
    for k in [-1e-6f32, -1e-9, 0.0, 1e-9, 1e-6] {
        for x in [0.0, 0.1, 0.5, 0.9, 1.0] {
            let y = edge_seeker::intersection_with_arc(x, k);
            assert!((y - x).abs() < 1e-6, "{x}, {k}: {y}");
            if k == 0.0 {
                assert_eq!(x.to_bits(), y.to_bits());
            }
        }
    }
    let mut mapper = EdgeSeeker::new();
    let mut out = [0.0; 3];
    mapper.map(&[0.9, 0.4, 96.02319], &mut out);
    let expected = [0.99999994, 0.86572343, 0.19619057];
    assert!(
        (0..3).all(|i| (out[i] - expected[i]).abs() < 2e-6),
        "{out:?}"
    );
}

#[test]
fn cached_cubics_match_oracle_at_their_bucket_hue() {
    let (mut cached, mut uncached) = (OklchCubic::new(), OklchCubicNoCache::new());
    let mut max = 0.0f64;
    let mut worst = [0.0; 3];
    for hi in 0..3600 {
        let hue = hi as f32 / 10.0;
        for h in [hue.next_down(), hue, hue.next_up()] {
            let mut normalized = h % 360.0;
            if normalized < 0.0 {
                normalized += 360.0;
            }
            let bucket = (normalized * 10.0).round() / 10.0;
            for l in [0.001f32, 0.1, 0.5, 0.9, 0.9999, 1.0f32.next_down()] {
                let input = [l, 0.4, h];
                let expected = encoded(l as f64, boundary(l as f64, bucket as f64), bucket as f64);
                let (mut a, mut b) = ([0.0; 3], [0.0; 3]);
                cached.map(&input, &mut a);
                uncached.map(&input, &mut b);
                assert_eq!(a, b, "{input:?}");
                for channel in 0..3 {
                    let error = (a[channel] as f64 - expected[channel]).abs();
                    if error > max {
                        max = error;
                        worst = input;
                    }
                }
            }
        }
    }
    eprintln!("cached cubics: bucket-hue oracle max {max:.9e} at {worst:?}");
    assert!(max <= 1e-4);
}

#[test]
fn raytrace_rounding_regressions() {
    let (mut narrow, mut wide) = (
        Raytrace::new(),
        crate::float64::rgb_solvers::Raytrace::<gamut::DisplayP3>::new(),
    );
    for input in [
        [0.9, 0.0698177, 9.75],
        [0.9482159, 0.4, 6.3630238],
        [0.01, 0.4, 54.0],
    ] {
        let (mut a, mut b) = ([0.0; 3], [0.0; 3]);
        narrow.map(&input, &mut a);
        wide.map(&input.map(f64::from), &mut b);
        assert!(
            (0..3).all(|i| (a[i] as f64 - b[i]).abs() < 1e-4),
            "{input:?}: {a:?} vs {b:?}"
        );
    }
}

#[test]
fn lut_and_caches_store_f32() {
    assert_eq!(
        std::mem::size_of_val(<gamut::DisplayP3 as edge_seeker::EdgeSeekerData>::LUT),
        710 * 4 * 4
    );
    assert_eq!(
        std::mem::size_of_val(OklchCubic::new().cache.as_slice()),
        3601 * 13 * 4
    );
    assert_eq!(
        std::mem::size_of_val(BottossonLightnessCached::new().cache.as_slice()),
        3601 * 5 * 4
    );
}

#[test]
fn indexed_and_uncached_variants_agree() {
    let (mut cached, mut uncached) = (OklchCubic::new(), OklchCubicNoCache::new());
    let (mut edge, mut indexed) = (EdgeSeeker::new(), EdgeSeekerIndexed::new());
    for hi in 0..3600 {
        let h = hi as f32 / 10.0;
        for hue in [h.next_down(), h, h.next_up()] {
            for l in [0.01, 0.5, 0.99] {
                let input = [l, 0.4, hue];
                let (mut a, mut b) = ([0.0; 3], [0.0; 3]);
                cached.map(&input, &mut a);
                uncached.map(&input, &mut b);
                assert_eq!(a, b, "cubic {input:?}");
                edge.map(&input, &mut a);
                indexed.map(&input, &mut b);
                assert_eq!(a, b, "edge seeker {input:?}");
            }
        }
    }
}

#[test]
fn endpoints_and_extreme_hues_stay_finite() {
    macro_rules! verify {
        ($name:literal, $method:ident) => {{
            let mut mapper = $method::new();
            for l in [
                0.0f32,
                f32::from_bits(1),
                1e-12,
                0.5,
                1.0f32.next_down(),
                1.0,
            ] {
                for c in [0.0, 0.001, 0.4] {
                    for h in [-f32::MAX, -720.25, -0.0, 90.0, 360.0, f32::MAX] {
                        for checked in [false, true] {
                            let input = [l, c, h];
                            let mut out = [0.0; 3];
                            if checked {
                                mapper.map_with_in_gamut_check(&input, &mut out);
                            } else {
                                mapper.map(&input, &mut out);
                            }
                            assert!(
                                out.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
                                "{} {input:?}: {out:?}",
                                $name
                            );
                        }
                    }
                }
            }
        }};
    }
    for_each_method!(verify);
}
