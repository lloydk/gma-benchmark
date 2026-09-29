use crate::{float32, float64};

fn budget(name: &str) -> f64 {
    match name {
        // f32 rounding can choose the adjacent 0.1-degree cache bucket.
        "oklch-cubic (cached)" | "oklch-cubic (no cache)" | "bottosson-lightness (cached)" => 0.002,
        "bottosson-lightness" => 0.001,
        "raytrace" => 0.0002,
        // Local MINDE's JND/epsilon comparisons can stop at different chromas
        // after f32 rounding. Spec-vector tests also bound the Oklab difference.
        "css-minde" => 0.004,
        _ => 0.0001,
    }
}

fn compare(
    name: &str,
    samples: &[[f32; 3]],
    mut narrow: impl FnMut(&[f32; 3], &mut [f32; 3]),
    mut wide: impl FnMut(&[f64; 3], &mut [f64; 3]),
) -> f64 {
    let mut max: f64 = 0.0;
    let mut worst = [0.0; 3];
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
        for channel in 0..3 {
            let error = (f64::from(actual[channel]) - expected[channel]).abs();
            if error > max {
                max = error;
                worst = *input;
            }
        }
    }
    assert!(
        max <= budget(name),
        "{name}: f32/f64 max channel error {max:e} at {worst:?}"
    );
    max
}

pub(crate) fn validate_workloads(grid: &[[f32; 3]], random: &[[f32; 3]]) {
    println!(
        "f32 validation: max encoded-channel difference from f64 on identical f32-rounded inputs"
    );
    println!("  {:<28} {:>12} {:>12}", "method", "plain", "prechecked");
    macro_rules! validate {
        ($name:literal, $method:ident) => {{
            let (mut narrow, mut wide) = (float32::$method::new(), float64::$method::new());
            let mut plain: f64 = 0.0;
            let mut checked: f64 = 0.0;
            for samples in [grid, random] {
                plain = plain.max(compare(
                    $name,
                    samples,
                    |input, out| narrow.map(input, out),
                    |input, out| wide.map(input, out),
                ));
                checked = checked.max(compare(
                    $name,
                    samples,
                    |input, out| narrow.map_with_in_gamut_check(input, out),
                    |input, out| wide.map_with_in_gamut_check(input, out),
                ));
            }
            println!("  {:<28} {:12.3e} {:12.3e}", $name, plain, checked);
        }};
    }
    for_each_method!(validate);
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
        validate_workloads(&narrow(&grid), &narrow(&random));
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
        validate_workloads(&mixed, &inside);
    }
}
