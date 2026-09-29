use super::*;

// Independent bisection of the scaled circle residual. This is monotone in y
// for |k| < 1, covering the Display-P3 LUT; no production arc formula is used.
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
    let item = get_lut_item(h);
    let input = [item[0].next_up(), 0.4, h];
    let x = (1.0 - input[0]) / (1.0 - item[0]);
    let expected_chroma = item[1] * arc_oracle(f64::from(x), f64::from(item[3])) as Float;
    let mut expected = [0.0; 3];
    oklch_to_clipped_p3(input[0], expected_chroma, h, &mut expected);
    assert!(expected[0] > 0.99 && expected[1] > 0.8 && expected[2] < 0.01);
    let (mut plain, mut indexed) = (EdgeSeeker::new(), EdgeSeekerIndexed::new());
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

#[test]
fn lut_cusp_and_white_neighbours_match_oracle() {
    let (mut plain, mut indexed) = (EdgeSeeker::new(), EdgeSeekerIndexed::new());
    let mut max_error = 0.0f64;
    for n in 0..=36000 {
        let h = n as Float / 100.0;
        let item = get_lut_item(normalized_hue(h));
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
        "{}-bit Edge Seeker: 576016 endpoint samples, max chroma error {max_error:e}",
        Float::MANTISSA_DIGITS
    );
}
