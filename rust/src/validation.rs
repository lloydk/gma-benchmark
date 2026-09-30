use crate::rgb_reference::{distance, Reference};
use crate::rgb_spaces::{DisplayP3, Rec2020, RgbSpace, SpaceId, Srgb};
use crate::{float32, float64};

// Explicit empirical policies: a new target must supply its own budget.
// These are corpus regression limits, not full-domain numerical guarantees.
pub(crate) trait ValidationProfile: RgbSpace {
    const MINDE_DELTA_LIMIT: f64;
    const BOUNDARY_LINEAR_LIMIT: f64;
    const BUCKET_LINEAR_LIMIT: f64;
    const RAYTRACE_LINEAR_LIMIT: f64;
    const BOUNDARY_DELTA_LIMIT: f64;
    const RAYTRACE_DELTA_LIMIT: f64;
    #[cfg(test)]
    const RAYTRACE_REFERENCE_LINEAR_LIMIT: f64;
}
impl ValidationProfile for Srgb {
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
    Encoded(f64),
}
impl Policy {
    fn metric(self) -> &'static str {
        match self {
            Self::Clip => "linear RGB",
            Self::Minde => "deltaEOK",
            Self::Boundary | Self::Iterative => "linear RGB",
            Self::Bucket => "linear/bucket",
            Self::Raytrace => "linear RGB",
            Self::Encoded(_) => "encoded RGB",
        }
    }
    fn delta_limit<G: ValidationProfile>(self) -> Option<f64> {
        match self {
            Self::Boundary | Self::Iterative | Self::Bucket => Some(G::BOUNDARY_DELTA_LIMIT),
            Self::Raytrace => Some(G::RAYTRACE_DELTA_LIMIT),
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
            Self::Encoded(limit) => limit,
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
    let Some([lo, hi]) = crate::rgb_spaces::blue_fold_window(G::ID) else {
        return false;
    };
    if !(f64::from(lo)..=f64::from(hi)).contains(&h.rem_euclid(360.0)) {
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
        if matches!(policy, Policy::Bucket) {
            let h = input[2].rem_euclid(360.0);
            input[2] = (h * 10.0).round() / 10.0;
        }
        let input = &input;
        let (mut actual, mut expected) = ([0.0; 3], [0.0; 3]);
        narrow(input, &mut actual, checked);
        wide(&input.map(f64::from), &mut expected, checked);
        // At a re-entry, tiny conversion roundoff can select different policy
        // branches. Check each accepted color bit-for-bit, then compare the
        // plain solvers separately when the classifications disagree.
        if checked
            && matches!(
                policy,
                Policy::Boundary | Policy::Iterative | Policy::Bucket | Policy::Raytrace
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
            if rgb32.in_gamut() != rgb64.in_gamut() {
                result.classifications += 1;
                result.branch_encoded = result.branch_encoded.max(
                    (0..3)
                        .map(|i| (f64::from(actual[i]) - expected[i]).abs())
                        .fold(0.0, f64::max),
                );
                let (mut plain32, mut plain64) = ([0.0; 3], [0.0; 3]);
                narrow(input, &mut plain32, false);
                wide(&input.map(f64::from), &mut plain64, false);
                if !rgb32.in_gamut() {
                    assert_eq!(actual.map(f32::to_bits), plain32.map(f32::to_bits));
                }
                if !rgb64.in_gamut() {
                    assert_eq!(expected.map(f64::to_bits), plain64.map(f64::to_bits));
                }
                actual = plain32;
                expected = plain64;
            }
        }
        assert!(
            actual
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "{name}: invalid f32 output at {input:?}: {actual:?}"
        );
        assert!(
            expected
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "{name}: invalid f64 reference at {input:?}: {expected:?}"
        );
        let actual = actual.map(f64::from);
        let encoded = max_finite_diff(actual, expected);
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
            | Policy::Raytrace => max_finite_diff(
                actual.map(|v| reference.decode(v)),
                expected.map(|v| reference.decode(v)),
            ),
            Policy::Minde => distance(
                reference.encoded_to_lab(actual),
                reference.encoded_to_lab(expected),
            ),
            Policy::Encoded(_) => encoded,
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
    G: float64::gamut::RgbGamut + float32::gamut::RgbGamut + ValidationProfile,
{
    validate_methods::<G>(grid, random, G::ID == SpaceId::DisplayP3);
}

fn validate_methods<G>(grid: &[[f32; 3]], random: &[[f32; 3]], p3_extras: bool)
where
    G: float64::gamut::RgbGamut + float32::gamut::RgbGamut + ValidationProfile,
{
    let reference = Reference::new(G::ID);
    println!(
        "{} f32 validation: identical f32-rounded inputs (cubic buckets aligned); encoded differences are also reported",
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
    macro_rules! extra {
        ($name:literal, $method:ident, $limit:literal) => {
            if p3_extras {
                validate!(
                    $name,
                    float32::$method,
                    float64::$method,
                    Policy::Encoded($limit)
                );
            }
        };
    }
    for_each_method!(core, extra);
    println!("branches: differing canonical prechecks; each branch is verified, then plain solvers are compared; branch max is encoded RGB");
    println!("sanity: all f32/f64 validation outputs are finite and in gamut\n");
}

// Compare only methods with matching policies. Bucketed/exact hues are
// compared on the integer grid; outer/first-exit policies only outside folds.
pub(crate) fn validate_solver_agreement<G: float64::gamut::RgbGamut>(
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
    let mut maxima = [0.0f64; 3];
    for (set, samples) in [grid, random].into_iter().enumerate() {
        for input in samples {
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
            if set == 0 {
                maxima[0] = maxima[0].max(max_finite_diff(rgb[0], rgb[2]));
            }
            let fold = crate::rgb_spaces::blue_fold_window(G::ID).is_some_and(|[lo, hi]| {
                (f64::from(lo)..=f64::from(hi)).contains(&input[2].rem_euclid(360.0))
            });
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
    println!("{} f64 cross-method linear maxima: cubic/direct (grid) {:.3e}, direct/Halley (outside folds) {:.3e}, Halley/Ostrowski {:.3e}; cached/uncached bit-identical", G::DEFINITION.name,maxima[0],maxima[1],maxima[2]);
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
        ] {
            for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert!(std::panic::catch_unwind(|| compare::<Rec2020>(
                    "injected non-finite",
                    policy,
                    &Reference::new(SpaceId::Rec2020),
                    &[[0.5, 0.4, 30.0]],
                    false,
                    |_, out, _| *out = [bad, 0.5, 0.5],
                    |_, out, _| *out = [0.5; 3]
                ))
                .is_err());
            }
        }
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                std::panic::catch_unwind(|| max_finite_diff([0.0, bad, 0.0], [0.0; 3])).is_err()
            );
        }
    }

    #[test]
    fn perceptual_gate_rejects_a_dark_regression_inside_the_linear_budget() {
        for policy in [Policy::Boundary, Policy::Bucket, Policy::Raytrace] {
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
    fn fold_window_endpoints_agree_across_precisions() {
        fn check<G: float32::gamut::RgbGamut + float64::gamut::RgbGamut>() {
            let [lo, hi] = crate::rgb_spaces::blue_fold_window(G::ID).unwrap();
            for h in [
                lo.next_down(),
                lo,
                lo.next_up(),
                hi.next_down(),
                hi,
                hi.next_up(),
            ] {
                assert_eq!(
                    float32::rgb_solvers::in_blue_fold::<G>(h),
                    float64::rgb_solvers::in_blue_fold::<G>(f64::from(h))
                );
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
        // The unported, hue-quantized P3 solvers retain their historical
        // workload budgets; this expanded corpus validates the ported methods.
        validate_methods::<DisplayP3>(
            &float32::rgb_tests::boundary_inputs::<DisplayP3>(),
            &[],
            false,
        );
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
