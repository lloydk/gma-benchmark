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

pub(crate) fn print_checksums(samples: &[[Float; 3]]) {
    println!("{PRECISION} checksums on grid (sum of all P3 channels):");
    macro_rules! print_method {
        ($name:literal, $method:ident) => {{
            let mut mapper = $method::new();
            let sum = checksum(samples, |input, out| mapper.map(input, out));
            println!("  {:<28} {:.10}", $name, sum);
        }};
    }
    for_each_method!(print_method);
    println!();
}

pub(crate) fn run_timings(
    label: &str,
    samples: &[[Float; 3]],
    warmup: usize,
    repeats: usize,
    check: bool,
) {
    let mut timings = Vec::new();
    macro_rules! time_mapper {
        ($name:literal, $method:ident) => {{
            let mut mapper = $method::new();
            // Mode selection and allocation occur outside the timed loops.
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
    for_each_method!(time_mapper);
    timings.sort_by(|a, b| a.1.total_cmp(&b.1));
    let fastest = timings[0].1;
    let width = timings.iter().map(|(name, _)| name.len()).max().unwrap();
    println!(
        "── {PRECISION} / {label} ── (median ns/call over {repeats} passes, fastest to slowest):"
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
