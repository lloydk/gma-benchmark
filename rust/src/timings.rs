// Compiled in each precision module; widening the output checksum is the only
// f64 arithmetic in an f32 pass. Input construction/casting stays outside timing.
pub(crate) fn checksum(
    samples: &[[Float; 3]],
    mut map: impl FnMut(&[Float; 3], &mut [Float; 3]),
) -> f64 {
    let mut out = [0.0; 3];
    let mut sum = 0.0;
    for input in samples {
        map(input, &mut out);
        sum += f64::from(out[0]) + f64::from(out[1]) + f64::from(out[2]);
    }
    sum
}

fn time_method(
    warmup: usize,
    repeats: usize,
    samples: &[[Float; 3]],
    mut map: impl FnMut(&[Float; 3], &mut [Float; 3]),
) -> f64 {
    crate::time_pass(warmup, repeats, samples.len(), || {
        checksum(std::hint::black_box(samples), &mut map)
    })
}

pub(crate) fn print_checksums<G: gamut::RgbGamut>(samples: &[[Float; 3]]) {
    println!(
        "{} / {PRECISION} checksums on grid (sum of all RGB channels):",
        G::DEFINITION.name
    );
    macro_rules! print_method {
        ($name:literal, $method:ty) => {{
            let mut mapper = <$method>::new();
            let sum = checksum(samples, |input, out| mapper.map(input, out));
            println!("  {:<28} {:.10}", $name, sum);
        }};
    }
    macro_rules! core {
        ($name:literal, $module:ident, $method:ident, $policy:ident) => {
            print_method!($name, $module::$method<G>);
        };
    }
    macro_rules! extra {
        ($name:literal, $method:ident, $limit:literal) => {
            print_method!($name, $method);
        };
    }
    for_each_rgb_method!(core);
    if G::ID == crate::rgb_spaces::SpaceId::DisplayP3 {
        for_each_p3_extra!(extra);
    }
    println!();
}

pub(crate) fn run_timings<G: gamut::RgbGamut>(
    label: &str,
    samples: &[[Float; 3]],
    warmup: usize,
    repeats: usize,
    check: bool,
) {
    let mut timings = Vec::new();
    macro_rules! time_mapper {
        ($name:literal, $method:ty) => {{
            let mut mapper = <$method>::new();
            // Mode selection, allocation and gamut dispatch stay outside timing.
            let ns = if check {
                time_method(warmup, repeats, samples, |input, out| {
                    mapper.map_with_in_gamut_check(input, out)
                })
            } else {
                time_method(warmup, repeats, samples, |input, out| {
                    mapper.map(input, out)
                })
            };
            timings.push(($name, ns));
        }};
    }
    macro_rules! core {
        ($name:literal, $module:ident, $method:ident, $policy:ident) => {
            time_mapper!($name, $module::$method<G>);
        };
    }
    macro_rules! extra {
        ($name:literal, $method:ident, $limit:literal) => {
            time_mapper!($name, $method);
        };
    }
    for_each_rgb_method!(core);
    if G::ID == crate::rgb_spaces::SpaceId::DisplayP3 {
        for_each_p3_extra!(extra);
    }
    print_timings(G::DEFINITION.name, label, repeats, timings);
}

fn print_timings(gamut: &str, label: &str, repeats: usize, mut timings: Vec<(&str, f64)>) {
    timings.sort_by(|a, b| a.1.total_cmp(&b.1));
    let fastest = timings[0].1;
    let width = timings.iter().map(|(name, _)| name.len()).max().unwrap();
    println!(
        "── {gamut} / {PRECISION} / {label} ── (median ns/call over {repeats} passes, fastest to slowest):"
    );
    for (name, ns) in timings {
        println!(
            "  {name:<width$} {ns:6.2} ns/call  ({:.2}× fastest)",
            ns / fastest
        );
    }
    println!();
}

#[cfg(test)]
mod checksum_tests {
    use super::*;

    #[test]
    fn every_channel_and_every_sample_contribute() {
        // The first sample's green and the second sample's blue must survive.
        let samples = [[0.0, 1.0, 0.0], [0.0, 0.0, 0.5]];
        assert_eq!(checksum(&samples, |input, out| *out = *input), 1.5);
    }

    #[test]
    fn accumulate_after_widening_each_channel() {
        let samples = [[16_777_216.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert_eq!(checksum(&samples, |input, out| *out = *input), 16_777_218.0);
    }
}
