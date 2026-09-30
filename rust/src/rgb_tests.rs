use super::color::{EncodedRgb, LinearRgb, Oklch};
use super::gamut::{DisplayP3, Rec2020, RgbGamut, Srgb};
use super::{Float, SINGLE};
use crate::rgb_reference::{distance, lch, Reference};
use crate::validation::ValidationProfile;

fn inputs() -> Vec<[Float; 3]> {
    let mut inputs = Vec::new();
    for l in [0.0001, 0.001, 0.01, 0.1, 0.5, 0.9, 0.99, 0.9999] {
        for c in [0.0, 0.001, 0.02, 0.4] {
            for hi in 0..360 {
                inputs.push([l, c, hi as Float + 0.375]);
            }
        }
    }
    let mut random = crate::mulberry32(0x72676233);
    for _ in 0..8192 {
        inputs.push([
            (0.001 + 0.998 * random()) as Float,
            (0.6 * random()) as Float,
            (360.0 * random()) as Float,
        ]);
    }
    inputs.extend([
        [-1.0, 0.4, 20.0],
        [0.0, 0.4, 20.0],
        [1.0, 0.4, 20.0],
        [2.0, 0.4, 20.0],
    ]);
    // Native f32 threshold decisions differ from f64 at these inputs.
    inputs.push([0.98f32 as Float, 0.4f32 as Float, 104.0]);
    inputs.push([
        0.84679353f32 as Float,
        0.39739943f32 as Float,
        209.40465f32 as Float,
    ]);
    inputs
}

pub(crate) fn boundary_inputs<G: RgbGamut>() -> Vec<[Float; 3]> {
    let reference = Reference::new(G::ID);
    let mut samples = vec![[0.88f32 as Float, 0.4643206f32 as Float, 18.5]];
    // Every RGB cube face, plus adjacent representable OKLCh coordinates.
    for axis in 0..3 {
        for face in [0.0, 1.0] {
            for i in 0..17 {
                for j in 0..17 {
                    let mut rgb = [0.0; 3];
                    rgb[axis] = face;
                    rgb[(axis + 1) % 3] = i as f64 / 16.0;
                    rgb[(axis + 2) % 3] = j as f64 / 16.0;
                    let base = lch(reference.linear_to_lab(rgb)).map(|v| v as Float);
                    samples.push(base);
                    for channel in 0..3 {
                        for adjacent in [base[channel].next_down(), base[channel].next_up()] {
                            let mut sample = base;
                            sample[channel] = adjacent;
                            samples.push(sample);
                        }
                    }
                }
            }
        }
    }
    samples
}

fn conversion<G: RgbGamut>() {
    let reference = Reference::new(G::ID);
    let (mut linear_error, mut encoded_error, mut reverse_error) = (0.0f64, 0.0f64, 0.0f64);
    for input in inputs().into_iter().chain(boundary_inputs::<G>()) {
        let expected = reference.linear_rgb(input.map(f64::from));
        let actual = Oklch::from(input).to_oklab().to_linear_rgb::<G>();
        let encoded = actual.encode_clamped().channels;
        for i in 0..3 {
            linear_error = linear_error.max(
                (f64::from(actual.channels[i]) - expected[i]).abs() / expected[i].abs().max(1.0),
            );
            let expected_encoded = reference.encode(expected[i].clamp(0.0, 1.0));
            encoded_error = encoded_error.max((f64::from(encoded[i]) - expected_encoded).abs());
            let linear_limit = if SINGLE { 2e-6 } else { 2e-12 } * expected[i].abs().max(1.0);
            let transfer_limit = if SINGLE { 3e-7 } else { 2e-15 };
            let actual_encoded = f64::from(encoded[i]);
            // Check the transfer independently of matrix/trigonometric error.
            let own_linear = f64::from(actual.channels[i]).clamp(0.0, 1.0);
            assert!((actual_encoded - reference.encode(own_linear)).abs() <= transfer_limit);
            // Carry the linear error through the reference transfer. Its slope
            // near zero is unbounded for Rec.2020, so no fixed channel limit.
            let lo = reference.encode((expected[i] - linear_limit).clamp(0.0, 1.0));
            let hi = reference.encode((expected[i] + linear_limit).clamp(0.0, 1.0));
            assert!(
                actual_encoded >= lo - transfer_limit && actual_encoded <= hi + transfer_limit,
                "{} {input:?}: channel {i}, {actual_encoded} outside [{lo}, {hi}]",
                G::DEFINITION.name
            );
        }
    }
    for r in 0..17 {
        for g in 0..17 {
            for b in 0..17 {
                let rgb = [r, g, b].map(|v| v as Float / 16.0);
                let actual = LinearRgb::<G>::new(rgb).to_oklab();
                let expected = reference.linear_to_lab(rgb.map(f64::from));
                reverse_error = reverse_error.max(distance(
                    [actual.l, actual.a, actual.b].map(f64::from),
                    expected,
                ));
            }
        }
    }
    eprintln!("{} {}-bit conversion: relative/absolute linear {linear_error:e}, encoded {encoded_error:e}, reverse deltaEOK {reverse_error:e}", G::DEFINITION.name, Float::MANTISSA_DIGITS);
    assert!(linear_error <= if SINGLE { 2e-6 } else { 2e-12 });
    assert!(reverse_error <= if SINGLE { 1e-6 } else { 2e-12 });
}

#[test]
fn conversions_match_independent_xyz_reference() {
    conversion::<Srgb>();
    conversion::<DisplayP3>();
    conversion::<Rec2020>();
}

fn minde<G: RgbGamut + ValidationProfile>() {
    let reference = Reference::new(G::ID);
    let mut mapper = super::css_minde::CssMinde::<G>::new();
    let (mut channel_error, mut delta_error) = (0.0f64, 0.0f64);
    let mut worst = [0.0; 3];
    let mut exits = [0; 6];
    for input in inputs() {
        let (expected, exit) = reference.css_minde(input.map(f64::from));
        exits[exit] += 1;
        let (mut actual, mut checked) = ([0.0; 3], [0.0; 3]);
        mapper.map(&input, &mut actual);
        mapper.map_with_in_gamut_check(&input, &mut checked);
        assert_eq!(actual.map(Float::to_bits), checked.map(Float::to_bits));
        assert!(actual
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        for i in 0..3 {
            let error = (f64::from(actual[i]) - expected[i]).abs();
            if error > channel_error {
                channel_error = error;
                worst = input;
            }
        }
        delta_error = delta_error.max(distance(
            reference.encoded_to_lab(actual.map(f64::from)),
            reference.encoded_to_lab(expected),
        ));
    }
    eprintln!("{} {}-bit CSS MINDE: channel {channel_error:e} at {worst:?}, deltaEOK {delta_error:e}, exits {exits:?}", G::DEFINITION.name, Float::MANTISSA_DIGITS);
    assert!(exits.iter().all(|&count| count > 0));
    // Encoded differences are diagnostic; the mapper's empirical acceptance
    // policy is perceptual. Conversion and transfer accuracy are tested above.
    if !SINGLE {
        assert!(channel_error <= 2e-11);
    }
    assert!(delta_error <= if SINGLE { G::MINDE_DELTA_LIMIT } else { 2e-11 });
}

#[test]
fn css_minde_matches_independent_spec_in_every_gamut() {
    minde::<Srgb>();
    minde::<DisplayP3>();
    minde::<Rec2020>();
}

fn preserve<G: RgbGamut>() {
    let mut mapper = super::css_minde::CssMinde::<G>::new();
    let mut clip = super::clip::Clip::<G>::new();
    let mut samples = inputs();
    samples.extend(boundary_inputs::<G>());
    let mut accepted = 0;
    for input in samples {
        if input[0] <= 0.0 || input[0] >= 1.0 || input[1] < 0.0 {
            continue;
        }
        let canonical = Oklch::from(input).to_oklab().to_linear_rgb::<G>();
        if !canonical.in_gamut() {
            continue;
        }
        let expected = canonical.encode_clamped().channels.map(Float::to_bits);
        let (mut actual, mut clipped) = ([0.0; 3], [0.0; 3]);
        mapper.map(&input, &mut actual);
        clip.map(&input, &mut clipped);
        assert_eq!(
            actual.map(Float::to_bits),
            expected,
            "{} {input:?}",
            G::DEFINITION.name
        );
        assert_eq!(clipped.map(Float::to_bits), expected);
        accepted += 1;
    }
    assert!(accepted > 5000);
    eprintln!(
        "{} {}-bit canonical preservation: {accepted} accepted inputs",
        G::DEFINITION.name,
        Float::MANTISSA_DIGITS
    );
}

#[test]
fn in_gamut_conversion_bits_survive_faces_and_neighbours() {
    preserve::<Srgb>();
    preserve::<DisplayP3>();
    preserve::<Rec2020>();
}

fn endpoints<G: RgbGamut>() {
    let mut mapper = super::css_minde::CssMinde::<G>::new();
    let mut clip = super::clip::Clip::<G>::new();
    let (mut actual, mut expected) = ([0.0; 3], [0.0; 3]);
    for l in [-1.0, 0.0, 1.0, 2.0] {
        mapper.map(&[l, 0.4, 20.0], &mut actual);
        assert_eq!(actual, [if l <= 0.0 { 0.0 } else { 1.0 }; 3]);
    }
    for c in [-0.1, 0.0] {
        mapper.map(&[0.5, c, Float::NAN], &mut actual);
        clip.map(&[0.5, 0.0, 0.0], &mut expected);
        assert_eq!(actual.map(Float::to_bits), expected.map(Float::to_bits));
    }
    for h in [-Float::MAX, -1e21, -1e9, 1e9, 1e21, Float::MAX] {
        mapper.map(&[0.5, 0.4, h], &mut actual);
        mapper.map(&[0.5, 0.4, h % 360.0], &mut expected);
        assert_eq!(actual.map(Float::to_bits), expected.map(Float::to_bits));
        assert!(actual
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
    }
}

#[test]
fn all_gamuts_retain_css_endpoint_and_hue_policy() {
    endpoints::<Srgb>();
    endpoints::<DisplayP3>();
    endpoints::<Rec2020>();
}

#[test]
fn transfer_endpoints_breakpoints_and_rec2020_dark_values() {
    use super::transfer::{Rec2020Transfer, SrgbTransfer, TransferFunction};
    for x in [-1.0, 0.0, 1.0, 2.0] {
        // Preserve the incumbent sRGB/P3 endpoint rounding (one ULP below 1).
        assert!((SrgbTransfer::encode_clamped(x) - x.clamp(0.0, 1.0)).abs() <= Float::EPSILON);
        assert_eq!(Rec2020Transfer::encode_clamped(x), x.clamp(0.0, 1.0));
    }
    let breakpoint: Float = 0.0031308;
    let reference = Reference::new(crate::rgb_spaces::SpaceId::Srgb);
    for x in [breakpoint.next_down(), breakpoint, breakpoint.next_up()] {
        assert!(
            (f64::from(SrgbTransfer::encode_clamped(x)) - reference.encode(f64::from(x))).abs()
                < if SINGLE { 1e-7 } else { 1e-15 }
        );
    }
    // 10^-6 raised to 1/2.4 is 10^-2.5. A piecewise video OETF fails this.
    assert!(
        (f64::from(Rec2020Transfer::encode_clamped(1e-6)) - 0.0031622776601683794).abs()
            < if SINGLE { 1e-8 } else { 1e-15 }
    );
    assert_eq!(
        std::mem::size_of::<LinearRgb<Rec2020>>(),
        3 * std::mem::size_of::<Float>()
    );
    assert_eq!(
        std::mem::size_of::<EncodedRgb<Rec2020>>(),
        3 * std::mem::size_of::<Float>()
    );
}
