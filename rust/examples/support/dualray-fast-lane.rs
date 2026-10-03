// Included once per precision module by examples/dualray-fast.rs. All mapping
// arithmetic uses that module's Float; outputs are widened only for analysis.

pub(crate) const NAMES: [&str; 7] = [
    "dualray",
    "dualray fast",
    "dualray fast+encode",
    "dualray fast+tables",
    "css-minde",
    "raytrace",
    "edge-seeker",
];

// The prototype needs only DualrayFastData; exact Dualray and Edge Seeker
// are the comparisons.
pub(crate) trait Target:
    dualray_fast::DualrayFastData + dualray::DualrayData + edge_seeker::EdgeSeekerData
{
}
impl<G: dualray_fast::DualrayFastData + dualray::DualrayData + edge_seeker::EdgeSeekerData> Target
    for G
{
}

pub(crate) struct Mapped {
    pub outputs: [[f64; 3]; 7],
    // The lane's canonical conversion when it is in gamut in this precision.
    pub canonical: Option<[f64; 3]>,
    pub hit: bool,
}

pub(crate) struct Mappers<G: Target> {
    dualray: dualray::Dualray<G>,
    fast: dualray_fast::DualrayFast<G, false>,
    fast_encode: dualray_fast::DualrayFast<G, true>,
    fast_tables: dualray_fast::DualrayFastTables<G>,
    minde: css_minde::CssMinde<G>,
    raytrace: rgb_solvers::Raytrace<G>,
    edge: edge_seeker::EdgeSeeker<G>,
}

impl<G: Target> Mappers<G> {
    pub(crate) fn new() -> Self {
        Self {
            dualray: dualray::Dualray::new(),
            fast: dualray_fast::DualrayFast::new(),
            fast_encode: dualray_fast::DualrayFast::new(),
            fast_tables: dualray_fast::DualrayFastTables::new(),
            minde: css_minde::CssMinde::new(),
            raytrace: rgb_solvers::Raytrace::new(),
            edge: edge_seeker::EdgeSeeker::new(),
        }
    }

    // Methods with a precheck variant use it: in-gamut colors must pass through.
    pub(crate) fn map_all(&mut self, input: [f64; 3]) -> Mapped {
        let x: [Float; 3] = input.map(|v| v as Float);
        let mut o = [[0.0 as Float; 3]; NAMES.len()];
        self.dualray.map(&x, &mut o[0]);
        self.fast.map(&x, &mut o[1]);
        self.fast_encode.map(&x, &mut o[2]);
        self.fast_tables.map(&x, &mut o[3]);
        self.minde.map_with_in_gamut_check(&x, &mut o[4]);
        self.raytrace.map_with_in_gamut_check(&x, &mut o[5]);
        self.edge.map_with_in_gamut_check(&x, &mut o[6]);
        let rgb = color::Oklch::from(x).to_oklab().to_linear_rgb::<G>();
        let canonical = rgb
            .in_gamut()
            .then(|| rgb.encode_clamped().channels.map(f64::from));
        let hit = dualray_fast::DualrayFast::<G>::path(&x) == dualray_fast::Path::Lower;
        Mapped {
            outputs: o.map(|c| c.map(f64::from)),
            canonical,
            hit,
        }
    }
}

fn time_pass(samples: &[[Float; 3]], mut map: impl FnMut(&[Float; 3], &mut [Float; 3])) -> f64 {
    let mut pass = || {
        let mut out = [0.0 as Float; 3];
        let mut sum = 0.0f64;
        for input in std::hint::black_box(samples) {
            map(input, &mut out);
            sum += f64::from(out[0]) + f64::from(out[1]) + f64::from(out[2]);
        }
        sum
    };
    for _ in 0..50 {
        std::hint::black_box(pass());
    }
    let mut times: Vec<f64> = (0..25)
        .map(|_| {
            let t0 = std::time::Instant::now();
            std::hint::black_box(pass());
            t0.elapsed().as_nanos() as f64 / samples.len() as f64
        })
        .collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    times[times.len() / 2]
}

// Median ns/color for exact Dualray, Fast, Fast+encode and Fast+tables over
// eight rounds in rotating order, and the shortcut's hit rate.
pub(crate) fn timing<G: Target>(samples: &[[f64; 3]]) -> ([f64; 4], f64) {
    let samples: Vec<[Float; 3]> = samples.iter().map(|s| s.map(|v| v as Float)).collect();
    let hits = samples
        .iter()
        .filter(|s| dualray_fast::DualrayFast::<G>::path(s) == dualray_fast::Path::Lower)
        .count();
    let mut t = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for round in 0..8 {
        let mut exact = dualray::Dualray::<G>::new();
        let mut fast = dualray_fast::DualrayFast::<G, false>::new();
        let mut encode = dualray_fast::DualrayFast::<G, true>::new();
        let mut tables = dualray_fast::DualrayFastTables::<G>::new();
        for k in 0..4 {
            let which = (k + round) % 4;
            t[which].push(match which {
                0 => time_pass(&samples, |c, o| exact.map(c, o)),
                1 => time_pass(&samples, |c, o| fast.map(c, o)),
                2 => time_pass(&samples, |c, o| encode.map(c, o)),
                _ => time_pass(&samples, |c, o| tables.map(c, o)),
            });
        }
    }
    let median = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };
    (t.map(median), hits as f64 / samples.len() as f64)
}

// Exact binary64 bits of this lane's conversion matrices, for the oracle bridge.
pub(crate) fn profile_json<G: Target>() -> String {
    let matrix = |rows: [[Float; 3]; 3]| {
        let rows: Vec<String> = rows
            .iter()
            .map(|r| {
                let cells: Vec<String> = r
                    .iter()
                    .map(|&x| format!("\"{:016x}\"", f64::from(x).to_bits()))
                    .collect();
                format!("[{}]", cells.join(","))
            })
            .collect();
        format!("[{}]", rows.join(","))
    };
    let oklab_to_lms = [
        [1.0, color::KA0, color::KB0],
        [1.0, color::KA1, color::KB1],
        [1.0, color::KA2, color::KB2],
    ];
    format!(
        "{{\"oklab_to_lms_prime\":{},\"lms_to_linear\":{},\"linear_to_lms\":{}}}",
        matrix(oklab_to_lms),
        matrix(G::LMS_TO_RGB),
        matrix(G::RGB_TO_LMS)
    )
}

// The full prototype only, for locating its worst inputs.
pub(crate) struct FastOnly<G: Target>(dualray_fast::DualrayFast<G, false>);
impl<G: Target> FastOnly<G> {
    pub(crate) fn new() -> Self {
        Self(dualray_fast::DualrayFast::new())
    }
    pub(crate) fn map(&mut self, input: [f64; 3]) -> [f64; 3] {
        let mut out = [0.0 as Float; 3];
        self.0.map(&input.map(|v| v as Float), &mut out);
        out.map(f64::from)
    }
}

// Shortcut sector edges and the red fold, in degrees.
pub(crate) fn seams<G: Target>() -> [f64; 5] {
    use dualray_fast::DualrayFastData;
    [G::BLUE_START, G::RED_START, G::RED_END, G::GREEN_START, G::RED_FOLD].map(f64::from)
}
