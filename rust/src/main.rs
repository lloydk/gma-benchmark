// Scalar OKLCh -> RGB benchmarks in native f64 and f32.
use std::hint::black_box;
use std::time::Instant;

#[macro_use]
mod methods;
mod cli;
mod rgb_spaces;
mod float64 {
    type Float = f64;
    const SINGLE: bool = false;
    const PRECISION: &str = "f64";
    include!("algorithms.rs");
    include!("timings.rs");
}
mod float32 {
    type Float = f32;
    const SINGLE: bool = true;
    const PRECISION: &str = "f32";
    include!("algorithms.rs");
    include!("timings.rs");
    #[cfg(test)]
    mod tests {
        include!("float32_tests.rs");
    }
}
use float64::*;
mod rgb_reference;
#[cfg(test)]
mod test_oracle;
mod validation;

// ── Benchmark harness ───────────────────────────────────────────────────────

fn build_grid() -> Vec<[f64; 3]> {
    let chroma = 0.4;
    let den: f64 = 100.0;
    let hi = ((1.0 - 0.01) * den).round() as i64;
    let lo = (0.01 * den).round() as i64;
    let mut samples = Vec::new();
    let mut li = hi;
    while li >= lo {
        let l = li as f64 / den;
        for h in 0..360 {
            samples.push([l, chroma, h as f64]);
        }
        li -= 1;
    }
    samples
}

// Small deterministic PRNG (mulberry32), mirroring the JS benchmark so the
// random workload is reproducible run to run.
fn mulberry32(seed: u32) -> impl FnMut() -> f64 {
    let mut a = seed;
    move || {
        a = a.wrapping_add(0x6D2B_79F5);
        let mut t = (a ^ (a >> 15)).wrapping_mul(a | 1);
        t = (t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61))) ^ t;
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

// `count` stratified/jittered values evenly covering [min, min+range) — one
// random sample per equal bin — then Fisher–Yates shuffled so they don't arrive
// in sorted order. Deterministic via `seed`.
fn stratified_shuffled(count: usize, min: f64, range: f64, seed: u32) -> Vec<f64> {
    let mut rand = mulberry32(seed);
    let mut values: Vec<f64> = (0..count)
        .map(|i| min + (i as f64 + rand()) * (range / count as f64))
        .collect();
    for i in (1..count).rev() {
        let j = (rand() * (i as f64 + 1.0)) as usize;
        values.swap(i, j);
    }
    values
}

// Random workload: same sample count as the grid, but every hue and lightness
// is an independent stratified/jittered fractional value (even coverage of its
// range, shuffled). Lightness covers the same 0.01..0.99 range as the grid.
fn build_random(n: usize) -> Vec<[f64; 3]> {
    let chroma = 0.4;
    let lightness_step = 0.01;
    let rand_h = stratified_shuffled(n, 0.0, 360.0, 0x9e37_79b9);
    let rand_l = stratified_shuffled(n, lightness_step, 1.0 - 2.0 * lightness_step, 0x85eb_ca6b);
    (0..n).map(|i| [rand_l[i], chroma, rand_h[i]]).collect()
}

fn time_pass(warmup: usize, repeats: usize, n: usize, mut pass: impl FnMut() -> f64) -> f64 {
    for _ in 0..warmup {
        black_box(pass());
    }
    let mut times = Vec::with_capacity(repeats);
    for _ in 0..repeats {
        let t0 = Instant::now();
        // Consume this pass before stopping the clock, not just after all passes.
        black_box(pass());
        times.push(t0.elapsed().as_nanos() as f64 / n as f64);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    times[times.len() / 2]
}

fn max_channel_diff(
    samples: &[[f64; 3]],
    mut a: impl FnMut(&[f64; 3], &mut [f64; 3]),
    mut b: impl FnMut(&[f64; 3], &mut [f64; 3]),
) -> f64 {
    let mut a_out = [0.0; 3];
    let mut b_out = [0.0; 3];
    let mut max = 0.0;
    for c in samples {
        a(c, &mut a_out);
        b(c, &mut b_out);
        for i in 0..3 {
            let diff = (a_out[i] - b_out[i]).abs();
            if diff > max {
                max = diff;
            }
        }
    }
    max
}

fn main() {
    let options = match cli::Options::parse() {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{}", cli::HELP);
            return;
        }
        Err(error) => {
            eprintln!("{error}\n{}", cli::HELP);
            std::process::exit(2);
        }
    };
    let workloads = Workloads::new();
    match options.gamut {
        cli::Gamut::DisplayP3 => run_gamut::<rgb_spaces::DisplayP3>(&options, &workloads),
        cli::Gamut::Srgb => run_gamut::<rgb_spaces::Srgb>(&options, &workloads),
        cli::Gamut::Rec2020 => run_gamut::<rgb_spaces::Rec2020>(&options, &workloads),
        cli::Gamut::All => {
            run_gamut::<rgb_spaces::DisplayP3>(&options, &workloads);
            run_gamut::<rgb_spaces::Srgb>(&options, &workloads);
            run_gamut::<rgb_spaces::Rec2020>(&options, &workloads);
        }
    }
}

struct Workloads {
    grid: Vec<[f64; 3]>,
    random: Vec<[f64; 3]>,
    grid32: Vec<[f32; 3]>,
    random32: Vec<[f32; 3]>,
}
impl Workloads {
    fn new() -> Self {
        let grid = build_grid();
        let random = build_random(grid.len());
        let grid32 = grid.iter().map(|c| c.map(|v| v as f32)).collect();
        let random32 = random.iter().map(|c| c.map(|v| v as f32)).collect();
        Self {
            grid,
            random,
            grid32,
            random32,
        }
    }
}

fn run_gamut<G>(options: &cli::Options, workloads: &Workloads)
where
    G: float64::gamut::RgbGamut + float32::gamut::RgbGamut + validation::ValidationProfile,
{
    let Workloads {
        grid,
        random,
        grid32,
        random32,
    } = workloads;
    let is_p3 = G::ID == rgb_spaces::SpaceId::DisplayP3;
    println!(
        "target: {} ({})",
        G::DEFINITION.name,
        if is_p3 {
            "all 13 methods"
        } else {
            "clip and css-minde"
        }
    );
    println!(
        "dataset: {} OKLCh colors per workload (grid + random)\n",
        grid.len()
    );
    println!("precisions: f64 and native f32 (inputs rounded before timing)\n");
    println!(
        "in-gamut precheck: {}\n",
        if options.check {
            "ENABLED (--in-gamut-check)"
        } else {
            "disabled (pass --in-gamut-check to enable)"
        }
    );
    float64::print_checksums::<G>(grid);
    if is_p3 {
        validate_p3_solvers(grid, random);
    }
    validation::validate_gamut::<G>(grid32, random32);
    float32::print_checksums::<G>(grid32);
    if options.validate_only {
        return;
    }

    // Use the historical P3 order for every target: all f64, then all f32.
    const GRID: &str = "grid (H = 0..359 step 1, repeated per L)";
    const RANDOM: &str = "random (stratified/jittered fractional H + L)";
    float64::run_timings::<G>(GRID, grid, 50, 25, options.check);
    float64::run_timings::<G>(RANDOM, random, 50, 25, options.check);
    float32::run_timings::<G>(GRID, grid32, 50, 25, options.check);
    float32::run_timings::<G>(RANDOM, random32, 50, 25, options.check);
}

fn validate_p3_solvers(grid: &[[f64; 3]], random: &[[f64; 3]]) {
    // Dualray uses intrinsic boundary checks in both modes.
    let mut dualray_diff: f64 = 0.0;
    for samples in [&grid, &random] {
        let mut solver = Dualray::new();
        let mut reference = OklchCubicDirect::new();
        dualray_diff = dualray_diff.max(max_channel_diff(
            samples,
            |c, o| solver.map(c, o),
            |c, o| reference.map(c, o),
        ));
    }
    assert!(
        dualray_diff <= 5e-8,
        "dualray/cubic-direct difference: {dualray_diff}"
    );
    println!(
        "equivalence: dualray/cubic-direct max channel diff {dualray_diff:.2e} (grid + random)\n"
    );

    // Equivalence across both workloads: the in-gamut-check fast path must match
    // the unchecked path for both methods.
    let mut max_diff: f64 = 0.0;
    for samples in [&grid, &random] {
        let mut cubic_eq = OklchCubic::new();
        let mut cubic_checked_eq = OklchCubic::new();
        let cubic_check_diff = max_channel_diff(
            samples,
            |c, o| cubic_eq.map(c, o),
            |c, o| cubic_checked_eq.map_with_in_gamut_check(c, o),
        );
        let mut cubic_no_cache_eq = OklchCubicNoCache::new();
        let mut cubic_no_cache_checked_eq = OklchCubicNoCache::new();
        let cubic_no_cache_check_diff = max_channel_diff(
            samples,
            |c, o| cubic_no_cache_eq.map(c, o),
            |c, o| cubic_no_cache_checked_eq.map_with_in_gamut_check(c, o),
        );
        let mut cubic_direct_eq = OklchCubicDirect::new();
        let mut cubic_direct_checked_eq = OklchCubicDirect::new();
        let cubic_direct_check_diff = max_channel_diff(
            samples,
            |c, o| cubic_direct_eq.map(c, o),
            |c, o| cubic_direct_checked_eq.map_with_in_gamut_check(c, o),
        );
        let mut halley_eq = OklchHalley::new();
        let mut halley_checked_eq = OklchHalley::new();
        let halley_check_diff = max_channel_diff(
            samples,
            |c, o| halley_eq.map(c, o),
            |c, o| halley_checked_eq.map_with_in_gamut_check(c, o),
        );
        let mut ostrowski_eq = OklchOstrowski::new();
        let mut ostrowski_checked_eq = OklchOstrowski::new();
        let ostrowski_check_diff = max_channel_diff(
            samples,
            |c, o| ostrowski_eq.map(c, o),
            |c, o| ostrowski_checked_eq.map_with_in_gamut_check(c, o),
        );
        let mut bottosson_eq = BottossonLightness::new();
        let mut bottosson_checked_eq = BottossonLightness::new();
        let bottosson_check_diff = max_channel_diff(
            samples,
            |c, o| bottosson_eq.map(c, o),
            |c, o| bottosson_checked_eq.map_with_in_gamut_check(c, o),
        );
        let mut bottosson_cached_eq = BottossonLightnessCached::new();
        let mut bottosson_cached_checked_eq = BottossonLightnessCached::new();
        let bottosson_cached_check_diff = max_channel_diff(
            samples,
            |c, o| bottosson_cached_eq.map(c, o),
            |c, o| bottosson_cached_checked_eq.map_with_in_gamut_check(c, o),
        );
        let mut raytrace_eq = Raytrace::new();
        let mut raytrace_checked_eq = Raytrace::new();
        let raytrace_check_diff = max_channel_diff(
            samples,
            |c, o| raytrace_eq.map(c, o),
            |c, o| raytrace_checked_eq.map_with_in_gamut_check(c, o),
        );
        let mut edge_eq = EdgeSeeker::new();
        let mut edge_checked_eq = EdgeSeeker::new();
        let edge_check_diff = max_channel_diff(
            samples,
            |c, o| edge_eq.map(c, o),
            |c, o| edge_checked_eq.map_with_in_gamut_check(c, o),
        );
        let mut edge_indexed_eq = EdgeSeekerIndexed::new();
        let mut edge_indexed_checked_eq = EdgeSeekerIndexed::new();
        let edge_indexed_check_diff = max_channel_diff(
            samples,
            |c, o| edge_indexed_eq.map(c, o),
            |c, o| edge_indexed_checked_eq.map_with_in_gamut_check(c, o),
        );
        max_diff = max_diff
            .max(cubic_check_diff)
            .max(cubic_no_cache_check_diff)
            .max(cubic_direct_check_diff)
            .max(halley_check_diff)
            .max(ostrowski_check_diff)
            .max(bottosson_check_diff)
            .max(bottosson_cached_check_diff)
            .max(raytrace_check_diff)
            .max(edge_check_diff)
            .max(edge_indexed_check_diff);
    }
    println!(
        "equivalence: unchecked/in-gamut-check max channel diff {} (grid + random)\n",
        max_diff
    );

    let mut cubic_no_cache_max_diff: f64 = 0.0;
    for samples in [&grid, &random] {
        let mut cubic_eq = OklchCubic::new();
        let mut cubic_no_cache_eq = OklchCubicNoCache::new();
        cubic_no_cache_max_diff = cubic_no_cache_max_diff.max(max_channel_diff(
            samples,
            |c, o| cubic_eq.map(c, o),
            |c, o| cubic_no_cache_eq.map(c, o),
        ));
    }
    if cubic_no_cache_max_diff > 1e-12 {
        panic!(
            "oklch-cubic no-cache differs from cached: max channel diff {}",
            cubic_no_cache_max_diff
        );
    }
    println!(
        "equivalence: oklch-cubic cached/no-cache max channel diff {} (grid + random)\n",
        cubic_no_cache_max_diff
    );

    let mut cubic_direct_eq = OklchCubicDirect::new();
    let mut cubic_exact_eq = OklchCubicNoCache::new();
    let cubic_direct_diff = max_channel_diff(
        &grid,
        |c, o| cubic_direct_eq.map(c, o),
        |c, o| cubic_exact_eq.map(c, o),
    );
    if cubic_direct_diff > 5e-8 {
        panic!(
            "oklch-cubic-direct differs from the exact cubic boundary: max channel diff {}",
            cubic_direct_diff
        );
    }
    println!(
        "equivalence: oklch-cubic-direct/cubic max channel diff {:.2e} (exact grid hues)\n",
        cubic_direct_diff
    );

    let mut halley_eq = OklchHalley::new();
    let mut cubic_exact_eq = OklchCubicNoCache::new();
    let halley_cubic_diff = max_channel_diff(
        &grid,
        |c, o| halley_eq.map(c, o),
        |c, o| cubic_exact_eq.map(c, o),
    );
    if halley_cubic_diff > 2e-8 {
        panic!(
            "oklch-halley differs from the exact cubic boundary: max channel diff {}",
            halley_cubic_diff
        );
    }
    println!(
        "equivalence: oklch-halley/cubic max channel diff {:.2e} (exact grid hues)\n",
        halley_cubic_diff
    );

    let mut ostrowski_eq = OklchOstrowski::new();
    let mut cubic_exact_eq = OklchCubicNoCache::new();
    let ostrowski_cubic_diff = max_channel_diff(
        &grid,
        |c, o| ostrowski_eq.map(c, o),
        |c, o| cubic_exact_eq.map(c, o),
    );
    if ostrowski_cubic_diff > 5e-8 {
        panic!(
            "oklch-ostrowski differs from the exact cubic boundary: max channel diff {}",
            ostrowski_cubic_diff
        );
    }
    println!(
        "equivalence: oklch-ostrowski/cubic max channel diff {:.2e} (exact grid hues)\n",
        ostrowski_cubic_diff
    );

    let mut cubic_direct_eq = OklchCubicDirect::new();
    let mut halley_exact_eq = OklchHalley::new();
    let direct_halley_diff = max_channel_diff(
        &random,
        |c, o| cubic_direct_eq.map(c, o),
        |c, o| halley_exact_eq.map(c, o),
    );
    if direct_halley_diff > 5e-8 {
        panic!(
            "oklch-cubic-direct differs from Halley on random exact hues: max channel diff {}",
            direct_halley_diff
        );
    }
    println!(
        "equivalence: oklch-cubic-direct/Halley max channel diff {:.2e} (random exact hues)\n",
        direct_halley_diff
    );

    // The cached bottosson variant evaluates the hue-dependent structure at the
    // 0.1° bucket hue: bucket-exact grid hues must match to float noise; random
    // fractional hues are bounded by the hue quantization.
    {
        let mut exact = BottossonLightness::new();
        let mut cached = BottossonLightnessCached::new();
        let grid_diff = max_channel_diff(&grid, |c, o| exact.map(c, o), |c, o| cached.map(c, o));
        if grid_diff > 1e-12 {
            panic!(
                "bottosson cached differs on bucket-exact grid hues: max channel diff {grid_diff}"
            );
        }
        let mut exact_r = BottossonLightness::new();
        let mut cached_r = BottossonLightnessCached::new();
        let random_diff =
            max_channel_diff(&random, |c, o| exact_r.map(c, o), |c, o| cached_r.map(c, o));
        if random_diff > 0.05 {
            panic!("bottosson cached exceeds the hue-quantization bound on random hues: max channel diff {random_diff}");
        }
        println!(
            "equivalence: bottosson cached/exact max channel diff {grid_diff} (grid, bucket-exact hues), {random_diff:.2e} (random, 0.1° hue quantization)\n"
        );
    }

    let mut indexed_max_diff: f64 = 0.0;
    for samples in [&grid, &random] {
        let mut edge_eq = EdgeSeeker::new();
        let mut edge_indexed_eq = EdgeSeekerIndexed::new();
        indexed_max_diff = indexed_max_diff.max(max_channel_diff(
            samples,
            |c, o| edge_eq.map(c, o),
            |c, o| edge_indexed_eq.map(c, o),
        ));
    }
    if indexed_max_diff != 0.0 {
        panic!(
            "edge-seeker indexed differs from edge-seeker: max channel diff {}",
            indexed_max_diff
        );
    }
    println!("equivalence: edge-seeker indexed max channel diff 0 (grid + random)\n");
}
