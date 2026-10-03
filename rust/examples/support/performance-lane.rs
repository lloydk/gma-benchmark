// Compiled in both native precision lanes. Dispatch and casts precede timing.
pub(crate) fn run<G>(name: &str, checked: bool, input: &[[f64; 3]], validate: bool)
where
    G: edge_seeker::EdgeSeekerData
        + bottosson::BottossonData
        + dualray::DualrayData
        + dualray_fast::DualrayFastData,
{
    let samples: Vec<[Float; 3]> = input.iter().map(|v| v.map(|x| x as Float)).collect();
    macro_rules! method {
        ($label:literal, $module:ident, $method:ident, $policy:ident) => {
            if name == $label {
                let mut mapper = $module::$method::<G>::new();
                if checked {
                    measure::<G>(
                        &samples,
                        |s, out| mapper.map_with_in_gamut_check(s, out),
                        validate,
                    );
                } else {
                    measure::<G>(&samples, |s, out| mapper.map(s, out), validate);
                }
                return;
            }
        };
    }
    for_each_rgb_method!(method);
    panic!("unknown method: {name}");
}

fn measure<G: gamut::RgbGamut>(
    samples: &[[Float; 3]],
    mut map: impl FnMut(&[Float; 3], &mut [Float; 3]),
    validate: bool,
) {
    let mut out = [0.0; 3];
    if validate {
        let mut inside = 0;
        let mut sum = 0.0f64;
        for sample in samples {
            inside += color::Oklch::from(*sample)
                .to_oklab()
                .to_linear_rgb::<G>()
                .in_gamut() as usize;
            map(sample, &mut out);
            assert!(out.iter().all(|x| x.is_finite() && *x >= 0.0 && *x <= 1.0));
            sum += f64::from(out[0]) + f64::from(out[1]) + f64::from(out[2]);
        }
        println!(
            "{{\"count\":{},\"inside\":{inside},\"checksum\":{sum}}}",
            samples.len()
        );
        return;
    }
    let mut pass = || {
        let mut sum = 0.0f64;
        for sample in std::hint::black_box(samples) {
            map(sample, &mut out);
            sum += f64::from(out[0]) + f64::from(out[1]) + f64::from(out[2]);
        }
        std::hint::black_box(sum)
    };
    let mut checksum = 0.0;
    for _ in 0..50 {
        checksum += pass();
    }
    let mut passes = Vec::with_capacity(25);
    for _ in 0..25 {
        let now = std::time::Instant::now();
        checksum += pass();
        passes.push(now.elapsed().as_nanos() as f64 / samples.len() as f64);
    }
    assert!(checksum.is_finite());
    let mut sorted = passes.clone();
    sorted.sort_by(f64::total_cmp);
    println!("{{\"count\":{},\"warmup\":50,\"measured\":25,\"ns\":{},\"passes\":{passes:?},\"checksum\":{checksum}}}", samples.len(), sorted[12]);
}
