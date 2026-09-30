use crate::rgb_reference::{distance, Reference};
use crate::rgb_spaces::{DisplayP3, Rec2020, RgbSpace, SpaceId, Srgb};
use crate::{float32, float64};

// Explicit empirical policies: a new target must supply its own budget.
// These are corpus regression limits, not full-domain numerical guarantees.
pub(crate) trait ValidationProfile: RgbSpace {
    const MINDE_DELTA_LIMIT: f64;
}
impl ValidationProfile for Srgb {
    const MINDE_DELTA_LIMIT: f64 = 0.0002;
}
impl ValidationProfile for DisplayP3 {
    const MINDE_DELTA_LIMIT: f64 = 0.00025;
}
impl ValidationProfile for Rec2020 {
    const MINDE_DELTA_LIMIT: f64 = 0.0015;
}
pub(crate) const CLIP_LINEAR_LIMIT: f64 = 2e-6;

#[derive(Clone, Copy)]
enum Policy {
    Clip,
    Minde,
    Encoded(f64),
}
impl Policy {
    fn metric(self) -> &'static str {
        match self {
            Self::Clip => "linear RGB",
            Self::Minde => "deltaEOK",
            Self::Encoded(_) => "encoded RGB",
        }
    }
    fn limit<G: ValidationProfile>(self) -> f64 {
        match self {
            Self::Clip => CLIP_LINEAR_LIMIT,
            Self::Minde => G::MINDE_DELTA_LIMIT,
            Self::Encoded(limit) => limit,
        }
    }
}
#[derive(Default)]
struct Measurement {
    error: f64,
    encoded: f64,
}

fn compare<G: ValidationProfile>(
    name: &str,
    policy: Policy,
    reference: &Reference,
    samples: &[[f32; 3]],
    mut narrow: impl FnMut(&[f32; 3], &mut [f32; 3]),
    mut wide: impl FnMut(&[f64; 3], &mut [f64; 3]),
) -> Measurement {
    let mut result = Measurement::default();
    for input in samples {
        let (mut actual, mut expected) = ([0.0; 3], [0.0; 3]);
        narrow(input, &mut actual);
        wide(&input.map(f64::from), &mut expected);
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
        let max_diff =
            |a: [f64; 3], b: [f64; 3]| (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0, f64::max);
        let encoded = max_diff(actual, expected);
        let error = match policy {
            Policy::Clip => max_diff(
                actual.map(|v| reference.decode(v)),
                expected.map(|v| reference.decode(v)),
            ),
            Policy::Minde => distance(
                reference.encoded_to_lab(actual),
                reference.encoded_to_lab(expected),
            ),
            Policy::Encoded(_) => encoded,
        };
        assert!(error <= policy.limit::<G>(),
            "{} {name}: {} error {error:e} exceeds {:e} at {input:?} (encoded difference {encoded:e})",
            G::DEFINITION.name, policy.metric(), policy.limit::<G>());
        result.error = result.error.max(error);
        result.encoded = result.encoded.max(encoded);
    }
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
        "{} f32 validation: identical f32-rounded inputs; encoded differences are also reported",
        G::DEFINITION.name
    );
    println!(
        "  {:<28} {:>12} {:>12} {:>12} {:>12} {:>12}",
        "method", "metric", "plain", "prechecked", "limit", "encoded max"
    );
    macro_rules! validate {
        ($name:literal, $narrow:ty, $wide:ty, $policy:expr) => {{
            let (mut narrow, mut wide) = (<$narrow>::new(), <$wide>::new());
            let (mut plain, mut checked, mut encoded) = (0.0f64, 0.0f64, 0.0f64);
            for samples in [grid, random] {
                let a = compare::<G>(
                    $name,
                    $policy,
                    &reference,
                    samples,
                    |i, o| narrow.map(i, o),
                    |i, o| wide.map(i, o),
                );
                let b = compare::<G>(
                    $name,
                    $policy,
                    &reference,
                    samples,
                    |i, o| narrow.map_with_in_gamut_check(i, o),
                    |i, o| wide.map_with_in_gamut_check(i, o),
                );
                plain = plain.max(a.error);
                checked = checked.max(b.error);
                encoded = encoded.max(a.encoded).max(b.encoded);
            }
            println!(
                "  {:<28} {:>12} {:12.3e} {:12.3e} {:12.3e} {:12.3e}",
                $name,
                $policy.metric(),
                plain,
                checked,
                $policy.limit::<G>(),
                encoded
            );
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
            validate!(
                $name,
                float32::$method,
                float64::$method,
                Policy::Encoded($limit)
            );
        };
    }
    for_each_rgb_method!(core);
    if p3_extras {
        for_each_p3_extra!(extra);
    }
    println!("sanity: all f32/f64 validation outputs are finite and in gamut\n");
}

#[cfg(test)]
mod tests {
    use super::*;

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
                    |_, out| *out = [0.59, 0.5, 0.5],
                    |_, out| *out = [0.5, 0.5, 0.5],
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
