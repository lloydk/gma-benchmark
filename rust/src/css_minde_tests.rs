use super::super::{oklch_to_clipped_p3, oklch_to_p3_if_in_gamut, SINGLE};
use super::*;

// Test-only independent P3 → XYZ → Oklab conversion, using the CSS sample
// matrices rather than the production precomposed linear-P3 conversion.
fn reference_lab(rgb: [f64; 3]) -> [f64; 3] {
    let multiply = |matrix: [[f64; 3]; 3], vector: [f64; 3]| {
        matrix.map(|row| row.iter().zip(vector).map(|(a, b)| a * b).sum::<f64>())
    };
    let linear = rgb.map(|x| {
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    });
    let xyz = multiply(
        [
            [
                608311.0 / 1250200.0,
                189793.0 / 714400.0,
                198249.0 / 1000160.0,
            ],
            [
                35783.0 / 156275.0,
                247089.0 / 357200.0,
                198249.0 / 2500400.0,
            ],
            [0.0, 32229.0 / 714400.0, 5220557.0 / 5000800.0],
        ],
        linear,
    );
    let lms = multiply(
        [
            [0.8190224379967030, 0.3619062600528904, -0.1288737815209879],
            [0.0329836539323885, 0.9292868615863434, 0.0361446663506424],
            [0.0481771893596242, 0.2642395317527308, 0.6335478284694309],
        ],
        xyz,
    )
    .map(f64::cbrt);
    multiply(
        [
            [0.2104542683093140, 0.7936177747023054, -0.0040720430116193],
            [1.9779985324311684, -2.4285922420485799, 0.4505937096174110],
            [0.0259040424655478, 0.7827717124575296, -0.8086757549230774],
        ],
        lms,
    )
}

#[test]
fn matches_independent_spec_vectors_in_both_modes() {
    let mut mapper = CssMinde::new();
    let mut max_channel: f64 = 0.0;
    let mut max_delta: f64 = 0.0;
    for line in include_str!("../../tests/fixtures/css-minde.csv")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let fields: Vec<_> = line.split(',').collect();
        let values: Vec<f64> = fields[..6].iter().map(|v| v.parse().unwrap()).collect();
        let input = [values[0] as Float, values[1] as Float, values[2] as Float];
        let expected = [values[3], values[4], values[5]];
        let (mut actual, mut checked) = ([0.0; 3], [0.0; 3]);
        mapper.map(&input, &mut actual);
        mapper.map_with_in_gamut_check(&input, &mut checked);
        assert_eq!(actual, checked, "{input:?}");
        assert!(
            actual
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "{input:?}: {actual:?}"
        );
        let channel_error = (0..3)
            .map(|i| (f64::from(actual[i]) - expected[i]).abs())
            .fold(0.0, f64::max);
        max_channel = max_channel.max(channel_error);
        let actual_lab = reference_lab(actual.map(f64::from));
        let expected_lab = reference_lab(expected);
        let delta = (0..3)
            .map(|i| (actual_lab[i] - expected_lab[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        max_delta = max_delta.max(delta);
        // f32 rounding around E=JND-epsilon can change the stopping iteration.
        // Check both encoded and perceptual differences for those regressions.
        assert!(
            channel_error <= if SINGLE { 0.004 } else { 2e-12 },
            "{input:?}: {actual:?} vs {expected:?}"
        );
        assert!(
            delta <= if SINGLE { 0.0002 } else { 2e-12 },
            "{input:?}: deltaEOK {delta:e}"
        );
    }
    eprintln!(
        "CSS MINDE {}-bit reference: channel {max_channel:e}, deltaEOK {max_delta:e}",
        Float::MANTISSA_DIGITS
    );
}

#[test]
fn in_gamut_inputs_preserve_canonical_conversion_bits() {
    let mut accepted = 0;
    let mut mapper = CssMinde::new();
    for l in [0.01, 0.1, 0.5, 0.9, 0.99] {
        for c in [0.0, 0.001, 0.02, 0.1] {
            for hi in -360..720 {
                let h = hi as Float + 0.375;
                let (mut expected, mut actual) = ([0.0; 3], [0.0; 3]);
                if oklch_to_p3_if_in_gamut(l, c, h, &mut expected) {
                    mapper.map(&[l, c, h], &mut actual);
                    assert_eq!(
                        actual.map(Float::to_bits),
                        expected.map(Float::to_bits),
                        "{l}, {c}, {h}"
                    );
                    accepted += 1;
                }
            }
        }
    }
    assert!(accepted > 1000);
}

#[test]
fn endpoints_achromatic_and_extreme_hues() {
    let mut mapper = CssMinde::new();
    let (mut actual, mut expected) = ([0.0; 3], [0.0; 3]);
    for l in [-1.0, 0.0, 1.0, 2.0] {
        mapper.map(&[l, 0.4, 20.0], &mut actual);
        assert_eq!(actual, [if l <= 0.0 { 0.0 } else { 1.0 }; 3]);
    }
    for c in [-0.1, 0.0] {
        mapper.map(&[0.5, c, Float::NAN], &mut actual);
        oklch_to_clipped_p3(0.5, 0.0, 0.0, &mut expected);
        assert_eq!(actual, expected);
    }
    for h in [-Float::MAX, -1e21, -1e9, 1e9, 1e21, Float::MAX] {
        mapper.map(&[0.5, 0.4, h], &mut actual);
        mapper.map(&[0.5, 0.4, h % 360.0], &mut expected);
        assert_eq!(actual, expected);
    }
}
