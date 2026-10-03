//! Dualray Fast prototype: accuracy against exact f64 Dualray for the
//! prototype, exact Dualray and the CSS methods, plus paired timing, in native
//! f64 and f32.
//!
//! RUSTFLAGS="-C target-cpu=native" cargo run --release --example dualray-fast -- \
//!     [accuracy|timing|all] [f64|f32 ...] [srgb|display-p3|rec2020 ...]
//! VERBOSE=1 prints every input set and the worst inputs.
#![allow(dead_code, unused_imports, unused_macros)]
#[path = "../src/dualray_config.rs"]
mod dualray_config;
#[path = "../src/rgb_spaces.rs"]
mod rgb_spaces;

mod float64 {
    type Float = f64;
    const SINGLE: bool = false;
    include!("../src/algorithms.rs");
    include!("support/dualray-fast-lane.rs");
}
mod float32 {
    type Float = f32;
    const SINGLE: bool = true;
    include!("../src/algorithms.rs");
    include!("support/dualray-fast-lane.rs");
}

use rgb_spaces::{RgbSpace, SpaceId};

trait Target: float64::Target + float32::Target {}
impl<G: float64::Target + float32::Target> Target for G {}

fn mulberry32(seed: u32) -> impl FnMut() -> f64 {
    let mut a = seed;
    move || {
        a = a.wrapping_add(0x6D2B_79F5);
        let mut t = (a ^ (a >> 15)).wrapping_mul(a | 1);
        t = (t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61))) ^ t;
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

// ── Workloads: grid and random match gma-bench exactly ─────────────────────
fn build_grid() -> Vec<[f64; 3]> {
    let mut samples = Vec::new();
    for li in (1..=99).rev() {
        for h in 0..360 {
            samples.push([li as f64 / 100.0, 0.4, h as f64]);
        }
    }
    samples
}
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
fn build_random(n: usize) -> Vec<[f64; 3]> {
    let rand_h = stratified_shuffled(n, 0.0, 360.0, 0x9e37_79b9);
    let rand_l = stratified_shuffled(n, 0.01, 0.98, 0x85eb_ca6b);
    (0..n).map(|i| [rand_l[i], 0.4, rand_h[i]]).collect()
}
// Same hues and lightnesses, chroma uniform in [0, 0.4): includes in-gamut colors.
fn build_mixed(n: usize) -> Vec<[f64; 3]> {
    let mut rand = mulberry32(0x2545_f491);
    build_random(n)
        .into_iter()
        .map(|[l, _, h]| [l, 0.4 * rand(), h])
        .collect()
}

// ── Accuracy ───────────────────────────────────────────────────────────────
fn decode<G: RgbSpace>(x: f64) -> f64 {
    if G::ID == SpaceId::Rec2020 {
        x.powf(2.4)
    } else if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}
fn lab<G: Target>(encoded: [f64; 3]) -> [f64; 3] {
    let o = float64::color::LinearRgb::<G>::new(encoded.map(decode::<G>)).to_oklab();
    [o.l, o.a, o.b]
}

// Log-binned ΔEOK histogram: 200 bins per decade from 1e-16 (about 1.2%).
const BINS_PER_DECADE: f64 = 200.0;
const BINS: usize = 16 * 200 + 1;
fn bin(x: f64) -> usize {
    if x <= 1e-16 {
        0
    } else {
        (((x.log10() + 16.0) * BINS_PER_DECADE).ceil() as usize).min(BINS - 1)
    }
}
fn bin_top(i: usize) -> f64 {
    if i == 0 {
        0.0
    } else {
        10f64.powf(i as f64 / BINS_PER_DECADE - 16.0)
    }
}

#[derive(Clone)]
struct Stats {
    outside: usize,
    inside: usize,
    histogram: Vec<u64>,
    de_max: f64,
    de_at: [f64; 3],
    byte_changed: usize,
    byte_max: i64,
    byte_at: [f64; 3],
    inside_mismatch: usize,
    inside_diff: f64,
    inside_at: [f64; 3],
    // Outputs outside [0, 1] or nonfinite.
    invalid: usize,
    invalid_example: ([f64; 3], [f64; 3]),
    // Largest encoded-channel error per outside color.
    encoded: Vec<u64>,
    enc_max: f64,
}
impl Default for Stats {
    fn default() -> Self {
        Self {
            outside: 0,
            inside: 0,
            histogram: vec![0; BINS],
            de_max: 0.0,
            de_at: [0.0; 3],
            byte_changed: 0,
            byte_max: 0,
            byte_at: [0.0; 3],
            inside_mismatch: 0,
            inside_diff: 0.0,
            inside_at: [0.0; 3],
            invalid: 0,
            invalid_example: ([0.0; 3], [0.0; 3]),
            encoded: vec![0; BINS],
            enc_max: 0.0,
        }
    }
}
impl Stats {
    // In-gamut colors (in the lane's precision) are compared with the lane's
    // canonical conversion, except in blue-fold windows, where Dualray's
    // first-exit policy can move colors in a re-entry island.
    fn add<G: Target>(
        &mut self,
        input: [f64; 3],
        want: [f64; 3],
        got: [f64; 3],
        canonical: Option<[f64; 3]>,
        fold: bool,
    ) {
        if !got.iter().all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0) {
            if self.invalid == 0 {
                self.invalid_example = (input, got);
            }
            self.invalid += 1;
        }
        if let Some(canonical) = canonical {
            if !fold {
                self.inside += 1;
                if got.map(f64::to_bits) != canonical.map(f64::to_bits) {
                    self.inside_mismatch += 1;
                    let d = (0..3)
                        .map(|k| (got[k] - canonical[k]).abs())
                        .fold(0.0, f64::max);
                    if d > self.inside_diff {
                        self.inside_diff = d;
                        self.inside_at = input;
                    }
                }
            }
            return;
        }
        self.outside += 1;
        let (a, b) = (lab::<G>(got), lab::<G>(want));
        let de = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        assert!(de.is_finite(), "non-finite ΔEOK at {input:?}");
        self.histogram[bin(de)] += 1;
        let enc = (0..3).map(|k| (got[k] - want[k]).abs()).fold(0.0, f64::max);
        self.encoded[bin(enc)] += 1;
        self.enc_max = self.enc_max.max(enc);
        if de > self.de_max {
            self.de_max = de;
            self.de_at = input;
        }
        let mut steps = 0;
        for k in 0..3 {
            steps = steps.max(((got[k] * 255.0).round() - (want[k] * 255.0).round()).abs() as i64);
        }
        if steps > 0 {
            self.byte_changed += 1;
        }
        if steps > self.byte_max {
            self.byte_max = steps;
            self.byte_at = input;
        }
    }
    fn p99(&self) -> f64 {
        Self::quantile(&self.histogram, self.outside)
    }
    fn enc_p99(&self) -> f64 {
        Self::quantile(&self.encoded, self.outside)
    }
    fn quantile(histogram: &[u64], n: usize) -> f64 {
        let target = (n as f64 * 0.99).ceil() as u64;
        let mut seen = 0;
        for (i, &count) in histogram.iter().enumerate() {
            seen += count;
            if seen >= target {
                return bin_top(i);
            }
        }
        0.0
    }
    fn byte_pct(&self) -> f64 {
        100.0 * self.byte_changed as f64 / self.outside.max(1) as f64
    }
}

const SETS: [&str; 4] = ["dense grid", "random", "near boundary", "dark/bright"];

// Every set's inputs, generated once in f64.
fn input_sets<G: Target>() -> Vec<Vec<[f64; 3]>> {
    let mut dense = Vec::new();
    for i in 0..300 {
        let l = (i as f64 + 0.5) / 300.0;
        for j in 0..3600 {
            let h = (j as f64 + 0.37) * 0.1;
            for c in [0.05, 0.1, 0.2, 0.3, 0.4] {
                dense.push([l, c, h]);
            }
        }
    }
    let mut rand = mulberry32(0x1234_5678);
    let random = (0..2_000_000)
        .map(|_| [rand(), 0.5 * rand(), 360.0 * rand()])
        .collect();
    // Exact boundary chroma C* from mapping C = 1, then both sides of it.
    let mut exact = float64::dualray::Dualray::<G>::new();
    let mut near = Vec::new();
    for _ in 0..200_000 {
        let (l, h) = (0.001 + 0.998 * rand(), 360.0 * rand());
        let mut boundary = [0.0; 3];
        exact.map(&[l, 1.0, h], &mut boundary);
        let o = lab::<G>(boundary);
        let c_star = o[1].hypot(o[2]);
        for d in [-1e-2, -1e-3, -1e-4, -1e-6, 1e-6, 1e-4, 1e-3, 1e-2] {
            near.push([l, c_star * (1.0 + d), h]);
        }
    }
    let ends = (0..500_000)
        .map(|_| {
            let r = rand();
            let l = if rand() < 0.5 {
                1e-6 + 0.02 * r
            } else {
                1.0 - 1e-9 - 0.02 * r
            };
            [l, 0.5 * rand(), 360.0 * rand()]
        })
        .collect();
    vec![dense, random, near, ends]
}

fn accuracy<G: Target>(name: &str, lanes: &[&str]) {
    let sets = input_sets::<G>();
    for &lane in lanes {
        let single = lane == "f32";
        let mut reference = float64::dualray::Dualray::<G>::new();
        let mut m64 = float64::Mappers::<G>::new();
        let mut m32 = float32::Mappers::<G>::new();
        let mut stats = vec![vec![Stats::default(); float64::NAMES.len()]; SETS.len()];
        let mut hits = vec![0usize; SETS.len()];
        for (s, set) in sets.iter().enumerate() {
            for &input in set {
                // f32 inputs are rounded once; the reference maps the rounded input.
                let input = if single {
                    input.map(|v| v as f32 as f64)
                } else {
                    input
                };
                let mut want = [0.0; 3];
                reference.map(&input, &mut want);
                // Each lane has its own result type.
                let (outputs, canonical, hit) = if single {
                    let m = m32.map_all(input);
                    (m.outputs, m.canonical, m.hit)
                } else {
                    let m = m64.map_all(input);
                    (m.outputs, m.canonical, m.hit)
                };
                let fold = float64::rgb_solvers::in_blue_fold::<G>(input[2]);
                hits[s] += hit as usize;
                for k in 0..float64::NAMES.len() {
                    stats[s][k].add::<G>(input, want, outputs[k], canonical, fold);
                }
            }
        }
        println!("\n{name} / {lane}: deviation from the constant-L/h first exit (exact f64 Dualray at the {lane} input)");
        println!("| method | ΔEOK p99 (random) | ΔEOK max (all sets) | 8-bit changed (random) | largest 8-bit change | in-gamut not canonical |");
        println!("|---|---:|---:|---:|---:|---:|");
        for k in 0..float64::NAMES.len() {
            let random = &stats[1][k];
            let de_max = (0..SETS.len())
                .map(|s| stats[s][k].de_max)
                .fold(0.0, f64::max);
            let byte_max = (0..SETS.len()).map(|s| stats[s][k].byte_max).max().unwrap();
            let mismatch: usize = (0..SETS.len()).map(|s| stats[s][k].inside_mismatch).sum();
            let inside: usize = (0..SETS.len()).map(|s| stats[s][k].inside).sum();
            let diff = (0..SETS.len())
                .map(|s| stats[s][k].inside_diff)
                .fold(0.0, f64::max);
            println!(
                "| {} | {:.1e} | {:.1e} | {:.2}% | {} | {} of {}{} |",
                float64::NAMES[k],
                random.p99(),
                de_max,
                random.byte_pct(),
                byte_max,
                mismatch,
                inside,
                if mismatch > 0 {
                    format!(" (max {diff:.1e})")
                } else {
                    String::new()
                }
            );
        }
        for k in 0..float64::NAMES.len() {
            let invalid: usize = (0..SETS.len()).map(|s| stats[s][k].invalid).sum();
            if invalid > 0 {
                let (input, got) = (0..SETS.len())
                    .map(|s| &stats[s][k])
                    .find(|st| st.invalid > 0)
                    .unwrap()
                    .invalid_example;
                println!(
                    "  {}: {invalid} outputs outside [0, 1], e.g. {input:?} -> {got:?}",
                    float64::NAMES[k]
                );
            }
        }
        if std::env::var("VERBOSE").is_ok() {
            for (s, set_name) in SETS.iter().enumerate() {
                println!(
                    "  {set_name}: {} inputs, {:.1}% outside, shortcut {:.1}%",
                    sets[s].len(),
                    100.0 * stats[s][0].outside as f64 / sets[s].len() as f64,
                    100.0 * hits[s] as f64 / sets[s].len() as f64
                );
                for k in 0..float64::NAMES.len() {
                    let st = &stats[s][k];
                    println!(
                        "    {:<20} p99 {:.1e} max {:.1e} at {:?}; 8-bit {:.2}%, max {} at {:?}; inside mismatch {} (max {:.1e} at {:?})",
                        float64::NAMES[k], st.p99(), st.de_max, st.de_at, st.byte_pct(),
                        st.byte_max, st.byte_at, st.inside_mismatch, st.inside_diff, st.inside_at
                    );
                }
            }
        }
    }
}

// ── Timing ─────────────────────────────────────────────────────────────────
fn timing<G: Target>(name: &str, lanes: &[&str]) {
    let grid = build_grid();
    let n = grid.len();
    let workloads = [
        ("grid", grid),
        ("random", build_random(n)),
        ("mixed", build_mixed(n)),
    ];
    for &lane in lanes {
        println!("\n{name} / {lane}: ns/color, median of 8 rounds (each the median of 25 passes), rotating order");
        for (label, samples) in &workloads {
            let ([e, f, b, t], hit) = if lane == "f32" {
                float32::timing::<G>(samples)
            } else {
                float64::timing::<G>(samples)
            };
            let pct = |x: f64| 100.0 * (x / e - 1.0);
            println!(
                "  {label:<7} dualray {e:>6.2} | fast {f:>6.2} ({:+.1}%) | fast+encode {b:>6.2} ({:+.1}%) | fast+tables {t:>6.2} ({:+.1}%) | shortcut {:.1}%",
                pct(f),
                pct(b),
                pct(t),
                100.0 * hit
            );
        }
    }
}

// ── Independent reference: gma-accuracy's oracle via scripts/dualray-fast-oracle.py ──
fn write_rows<const N: usize>(path: &str, rows: &[[f64; N]]) {
    let bytes: Vec<u8> = rows
        .iter()
        .flat_map(|r| r.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    std::fs::write(path, bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
}
fn read_rows<const N: usize>(path: &str) -> Vec<[f64; N]> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!(bytes.len() % (8 * N), 0, "{path}: truncated");
    bytes
        .chunks_exact(8 * N)
        .map(|row| {
            std::array::from_fn(|i| f64::from_le_bytes(row[8 * i..8 * i + 8].try_into().unwrap()))
        })
        .collect()
}

// Inputs for the oracle: samples of the accuracy sets, sector seams and fold
// edges, and the prototype's worst inputs from the full sweep in this lane.
fn export<G: Target>(name: &str, dir: &str) {
    std::fs::write(
        format!("{dir}/{name}.profile.json"),
        float64::profile_json::<G>(),
    )
    .unwrap();
    let sets = input_sets::<G>();
    for lane in ["f64", "f32"] {
        let single = lane == "f32";
        let round = |v: [f64; 3]| {
            if single {
                v.map(|x| x as f32 as f64)
            } else {
                v
            }
        };
        let mut rows: Vec<[f64; 3]> = Vec::new();
        let mut rand = mulberry32(0xfeed_beef);
        for _ in 0..30_000 {
            rows.push([rand(), 0.5 * rand(), 360.0 * rand()]);
        }
        rows.extend_from_slice(&sets[2][..32_000]);
        rows.extend_from_slice(&sets[3][..5_000]);
        for edge in float64::seams::<G>() {
            for d in [
                -0.1, -0.01, -1e-3, -1e-6, -1e-9, 1e-9, 1e-6, 1e-3, 0.01, 0.1,
            ] {
                for i in 0..50 {
                    for c in [0.2, 0.4] {
                        rows.push([(i as f64 + 0.5) / 50.0, c, edge + d]);
                    }
                }
            }
        }
        let mut reference = float64::dualray::Dualray::<G>::new();
        let mut f64_fast = float64::FastOnly::<G>::new();
        let mut f32_fast = float32::FastOnly::<G>::new();
        let mut worst: Vec<(f64, f64, [f64; 3])> = Vec::new();
        for set in &sets {
            for &input in set {
                let input = round(input);
                let mut want = [0.0; 3];
                reference.map(&input, &mut want);
                let got = if single {
                    f32_fast.map(input)
                } else {
                    f64_fast.map(input)
                };
                if got == want {
                    continue;
                }
                let (a, b) = (lab::<G>(got), lab::<G>(want));
                let de =
                    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                let enc = (0..3).map(|k| (got[k] - want[k]).abs()).fold(0.0, f64::max);
                if de > 3e-5 || enc > 1e-3 {
                    worst.push((de, enc, input));
                }
            }
        }
        worst.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        rows.extend(worst.iter().take(300).map(|w| w.2));
        worst.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        rows.extend(worst.iter().take(100).map(|w| w.2));
        let rows: Vec<[f64; 3]> = rows.into_iter().map(round).collect();
        let path = format!("{dir}/{name}-{lane}.in");
        write_rows(&path, &rows);
        println!("{path}: {} inputs", rows.len());
    }
}

// Scores every method against certified references: rows of L, C, h,
// membership (0 inside, 1 outside, 2 endpoint, 3 unresolved) and the
// reference's encoded output.
fn check<G: Target>(label: &str, path: &str, lane: &str) {
    let rows = read_rows::<7>(path);
    let single = lane == "f32";
    let mut m64 = float64::Mappers::<G>::new();
    let mut m32 = float32::Mappers::<G>::new();
    let mut stats = vec![Stats::default(); float64::NAMES.len()];
    // Inside: max difference from the reference conversion, 8-bit changes;
    // blue-fold hues separately (Dualray's first-exit policy).
    let mut inside = [(0usize, 0.0f64, 0usize); float64::NAMES.len()];
    let mut inside_at = [[0.0f64; 3]; float64::NAMES.len()];
    let mut fold_changed = [0usize; float64::NAMES.len()];
    let (mut n_inside, mut n_fold, mut skipped) = (0usize, 0usize, 0usize);
    for row in &rows {
        let input = [row[0], row[1], row[2]];
        let reference = [row[4], row[5], row[6]];
        if row[3] >= 2.0 {
            skipped += 1;
            continue;
        }
        let outputs = if single {
            m32.map_all(input).outputs
        } else {
            m64.map_all(input).outputs
        };
        if row[3] == 0.0 {
            let fold = float64::rgb_solvers::in_blue_fold::<G>(input[2]);
            if fold {
                n_fold += 1;
            } else {
                n_inside += 1;
            }
            for k in 0..float64::NAMES.len() {
                let diff = (0..3)
                    .map(|c| (outputs[k][c] - reference[c]).abs())
                    .fold(0.0, f64::max);
                let bytes = (0..3)
                    .any(|c| (outputs[k][c] * 255.0).round() != (reference[c] * 255.0).round());
                if fold {
                    // f32 conversion rounding alone reaches about 1e-5 encoded.
                    let moved = diff > if single { 1e-4 } else { 1e-6 };
                    fold_changed[k] += moved as usize;
                    if moved && std::env::var("VERBOSE").is_ok() {
                        println!(
                            "  fold-window in-gamut color moved by {}: {input:?} -> {:?} (reference {reference:?})",
                            float64::NAMES[k], outputs[k]
                        );
                    }
                } else {
                    if diff > inside[k].1 {
                        inside[k].1 = diff;
                        inside_at[k] = input;
                    }
                    inside[k].2 += bytes as usize;
                }
            }
            continue;
        }
        for k in 0..float64::NAMES.len() {
            stats[k].add::<G>(input, reference, outputs[k], None, false);
        }
    }
    println!(
        "\n{label} / {lane}: {} certified outside, {n_inside} inside, {n_fold} inside in blue-fold windows, {skipped} endpoint or unresolved",
        stats[0].outside
    );
    println!("| method | ΔEOK p99 | ΔEOK max | encoded p99 | encoded max | 8-bit changed | largest 8-bit change | inside: max diff / 8-bit changes |");
    println!("|---|---:|---:|---:|---:|---:|---:|---:|");
    for k in 0..float64::NAMES.len() {
        let st = &stats[k];
        println!(
            "| {} | {:.1e} | {:.1e} | {:.1e} | {:.1e} | {:.2}% | {} | {:.1e} / {} |",
            float64::NAMES[k],
            st.p99(),
            st.de_max,
            st.enc_p99(),
            st.enc_max,
            st.byte_pct(),
            st.byte_max,
            inside[k].1,
            inside[k].2
        );
    }
    if n_fold > 0 {
        let changed: Vec<String> = (0..float64::NAMES.len())
            .map(|k| format!("{} {}", float64::NAMES[k], fold_changed[k]))
            .collect();
        println!(
            "  blue-fold inside colors moved (> {}): {}",
            if single { "1e-4" } else { "1e-6" },
            changed.join(", ")
        );
    }
    for k in 0..float64::NAMES.len() {
        if stats[k].invalid > 0 {
            println!(
                "  {}: {} outputs outside [0, 1]",
                float64::NAMES[k],
                stats[k].invalid
            );
        }
    }
    if std::env::var("VERBOSE").is_ok() {
        for k in 0..float64::NAMES.len() {
            println!(
                "  {:<20} worst ΔEOK at {:?}; largest 8-bit change at {:?}; largest in-gamut change at {:?}",
                float64::NAMES[k], stats[k].de_at, stats[k].byte_at, inside_at[k]
            );
        }
    }
}

// How often each path is taken, per input set and workload (f64 lane).
fn paths<G: Target>(name: &str) {
    let grid = build_grid();
    let n = grid.len();
    let mut sets: Vec<(&str, Vec<[f64; 3]>)> = vec![
        ("grid", grid),
        ("random C=0.4", build_random(n)),
        ("mixed", build_mixed(n)),
    ];
    for (label, set) in SETS.iter().zip(input_sets::<G>()) {
        sets.push((label, set));
    }
    println!("\n{name}: share of colors per path (f64)");
    for (label, set) in &sets {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for input in set {
            let p = format!(
                "{:?}",
                float64::dualray_fast::DualrayFast::<G, false>::path(input)
            );
            match counts.iter_mut().find(|(k, _)| *k == p) {
                Some((_, c)) => *c += 1,
                None => counts.push((p, 1)),
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1));
        let parts: Vec<String> = counts
            .iter()
            .map(|(k, c)| format!("{k} {:.3}%", 100.0 * *c as f64 / set.len() as f64))
            .collect();
        println!("  {label:<14} {}", parts.join(" | "));
    }
}

// One input in both lanes: path, output and canonical membership.
fn probe<G: Target>(input: [f64; 3]) {
    let mut out = [0.0f64; 3];
    let path64 = float64::dualray_fast::DualrayFast::<G>::map_path(&input, &mut out);
    let x32 = input.map(|v| v as f32);
    let mut out32 = [0.0f32; 3];
    let path32 = float32::dualray_fast::DualrayFast::<G>::map_path(&x32, &mut out32);
    let in64 = float64::color::Oklch::from(input).to_oklab().to_linear_rgb::<G>();
    let in32 = float32::color::Oklch::from(x32).to_oklab().to_linear_rgb::<G>();
    let mut exact = [0.0f64; 3];
    float64::dualray::Dualray::<G>::new().map(&input, &mut exact);
    println!("f64 {path64:?} {out:?} canonical inside {} linear {:?}", in64.in_gamut(), in64.channels);
    println!("f32 {path32:?} {out32:?} canonical inside {} linear {:?}", in32.in_gamut(), in32.channels);
    println!("exact f64 Dualray {exact:?}; hue reduced f64 {} f32 {}", input[2].rem_euclid(360.0), x32[2].rem_euclid(360.0));
}

fn run<G: Target>(name: &str, mode: &str, lanes: &[&str]) {
    if mode == "paths" {
        paths::<G>(name);
        return;
    }
    if mode == "accuracy" || mode == "all" {
        accuracy::<G>(name, lanes);
    }
    if mode == "timing" || mode == "all" {
        timing::<G>(name, lanes);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("all");
    assert!(
        matches!(
            mode,
            "accuracy" | "timing" | "all" | "export" | "check" | "paths" | "probe"
        ),
        "mode: accuracy|timing|all|paths|export <dir>|check <dir>|probe <gamut> L C h"
    );
    let (dir, skip) = if matches!(mode, "export" | "check" | "probe") {
        (args.get(1).expect("directory").clone(), 2)
    } else {
        (String::new(), 1)
    };
    let rest: Vec<&str> = args.iter().skip(skip).map(String::as_str).collect();
    let mut lanes: Vec<&str> = rest
        .iter()
        .copied()
        .filter(|a| matches!(*a, "f64" | "f32"))
        .collect();
    let mut gamuts: Vec<&str> = rest
        .iter()
        .copied()
        .filter(|a| !matches!(*a, "f64" | "f32"))
        .collect();
    if lanes.is_empty() {
        lanes = vec!["f64", "f32"];
    }
    if gamuts.is_empty() {
        gamuts = vec!["srgb", "display-p3", "rec2020"];
    }
    if mode == "probe" {
        let v: Vec<f64> = args[2..5].iter().map(|a| a.parse().unwrap()).collect();
        let input = [v[0], v[1], v[2]];
        match args[1].as_str() {
            "srgb" => probe::<rgb_spaces::Srgb>(input),
            "display-p3" => probe::<rgb_spaces::DisplayP3>(input),
            "rec2020" => probe::<rgb_spaces::Rec2020>(input),
            g => panic!("unknown gamut {g}"),
        }
        return;
    }
    if mode == "export" {
        for gamut in gamuts {
            match gamut {
                "srgb" => export::<rgb_spaces::Srgb>(gamut, &dir),
                "display-p3" => export::<rgb_spaces::DisplayP3>(gamut, &dir),
                "rec2020" => export::<rgb_spaces::Rec2020>(gamut, &dir),
                _ => panic!("unknown argument {gamut}"),
            }
        }
        return;
    }
    if mode == "check" {
        for &lane in &lanes {
            // gma-accuracy's own standard corpus and certified references (P3).
            let standard = format!("{dir}/standard-{lane}.ref");
            if std::path::Path::new(&standard).exists() {
                check::<rgb_spaces::DisplayP3>(
                    "display-p3 gma-accuracy standard corpus",
                    &standard,
                    lane,
                );
            }
            for &gamut in &gamuts {
                let path = format!("{dir}/{gamut}-{lane}.ref");
                if !std::path::Path::new(&path).exists() {
                    continue;
                }
                let label = format!("{gamut} prototype sample");
                match gamut {
                    "srgb" => check::<rgb_spaces::Srgb>(&label, &path, lane),
                    "display-p3" => check::<rgb_spaces::DisplayP3>(&label, &path, lane),
                    "rec2020" => check::<rgb_spaces::Rec2020>(&label, &path, lane),
                    _ => panic!("unknown argument {gamut}"),
                }
            }
        }
        return;
    }
    for gamut in gamuts {
        match gamut {
            "srgb" => run::<rgb_spaces::Srgb>(gamut, mode, &lanes),
            "display-p3" => run::<rgb_spaces::DisplayP3>(gamut, mode, &lanes),
            "rec2020" => run::<rgb_spaces::Rec2020>(gamut, mode, &lanes),
            _ => panic!("unknown argument {gamut}"),
        }
    }
}
