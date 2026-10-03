// Appended inside the production edge_seeker module in disposable checkouts.
// Stage inputs are prepared before timing; the individual costs are not additive.
pub(crate) fn profile<G: EdgeSeekerData>(stage: &str, wide: &[[f64; 3]]) {
    let samples: Vec<[Float; 3]> = wide.iter().map(|s| s.map(|v| v as Float)).collect();
    let lookup = |h| {
        conditioned_item::<G>(h)
            .unwrap_or_else(|| get_lut_item_indexed::<G>(normalized_hue(h), G::INTERVAL_INDEX))
    };
    let normalized: Vec<_> = samples
        .iter()
        .map(|s| [s[0], s[1], normalized_hue(s[2])])
        .collect();
    let items: Vec<_> = samples.iter().map(|s| lookup(s[2])).collect();
    let boundary_inputs: Vec<_> = samples
        .iter()
        .zip(&items)
        .map(|(s, item)| (s[0], *item))
        .collect();
    let converted: Vec<_> = samples
        .iter()
        .zip(&items)
        .map(|(s, item)| [s[0], s[1].min(max_chroma_from_item(s[0], *item)), s[2]])
        .collect();

    let mut indexed = EdgeSeekerIndexed::<G>::new();
    let mut binary = EdgeSeeker::<G>::new();
    let mut clip = super::clip::Clip::<G>::new();
    let mut fingerprint = 0xcbf29ce484222325u64;
    for (s, mapped) in samples.iter().zip(&converted) {
        let (mut a, mut b, mut c) = ([0.0; 3], [0.0; 3], [0.0; 3]);
        indexed.map(s, &mut a);
        binary.map(s, &mut b);
        clip.map(mapped, &mut c);
        assert_eq!(a.map(Float::to_bits), b.map(Float::to_bits));
        assert_eq!(a.map(Float::to_bits), c.map(Float::to_bits));
        for channel in a {
            assert!(channel.is_finite());
            fingerprint ^= channel.to_bits() as u64;
            fingerprint = fingerprint.wrapping_mul(0x100000001b3);
        }
    }
    let below = samples
        .iter()
        .zip(&items)
        .filter(|(s, item)| s[0] <= item[0])
        .count();
    let exact_knots = normalized
        .iter()
        .filter(|s| {
            let h = s[2];
            G::LUT.binary_search_by(|row| row[2].total_cmp(&h)).is_ok()
        })
        .count();
    macro_rules! measure {
        ($inputs:expr, $map:expr) => {
            profile_measure($inputs, $map, fingerprint, below, exact_knots)
        };
    }
    match stage {
        "read-hue" => measure!(&samples, |s: &[Float; 3]| [s[2], 0.0, 0.0]),
        "normalize" => measure!(&samples, |s: &[Float; 3]| [normalized_hue(s[2]), 0.0, 0.0]),
        "lookup-only" => measure!(&normalized, |s: &[Float; 3]| {
            let item = get_lut_item_indexed::<G>(s[2], G::INTERVAL_INDEX);
            [item[0], item[1], item[3]]
        }),
        "lookup" => measure!(&samples, |s: &[Float; 3]| {
            let item = lookup(s[2]);
            [item[0], item[1], item[3]]
        }),
        "lookup-boundary" => measure!(&samples, |s: &[Float; 3]| {
            [indexed.max_chroma(s[0], s[2]), 0.0, 0.0]
        }),
        "boundary" => measure!(&boundary_inputs, |&(l, item): &(Float, [Float; 4])| {
            [max_chroma_from_item(l, item), 0.0, 0.0]
        }),
        "conversion" => measure!(&converted, |s: &[Float; 3]| {
            let mut out = [0.0; 3];
            clip.map(s, &mut out);
            out
        }),
        "full-indexed" => measure!(&samples, |s: &[Float; 3]| {
            let mut out = [0.0; 3];
            indexed.map(s, &mut out);
            out
        }),
        "full-binary" => measure!(&samples, |s: &[Float; 3]| {
            let mut out = [0.0; 3];
            binary.map(s, &mut out);
            out
        }),
        "cubic-cached" => {
            let mut cubic = super::rgb_solvers::OklchCubic::<G>::new();
            measure!(&samples, |s: &[Float; 3]| {
                let mut out = [0.0; 3];
                cubic.map(s, &mut out);
                out
            });
        }
        "clip" => measure!(&samples, |s: &[Float; 3]| {
            let mut out = [0.0; 3];
            clip.map(s, &mut out);
            out
        }),
        _ => panic!("unknown stage: {stage}"),
    }
}

fn profile_measure<T>(
    samples: &[T],
    mut map: impl FnMut(&T) -> [Float; 3],
    fingerprint: u64,
    below: usize,
    exact_knots: usize,
) {
    let mut pass = || {
        let mut sum = 0.0f64;
        for sample in std::hint::black_box(samples) {
            let out = map(sample);
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
        let start = std::time::Instant::now();
        checksum += pass();
        passes.push(start.elapsed().as_nanos() as f64 / samples.len() as f64);
    }
    assert!(checksum.is_finite());
    let mut sorted = passes.clone();
    sorted.sort_by(f64::total_cmp);
    println!("{{\"ns\":{},\"passes\":{passes:?},\"checksum\":{checksum},\"fingerprint\":\"{fingerprint:016x}\",\"count\":{},\"belowCusp\":{below},\"exactKnots\":{exact_knots}}}", sorted[12], samples.len());
}
