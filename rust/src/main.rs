// Scalar (one-color-at-a-time) Rust port of the gma-benchmark methods.
//
// Apples-to-apples with the JS benchmark: same algorithms, same conversion
// math, same 35,640-color workloads (the canonical grid plus a random
// hue/lightness workload), one color per call. Built to anchor the
// native-vs-JS comparison.

use std::hint::black_box;
use std::time::Instant;

mod lut;
use lut::LUT;

const PI: f64 = std::f64::consts::PI;

// ── OKLab → LMS' (a,b columns; the L column is all 1s) ──
const KA0: f64 = 0.3963377773761749;
const KB0: f64 = 0.2158037573099136;
const KA1: f64 = -0.1055613458156586;
const KB1: f64 = -0.0638541728258133;
const KA2: f64 = -0.0894841775298119;
const KB2: f64 = -1.2914855480194092;

// ── LMS (cubed) → linear Display-P3 ──
const RL: f64 = 3.127768971361874;
const RM: f64 = -2.2571357625916395;
const RS: f64 = 0.12936679122976516;
const GL: f64 = -1.0910090184377979;
const GM: f64 = 2.413331710306922;
const GS: f64 = -0.32232269186912466;
const BL: f64 = -0.02601080193857028;
const BM: f64 = -0.508041331704167;
const BS: f64 = 1.5340521336427373;

// ── Bottosson Display-P3 cusp approximation constants ──
const BOTTOSSON_EPSILON: f64 = 1e-12;
const P3_RED1: f64 = -1.772343927512981;
const P3_RED2: f64 = -0.8207587433674072;
const P3_GREEN1: f64 = 1.8031987175305495;
const P3_GREEN2: f64 = -1.1932813966558915;

const P3_RED_K0: f64 = 1.1941401817282744;
const P3_RED_K1: f64 = 1.7629811997119493;
const P3_RED_K2: f64 = 0.5958599382477117;
const P3_RED_K3: f64 = 0.7575999740542505;
const P3_RED_K4: f64 = 0.5681684967813678;
const P3_GREEN_K0: f64 = 0.7395668192259771;
const P3_GREEN_K1: f64 = -0.45954279991477065;
const P3_GREEN_K2: f64 = 0.08285308768965816;
const P3_GREEN_K3: f64 = 0.1254116495192955;
const P3_GREEN_K4: f64 = -0.14503290744357106;
const P3_BLUE_K0: f64 = 1.3650944117698118;
const P3_BLUE_K1: f64 = -0.013962295571040945;
const P3_BLUE_K2: f64 = -1.1452305089885595;
const P3_BLUE_K3: f64 = -0.5025987876721942;
const P3_BLUE_K4: f64 = 0.003174713114731378;

// ── Raytrace constants ──
const RAYTRACE_EPSILON: f64 = 1e-12;
const RAYTRACE_LOW: f64 = 1e-12;
const RAYTRACE_HIGH: f64 = 1.0 - RAYTRACE_LOW;

// ── sRGB / Display-P3 transfer function, clamped to [0,1] ──
#[inline(always)]
fn clamped_gamma(x: f64) -> f64 {
    let x = if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else {
        x
    };
    if x <= 0.0031308 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

#[inline(always)]
fn oklab_to_clipped_p3(l: f64, a: f64, b: f64, out: &mut [f64; 3]) {
    let l_ = (l + KA0 * a + KB0 * b).powi(3);
    let m_ = (l + KA1 * a + KB1 * b).powi(3);
    let s_ = (l + KA2 * a + KB2 * b).powi(3);
    out[0] = clamped_gamma(RL * l_ + RM * m_ + RS * s_);
    out[1] = clamped_gamma(GL * l_ + GM * m_ + GS * s_);
    out[2] = clamped_gamma(BL * l_ + BM * m_ + BS * s_);
}

#[inline(always)]
fn oklch_to_clipped_p3(l: f64, c: f64, h: f64, out: &mut [f64; 3]) {
    let hr = h * PI / 180.0;
    oklab_to_clipped_p3(l, c * hr.cos(), c * hr.sin(), out);
}

#[inline(always)]
fn oklch_to_p3_if_in_gamut(l: f64, c: f64, h: f64, out: &mut [f64; 3]) -> bool {
    let hr = h * PI / 180.0;
    let a = c * hr.cos();
    let b = c * hr.sin();
    let l_ = (l + KA0 * a + KB0 * b).powi(3);
    let m_ = (l + KA1 * a + KB1 * b).powi(3);
    let s_ = (l + KA2 * a + KB2 * b).powi(3);
    let r = RL * l_ + RM * m_ + RS * s_;
    let g = GL * l_ + GM * m_ + GS * s_;
    let bl = BL * l_ + BM * m_ + BS * s_;
    if r < 0.0 || r > 1.0 || g < 0.0 || g > 1.0 || bl < 0.0 || bl > 1.0 {
        return false;
    }
    out[0] = clamped_gamma(r);
    out[1] = clamped_gamma(g);
    out[2] = clamped_gamma(bl);
    true
}

#[inline(always)]
fn clamp01(x: f64) -> f64 {
    if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else {
        x
    }
}

#[inline(always)]
fn oklab_to_linear_p3_components(l: f64, a: f64, b: f64) -> (f64, f64, f64) {
    let l_ = l + KA0 * a + KB0 * b;
    let m_ = l + KA1 * a + KB1 * b;
    let s_ = l + KA2 * a + KB2 * b;
    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;
    (
        RL * l3 + RM * m3 + RS * s3,
        GL * l3 + GM * m3 + GS * s3,
        BL * l3 + BM * m3 + BS * s3,
    )
}

#[inline(always)]
fn oklab_to_clipped_p3_fast(l: f64, a: f64, b: f64, out: &mut [f64; 3]) {
    let (r, g, bl) = oklab_to_linear_p3_components(l, a, b);
    out[0] = clamped_gamma(r);
    out[1] = clamped_gamma(g);
    out[2] = clamped_gamma(bl);
}

#[inline(always)]
fn oklab_to_p3_if_in_gamut(l: f64, a: f64, b: f64, out: &mut [f64; 3]) -> bool {
    let (r, g, bl) = oklab_to_linear_p3_components(l, a, b);
    if r < 0.0 || r > 1.0 || g < 0.0 || g > 1.0 || bl < 0.0 || bl > 1.0 {
        return false;
    }
    out[0] = clamped_gamma(r);
    out[1] = clamped_gamma(g);
    out[2] = clamped_gamma(bl);
    true
}

#[inline(always)]
fn linear_p3_to_oklab_chroma(r: f64, g: f64, b: f64) -> f64 {
    let l = (0.4813798527499543 * r + 0.4621183710113182 * g + 0.05650177623872754 * b).cbrt();
    let m = (0.2288319418112447 * r + 0.6532168193835677 * g + 0.11795123880518772 * b).cbrt();
    let s = (0.08394575232299314 * r + 0.22416527097756647 * g + 0.6918889766994405 * b).cbrt();
    let a = 1.9779985324311684 * l - 2.4285922420485799 * m + 0.4505937096174110 * s;
    let lab_b = 0.0259040424655478 * l + 0.7827717124575296 * m - 0.8086757549230774 * s;
    (a * a + lab_b * lab_b).sqrt()
}

// Exit distance of a ray from a point strictly inside the unit box:
// min over axes of max((1 - a) / d, -a / d). The raytrace anchor always
// satisfies "strictly inside" (it starts at gray (L^3, L^3, L^3) with
// 0 < L < 1, and anchor updates are gated on RAYTRACE_LOW..RAYTRACE_HIGH),
// so the slab method always resolves to this exit distance. |d| <= epsilon
// is flushed to 0 so that axis contributes max(-inf, +inf) = +inf (no
// constraint), matching the slab method's parallel-axis skip; if all three
// axes are parallel the result is +inf, which the caller treats as "no hit".
#[inline(always)]
fn exit_t(ar: f64, ag: f64, ab: f64, dr: f64, dg: f64, db: f64) -> f64 {
    let dr = if dr <= RAYTRACE_EPSILON && dr >= -RAYTRACE_EPSILON {
        0.0
    } else {
        dr
    };
    let dg = if dg <= RAYTRACE_EPSILON && dg >= -RAYTRACE_EPSILON {
        0.0
    } else {
        dg
    };
    let db = if db <= RAYTRACE_EPSILON && db >= -RAYTRACE_EPSILON {
        0.0
    } else {
        db
    };
    let ir = 1.0 / dr;
    let ig = 1.0 / dg;
    let ib = 1.0 / db;
    let tr = ((1.0 - ar) * ir).max(-ar * ir);
    let tg = ((1.0 - ag) * ig).max(-ag * ig);
    let tb = ((1.0 - ab) * ib).max(-ab * ib);
    tr.min(tg).min(tb)
}

// ── Method 1: clip ──
#[inline(always)]
fn clip(oklch: &[f64; 3], out: &mut [f64; 3]) {
    oklch_to_clipped_p3(oklch[0], oklch[1], oklch[2], out);
}

// ── Method 2: oklch-cubic (cached) ──────────────────────────────────────────
// Solve, per linear-P3 channel, the cubic in t = C/L where the channel exits
// [0,1]; the smallest root is the max in-gamut chroma. Per-hue structure cached.

#[derive(Clone, Copy)]
struct HueData {
    a: [f64; 3],
    b: [f64; 3],
    d: [f64; 3],
    t_lower: f64,
    turn: [f64; 3],
}

// Smallest real root of a*x^3 + b*x^2 + c*x + d in (lo, hi), else +inf.
#[inline(always)]
fn first_root(a: f64, mut b: f64, mut c: f64, mut d: f64, lo: f64, hi: f64) -> f64 {
    let mut r0 = f64::INFINITY;
    let mut r1 = f64::INFINITY;
    let mut r2 = f64::INFINITY;
    if a.abs() < 1e-12 {
        if b.abs() < 1e-12 {
            if c.abs() >= 1e-12 {
                r0 = -d / c;
            }
        } else {
            let disc = c * c - 4.0 * b * d;
            if disc >= 0.0 {
                let s = disc.sqrt();
                r0 = (-c + s) / (2.0 * b);
                r1 = (-c - s) / (2.0 * b);
            }
        }
    } else {
        b /= a;
        c /= a;
        d /= a;
        let p = c - b * b / 3.0;
        let q = 2.0 * b * b * b / 27.0 - b * c / 3.0 + d;
        let off = -b / 3.0;
        let disc = q * q / 4.0 + p * p * p / 27.0;
        if disc > 1e-14 {
            let s = disc.sqrt();
            r0 = (-q / 2.0 + s).cbrt() + (-q / 2.0 - s).cbrt() + off;
        } else if disc > -1e-14 {
            let u = (-q / 2.0).cbrt();
            r0 = 2.0 * u + off;
            r1 = -u + off;
        } else {
            let m = 2.0 * (-p / 3.0).sqrt();
            let phi = (3.0 * q / (p * m)).clamp(-1.0, 1.0).acos();
            r0 = m * (phi / 3.0).cos() + off;
            r1 = m * ((phi - 2.0 * PI) / 3.0).cos() + off;
            r2 = m * ((phi - 4.0 * PI) / 3.0).cos() + off;
        }
    }
    let mut best = f64::INFINITY;
    if r0 > lo && r0 < hi {
        best = r0;
    }
    if r1 > lo && r1 < hi && r1 < best {
        best = r1;
    }
    if r2 > lo && r2 < hi && r2 < best {
        best = r2;
    }
    best
}

#[inline(always)]
fn first_turn(d: f64, b: f64, a: f64) -> f64 {
    first_root(0.0, d, 2.0 * b, a, 1e-12, f64::INFINITY)
}

fn get_hue_data(h: f64) -> HueData {
    let rad = h * PI / 180.0;
    let (cos, sin) = (rad.cos(), rad.sin());
    let q0 = KA0 * cos + KB0 * sin;
    let q1 = KA1 * cos + KB1 * sin;
    let q2 = KA2 * cos + KB2 * sin;
    let a = [
        RL * q0 + RM * q1 + RS * q2,
        GL * q0 + GM * q1 + GS * q2,
        BL * q0 + BM * q1 + BS * q2,
    ];
    let (q0b, q1b, q2b) = (q0 * q0, q1 * q1, q2 * q2);
    let b = [
        RL * q0b + RM * q1b + RS * q2b,
        GL * q0b + GM * q1b + GS * q2b,
        BL * q0b + BM * q1b + BS * q2b,
    ];
    let (q0c, q1c, q2c) = (q0b * q0, q1b * q1, q2b * q2);
    let d = [
        RL * q0c + RM * q1c + RS * q2c,
        GL * q0c + GM * q1c + GS * q2c,
        BL * q0c + BM * q1c + BS * q2c,
    ];
    let mut t_lower = f64::INFINITY;
    let mut turn = [0.0; 3];
    for i in 0..3 {
        t_lower = t_lower.min(first_root(
            d[i],
            3.0 * b[i],
            3.0 * a[i],
            1.0,
            1e-9,
            f64::INFINITY,
        ));
        turn[i] = first_turn(d[i], b[i], a[i]);
    }
    HueData {
        a,
        b,
        d,
        t_lower,
        turn,
    }
}

struct OklchCubic {
    // 3601 buckets of 0.1°, stored without Option so the cache stays a dense
    // 104 B/bucket array (~366 KiB); t_lower (a root strictly greater than
    // 1e-9, or +inf) doubles as the bucket-filled marker: 0 means empty.
    cache: Vec<HueData>,
}

impl OklchCubic {
    fn new() -> Self {
        OklchCubic {
            cache: vec![
                HueData {
                    a: [0.0; 3],
                    b: [0.0; 3],
                    d: [0.0; 3],
                    t_lower: 0.0,
                    turn: [0.0; 3],
                };
                3601
            ],
        }
    }

    #[inline(always)]
    fn hue_data(&mut self, h: f64) -> HueData {
        let mut hh = h % 360.0;
        if hh < 0.0 {
            hh += 360.0;
        }
        let key = (hh * 10.0).round() as usize;
        let d = self.cache[key];
        if d.t_lower != 0.0 {
            return d;
        }
        let d = get_hue_data(key as f64 / 10.0);
        self.cache[key] = d;
        d
    }

    #[inline(always)]
    fn map(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    fn map_with_in_gamut_check(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[f64; 3], out: &mut [f64; 3], check_in_gamut: bool) {
        let (l, c, h) = (oklch[0], oklch[1], oklch[2]);
        if l <= 0.0 || l >= 1.0 || c <= 0.0 {
            let ll = if l <= 0.0 {
                0.0
            } else if l >= 1.0 {
                1.0
            } else {
                l
            };
            oklch_to_clipped_p3(ll, 0.0, h, out);
            return;
        }
        if check_in_gamut && oklch_to_p3_if_in_gamut(l, c, h, out) {
            return;
        }
        let hd = self.hue_data(h);
        let (a, b, d, t_lower, turn) = (hd.a, hd.b, hd.d, hd.t_lower, hd.turn);
        let t0 = c / l;
        let mut max_t = t0.min(t_lower);
        let target = 1.0 / (l * l * l);
        let dd = 1.0 - target;
        for i in 0..3 {
            if turn[i] > max_t {
                if a[i] <= 0.0 {
                    continue;
                }
                let p_max = ((d[i] * max_t + 3.0 * b[i]) * max_t + 3.0 * a[i]) * max_t + 1.0;
                if p_max < target {
                    continue;
                }
            }
            max_t = max_t.min(first_root(d[i], 3.0 * b[i], 3.0 * a[i], dd, 1e-9, max_t));
        }

        let l3 = l * l * l;
        out[0] =
            clamped_gamma(l3 * (((d[0] * max_t + 3.0 * b[0]) * max_t + 3.0 * a[0]) * max_t + 1.0));
        out[1] =
            clamped_gamma(l3 * (((d[1] * max_t + 3.0 * b[1]) * max_t + 3.0 * a[1]) * max_t + 1.0));
        out[2] =
            clamped_gamma(l3 * (((d[2] * max_t + 3.0 * b[2]) * max_t + 3.0 * a[2]) * max_t + 1.0));
    }
}

// ── Method 3: oklch-cubic (no cache) ─────────────────────────────────────────
// Same fixed 0.1° bucket semantics as the cached variant, but recomputes the
// per-hue cubic structure for every call. Kept separate so this row cannot affect
// the cached implementation.

#[derive(Clone, Copy)]
struct NoCacheHueData {
    a: [f64; 3],
    b: [f64; 3],
    d: [f64; 3],
    t_lower: f64,
    turn: [f64; 3],
}

#[inline(always)]
fn first_root_no_cache(a: f64, mut b: f64, mut c: f64, mut d: f64, lo: f64, hi: f64) -> f64 {
    let mut r0 = f64::INFINITY;
    let mut r1 = f64::INFINITY;
    let mut r2 = f64::INFINITY;
    if a.abs() < 1e-12 {
        if b.abs() < 1e-12 {
            if c.abs() >= 1e-12 {
                r0 = -d / c;
            }
        } else {
            let disc = c * c - 4.0 * b * d;
            if disc >= 0.0 {
                let s = disc.sqrt();
                r0 = (-c + s) / (2.0 * b);
                r1 = (-c - s) / (2.0 * b);
            }
        }
    } else {
        b /= a;
        c /= a;
        d /= a;
        let p = c - b * b / 3.0;
        let q = 2.0 * b * b * b / 27.0 - b * c / 3.0 + d;
        let off = -b / 3.0;
        let disc = q * q / 4.0 + p * p * p / 27.0;
        if disc > 1e-14 {
            let s = disc.sqrt();
            r0 = (-q / 2.0 + s).cbrt() + (-q / 2.0 - s).cbrt() + off;
        } else if disc > -1e-14 {
            let u = (-q / 2.0).cbrt();
            r0 = 2.0 * u + off;
            r1 = -u + off;
        } else {
            let m = 2.0 * (-p / 3.0).sqrt();
            let phi = (3.0 * q / (p * m)).clamp(-1.0, 1.0).acos();
            r0 = m * (phi / 3.0).cos() + off;
            r1 = m * ((phi - 2.0 * PI) / 3.0).cos() + off;
            r2 = m * ((phi - 4.0 * PI) / 3.0).cos() + off;
        }
    }
    let mut best = f64::INFINITY;
    if r0 > lo && r0 < hi {
        best = r0;
    }
    if r1 > lo && r1 < hi && r1 < best {
        best = r1;
    }
    if r2 > lo && r2 < hi && r2 < best {
        best = r2;
    }
    best
}

#[inline(always)]
fn first_turn_no_cache(d: f64, b: f64, a: f64) -> f64 {
    first_root_no_cache(0.0, d, 2.0 * b, a, 1e-12, f64::INFINITY)
}

fn get_hue_data_no_cache(h: f64) -> NoCacheHueData {
    let mut hh = h % 360.0;
    if hh < 0.0 {
        hh += 360.0;
    }
    let bucket_h = ((hh * 10.0).round() as usize) as f64 / 10.0;
    let rad = bucket_h * PI / 180.0;
    let (cos, sin) = (rad.cos(), rad.sin());
    let q0 = KA0 * cos + KB0 * sin;
    let q1 = KA1 * cos + KB1 * sin;
    let q2 = KA2 * cos + KB2 * sin;
    let a = [
        RL * q0 + RM * q1 + RS * q2,
        GL * q0 + GM * q1 + GS * q2,
        BL * q0 + BM * q1 + BS * q2,
    ];
    let (q0b, q1b, q2b) = (q0 * q0, q1 * q1, q2 * q2);
    let b = [
        RL * q0b + RM * q1b + RS * q2b,
        GL * q0b + GM * q1b + GS * q2b,
        BL * q0b + BM * q1b + BS * q2b,
    ];
    let (q0c, q1c, q2c) = (q0b * q0, q1b * q1, q2b * q2);
    let d = [
        RL * q0c + RM * q1c + RS * q2c,
        GL * q0c + GM * q1c + GS * q2c,
        BL * q0c + BM * q1c + BS * q2c,
    ];
    let mut t_lower = f64::INFINITY;
    let mut turn = [0.0; 3];
    for i in 0..3 {
        t_lower = t_lower.min(first_root_no_cache(
            d[i],
            3.0 * b[i],
            3.0 * a[i],
            1.0,
            1e-9,
            f64::INFINITY,
        ));
        turn[i] = first_turn_no_cache(d[i], b[i], a[i]);
    }
    NoCacheHueData {
        a,
        b,
        d,
        t_lower,
        turn,
    }
}

struct OklchCubicNoCache;

impl OklchCubicNoCache {
    fn new() -> Self {
        OklchCubicNoCache
    }

    #[inline(always)]
    fn map(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    fn map_with_in_gamut_check(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[f64; 3], out: &mut [f64; 3], check_in_gamut: bool) {
        let (l, c, h) = (oklch[0], oklch[1], oklch[2]);
        if l <= 0.0 || l >= 1.0 || c <= 0.0 {
            let ll = if l <= 0.0 {
                0.0
            } else if l >= 1.0 {
                1.0
            } else {
                l
            };
            oklch_to_clipped_p3(ll, 0.0, h, out);
            return;
        }
        if check_in_gamut && oklch_to_p3_if_in_gamut(l, c, h, out) {
            return;
        }

        let hd = get_hue_data_no_cache(h);
        let (a, b, d, t_lower, turn) = (hd.a, hd.b, hd.d, hd.t_lower, hd.turn);
        let t0 = c / l;
        let mut max_t = t0.min(t_lower);
        let target = 1.0 / (l * l * l);
        let dd = 1.0 - target;
        for i in 0..3 {
            if turn[i] > max_t {
                if a[i] <= 0.0 {
                    continue;
                }
                let p_max = ((d[i] * max_t + 3.0 * b[i]) * max_t + 3.0 * a[i]) * max_t + 1.0;
                if p_max < target {
                    continue;
                }
            }
            max_t = max_t.min(first_root_no_cache(
                d[i],
                3.0 * b[i],
                3.0 * a[i],
                dd,
                1e-9,
                max_t,
            ));
        }

        let l3 = l * l * l;
        out[0] =
            clamped_gamma(l3 * (((d[0] * max_t + 3.0 * b[0]) * max_t + 3.0 * a[0]) * max_t + 1.0));
        out[1] =
            clamped_gamma(l3 * (((d[1] * max_t + 3.0 * b[1]) * max_t + 3.0 * a[1]) * max_t + 1.0));
        out[2] =
            clamped_gamma(l3 * (((d[2] * max_t + 3.0 * b[2]) * max_t + 3.0 * a[2]) * max_t + 1.0));
    }
}

// ── Method 4: oklch-halley ──────────────────────────────────────────────────

// Bracketed Halley solve for the first Display-P3 channel boundary along a
// constant-lightness, constant-hue chroma ray. Ported from color-js/apps#44.
#[inline(always)]
fn solve_halley(l_value: f64, q0: f64, q1: f64, q2: f64) -> f64 {
    let rows = [[RL, RM, RS], [GL, GM, GS], [BL, BM, BS]];
    let mut lo = 0.0;
    let mut hi = 0.5;

    let l3 = l_value * l_value * l_value;
    let l2x3 = 3.0 * l_value * l_value;
    let mut c = hi;
    for row in rows {
        let slope = l2x3 * (row[0] * q0 + row[1] * q1 + row[2] * q2);
        let crossing = if slope > 0.0 {
            (1.0 - l3) / slope
        } else if slope < 0.0 {
            -l3 / slope
        } else {
            f64::INFINITY
        };
        if crossing < c {
            c = crossing;
        }
    }

    let mut best = c;
    let mut best_err = f64::INFINITY;

    for _ in 0..16 {
        let l = l_value + c * q0;
        let m = l_value + c * q1;
        let s = l_value + c * q2;
        let l2 = l * l;
        let m2 = m * m;
        let s2 = s * s;
        let l3 = l2 * l;
        let m3 = m2 * m;
        let s3 = s2 * s;

        let mut g = f64::NEG_INFINITY;
        let mut g1 = 0.0;
        let mut g2 = 0.0;
        for row in rows {
            let v = row[0] * l3 + row[1] * m3 + row[2] * s3;
            let d1 = 3.0 * (row[0] * q0 * l2 + row[1] * q1 * m2 + row[2] * q2 * s2);
            let d2 = 6.0 * (row[0] * q0 * q0 * l + row[1] * q1 * q1 * m + row[2] * q2 * q2 * s);
            if v - 1.0 > g {
                g = v - 1.0;
                g1 = d1;
                g2 = d2;
            }
            if -v > g {
                g = -v;
                g1 = -d1;
                g2 = -d2;
            }
        }

        if g > 0.0 {
            hi = c;
        } else {
            lo = c;
        }

        let err = if g1 != 0.0 { (g / g1).abs() } else { g.abs() };
        if err < best_err {
            best_err = err;
            best = c;
        }

        let denom = 2.0 * g1 * g1 - g * g2;
        let step = if denom != 0.0 {
            (2.0 * g * g1) / denom
        } else if g1 != 0.0 {
            g / g1
        } else {
            0.0
        };
        if step > -1e-9 && step < 1e-9 {
            return c;
        }

        let next = c - step;
        c = if next > lo && next < hi {
            next
        } else {
            (lo + hi) / 2.0
        };
    }

    best
}

struct OklchHalley;

impl OklchHalley {
    fn new() -> Self {
        OklchHalley
    }

    #[inline(always)]
    fn map(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    fn map_with_in_gamut_check(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[f64; 3], out: &mut [f64; 3], check_in_gamut: bool) {
        let (l, c, h) = (oklch[0], oklch[1], oklch[2]);
        if l <= 0.0 || l >= 1.0 || c <= 0.0 {
            let ll = if l <= 0.0 {
                0.0
            } else if l >= 1.0 {
                1.0
            } else {
                l
            };
            oklch_to_clipped_p3(ll, 0.0, h, out);
            return;
        }
        if check_in_gamut && oklch_to_p3_if_in_gamut(l, c, h, out) {
            return;
        }

        let rad = h * PI / 180.0;
        let (cos, sin) = (rad.cos(), rad.sin());
        let q0 = KA0 * cos + KB0 * sin;
        let q1 = KA1 * cos + KB1 * sin;
        let q2 = KA2 * cos + KB2 * sin;
        let mapped_c = c.min(solve_halley(l, q0, q1, q2));
        lms_slopes_to_clipped_p3(l, mapped_c, q0, q1, q2, out);
    }
}

// ── Method 5: Bottosson constant lightness ───────────────────────────────────

#[inline(always)]
fn compute_max_saturation_p3(a: f64, b: f64) -> f64 {
    let (k0, k1, k2, k3, k4, wl, wm, ws) = if a * P3_RED1 + b * P3_RED2 > 1.0 {
        (
            P3_RED_K0, P3_RED_K1, P3_RED_K2, P3_RED_K3, P3_RED_K4, RL, RM, RS,
        )
    } else if a * P3_GREEN1 + b * P3_GREEN2 > 1.0 {
        (
            P3_GREEN_K0,
            P3_GREEN_K1,
            P3_GREEN_K2,
            P3_GREEN_K3,
            P3_GREEN_K4,
            GL,
            GM,
            GS,
        )
    } else {
        (
            P3_BLUE_K0, P3_BLUE_K1, P3_BLUE_K2, P3_BLUE_K3, P3_BLUE_K4, BL, BM, BS,
        )
    };

    let a2 = a * a;
    let sat = k0 + k1 * a + k2 * b + k3 * a2 + k4 * a * b;
    let kl = KA0 * a + KB0 * b;
    let km = KA1 * a + KB1 * b;
    let ks = KA2 * a + KB2 * b;
    let l = 1.0 + sat * kl;
    let m = 1.0 + sat * km;
    let s = 1.0 + sat * ks;
    let l2 = l * l;
    let m2 = m * m;
    let s2 = s * s;
    let f = wl * l2 * l + wm * m2 * m + ws * s2 * s;
    let f1 = 3.0 * (wl * kl * l2 + wm * km * m2 + ws * ks * s2);
    let f2 = 6.0 * (wl * kl * kl * l + wm * km * km * m + ws * ks * ks * s);

    sat - (f * f1) / (f1 * f1 - 0.5 * f * f2)
}

#[inline(always)]
fn find_cusp_p3(a: f64, b: f64) -> [f64; 2] {
    let s_cusp = compute_max_saturation_p3(a, b);
    let l = 1.0 + s_cusp * (KA0 * a + KB0 * b);
    let m = 1.0 + s_cusp * (KA1 * a + KB1 * b);
    let s = 1.0 + s_cusp * (KA2 * a + KB2 * b);
    let l3 = l * l * l;
    let m3 = m * m * m;
    let s3 = s * s * s;
    let r = RL * l3 + RM * m3 + RS * s3;
    let g = GL * l3 + GM * m3 + GS * s3;
    let blue = BL * l3 + BM * m3 + BS * s3;
    let l_cusp = (1.0 / r.max(g).max(blue)).cbrt();

    [l_cusp, l_cusp * s_cusp]
}

#[inline(always)]
fn find_gamut_intersection_p3(a: f64, b: f64, l1: f64, c1: f64, l0: f64, cusp: [f64; 2]) -> f64 {
    let mut t: f64;

    if (l1 - l0) * cusp[1] - (cusp[0] - l0) * c1 <= 0.0 {
        let denom = c1 * cusp[0] + cusp[1] * (l0 - l1);
        t = if denom == 0.0 {
            0.0
        } else {
            (cusp[1] * l0) / denom
        };
    } else {
        let denom = c1 * (cusp[0] - 1.0) + cusp[1] * (l0 - l1);
        t = if denom == 0.0 {
            0.0
        } else {
            (cusp[1] * (l0 - 1.0)) / denom
        };

        let dl = l1 - l0;
        let kl = a * KA0 + b * KB0;
        let km = a * KA1 + b * KB1;
        let ks = a * KA2 + b * KB2;
        let ldt_base = dl + c1 * kl;
        let mdt_base = dl + c1 * km;
        let sdt_base = dl + c1 * ks;
        let l_value = l0 * (1.0 - t) + t * l1;
        let c = t * c1;
        let l = l_value + c * kl;
        let m = l_value + c * km;
        let s = l_value + c * ks;
        let l2 = l * l;
        let m2 = m * m;
        let s2 = s * s;
        let l3 = l2 * l;
        let m3 = m2 * m;
        let s3 = s2 * s;
        let ldt = 3.0 * ldt_base * l2;
        let mdt = 3.0 * mdt_base * m2;
        let sdt = 3.0 * sdt_base * s2;
        let ldt2 = 6.0 * ldt_base * ldt_base * l;
        let mdt2 = 6.0 * mdt_base * mdt_base * m;
        let sdt2 = 6.0 * sdt_base * sdt_base * s;

        let r = RL * l3 + RM * m3 + RS * s3 - 1.0;
        let r1 = RL * ldt + RM * mdt + RS * sdt;
        let r2 = RL * ldt2 + RM * mdt2 + RS * sdt2;
        let ur = r1 / (r1 * r1 - 0.5 * r * r2);
        let tr = if ur >= 0.0 { -r * ur } else { f64::MAX };

        let g = GL * l3 + GM * m3 + GS * s3 - 1.0;
        let g1 = GL * ldt + GM * mdt + GS * sdt;
        let g2 = GL * ldt2 + GM * mdt2 + GS * sdt2;
        let ug = g1 / (g1 * g1 - 0.5 * g * g2);
        let tg = if ug >= 0.0 { -g * ug } else { f64::MAX };

        let blue = BL * l3 + BM * m3 + BS * s3 - 1.0;
        let blue1 = BL * ldt + BM * mdt + BS * sdt;
        let blue2 = BL * ldt2 + BM * mdt2 + BS * sdt2;
        let ub = blue1 / (blue1 * blue1 - 0.5 * blue * blue2);
        let tb = if ub >= 0.0 { -blue * ub } else { f64::MAX };

        t += tr.min(tg.min(tb));
    }

    t
}

struct BottossonLightness;

impl BottossonLightness {
    fn new() -> Self {
        BottossonLightness
    }

    #[inline(always)]
    fn map(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    fn map_with_in_gamut_check(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[f64; 3], out: &mut [f64; 3], check_in_gamut: bool) {
        let mut l = oklch[0];
        let c = oklch[1].max(0.0);
        let h = oklch[2];

        if c <= BOTTOSSON_EPSILON {
            l = clamp01(l);
            let gray = clamped_gamma(l * l * l);
            *out = [gray, gray, gray];
            return;
        }

        let hr = h * PI / 180.0;
        let unit_a = hr.cos();
        let unit_b = hr.sin();
        let lab_a = c * unit_a;
        let lab_b = c * unit_b;

        if check_in_gamut && oklab_to_p3_if_in_gamut(l, lab_a, lab_b, out) {
            return;
        }

        let cusp = find_cusp_p3(unit_a, unit_b);
        let l0 = clamp01(l);
        let t = find_gamut_intersection_p3(unit_a, unit_b, l, c, l0, cusp);
        let mapped_l = l0 * (1.0 - t) + t * l;
        let mapped_c = t * c;

        oklab_to_clipped_p3_fast(mapped_l, mapped_c * unit_a, mapped_c * unit_b, out);
    }
}

// ── Method 5b: Bottosson constant lightness, cached ─────────────────────────
// The cusp and the LMS' hue slopes depend only on hue, so they are memoized in
// 0.1° buckets (same bucketed-hue semantics as oklch-cubic's hue structure).
// Each bucket stores [cusp_l, cusp_c, q0, q1, q2] where q_i is the LMS' slope
// for the hue direction: LMS'_i(L, C) = L + C * q_i. With those cached, the
// per-call path needs no trig at all.

// find_gamut_intersection_p3 with the hue expressed as LMS' slopes q0..q2 (the
// kl/km/ks it would otherwise recompute) and the cusp as scalars.
#[inline(always)]
fn find_gamut_intersection_q(
    q0: f64,
    q1: f64,
    q2: f64,
    l1: f64,
    c1: f64,
    l0: f64,
    cusp_l: f64,
    cusp_c: f64,
) -> f64 {
    let mut t: f64;

    if (l1 - l0) * cusp_c - (cusp_l - l0) * c1 <= 0.0 {
        let denom = c1 * cusp_l + cusp_c * (l0 - l1);
        t = if denom == 0.0 {
            0.0
        } else {
            (cusp_c * l0) / denom
        };
    } else {
        let denom = c1 * (cusp_l - 1.0) + cusp_c * (l0 - l1);
        t = if denom == 0.0 {
            0.0
        } else {
            (cusp_c * (l0 - 1.0)) / denom
        };

        let dl = l1 - l0;
        let ldt_base = dl + c1 * q0;
        let mdt_base = dl + c1 * q1;
        let sdt_base = dl + c1 * q2;
        let l_value = l0 * (1.0 - t) + t * l1;
        let c = t * c1;
        let l = l_value + c * q0;
        let m = l_value + c * q1;
        let s = l_value + c * q2;
        let l2 = l * l;
        let m2 = m * m;
        let s2 = s * s;
        let l3 = l2 * l;
        let m3 = m2 * m;
        let s3 = s2 * s;
        let ldt = 3.0 * ldt_base * l2;
        let mdt = 3.0 * mdt_base * m2;
        let sdt = 3.0 * sdt_base * s2;
        let ldt2 = 6.0 * ldt_base * ldt_base * l;
        let mdt2 = 6.0 * mdt_base * mdt_base * m;
        let sdt2 = 6.0 * sdt_base * sdt_base * s;

        let r = RL * l3 + RM * m3 + RS * s3 - 1.0;
        let r1 = RL * ldt + RM * mdt + RS * sdt;
        let r2 = RL * ldt2 + RM * mdt2 + RS * sdt2;
        let ur = r1 / (r1 * r1 - 0.5 * r * r2);
        let tr = if ur >= 0.0 { -r * ur } else { f64::MAX };

        let g = GL * l3 + GM * m3 + GS * s3 - 1.0;
        let g1 = GL * ldt + GM * mdt + GS * sdt;
        let g2 = GL * ldt2 + GM * mdt2 + GS * sdt2;
        let ug = g1 / (g1 * g1 - 0.5 * g * g2);
        let tg = if ug >= 0.0 { -g * ug } else { f64::MAX };

        let blue = BL * l3 + BM * m3 + BS * s3 - 1.0;
        let blue1 = BL * ldt + BM * mdt + BS * sdt;
        let blue2 = BL * ldt2 + BM * mdt2 + BS * sdt2;
        let ub = blue1 / (blue1 * blue1 - 0.5 * blue * blue2);
        let tb = if ub >= 0.0 { -blue * ub } else { f64::MAX };

        t += tr.min(tg.min(tb));
    }

    t
}

// LMS'-slope form of the OKLab -> clipped P3 conversion: LMS'_i = L + C * q_i.
#[inline(always)]
fn lms_slopes_to_clipped_p3(l: f64, c: f64, q0: f64, q1: f64, q2: f64, out: &mut [f64; 3]) {
    let l0 = l + c * q0;
    let m0 = l + c * q1;
    let s0 = l + c * q2;
    let l3 = l0 * l0 * l0;
    let m3 = m0 * m0 * m0;
    let s3 = s0 * s0 * s0;
    out[0] = clamped_gamma(RL * l3 + RM * m3 + RS * s3);
    out[1] = clamped_gamma(GL * l3 + GM * m3 + GS * s3);
    out[2] = clamped_gamma(BL * l3 + BM * m3 + BS * s3);
}

#[inline(always)]
fn lms_slopes_to_p3_if_in_gamut(
    l: f64,
    c: f64,
    q0: f64,
    q1: f64,
    q2: f64,
    out: &mut [f64; 3],
) -> bool {
    let l0 = l + c * q0;
    let m0 = l + c * q1;
    let s0 = l + c * q2;
    let l3 = l0 * l0 * l0;
    let m3 = m0 * m0 * m0;
    let s3 = s0 * s0 * s0;
    let r = RL * l3 + RM * m3 + RS * s3;
    let g = GL * l3 + GM * m3 + GS * s3;
    let bl = BL * l3 + BM * m3 + BS * s3;
    if r < 0.0 || r > 1.0 || g < 0.0 || g > 1.0 || bl < 0.0 || bl > 1.0 {
        return false;
    }
    out[0] = clamped_gamma(r);
    out[1] = clamped_gamma(g);
    out[2] = clamped_gamma(bl);
    true
}

struct BottossonLightnessCached {
    cache: Vec<[f64; 5]>, // 3601 buckets of 0.1°; slot 0 (cusp L) is never 0 once filled
}

impl BottossonLightnessCached {
    fn new() -> Self {
        BottossonLightnessCached {
            cache: vec![[0.0; 5]; 3601],
        }
    }

    #[inline(always)]
    fn cusp_data(&mut self, h: f64) -> [f64; 5] {
        let mut hh = h % 360.0;
        if hh < 0.0 {
            hh += 360.0;
        }
        let key = (hh * 10.0).round() as usize;
        if self.cache[key][0] != 0.0 {
            return self.cache[key];
        }
        let rad = key as f64 / 10.0 * PI / 180.0;
        let unit_a = rad.cos();
        let unit_b = rad.sin();
        let cusp = find_cusp_p3(unit_a, unit_b);
        let d = [
            cusp[0],
            cusp[1],
            unit_a * KA0 + unit_b * KB0,
            unit_a * KA1 + unit_b * KB1,
            unit_a * KA2 + unit_b * KB2,
        ];
        self.cache[key] = d;
        d
    }

    #[inline(always)]
    fn map(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    fn map_with_in_gamut_check(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[f64; 3], out: &mut [f64; 3], check_in_gamut: bool) {
        let mut l = oklch[0];
        let c = oklch[1].max(0.0);
        let h = oklch[2];

        if c <= BOTTOSSON_EPSILON {
            l = clamp01(l);
            let gray = clamped_gamma(l * l * l);
            *out = [gray, gray, gray];
            return;
        }

        let d = self.cusp_data(h);
        let (cusp_l, cusp_c, q0, q1, q2) = (d[0], d[1], d[2], d[3], d[4]);

        if check_in_gamut && lms_slopes_to_p3_if_in_gamut(l, c, q0, q1, q2, out) {
            return;
        }

        let l0 = clamp01(l);
        let t = find_gamut_intersection_q(q0, q1, q2, l, c, l0, cusp_l, cusp_c);
        let mapped_l = l0 * (1.0 - t) + t * l;
        let mapped_c = t * c;

        lms_slopes_to_clipped_p3(mapped_l, mapped_c, q0, q1, q2, out);
    }
}

// ── Method 6: raytrace ───────────────────────────────────────────────────────

struct Raytrace;

impl Raytrace {
    fn new() -> Self {
        Raytrace
    }

    #[inline(always)]
    fn map(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    fn map_with_in_gamut_check(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[f64; 3], out: &mut [f64; 3], check_in_gamut: bool) {
        let l = oklch[0];
        let c = oklch[1];
        let h = oklch[2];

        if l <= 0.0 {
            *out = [0.0, 0.0, 0.0];
            return;
        }
        if l >= 1.0 {
            *out = [1.0, 1.0, 1.0];
            return;
        }
        if c <= 0.0 {
            let gray = clamped_gamma(l * l * l);
            *out = [gray, gray, gray];
            return;
        }
        let hr = h * PI / 180.0;
        let unit_a = hr.cos();
        let unit_b = hr.sin();
        let (mut mr, mut mg, mut mb) = oklab_to_linear_p3_components(l, c * unit_a, c * unit_b);
        if check_in_gamut
            && mr >= 0.0
            && mr <= 1.0
            && mg >= 0.0
            && mg <= 1.0
            && mb >= 0.0
            && mb <= 1.0
        {
            out[0] = clamped_gamma(mr);
            out[1] = clamped_gamma(mg);
            out[2] = clamped_gamma(mb);
            return;
        }

        let anchor = l * l * l;
        // L^3 underflows to 0 for L below ~1.7e-108; the anchor then sits on
        // the cube corner, breaking the strictly-inside invariant exit_t
        // relies on (a flushed-parallel axis would yield -0 * inf = NaN).
        // Lightness that underflows in linear space is black, same as the
        // l <= 0 early-return.
        if anchor == 0.0 {
            *out = [0.0, 0.0, 0.0];
            return;
        }
        let mut ar = anchor;
        let mut ag = anchor;
        let mut ab = anchor;
        let mut last_r = mr;
        let mut last_g = mg;
        let mut last_b = mb;

        for i in 0..4 {
            if i != 0 {
                let corrected_c = linear_p3_to_oklab_chroma(mr, mg, mb);
                (mr, mg, mb) =
                    oklab_to_linear_p3_components(l, corrected_c * unit_a, corrected_c * unit_b);
            }

            let t = exit_t(ar, ag, ab, mr - ar, mg - ag, mb - ab);
            if !t.is_finite() {
                mr = last_r;
                mg = last_g;
                mb = last_b;
                break;
            }

            let hit_r = ar + (mr - ar) * t;
            let hit_g = ag + (mg - ag) * t;
            let hit_b = ab + (mb - ab) * t;

            if i != 0
                && mr > RAYTRACE_LOW
                && mr < RAYTRACE_HIGH
                && mg > RAYTRACE_LOW
                && mg < RAYTRACE_HIGH
                && mb > RAYTRACE_LOW
                && mb < RAYTRACE_HIGH
            {
                ar = mr;
                ag = mg;
                ab = mb;
            }

            last_r = hit_r;
            last_g = hit_g;
            last_b = hit_b;
            mr = last_r;
            mg = last_g;
            mb = last_b;
        }

        out[0] = clamped_gamma(mr);
        out[1] = clamped_gamma(mg);
        out[2] = clamped_gamma(mb);
    }
}

// ── Method 7: edge-seeker ───────────────────────────────────────────────────
// Reduce chroma to a precomputed LUT of the gamut edge. The lookup evaluates the
// LUT at the exact normalized hue.

#[inline(always)]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    if t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    a * (1.0 - t) + b * t
}

// LUT row = [l, c, h, curvature].
const HUE_INDEX_SCALE: usize = 10;
const HUE_INDEX_BUCKETS: usize = 360 * HUE_INDEX_SCALE;

fn find_closest(hue: f64) -> (usize, usize) {
    let mut start: i64 = 0;
    let mut end: i64 = LUT.len() as i64 - 1;
    let mut mid = (start + end) / 2;
    while start <= end {
        let mh = LUT[mid as usize][2];
        if mh == hue {
            return (mid as usize, mid as usize);
        } else if mh < hue {
            start = mid + 1;
        } else {
            end = mid - 1;
        }
        mid = (start + end) / 2;
    }
    let last = LUT.len() as i64 - 1;
    (
        mid.clamp(0, last) as usize,
        (mid + 1).clamp(0, last) as usize,
    )
}

fn lerp_lut(start: &[f64; 4], end: &[f64; 4], hue: f64) -> [f64; 4] {
    if hue == start[2] {
        return *start;
    }
    if hue == end[2] {
        return *end;
    }
    let t = (hue - start[2]) / (end[2] - start[2]);
    [
        lerp(start[0], end[0], t),
        lerp(start[1], end[1], t),
        hue,
        lerp(start[3], end[3], t),
    ]
}

fn get_lut_item(h: f64) -> [f64; 4] {
    let (lo, hi) = find_closest(h);
    lerp_lut(&LUT[lo], &LUT[hi], h)
}

fn build_interval_index() -> Vec<usize> {
    let mut intervals = vec![0; HUE_INDEX_BUCKETS];
    let mut interval = 0;
    for (bucket, slot) in intervals.iter_mut().enumerate() {
        let hue = bucket as f64 / HUE_INDEX_SCALE as f64;
        while interval + 1 < LUT.len() - 1 && LUT[interval + 1][2] <= hue {
            interval += 1;
        }
        *slot = interval;
    }
    intervals
}

fn get_lut_item_indexed(h: f64, interval_index: &[usize]) -> [f64; 4] {
    let bucket = ((h * HUE_INDEX_SCALE as f64) as usize).min(HUE_INDEX_BUCKETS - 1);
    let mut interval = interval_index[bucket];
    while interval > 0 && h < LUT[interval][2] {
        interval -= 1;
    }
    while interval + 1 < LUT.len() - 1 && h > LUT[interval + 1][2] {
        interval += 1;
    }
    lerp_lut(&LUT[interval], &LUT[interval + 1], h)
}

#[inline(always)]
fn normalized_hue(h: f64) -> f64 {
    if h < 0.0 {
        (h % 360.0) + 360.0
    } else {
        h % 360.0
    }
}

#[inline(always)]
fn intersection_with_arc(x: f64, curvature: f64) -> f64 {
    if curvature == 0.0 {
        return x;
    }
    let radius = (1.0 / curvature).abs();
    let half_diagonal = 0.5f64.sqrt(); // sqrt(0.5^2 + 0.5^2)
    let distance_to_center = (radius * radius - half_diagonal * half_diagonal).sqrt();
    let offset = distance_to_center / 2.0f64.sqrt();
    let center_x = (if curvature > 0.0 { offset } else { -offset }) + 0.5;
    let center_y = (if curvature > 0.0 { -offset } else { offset }) + 0.5;
    let under_root = radius * radius - (x - center_x) * (x - center_x);
    if under_root < 0.0 {
        return 0.0;
    }
    let sqrt_val = under_root.sqrt();
    let res1 = center_y + sqrt_val;
    if res1 >= 0.0 && res1 <= 1.0 {
        res1
    } else {
        center_y - sqrt_val
    }
}

#[inline(always)]
fn max_chroma_from_item(l: f64, item: [f64; 4]) -> f64 {
    let (il, ic, icv) = (item[0], item[1], item[3]);
    if l <= il {
        return (l / il) * ic;
    }
    let x = (1.0 - l) / (1.0 - il);
    ic * intersection_with_arc(x, icv)
}

#[inline(always)]
fn map_edge_seeker(oklch: &[f64; 3], max_chroma: f64, out: &mut [f64; 3]) {
    let (l, c, h) = (oklch[0], oklch[1], oklch[2]);
    if l <= 0.0 {
        *out = [0.0, 0.0, 0.0];
        return;
    }
    if l >= 1.0 {
        *out = [1.0, 1.0, 1.0];
        return;
    }
    oklch_to_clipped_p3(l, if c > max_chroma { max_chroma } else { c }, h, out);
}

struct EdgeSeeker;

impl EdgeSeeker {
    fn new() -> Self {
        EdgeSeeker
    }

    #[inline(always)]
    fn max_chroma(&mut self, l: f64, h: f64) -> f64 {
        if l <= 0.0 || l >= 1.0 {
            return 0.0;
        }
        max_chroma_from_item(l, get_lut_item(normalized_hue(h)))
    }

    #[inline(always)]
    fn map(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    fn map_with_in_gamut_check(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[f64; 3], out: &mut [f64; 3], check_in_gamut: bool) {
        if check_in_gamut && oklch_to_p3_if_in_gamut(oklch[0], oklch[1], oklch[2], out) {
            return;
        }
        let mc = self.max_chroma(oklch[0], oklch[2]);
        map_edge_seeker(oklch, mc, out);
    }
}

struct EdgeSeekerIndexed {
    interval_index: Vec<usize>,
}

impl EdgeSeekerIndexed {
    fn new() -> Self {
        EdgeSeekerIndexed {
            interval_index: build_interval_index(),
        }
    }

    #[inline(always)]
    fn max_chroma(&mut self, l: f64, h: f64) -> f64 {
        if l <= 0.0 || l >= 1.0 {
            return 0.0;
        }
        max_chroma_from_item(
            l,
            get_lut_item_indexed(normalized_hue(h), &self.interval_index),
        )
    }

    #[inline(always)]
    fn map(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    fn map_with_in_gamut_check(&mut self, oklch: &[f64; 3], out: &mut [f64; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[f64; 3], out: &mut [f64; 3], check_in_gamut: bool) {
        if check_in_gamut && oklch_to_p3_if_in_gamut(oklch[0], oklch[1], oklch[2], out) {
            return;
        }
        let mc = self.max_chroma(oklch[0], oklch[2]);
        map_edge_seeker(oklch, mc, out);
    }
}

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
    let mut s = 0.0;
    for _ in 0..warmup {
        s += pass();
    }
    let mut times = Vec::with_capacity(repeats);
    for _ in 0..repeats {
        let t0 = Instant::now();
        s += pass();
        times.push(t0.elapsed().as_nanos() as f64 / n as f64);
    }
    black_box(s);
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    times[times.len() / 2]
}

// Checksum (sum of all output channels) for cross-validation against JS.
fn checksum(samples: &[[f64; 3]], mut f: impl FnMut(&[f64; 3], &mut [f64; 3])) -> f64 {
    let mut out = [0.0; 3];
    let mut s = 0.0;
    for c in samples {
        f(c, &mut out);
        s += out[0] + out[1] + out[2];
    }
    s
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

fn time_method(
    warmup: usize,
    repeats: usize,
    samples: &[[f64; 3]],
    mut map: impl FnMut(&[f64; 3], &mut [f64; 3]),
) -> f64 {
    time_pass(warmup, repeats, samples.len(), || {
        let mut out = [0.0; 3];
        let mut sink = 0.0;
        for s in samples {
            map(s, &mut out);
            sink += out[0];
        }
        sink
    })
}

fn run_timings(label: &str, samples: &[[f64; 3]], warmup: usize, repeats: usize, check: bool) {
    // `check` selects the in-gamut-precheck variant of every method; the branch
    // is outside the timed loop so the toggle adds no per-call overhead.
    let clip_ns = time_method(warmup, repeats, samples, |s, o| clip(s, o));

    let mut cubic = OklchCubic::new();
    let cubic_ns = if check {
        time_method(warmup, repeats, samples, |s, o| {
            cubic.map_with_in_gamut_check(s, o)
        })
    } else {
        time_method(warmup, repeats, samples, |s, o| cubic.map(s, o))
    };

    let mut cubic_no_cache = OklchCubicNoCache::new();
    let cubic_no_cache_ns = if check {
        time_method(warmup, repeats, samples, |s, o| {
            cubic_no_cache.map_with_in_gamut_check(s, o)
        })
    } else {
        time_method(warmup, repeats, samples, |s, o| cubic_no_cache.map(s, o))
    };

    let mut halley = OklchHalley::new();
    let halley_ns = if check {
        time_method(warmup, repeats, samples, |s, o| {
            halley.map_with_in_gamut_check(s, o)
        })
    } else {
        time_method(warmup, repeats, samples, |s, o| halley.map(s, o))
    };

    let mut bottosson = BottossonLightness::new();
    let bottosson_ns = if check {
        time_method(warmup, repeats, samples, |s, o| {
            bottosson.map_with_in_gamut_check(s, o)
        })
    } else {
        time_method(warmup, repeats, samples, |s, o| bottosson.map(s, o))
    };

    let mut bottosson_cached = BottossonLightnessCached::new();
    let bottosson_cached_ns = if check {
        time_method(warmup, repeats, samples, |s, o| {
            bottosson_cached.map_with_in_gamut_check(s, o)
        })
    } else {
        time_method(warmup, repeats, samples, |s, o| bottosson_cached.map(s, o))
    };

    let mut raytrace = Raytrace::new();
    let raytrace_ns = if check {
        time_method(warmup, repeats, samples, |s, o| {
            raytrace.map_with_in_gamut_check(s, o)
        })
    } else {
        time_method(warmup, repeats, samples, |s, o| raytrace.map(s, o))
    };

    let mut edge = EdgeSeeker::new();
    let edge_ns = if check {
        time_method(warmup, repeats, samples, |s, o| {
            edge.map_with_in_gamut_check(s, o)
        })
    } else {
        time_method(warmup, repeats, samples, |s, o| edge.map(s, o))
    };

    let mut edge_indexed = EdgeSeekerIndexed::new();
    let edge_indexed_ns = if check {
        time_method(warmup, repeats, samples, |s, o| {
            edge_indexed.map_with_in_gamut_check(s, o)
        })
    } else {
        time_method(warmup, repeats, samples, |s, o| edge_indexed.map(s, o))
    };

    let mut timings = vec![
        ("clip", clip_ns),
        ("oklch-cubic (cached)", cubic_ns),
        ("oklch-cubic (no cache)", cubic_no_cache_ns),
        ("oklch-halley", halley_ns),
        ("bottosson-lightness", bottosson_ns),
        ("bottosson-lightness (cached)", bottosson_cached_ns),
        ("edge-seeker", edge_ns),
        ("edge-seeker (indexed)", edge_indexed_ns),
        ("raytrace", raytrace_ns),
    ];
    timings.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let fastest_ns = timings[0].1;
    let name_width = timings
        .iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(0);

    println!(
        "── {label} ── (median ns/call over {} passes, fastest to slowest):",
        repeats
    );
    for (name, ns) in timings {
        println!(
            "  {:<name_width$} {:6.2} ns/call  ({:.2}× fastest)",
            name,
            ns,
            ns / fastest_ns,
            name_width = name_width
        );
    }
    println!();
}

fn main() {
    let grid = build_grid();
    let random = build_random(grid.len());
    let n = grid.len();
    println!("dataset: {} OKLCh colors per workload (grid + random)\n", n);

    // `--in-gamut-check` times the in-gamut-precheck variant of every method
    // instead of the plain one, so a run shows one mode rather than both mixed.
    let check = std::env::args().any(|a| a == "--in-gamut-check");
    println!(
        "in-gamut precheck: {}\n",
        if check {
            "ENABLED (--in-gamut-check)"
        } else {
            "disabled (pass --in-gamut-check to enable)"
        }
    );

    let warmup = 50;
    let repeats = 25;

    // Checksums on the grid (for parity with the JS port).
    let mut cubic_cs = OklchCubic::new();
    let mut cubic_no_cache_cs = OklchCubicNoCache::new();
    let mut halley_cs = OklchHalley::new();
    let mut bottosson_cs = BottossonLightness::new();
    let mut bottosson_cached_cs = BottossonLightnessCached::new();
    let mut raytrace_cs = Raytrace::new();
    let mut edge_cs = EdgeSeeker::new();
    let cs_clip = checksum(&grid, |c, o| clip(c, o));
    let cs_cubic = checksum(&grid, |c, o| cubic_cs.map(c, o));
    let cs_cubic_no_cache = checksum(&grid, |c, o| cubic_no_cache_cs.map(c, o));
    let cs_halley = checksum(&grid, |c, o| halley_cs.map(c, o));
    let cs_bottosson = checksum(&grid, |c, o| bottosson_cs.map(c, o));
    let cs_bottosson_cached = checksum(&grid, |c, o| bottosson_cached_cs.map(c, o));
    let cs_raytrace = checksum(&grid, |c, o| raytrace_cs.map(c, o));
    let cs_edge = checksum(&grid, |c, o| edge_cs.map(c, o));
    println!("checksums on grid (sum of all P3 channels):");
    println!("  clip                 {:.10}", cs_clip);
    println!("  oklch-cubic-cached   {:.10}", cs_cubic);
    println!("  oklch-cubic-nocache  {:.10}", cs_cubic_no_cache);
    println!("  oklch-halley         {:.10}", cs_halley);
    println!("  bottosson-lightness  {:.10}", cs_bottosson);
    println!("  bottosson-cached     {:.10}", cs_bottosson_cached);
    println!("  raytrace             {:.10}", cs_raytrace);
    println!("  edge-seeker          {:.10}\n", cs_edge);

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
        let mut halley_eq = OklchHalley::new();
        let mut halley_checked_eq = OklchHalley::new();
        let halley_check_diff = max_channel_diff(
            samples,
            |c, o| halley_eq.map(c, o),
            |c, o| halley_checked_eq.map_with_in_gamut_check(c, o),
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
            .max(halley_check_diff)
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

    // The cached bottosson variant evaluates the hue-dependent structure at the
    // 0.1° bucket hue: bucket-exact grid hues must match to float noise; random
    // fractional hues are bounded by the hue quantization.
    {
        let mut exact = BottossonLightness::new();
        let mut cached = BottossonLightnessCached::new();
        let grid_diff = max_channel_diff(
            &grid,
            |c, o| exact.map(c, o),
            |c, o| cached.map(c, o),
        );
        if grid_diff > 1e-12 {
            panic!("bottosson cached differs on bucket-exact grid hues: max channel diff {grid_diff}");
        }
        let mut exact_r = BottossonLightness::new();
        let mut cached_r = BottossonLightnessCached::new();
        let random_diff = max_channel_diff(
            &random,
            |c, o| exact_r.map(c, o),
            |c, o| cached_r.map(c, o),
        );
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

    run_timings(
        "grid (H = 0..359 step 1, repeated per L)",
        &grid,
        warmup,
        repeats,
        check,
    );
    run_timings(
        "random (stratified/jittered fractional H + L)",
        &random,
        warmup,
        repeats,
        check,
    );
}
