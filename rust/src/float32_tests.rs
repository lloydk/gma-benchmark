use super::*;

// Independent f64 physical-chroma oracle. It partitions each channel cubic
// at its stationary points, then bisects the first crossing of either face.
// It never calls the production Cardano, Halley, or Ostrowski solvers.
const A: [[f64; 2]; 3] = [
    [0.3963377773761749, 0.2158037573099136],
    [-0.1055613458156586, -0.0638541728258133],
    [-0.0894841775298119, -1.2914855480194092],
];
const M: [[f64; 3]; 3] = [
    [3.127768971361874, -2.2571357625916395, 0.12936679122976516],
    [-1.0910090184377979, 2.413331710306922, -0.32232269186912466],
    [-0.02601080193857028, -0.508041331704167, 1.5340521336427373],
];

fn first_crossing(p: [f64; 4], limit: f64) -> f64 {
    let [a, b, c, d] = p;
    let eval = |x| ((a * x + b) * x + c) * x + d;
    let mut points = vec![0.0, limit];
    if a == 0.0 {
        let x = -c / (2.0 * b);
        if x > 0.0 && x < limit {
            points.push(x);
        }
    } else {
        let discriminant = b * b - 3.0 * a * c;
        if discriminant >= 0.0 {
            let q = -b - discriminant.sqrt().copysign(b);
            for x in [q / (3.0 * a), c / q] {
                if x > 0.0 && x < limit {
                    points.push(x);
                }
            }
        }
    }
    points.sort_by(f64::total_cmp);
    for pair in points.windows(2) {
        let (mut lo, mut hi) = (pair[0], pair[1]);
        let sign = eval(lo).is_sign_negative();
        if eval(hi) == 0.0 {
            return hi;
        }
        if eval(hi).is_sign_negative() == sign {
            continue;
        }
        for _ in 0..64 {
            let mid = lo + (hi - lo) * 0.5;
            if mid == lo || mid == hi {
                break;
            }
            if eval(mid).is_sign_negative() == sign {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        return lo + (hi - lo) * 0.5;
    }
    f64::INFINITY
}

fn direction(h: f64) -> [f64; 3] {
    let h = h * std::f64::consts::PI / 180.0;
    A.map(|[a, b]| a * h.cos() + b * h.sin())
}

fn boundary(l: f64, h: f64) -> f64 {
    let q = direction(h);
    let mut chroma: f64 = 0.5;
    for row in M {
        let a = (0..3).map(|i| row[i] * q[i].powi(3)).sum();
        let b = 3.0 * l * (0..3).map(|i| row[i] * q[i].powi(2)).sum::<f64>();
        let c = 3.0 * l * l * (0..3).map(|i| row[i] * q[i]).sum::<f64>();
        let d = l.powi(3) * row.iter().sum::<f64>();
        chroma = chroma.min(first_crossing([a, b, c, d], chroma));
        chroma = chroma.min(first_crossing([a, b, c, d - 1.0], chroma));
    }
    chroma
}

fn encoded(l: f64, c: f64, h: f64) -> [f64; 3] {
    let lms = direction(h).map(|q| (l + c * q).powi(3));
    M.map(|row| {
        let linear = (0..3).map(|i| row[i] * lms[i]).sum::<f64>().clamp(0.0, 1.0);
        if linear <= 0.0031308 {
            linear * 12.92
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        }
    })
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
            let y = intersection_with_arc(x, k);
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
    let (mut narrow, mut wide) = (Raytrace::new(), crate::float64::Raytrace::new());
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
    assert_eq!(std::mem::size_of_val(&LUT), 710 * 4 * 4);
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
