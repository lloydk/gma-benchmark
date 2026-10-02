use crate::rgb_reference::{distance, Reference};
#[cfg(test)]
use crate::rgb_spaces::SpaceId;
use crate::rgb_spaces::{DisplayP3, Rec2020, RgbSpace, Srgb};
use crate::{float32, float64};

// Explicit empirical policies: a new target must supply its own budget.
// These are corpus regression limits, not full-domain numerical guarantees.
pub(crate) trait ValidationProfile: RgbSpace {
    const MINDE_DELTA_LIMIT: f64;
    const DUALRAY_LINEAR_LIMIT: f64;
    const DUALRAY_DELTA_LIMIT: f64;
    const DUALRAY_ENCODED_LIMIT: Option<f64>;
    // Dualray Fast f32 vs f64: both approximate the same fits, but near a
    // seam, the cusp test or the red fold the lanes can take different paths.
    const DUALRAY_FAST_LINEAR_LIMIT: f64;
    const DUALRAY_FAST_DELTA_LIMIT: f64;
    const BOTTOSSON_LINEAR_LIMIT: f64;
    const BOTTOSSON_DELTA_LIMIT: f64;
    const EDGE_LINEAR_LIMIT: f64;
    const EDGE_DELTA_LIMIT: f64;
    const BOUNDARY_LINEAR_LIMIT: f64;
    const BUCKET_LINEAR_LIMIT: f64;
    const RAYTRACE_LINEAR_LIMIT: f64;
    const BOUNDARY_DELTA_LIMIT: f64;
    const RAYTRACE_DELTA_LIMIT: f64;
    #[cfg(test)]
    const RAYTRACE_REFERENCE_LINEAR_LIMIT: f64;
}
impl ValidationProfile for Srgb {
    const DUALRAY_FAST_LINEAR_LIMIT: f64 = 5e-4;
    const DUALRAY_FAST_DELTA_LIMIT: f64 = 1.5e-4;
    const DUALRAY_ENCODED_LIMIT: Option<f64> = None;
    const DUALRAY_LINEAR_LIMIT: f64 = 2e-5;
    const DUALRAY_DELTA_LIMIT: f64 = 5e-6;
    const BOTTOSSON_LINEAR_LIMIT: f64 = 1.2e-5;
    const BOTTOSSON_DELTA_LIMIT: f64 = 5e-6;
    const EDGE_LINEAR_LIMIT: f64 = 1e-5;
    const EDGE_DELTA_LIMIT: f64 = 3e-6;
    const MINDE_DELTA_LIMIT: f64 = 0.0002;
    const BOUNDARY_LINEAR_LIMIT: f64 = 5e-5;
    const BOUNDARY_DELTA_LIMIT: f64 = 2e-5;
    const RAYTRACE_DELTA_LIMIT: f64 = 0.002;
    const BUCKET_LINEAR_LIMIT: f64 = 5e-5;
    const RAYTRACE_LINEAR_LIMIT: f64 = 2e-4;
    #[cfg(test)]
    const RAYTRACE_REFERENCE_LINEAR_LIMIT: f64 = 5e-4;
}
impl ValidationProfile for DisplayP3 {
    const DUALRAY_FAST_LINEAR_LIMIT: f64 = 3e-4;
    const DUALRAY_FAST_DELTA_LIMIT: f64 = 2e-4;
    const DUALRAY_ENCODED_LIMIT: Option<f64> = Some(1e-4);
    const DUALRAY_LINEAR_LIMIT: f64 = 2e-5;
    const DUALRAY_DELTA_LIMIT: f64 = 5e-6;
    const BOTTOSSON_LINEAR_LIMIT: f64 = 1.2e-5;
    const BOTTOSSON_DELTA_LIMIT: f64 = 5e-6;
    const EDGE_LINEAR_LIMIT: f64 = 1e-5;
    const EDGE_DELTA_LIMIT: f64 = 3e-6;
    const MINDE_DELTA_LIMIT: f64 = 0.00025;
    const BOUNDARY_LINEAR_LIMIT: f64 = 5e-5;
    const BOUNDARY_DELTA_LIMIT: f64 = 2e-5;
    const RAYTRACE_DELTA_LIMIT: f64 = 0.0015;
    const BUCKET_LINEAR_LIMIT: f64 = 5e-5;
    const RAYTRACE_LINEAR_LIMIT: f64 = 5e-5;
    #[cfg(test)]
    const RAYTRACE_REFERENCE_LINEAR_LIMIT: f64 = 5e-5;
}
impl ValidationProfile for Rec2020 {
    const DUALRAY_FAST_LINEAR_LIMIT: f64 = 4e-4;
    const DUALRAY_FAST_DELTA_LIMIT: f64 = 2e-4;
    const DUALRAY_ENCODED_LIMIT: Option<f64> = None;
    const DUALRAY_LINEAR_LIMIT: f64 = 2e-5;
    const DUALRAY_DELTA_LIMIT: f64 = 5e-6;
    const BOTTOSSON_LINEAR_LIMIT: f64 = 1.2e-5;
    const BOTTOSSON_DELTA_LIMIT: f64 = 5e-6;
    const EDGE_LINEAR_LIMIT: f64 = 1e-5;
    const EDGE_DELTA_LIMIT: f64 = 3e-6;
    const MINDE_DELTA_LIMIT: f64 = 0.0015;
    const BOUNDARY_LINEAR_LIMIT: f64 = 5e-5;
    const BOUNDARY_DELTA_LIMIT: f64 = 2e-5;
    const RAYTRACE_DELTA_LIMIT: f64 = 0.0035;
    const BUCKET_LINEAR_LIMIT: f64 = 5e-5;
    const RAYTRACE_LINEAR_LIMIT: f64 = 5e-5;
    #[cfg(test)]
    const RAYTRACE_REFERENCE_LINEAR_LIMIT: f64 = 5e-5;
}
pub(crate) const CLIP_LINEAR_LIMIT: f64 = 2e-6;

#[derive(Clone, Copy)]
enum Policy {
    Clip,
    Minde,
    Boundary,
    Iterative,
    Bucket,
    Raytrace,
    Bottosson,
    BottossonBucket,
    EdgeSeeker,
    Dualray,
    DualrayFast,
}
impl Policy {
    fn metric(self) -> &'static str {
        match self {
            Self::Clip => "linear RGB",
            Self::Minde => "deltaEOK",
            Self::Boundary | Self::Iterative => "linear RGB",
            Self::Bucket | Self::BottossonBucket => "linear/bucket",
            Self::Raytrace | Self::EdgeSeeker | Self::Bottosson => "linear RGB",
            Self::Dualray | Self::DualrayFast => "linear RGB",
        }
    }
    fn delta_limit<G: ValidationProfile>(self) -> Option<f64> {
        match self {
            Self::Boundary | Self::Iterative | Self::Bucket => Some(G::BOUNDARY_DELTA_LIMIT),
            Self::Raytrace => Some(G::RAYTRACE_DELTA_LIMIT),
            Self::EdgeSeeker => Some(G::EDGE_DELTA_LIMIT),
            Self::Bottosson | Self::BottossonBucket => Some(G::BOTTOSSON_DELTA_LIMIT),
            Self::Dualray => Some(G::DUALRAY_DELTA_LIMIT),
            Self::DualrayFast => Some(G::DUALRAY_FAST_DELTA_LIMIT),
            _ => None,
        }
    }
    fn limit<G: ValidationProfile>(self) -> f64 {
        match self {
            Self::Clip => CLIP_LINEAR_LIMIT,
            Self::Minde => G::MINDE_DELTA_LIMIT,
            Self::Boundary | Self::Iterative => G::BOUNDARY_LINEAR_LIMIT,
            Self::Bucket => G::BUCKET_LINEAR_LIMIT,
            Self::Raytrace => G::RAYTRACE_LINEAR_LIMIT,
            Self::EdgeSeeker => G::EDGE_LINEAR_LIMIT,
            Self::Bottosson | Self::BottossonBucket => G::BOTTOSSON_LINEAR_LIMIT,
            Self::Dualray => G::DUALRAY_LINEAR_LIMIT,
            Self::DualrayFast => G::DUALRAY_FAST_LINEAR_LIMIT,
        }
    }
}
#[derive(Default)]
struct Measurement {
    error: f64,
    encoded: f64,
    delta: f64,
    classifications: usize,
    branch_encoded: f64,
    fold_branches: usize,
    fold_encoded: f64,
}

fn max_finite_diff(a: [f64; 3], b: [f64; 3]) -> f64 {
    let mut maximum: f64 = 0.0;
    for (a, b) in a.into_iter().zip(b) {
        let error = (a - b).abs();
        assert!(error.is_finite(), "non-finite channel difference: {a}, {b}");
        maximum = maximum.max(error);
    }
    maximum
}

// The upstream outside-in solve can select different blue-fold branches when
// rounding changes whether an outer island touches the cube. Only recognize
// that ambiguity for outputs on a cube face that retain authored L/h and do
// not increase C. Unit tests separately check the allowed face intersections
// with an independent stationary-interval oracle, including stable outer cases.
fn fold_branch<G: ValidationProfile>(
    reference: &Reference,
    input: [f32; 3],
    output: [f64; 3],
) -> bool {
    let h = f64::from(input[2]);
    if !float32::rgb_solvers::in_blue_fold_id(G::ID, input[2]) {
        return false;
    }
    let [l, a, b] = reference.encoded_to_lab(output);
    let (sin, cos) = (h * std::f64::consts::PI / 180.0).sin_cos();
    let c = a * cos + b * sin;
    let ray_error =
        ((l - f64::from(input[0])).powi(2) + (a - c * cos).powi(2) + (b - c * sin).powi(2)).sqrt();
    let on_face = output.iter().any(|&v| {
        let linear = reference.decode(v);
        linear <= 2e-6 || linear >= 1.0 - 2e-6
    });
    ray_error <= 2e-6 && c >= 0.0 && c <= f64::from(input[1]) + 2e-6 && on_face
}

fn compare<G: ValidationProfile + float32::gamut::RgbGamut + float64::gamut::RgbGamut>(
    name: &str,
    policy: Policy,
    reference: &Reference,
    samples: &[[f32; 3]],
    checked: bool,
    mut narrow: impl FnMut(&[f32; 3], &mut [f32; 3], bool),
    mut wide: impl FnMut(&[f64; 3], &mut [f64; 3], bool),
) -> Measurement {
    let mut result = Measurement::default();
    let mut worst = [0.0; 3];
    for authored in samples {
        // Quantization is a mapping policy, not solver roundoff. Compare the
        // same 0.1-degree bucket; tests separately cover authored-hue prechecks.
        let mut input = *authored;
        if matches!(policy, Policy::Bucket | Policy::BottossonBucket) {
            let h = input[2].rem_euclid(360.0);
            input[2] = (h * 10.0).round() / 10.0;
        }
        let input = &input;
        let (mut actual, mut expected) = ([0.0; 3], [0.0; 3]);
        let mut branch_only = false;
        narrow(input, &mut actual, checked);
        wide(&input.map(f64::from), &mut expected, checked);
        // At a re-entry, tiny conversion roundoff can select different policy
        // branches. Check each accepted color bit-for-bit, then compare the
        // plain solvers separately when the classifications disagree.
        // Dualray Fast's precheck is part of both modes.
        if (checked || matches!(policy, Policy::DualrayFast))
            && matches!(
                policy,
                Policy::Boundary
                    | Policy::Iterative
                    | Policy::Bucket
                    | Policy::Raytrace
                    | Policy::EdgeSeeker
                    | Policy::Bottosson
                    | Policy::BottossonBucket
                    | Policy::DualrayFast
            )
            && input[0] > 0.0
            && input[0] < 1.0
            && input[1] >= 0.0
        {
            let rgb32 = float32::color::Oklch::from(*input)
                .to_oklab()
                .to_linear_rgb::<G>();
            let rgb64 = float64::color::Oklch::from(input.map(f64::from))
                .to_oklab()
                .to_linear_rgb::<G>();
            if rgb32.in_gamut() {
                assert_eq!(
                    actual.map(f32::to_bits),
                    rgb32.encode_clamped().channels.map(f32::to_bits),
                    "{name}: f32 canonical pass-through at {input:?}"
                );
            }
            if rgb64.in_gamut() {
                assert_eq!(
                    expected.map(f64::to_bits),
                    rgb64.encode_clamped().channels.map(f64::to_bits),
                    "{name}: f64 canonical pass-through at {input:?}"
                );
            }
            if !rgb32.in_gamut() || !rgb64.in_gamut() {
                let (mut plain32, mut plain64) = ([0.0; 3], [0.0; 3]);
                narrow(input, &mut plain32, false);
                wide(&input.map(f64::from), &mut plain64, false);
                if !rgb32.in_gamut() {
                    assert_eq!(
                        actual.map(f32::to_bits),
                        plain32.map(f32::to_bits),
                        "{name}: rejected f32 precheck changed output at {input:?}"
                    );
                }
                if !rgb64.in_gamut() {
                    assert_eq!(
                        expected.map(f64::to_bits),
                        plain64.map(f64::to_bits),
                        "{name}: rejected f64 precheck changed output at {input:?}"
                    );
                }
                if rgb32.in_gamut() != rgb64.in_gamut() {
                    result.classifications += 1;
                    result.branch_encoded = result
                        .branch_encoded
                        .max(max_finite_diff(actual.map(f64::from), expected));
                    if matches!(policy, Policy::DualrayFast) {
                        // Its precheck is part of the method, so there is no
                        // separate solver to compare. At a re-entry island's
                        // edge a membership flip switches between the color
                        // and its first exit; each branch was checked above.
                        branch_only = true;
                    } else {
                        actual = plain32;
                        expected = plain64;
                    }
                }
            }
        }
        assert!(
            actual
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "{name}: invalid f32 output at {input:?}: {actual:?}"
        );
        if checked && matches!(policy, Policy::Dualray | Policy::DualrayFast) {
            let (mut plain32, mut plain64) = ([0.0; 3], [0.0; 3]);
            narrow(input, &mut plain32, false);
            wide(&input.map(f64::from), &mut plain64, false);
            assert_eq!(
                actual.map(f32::to_bits),
                plain32.map(f32::to_bits),
                "{name}: intrinsic f32 modes differ at {input:?}"
            );
            assert_eq!(
                expected.map(f64::to_bits),
                plain64.map(f64::to_bits),
                "{name}: intrinsic f64 modes differ at {input:?}"
            );
        }
        assert!(
            expected
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "{name}: invalid f64 reference at {input:?}: {expected:?}"
        );
        if branch_only {
            continue;
        }
        let actual = actual.map(f64::from);
        let encoded = max_finite_diff(actual, expected);
        if matches!(policy, Policy::Dualray) {
            if let Some(limit) = G::DUALRAY_ENCODED_LIMIT {
                assert!(encoded <= limit, "{name}: historical encoded budget {limit:e} exceeded: {encoded:e} at {input:?}");
            }
        }
        let mut delta = distance(
            reference.encoded_to_lab(actual),
            reference.encoded_to_lab(expected),
        );
        assert!(
            delta.is_finite(),
            "{name}: non-finite deltaEOK at {input:?}"
        );
        let mut error = match policy {
            Policy::Clip
            | Policy::Boundary
            | Policy::Iterative
            | Policy::Bucket
            | Policy::Raytrace
            | Policy::EdgeSeeker
            | Policy::Bottosson
            | Policy::BottossonBucket
            | Policy::Dualray
            | Policy::DualrayFast => max_finite_diff(
                actual.map(|v| reference.decode(v)),
                expected.map(|v| reference.decode(v)),
            ),
            Policy::Minde => distance(
                reference.encoded_to_lab(actual),
                reference.encoded_to_lab(expected),
            ),
        };
        assert!(
            error.is_finite() && encoded.is_finite(),
            "{name}: non-finite error at {input:?}"
        );
        if matches!(policy, Policy::Iterative)
            && (error > policy.limit::<G>()
                || policy.delta_limit::<G>().is_some_and(|limit| delta > limit))
            && fold_branch::<G>(reference, *input, actual)
            && fold_branch::<G>(reference, *input, expected)
        {
            result.fold_branches += 1;
            result.fold_encoded = result.fold_encoded.max(encoded);
            // Report branch selection separately; it is not numerical error
            // between two approximations to the same boundary intersection.
            error = 0.0;
            delta = 0.0;
        }
        if let Some(limit) = policy.delta_limit::<G>() {
            assert!(
                delta <= limit,
                "{} {name}: deltaEOK {delta:e} exceeds {limit:e} at {input:?}",
                G::DEFINITION.name
            );
        }
        result.delta = result.delta.max(delta);
        if error > result.error {
            result.error = error;
            worst = *input;
        }
        result.encoded = result.encoded.max(encoded);
    }
    assert!(
        result.error <= policy.limit::<G>(),
        "{} {name}: {} error {:e} exceeds {:e} at {worst:?} (encoded max {:e}, precheck {checked})",
        G::DEFINITION.name,
        policy.metric(),
        result.error,
        policy.limit::<G>(),
        result.encoded
    );
    result
}

pub(crate) fn validate_gamut<G>(grid: &[[f32; 3]], random: &[[f32; 3]])
where
    G: float64::edge_seeker::EdgeSeekerData
        + float32::edge_seeker::EdgeSeekerData
        + float64::bottosson::BottossonData
        + float32::bottosson::BottossonData
        + float64::dualray::DualrayData
        + float32::dualray::DualrayData
        + float64::dualray_fast::DualrayFastData
        + float32::dualray_fast::DualrayFastData
        + ValidationProfile,
{
    let reference = Reference::new(G::ID);
    println!(
        "{} f32 validation: identical f32-rounded inputs (hue buckets aligned); encoded differences are also reported",
        G::DEFINITION.name
    );
    println!(
        "  {:<28} {:>12} {:>12} {:>12} {:>12} {:>12} {:>9} {:>12}",
        "method", "metric", "plain", "prechecked", "limit", "encoded max", "branches", "branch max"
    );
    macro_rules! validate {
        ($name:literal, $narrow:ty, $wide:ty, $policy:expr) => {{
            let (mut narrow, mut wide) = (<$narrow>::new(), <$wide>::new());
            let (mut plain, mut checked, mut encoded, mut delta) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            let mut classifications = 0;
            let mut branch_encoded = 0.0f64;
            let mut fold_branches = 0;
            let mut fold_encoded = 0.0f64;
            for samples in [grid, random] {
                let a = compare::<G>(
                    $name,
                    $policy,
                    &reference,
                    samples,
                    false,
                    |i, o, c| {
                        if c {
                            narrow.map_with_in_gamut_check(i, o)
                        } else {
                            narrow.map(i, o)
                        }
                    },
                    |i, o, c| {
                        if c {
                            wide.map_with_in_gamut_check(i, o)
                        } else {
                            wide.map(i, o)
                        }
                    },
                );
                let b = compare::<G>(
                    $name,
                    $policy,
                    &reference,
                    samples,
                    true,
                    |i, o, c| {
                        if c {
                            narrow.map_with_in_gamut_check(i, o)
                        } else {
                            narrow.map(i, o)
                        }
                    },
                    |i, o, c| {
                        if c {
                            wide.map_with_in_gamut_check(i, o)
                        } else {
                            wide.map(i, o)
                        }
                    },
                );
                delta = delta.max(a.delta).max(b.delta);
                plain = plain.max(a.error);
                checked = checked.max(b.error);
                classifications += b.classifications;
                branch_encoded = branch_encoded.max(b.branch_encoded);
                encoded = encoded.max(a.encoded).max(b.encoded);
                fold_branches += a.fold_branches + b.fold_branches;
                fold_encoded = fold_encoded.max(a.fold_encoded).max(b.fold_encoded);
            }
            println!(
                "  {:<28} {:>12} {:12.3e} {:12.3e} {:12.3e} {:12.3e} {:9} {:12.3e}",
                $name,
                $policy.metric(),
                plain,
                checked,
                $policy.limit::<G>(),
                encoded,
                classifications,
                branch_encoded
            );
            if let Some(limit) = $policy.delta_limit::<G>() {
                println!("    deltaEOK max {delta:.3e}, limit {limit:.3e}");
            }
            if fold_branches > 0 {
                println!("    {fold_branches} independently constrained blue-fold branch differences; encoded max {fold_encoded:.3e}");
            }
        }};
    }
    macro_rules! core {
        ($name:literal, $module:ident, $method:ident, $policy:ident) => {
            validate!(
                $name,
                float32::$module::$method<G>,
                float64::$module::$method<G>,
                Policy::$policy
            );
        };
    }
    for_each_rgb_method!(core);
    println!("branches: differing canonical prechecks; each branch is verified, then plain solvers are compared; branch max is encoded RGB");
    println!("sanity: outputs are finite and in gamut; explicit prechecks preserve canonical bits or match plain mapping; Dualray modes are identical\n");
}

// Compare only methods with matching policies. Bucketed/exact hues are
// compared on the integer grid; outer/first-exit policies only outside folds.
pub(crate) fn validate_solver_agreement<
    G: float64::edge_seeker::EdgeSeekerData
        + float64::bottosson::BottossonData
        + float64::dualray::DualrayData
        + float64::dualray_fast::DualrayFastData,
>(
    grid: &[[f64; 3]],
    random: &[[f64; 3]],
) {
    use float64::rgb_solvers::*;
    let reference = Reference::new(G::ID);
    let (mut cached, mut uncached, mut direct, mut halley, mut ostrowski) = (
        OklchCubic::<G>::new(),
        OklchCubicNoCache::<G>::new(),
        OklchCubicDirect::<G>::new(),
        OklchHalley::<G>::new(),
        OklchOstrowski::<G>::new(),
    );
    let (mut edge, mut indexed) = (
        float64::edge_seeker::EdgeSeeker::<G>::new(),
        float64::edge_seeker::EdgeSeekerIndexed::<G>::new(),
    );
    let (mut bottosson, mut bottosson_cached) = (
        float64::bottosson::BottossonLightness::<G>::new(),
        float64::bottosson::BottossonLightnessCached::<G>::new(),
    );
    let mut bottosson_max = 0.0f64;
    let mut dualray = float64::dualray::Dualray::<G>::new();
    let mut dualray_max = 0.0f64;
    let mut fast = float64::dualray_fast::DualrayFast::<G>::new();
    let mut fast_max = 0.0f64;
    let mut maxima = [0.0f64; 3];
    for (set, samples) in [grid, random].into_iter().enumerate() {
        for input in samples {
            let (mut a, mut b) = ([0.0; 3], [0.0; 3]);
            edge.map(input, &mut a);
            indexed.map(input, &mut b);
            assert_eq!(
                a.map(f64::to_bits),
                b.map(f64::to_bits),
                "Edge Seeker lookup mismatch at {input:?}"
            );
            if set == 0 {
                bottosson.map(input, &mut a);
                bottosson_cached.map(input, &mut b);
                bottosson_max = bottosson_max.max(max_finite_diff(
                    a.map(|v| reference.decode(v)),
                    b.map(|v| reference.decode(v)),
                ));
            }
            dualray.map(input, &mut a);
            // Outside blue-fold windows, where Dualray maps re-entry islands.
            if !in_blue_fold::<G>(input[2]) {
                fast.map(input, &mut b);
                fast_max = fast_max.max(distance(
                    reference.encoded_to_lab(a),
                    reference.encoded_to_lab(b),
                ));
            }
            let mut out = [[0.0; 3]; 5];
            cached.map(input, &mut out[0]);
            uncached.map(input, &mut out[1]);
            direct.map(input, &mut out[2]);
            halley.map(input, &mut out[3]);
            ostrowski.map(input, &mut out[4]);
            assert_eq!(
                out[0].map(f64::to_bits),
                out[1].map(f64::to_bits),
                "{} cached/uncached at {input:?}",
                G::DEFINITION.name
            );
            let rgb = out.map(|v| v.map(|v| reference.decode(v)));
            dualray_max = dualray_max.max(max_finite_diff(a.map(|v| reference.decode(v)), rgb[2]));
            if set == 0 {
                maxima[0] = maxima[0].max(max_finite_diff(rgb[0], rgb[2]));
            }
            let fold = in_blue_fold::<G>(input[2]);
            if !fold {
                maxima[1] = maxima[1].max(max_finite_diff(rgb[2], rgb[3]));
            }
            maxima[2] = maxima[2].max(max_finite_diff(rgb[3], rgb[4]));
        }
    }
    for (i, limit) in [1e-6, 2e-8, 2e-8].into_iter().enumerate() {
        assert!(
            maxima[i] <= limit,
            "{} cross-method comparison {i}: {:e} > {limit:e}",
            G::DEFINITION.name,
            maxima[i]
        );
    }
    assert!(
        dualray_max <= 5e-10,
        "{} Dualray/direct first-exit error {dualray_max:e}",
        G::DEFINITION.name
    );
    println!(
        "{} f64 Dualray/direct linear maximum: {dualray_max:e}",
        G::DEFINITION.name
    );
    // The approximation's design budget, not a measured ceiling.
    assert!(
        fast_max <= 1e-3,
        "{} Dualray Fast/Dualray deltaEOK {fast_max:e}",
        G::DEFINITION.name
    );
    println!(
        "{} f64 Dualray Fast/Dualray deltaEOK maximum (outside blue-fold windows): {fast_max:e}",
        G::DEFINITION.name
    );
    assert!(
        bottosson_max <= 2e-12,
        "{} Bottosson cached/direct grid error {bottosson_max:e}",
        G::DEFINITION.name
    );
    println!(
        "{} f64 Bottosson cached/direct linear maximum on integer-hue grid: {bottosson_max:e}",
        G::DEFINITION.name
    );
    println!("{} f64 cross-method linear maxima: cubic/direct (grid) {:.3e}, direct/Halley (outside folds) {:.3e}, Halley/Ostrowski {:.3e}; cached/uncached and Edge Seeker lookups bit-identical", G::DEFINITION.name,maxima[0],maxima[1],maxima[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_finite_outputs_and_metrics_are_rejected() {
        for policy in [
            Policy::Clip,
            Policy::Minde,
            Policy::Boundary,
            Policy::Iterative,
            Policy::Bucket,
            Policy::Raytrace,
            Policy::EdgeSeeker,
            Policy::Bottosson,
            Policy::BottossonBucket,
            Policy::Dualray,
            Policy::DualrayFast,
        ] {
            for checked in [false, true] {
                for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                    for narrow_lane in [false, true] {
                        assert!(std::panic::catch_unwind(|| compare::<Rec2020>(
                            "injected non-finite",
                            policy,
                            &Reference::new(SpaceId::Rec2020),
                            &[[0.5, 0.4, 30.0]],
                            checked,
                            |_, out, _| *out = if narrow_lane {
                                [bad as f32, 0.5, 0.5]
                            } else {
                                [0.5; 3]
                            },
                            |_, out, _| *out = if narrow_lane {
                                [0.5; 3]
                            } else {
                                [bad, 0.5, 0.5]
                            }
                        ))
                        .is_err());
                    }
                }
            }
        }
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                std::panic::catch_unwind(|| max_finite_diff([0.0, bad, 0.0], [0.0; 3])).is_err()
            );
        }
    }

    #[test]
    fn dualray_keeps_the_historical_p3_encoded_gate() {
        let reference = Reference::new(SpaceId::DisplayP3);
        let actual = [0.00011f32, 1.0, 1.0];
        let expected = [0.0, 1.0, 1.0];
        // Isolate this gate: both other metrics must accept the injected error.
        assert!(
            max_finite_diff(actual.map(f64::from), expected)
                > DisplayP3::DUALRAY_ENCODED_LIMIT.unwrap()
        );
        assert!(
            max_finite_diff(
                actual.map(|v| reference.decode(f64::from(v))),
                expected.map(|v| reference.decode(v))
            ) < Policy::Dualray.limit::<DisplayP3>()
        );
        assert!(
            distance(
                reference.encoded_to_lab(actual.map(f64::from)),
                reference.encoded_to_lab(expected)
            ) < Policy::Dualray.delta_limit::<DisplayP3>().unwrap()
        );
        let failure = std::panic::catch_unwind(|| {
            compare::<DisplayP3>(
                "injected dark Dualray error",
                Policy::Dualray,
                &reference,
                &[[0.5, 0.4, 30.0]],
                false,
                |_, out, _| *out = actual,
                |_, out, _| *out = expected,
            )
        })
        .err()
        .expect("encoded budget must reject the injection");
        let message = failure.downcast_ref::<String>().expect("formatted panic");
        assert!(message.contains("historical encoded budget"), "{message}");
    }

    #[test]
    fn perceptual_gate_rejects_a_dark_regression_inside_the_linear_budget() {
        for policy in [
            Policy::Boundary,
            Policy::Bucket,
            Policy::Raytrace,
            Policy::EdgeSeeker,
            Policy::Bottosson,
            Policy::BottossonBucket,
            Policy::Dualray,
            Policy::DualrayFast,
        ] {
            assert!(std::panic::catch_unwind(|| compare::<Rec2020>(
                "injected dark error",
                policy,
                &Reference::new(SpaceId::Rec2020),
                &[[0.001, 0.4, 30.0]],
                false,
                |_, out, _| *out = [0.01; 3],
                |_, out, _| *out = [0.0; 3]
            ))
            .is_err());
        }
    }

    #[test]
    fn srgb_blue_primary_review_reproducer_passes_startup_validation() {
        validate_gamut::<Srgb>(&[[0.49, 0.4, 264.04913]], &[]);
    }

    #[test]
    fn rejected_prechecks_must_match_plain_mapping() {
        for policy in [
            Policy::Boundary,
            Policy::Bucket,
            Policy::Raytrace,
            Policy::EdgeSeeker,
            Policy::Bottosson,
            Policy::BottossonBucket,
            Policy::Dualray,
            Policy::DualrayFast,
        ] {
            assert!(std::panic::catch_unwind(|| compare::<Srgb>(
                "injected mode mismatch",
                policy,
                &Reference::new(SpaceId::Srgb),
                &[[0.5, 1.0, 30.0]],
                true,
                |_, out, checked| *out = if checked { [0.25; 3] } else { [0.5; 3] },
                |_, out, checked| *out = if checked { [0.25; 3] } else { [0.5; 3] },
            ))
            .is_err());
        }
    }

    #[test]
    fn fold_window_endpoints_agree_across_precisions() {
        fn check<G: float32::gamut::RgbGamut + float64::gamut::RgbGamut>() {
            let [lo, hi] = crate::rgb_spaces::blue_fold_window(G::ID).unwrap();
            let oracle = crate::test_oracle::BoundaryOracle::new(G::ID);
            for (edge, below, above) in [
                (lo, false, true),
                (hi, true, false),
                (lo - 360.0, false, true),
                (hi - 360.0, true, false),
            ] {
                for (h, expected) in [
                    (edge.next_down(), below),
                    (edge, true),
                    (edge.next_up(), above),
                ] {
                    assert_eq!(
                        float32::rgb_solvers::in_blue_fold::<G>(h),
                        expected,
                        "f32 edge {h}"
                    );
                    assert_eq!(
                        float64::rgb_solvers::in_blue_fold::<G>(f64::from(h)),
                        expected,
                        "f64 edge {h}"
                    );
                    assert_eq!(oracle.in_fold(f64::from(h)), expected, "oracle edge {h}");
                }
            }
            for h in [
                lo.next_down(),
                lo,
                lo.next_up(),
                hi.next_down(),
                hi,
                hi.next_up(),
            ] {
                for h in [h, h - 360.0, h + 360.0, h - 720.0, h + 720.0] {
                    assert_eq!(
                        float32::rgb_solvers::in_blue_fold::<G>(h),
                        float64::rgb_solvers::in_blue_fold::<G>(f64::from(h)),
                        "{} hue {h}",
                        G::DEFINITION.name
                    );
                }
            }
        }
        check::<Srgb>();
        check::<Rec2020>();
    }

    #[test]
    fn cross_method_checks_cover_all_gamuts() {
        let grid = crate::build_grid();
        let random = crate::build_random(grid.len());
        validate_solver_agreement::<Srgb>(&grid, &random);
        validate_solver_agreement::<DisplayP3>(&grid, &random);
        validate_solver_agreement::<Rec2020>(&grid, &random);
    }

    #[test]
    fn all_methods_match_on_benchmark_workloads() {
        let grid = crate::build_grid();
        let random = crate::build_random(grid.len());
        let narrow = |v: &[[f64; 3]]| v.iter().map(|c| c.map(|x| x as f32)).collect::<Vec<_>>();
        validate_gamut::<DisplayP3>(&narrow(&grid), &narrow(&random));
    }

    #[test]
    fn new_gamuts_match_on_benchmark_workloads() {
        let grid = crate::build_grid();
        let random = crate::build_random(grid.len());
        let narrow = |v: &[[f64; 3]]| v.iter().map(|c| c.map(|x| x as f32)).collect::<Vec<_>>();
        let (grid, random) = (narrow(&grid), narrow(&random));
        validate_gamut::<Srgb>(&grid, &random);
        validate_gamut::<Rec2020>(&grid, &random);
    }

    #[test]
    fn mixed_chroma_and_in_gamut_workloads() {
        let mut random = crate::mulberry32(0x463332);
        let mixed: Vec<_> = (0..8192)
            .map(|_| {
                [
                    (0.01 + 0.98 * random()) as f32,
                    (0.45 * random()) as f32,
                    (360.0 * random()) as f32,
                ]
            })
            .collect();
        let inside: Vec<_> = (0..8192)
            .map(|_| {
                [
                    (0.1 + 0.8 * random()) as f32,
                    0.001,
                    (360.0 * random()) as f32,
                ]
            })
            .collect();
        validate_gamut::<DisplayP3>(&mixed, &inside);
        validate_gamut::<Srgb>(&mixed, &inside);
        validate_gamut::<Rec2020>(&mixed, &inside);
    }
    #[test]
    fn core_methods_accept_faces_and_neighbours_in_every_gamut() {
        validate_gamut::<Srgb>(&float32::rgb_tests::boundary_inputs::<Srgb>(), &[]);
        validate_gamut::<DisplayP3>(&float32::rgb_tests::boundary_inputs::<DisplayP3>(), &[]);
        validate_gamut::<Rec2020>(&float32::rgb_tests::boundary_inputs::<Rec2020>(), &[]);
    }

    #[test]
    fn metrics_reject_regressions_away_from_zero() {
        let reference = Reference::new(SpaceId::Rec2020);
        for policy in [Policy::Clip, Policy::Minde] {
            let rejected = std::panic::catch_unwind(|| {
                compare::<Rec2020>(
                    "injected regression",
                    policy,
                    &reference,
                    &[[0.5, 0.1, 90.0]],
                    false,
                    |_, out, _| *out = [0.59, 0.5, 0.5],
                    |_, out, _| *out = [0.5, 0.5, 0.5],
                )
            });
            assert!(
                rejected.is_err(),
                "{} accepted a 0.09-channel regression",
                policy.metric()
            );
        }
    }
}
