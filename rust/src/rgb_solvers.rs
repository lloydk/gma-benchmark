// Matrix-driven gamut mappers, monomorphized for each target and precision.
use super::color::{LinearRgb, Oklab, Oklch};
use super::gamut::RgbGamut;
use super::transfer::TransferFunction;
use super::*;
use std::marker::PhantomData;

#[inline(always)]
fn encode<G: RgbGamut>(x: Float) -> Float {
    G::Transfer::encode_clamped(x)
}
#[inline(always)]
pub(super) fn oklch_to_clipped_rgb<G: RgbGamut>(
    l: Float,
    c: Float,
    h: Float,
    out: &mut [Float; 3],
) {
    super::clip::Clip::<G>::new().map(&[l, c, h], out);
}
#[inline(always)]
pub(super) fn oklch_to_rgb_if_in_gamut<G: RgbGamut>(
    l: Float,
    c: Float,
    h: Float,
    out: &mut [Float; 3],
) -> bool {
    let lab = Oklch::from([l, c, h]).to_oklab();
    oklab_to_rgb_if_in_gamut::<G>(lab.l, lab.a, lab.b, out)
}
#[inline(always)]
pub(super) fn oklab_to_rgb_if_in_gamut<G: RgbGamut>(
    l: Float,
    a: Float,
    b: Float,
    out: &mut [Float; 3],
) -> bool {
    let rgb = Oklab { l, a, b }.to_linear_rgb::<G>();
    if !rgb.in_gamut() {
        return false;
    }
    *out = rgb.encode_clamped().channels;
    true
}
#[inline(always)]
fn oklab_to_linear_rgb_components<G: RgbGamut>(
    l: Float,
    a: Float,
    b: Float,
) -> (Float, Float, Float) {
    let [r, g, b] = Oklab { l, a, b }.to_linear_rgb::<G>().channels;
    (r, g, b)
}
#[inline(always)]
fn linear_rgb_to_oklab_chroma<G: RgbGamut>(r: Float, g: Float, b: Float) -> Float {
    let lab = LinearRgb::<G>::new([r, g, b]).to_oklab();
    (lab.a * lab.a + lab.b * lab.b).sqrt()
}

#[inline(always)]
pub(super) fn lms_slopes_to_clipped_rgb<G: RgbGamut>(
    l: Float,
    c: Float,
    q0: Float,
    q1: Float,
    q2: Float,
    out: &mut [Float; 3],
) {
    let l0 = l + c * q0;
    let m0 = l + c * q1;
    let s0 = l + c * q2;
    let l3 = l0 * l0 * l0;
    let m3 = m0 * m0 * m0;
    let s3 = s0 * s0 * s0;
    out[0] =
        encode::<G>(G::LMS_TO_RGB[0][0] * l3 + G::LMS_TO_RGB[0][1] * m3 + G::LMS_TO_RGB[0][2] * s3);
    out[1] =
        encode::<G>(G::LMS_TO_RGB[1][0] * l3 + G::LMS_TO_RGB[1][1] * m3 + G::LMS_TO_RGB[1][2] * s3);
    out[2] =
        encode::<G>(G::LMS_TO_RGB[2][0] * l3 + G::LMS_TO_RGB[2][1] * m3 + G::LMS_TO_RGB[2][2] * s3);
}

// ── Raytrace constants ──
const RAYTRACE_LOW: Float = if SINGLE { 8.0 * Float::EPSILON } else { 1e-12 };
const RAYTRACE_HIGH: Float = 1.0 - RAYTRACE_LOW;

// Keep small, nonzero directions: flushing them can ignore the nearest face
// close to white or black in either precision.
#[inline(always)]
fn exit_t(ar: Float, ag: Float, ab: Float, dr: Float, dg: Float, db: Float) -> Float {
    stable_exit_t(ar, ag, ab, dr, dg, db)
}

// ── Method 2: oklch-cubic (cached) ──────────────────────────────────────────
// Solve, per linear-RGB channel, the cubic in t = C/L where the channel exits
// [0,1]; the smallest root is the max in-gamut chroma. Per-hue structure cached.

#[derive(Clone, Copy)]
pub(super) struct HueData {
    a: [Float; 3],
    b: [Float; 3],
    d: [Float; 3],
    t_lower: Float,
    turn: [Float; 3],
}

#[inline(always)]
fn first_turn(d: Float, b: Float, a: Float) -> Float {
    first_root(0.0, d, 2.0 * b, a, 1e-12, Float::INFINITY)
}

fn get_hue_data<G: RgbGamut>(h: Float) -> HueData {
    let rad = hue_radians(h);
    let (sin, cos) = rad.sin_cos();
    let q0 = KA0 * cos + KB0 * sin;
    let q1 = KA1 * cos + KB1 * sin;
    let q2 = KA2 * cos + KB2 * sin;
    let a = [
        G::LMS_TO_RGB[0][0] * q0 + G::LMS_TO_RGB[0][1] * q1 + G::LMS_TO_RGB[0][2] * q2,
        G::LMS_TO_RGB[1][0] * q0 + G::LMS_TO_RGB[1][1] * q1 + G::LMS_TO_RGB[1][2] * q2,
        G::LMS_TO_RGB[2][0] * q0 + G::LMS_TO_RGB[2][1] * q1 + G::LMS_TO_RGB[2][2] * q2,
    ];
    let (q0b, q1b, q2b) = (q0 * q0, q1 * q1, q2 * q2);
    let b = [
        G::LMS_TO_RGB[0][0] * q0b + G::LMS_TO_RGB[0][1] * q1b + G::LMS_TO_RGB[0][2] * q2b,
        G::LMS_TO_RGB[1][0] * q0b + G::LMS_TO_RGB[1][1] * q1b + G::LMS_TO_RGB[1][2] * q2b,
        G::LMS_TO_RGB[2][0] * q0b + G::LMS_TO_RGB[2][1] * q1b + G::LMS_TO_RGB[2][2] * q2b,
    ];
    let (q0c, q1c, q2c) = (q0b * q0, q1b * q1, q2b * q2);
    let d = [
        G::LMS_TO_RGB[0][0] * q0c + G::LMS_TO_RGB[0][1] * q1c + G::LMS_TO_RGB[0][2] * q2c,
        G::LMS_TO_RGB[1][0] * q0c + G::LMS_TO_RGB[1][1] * q1c + G::LMS_TO_RGB[1][2] * q2c,
        G::LMS_TO_RGB[2][0] * q0c + G::LMS_TO_RGB[2][1] * q1c + G::LMS_TO_RGB[2][2] * q2c,
    ];
    let mut t_lower = Float::INFINITY;
    let mut turn = [0.0; 3];
    for i in 0..3 {
        t_lower = t_lower.min(first_root(
            d[i],
            3.0 * b[i],
            3.0 * a[i],
            1.0,
            1e-9,
            Float::INFINITY,
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

pub(crate) struct OklchCubic<G> {
    gamut: PhantomData<G>,
    // 3601 buckets of 0.1°, stored without Option so the cache stays a dense
    // 13 scalars/bucket array (104 B f64, 52 B f32); t_lower (a root strictly
    // greater than 1e-9, or +inf) doubles as the filled marker: 0 means empty.
    pub(super) cache: Vec<HueData>,
}

impl<G: RgbGamut> OklchCubic<G> {
    pub(crate) fn new() -> Self {
        OklchCubic {
            gamut: PhantomData,
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
    fn hue_data(&mut self, h: Float) -> &HueData {
        let key = hue_bucket(h);
        if self.cache[key].t_lower == 0.0 {
            self.cache[key] = get_hue_data::<G>(key as Float / 10.0);
        }
        // Borrow the entry to avoid a full-record stack copy on every cache hit.
        // The native f64 copy was sensitive to the caller's stack alignment.
        &self.cache[key]
    }

    #[inline(always)]
    pub(crate) fn map(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    pub(crate) fn map_with_in_gamut_check(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[Float; 3], out: &mut [Float; 3], check_in_gamut: bool) {
        let (l, c, h) = (oklch[0], oklch[1], oklch[2]);
        if l <= 0.0 || l >= 1.0 || c <= 0.0 {
            let ll = if l <= 0.0 {
                0.0
            } else if l >= 1.0 {
                1.0
            } else {
                l
            };
            oklch_to_clipped_rgb::<G>(ll, 0.0, h, out);
            return;
        }
        if check_in_gamut && oklch_to_rgb_if_in_gamut::<G>(l, c, h, out) {
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
            max_t = max_t.min(first_face_root(
                d[i],
                3.0 * b[i],
                3.0 * a[i],
                dd,
                max_t,
                true,
            ));
        }

        let l3 = l * l * l;
        out[0] =
            encode::<G>(l3 * (((d[0] * max_t + 3.0 * b[0]) * max_t + 3.0 * a[0]) * max_t + 1.0));
        out[1] =
            encode::<G>(l3 * (((d[1] * max_t + 3.0 * b[1]) * max_t + 3.0 * a[1]) * max_t + 1.0));
        out[2] =
            encode::<G>(l3 * (((d[2] * max_t + 3.0 * b[2]) * max_t + 3.0 * a[2]) * max_t + 1.0));
    }
}

// ── Method 3: oklch-cubic (no cache) ─────────────────────────────────────────
// Same fixed 0.1° bucket semantics as the cached variant, but recomputes the
// per-hue cubic structure for every call. The root solver and conditioning
// are shared with the cached implementation.

#[derive(Clone, Copy)]
struct NoCacheHueData {
    a: [Float; 3],
    b: [Float; 3],
    d: [Float; 3],
    t_lower: Float,
    turn: [Float; 3],
}

#[inline(always)]
fn first_turn_no_cache(d: Float, b: Float, a: Float) -> Float {
    first_root(0.0, d, 2.0 * b, a, 1e-12, Float::INFINITY)
}

fn get_hue_data_no_cache<G: RgbGamut>(h: Float) -> NoCacheHueData {
    let bucket_h = hue_bucket(h) as Float / 10.0;
    let rad = hue_radians(bucket_h);
    let (sin, cos) = rad.sin_cos();
    let q0 = KA0 * cos + KB0 * sin;
    let q1 = KA1 * cos + KB1 * sin;
    let q2 = KA2 * cos + KB2 * sin;
    let a = [
        G::LMS_TO_RGB[0][0] * q0 + G::LMS_TO_RGB[0][1] * q1 + G::LMS_TO_RGB[0][2] * q2,
        G::LMS_TO_RGB[1][0] * q0 + G::LMS_TO_RGB[1][1] * q1 + G::LMS_TO_RGB[1][2] * q2,
        G::LMS_TO_RGB[2][0] * q0 + G::LMS_TO_RGB[2][1] * q1 + G::LMS_TO_RGB[2][2] * q2,
    ];
    let (q0b, q1b, q2b) = (q0 * q0, q1 * q1, q2 * q2);
    let b = [
        G::LMS_TO_RGB[0][0] * q0b + G::LMS_TO_RGB[0][1] * q1b + G::LMS_TO_RGB[0][2] * q2b,
        G::LMS_TO_RGB[1][0] * q0b + G::LMS_TO_RGB[1][1] * q1b + G::LMS_TO_RGB[1][2] * q2b,
        G::LMS_TO_RGB[2][0] * q0b + G::LMS_TO_RGB[2][1] * q1b + G::LMS_TO_RGB[2][2] * q2b,
    ];
    let (q0c, q1c, q2c) = (q0b * q0, q1b * q1, q2b * q2);
    let d = [
        G::LMS_TO_RGB[0][0] * q0c + G::LMS_TO_RGB[0][1] * q1c + G::LMS_TO_RGB[0][2] * q2c,
        G::LMS_TO_RGB[1][0] * q0c + G::LMS_TO_RGB[1][1] * q1c + G::LMS_TO_RGB[1][2] * q2c,
        G::LMS_TO_RGB[2][0] * q0c + G::LMS_TO_RGB[2][1] * q1c + G::LMS_TO_RGB[2][2] * q2c,
    ];
    let mut t_lower = Float::INFINITY;
    let mut turn = [0.0; 3];
    for i in 0..3 {
        t_lower = t_lower.min(first_root(
            d[i],
            3.0 * b[i],
            3.0 * a[i],
            1.0,
            1e-9,
            Float::INFINITY,
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

pub(crate) struct OklchCubicNoCache<G>(PhantomData<G>);

impl<G: RgbGamut> OklchCubicNoCache<G> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }

    #[inline(always)]
    pub(crate) fn map(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    pub(crate) fn map_with_in_gamut_check(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[Float; 3], out: &mut [Float; 3], check_in_gamut: bool) {
        let (l, c, h) = (oklch[0], oklch[1], oklch[2]);
        if l <= 0.0 || l >= 1.0 || c <= 0.0 {
            let ll = if l <= 0.0 {
                0.0
            } else if l >= 1.0 {
                1.0
            } else {
                l
            };
            oklch_to_clipped_rgb::<G>(ll, 0.0, h, out);
            return;
        }
        if check_in_gamut && oklch_to_rgb_if_in_gamut::<G>(l, c, h, out) {
            return;
        }

        let hd = get_hue_data_no_cache::<G>(h);
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
            max_t = max_t.min(first_face_root(
                d[i],
                3.0 * b[i],
                3.0 * a[i],
                dd,
                max_t,
                true,
            ));
        }

        let l3 = l * l * l;
        out[0] =
            encode::<G>(l3 * (((d[0] * max_t + 3.0 * b[0]) * max_t + 3.0 * a[0]) * max_t + 1.0));
        out[1] =
            encode::<G>(l3 * (((d[1] * max_t + 3.0 * b[1]) * max_t + 3.0 * a[1]) * max_t + 1.0));
        out[2] =
            encode::<G>(l3 * (((d[2] * max_t + 3.0 * b[2]) * max_t + 3.0 * a[2]) * max_t + 1.0));
    }
}

// ── Method 4: oklch-cubic-direct ────────────────────────────────────────────
// Solve each linear-RGB channel's complete cubic in chroma at the input
// lightness and exact hue. At C = 0.5, choose whichever of the channel's 0 or 1
// bounds is nearer, solve that one cubic, then take the earliest of the three
// channel roots. Tiny-root and Newton conditioning lives in polynomial.rs.
// Ported from
// color-js/apps#44 issuecomment-4998357355.

#[inline(always)]
fn max_chroma_cubic_direct<G: RgbGamut>(l: Float, q0: Float, q1: Float, q2: Float) -> Float {
    let rows = G::LMS_TO_RGB;
    let l2 = l * l;
    let l3 = l * l2;
    let lx3 = l * 3.0;
    let l2x3 = l2 * 3.0;
    let (q0b, q1b, q2b) = (q0 * q0, q1 * q1, q2 * q2);
    let (q0c, q1c, q2c) = (q0b * q0, q1b * q1, q2b * q2);

    let mut best = Float::INFINITY;
    for [w0, w1, w2] in rows {
        let c1 = q0c * w0 + q1c * w1 + q2c * w2;
        let c2 = lx3 * (q0b * w0 + q1b * w1 + q2b * w2);
        let c3 = l2x3 * (q0 * w0 + q1 * w1 + q2 * w2);
        let c4 = l3 * (w0 + w1 + w2);

        // Rounded matrix row sums can put a near-white neutral just
        // outside this upper face. There is then no positive departure root.
        if c4 > 1.0 {
            return 0.0;
        }
        if c4 == 1.0 {
            best = best.min(face_exit(c1, c2, c3, true, best));
        }

        let at_half = ((c1 * 0.5 + c2) * 0.5 + c3) * 0.5 + c4;
        let target = if (at_half - 1.0).abs() < at_half.abs() {
            1.0
        } else {
            0.0
        };
        best = best.min(first_root_cubic_direct(
            c1,
            c2,
            c3,
            c4 - target,
            best,
            target == 1.0,
        ));
    }
    best
}

pub(crate) struct OklchCubicDirect<G>(PhantomData<G>);

impl<G: RgbGamut> OklchCubicDirect<G> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }

    #[inline(always)]
    pub(crate) fn map(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    pub(crate) fn map_with_in_gamut_check(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[Float; 3], out: &mut [Float; 3], check_in_gamut: bool) {
        let (l, c, h) = (oklch[0], oklch[1], oklch[2]);
        if l <= 0.0 || l >= 1.0 || c <= 0.0 {
            let ll = if l <= 0.0 {
                0.0
            } else if l >= 1.0 {
                1.0
            } else {
                l
            };
            oklch_to_clipped_rgb::<G>(ll, 0.0, h, out);
            return;
        }
        if check_in_gamut && oklch_to_rgb_if_in_gamut::<G>(l, c, h, out) {
            return;
        }

        let rad = hue_radians(h);
        let (sin, cos) = rad.sin_cos();
        let q0 = KA0 * cos + KB0 * sin;
        let q1 = KA1 * cos + KB1 * sin;
        let q2 = KA2 * cos + KB2 * sin;
        let mapped_c = c.min(max_chroma_cubic_direct::<G>(l, q0, q1, q2));

        let l0 = l + mapped_c * q0;
        let m0 = l + mapped_c * q1;
        let s0 = l + mapped_c * q2;
        let ll = l0 * l0 * l0;
        let mm = m0 * m0 * m0;
        let ss = s0 * s0 * s0;
        out[0] = encode::<G>(
            G::LMS_TO_RGB[0][0] * ll + G::LMS_TO_RGB[0][1] * mm + G::LMS_TO_RGB[0][2] * ss,
        );
        out[1] = encode::<G>(
            G::LMS_TO_RGB[1][0] * ll + G::LMS_TO_RGB[1][1] * mm + G::LMS_TO_RGB[1][2] * ss,
        );
        out[2] = encode::<G>(
            G::LMS_TO_RGB[2][0] * ll + G::LMS_TO_RGB[2][1] * mm + G::LMS_TO_RGB[2][2] * ss,
        );
    }
}

// Preserve the iterative methods' outer blue intersection policy. The max
// of six constraints is not smooth at a corner, so a small iteration step
// does not certify a boundary there. Restrict geometric recovery to the fold.
#[inline(always)]
pub(crate) fn in_blue_fold<G: RgbGamut>(h: Float) -> bool {
    in_blue_fold_id(G::ID, h)
}

#[inline(always)]
pub(crate) fn in_blue_fold_id(id: crate::rgb_spaces::SpaceId, h: Float) -> bool {
    let Some([lo, hi]) = crate::rgb_spaces::blue_fold_window(id) else {
        return false;
    };
    let h = if h > -360.0 && h < 360.0 {
        h
    } else {
        h % 360.0
    };
    // Avoid both a remainder for ordinary hues and cancellation from adding
    // 360 to negative f32 hues near a window endpoint.
    (h >= lo as Float && h <= hi as Float) || (h >= lo as Float - 360.0 && h <= hi as Float - 360.0)
}

// Every monotone face interval is bracketed separately. This finds re-entries
// as well as first exits, without assuming ray membership is monotone.
fn outer_fold_boundary<G: RgbGamut>(l: Float, q: [Float; 3], input_c: Float) -> Float {
    let polynomials = G::LMS_TO_RGB.map(|w| {
        [
            w[0] * q[0] * q[0] * q[0] + w[1] * q[1] * q[1] * q[1] + w[2] * q[2] * q[2] * q[2],
            3.0 * (w[0] * q[0] * q[0] + w[1] * q[1] * q[1] + w[2] * q[2] * q[2]),
            3.0 * (w[0] * q[0] + w[1] * q[1] + w[2] * q[2]),
            w[0] + w[1] + w[2],
        ]
    });
    let white = 1.0 / (l * l * l);
    let cap = input_c.min(0.5);
    let limit = cap / l;
    // A chroma-reducing policy: retain valid re-entry colors, but an input
    // in a gap must map to the preceding feasible exit, not be clipped.
    if polynomials.iter().all(|&p| {
        let value = fold_eval(p, limit);
        value >= 0.0 && value <= white
    }) {
        return cap;
    }
    let mut best: Float = 0.0;
    for [a, b, c, d] in polynomials {
        let mut points = [0.0, limit, limit, limit];
        let disc = b * b - 3.0 * a * c;
        if a != 0.0 && disc >= 0.0 {
            let v = -b - disc.sqrt().copysign(b);
            for (i, x) in [v / (3.0 * a), c / v].into_iter().enumerate() {
                if x > 0.0 && x < limit {
                    points[i + 1] = x;
                }
            }
        } else if a == 0.0 && b != 0.0 {
            let x = -c / (2.0 * b);
            if x > 0.0 && x < limit {
                points[1] = x;
            }
        }
        points.sort_by(Float::total_cmp);
        for face in [0.0, white] {
            let p = [a, b, c, d - face];
            for pair in points.windows(2) {
                let (mut lo, mut hi) = (pair[0], pair[1]);
                let (fl, fh) = (fold_eval(p, lo), fold_eval(p, hi));
                // Outward crossings only; tangencies do not leave the cube.
                if if face == 0.0 {
                    !(fl >= 0.0 && fh < 0.0)
                } else {
                    !(fl <= 0.0 && fh > 0.0)
                } {
                    continue;
                }
                // The initial bracket scales as 1/L. Near black it can need
                // more than one mantissa of halvings before adjacent floats.
                for _ in 0..(Float::MAX_EXP as u32 + Float::MANTISSA_DIGITS) {
                    let mid = lo + (hi - lo) * 0.5;
                    if mid == lo || mid == hi {
                        break;
                    }
                    if (fold_eval(p, mid) < 0.0) == (fl < 0.0) {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let x = lo + (hi - lo) * 0.5;
                // Feasibility includes the other two channels. The slack
                // covers arithmetic roundoff, not an iteration's residual.
                let feasible = polynomials.iter().all(|&p| {
                    let v = fold_eval(p, x);
                    let scale = ((p[0].abs() * x + p[1].abs()) * x + p[2].abs()) * x + p[3].abs();
                    let slack = 8.0 * Float::EPSILON * scale;
                    v >= -slack && v <= white + slack
                });
                if feasible {
                    best = best.max(x);
                }
            }
        }
    }
    l * best
}

// Compensated Horner, still native to the lane. Cancellation at the narrow
// blue gap must not determine which disconnected interval is feasible.
#[inline(always)]
fn fold_eval([a, b, c, d]: [Float; 4], x: Float) -> Float {
    let mut value = a;
    let mut error = 0.0;
    for coefficient in [b, c, d] {
        let product = value * x;
        let product_error = super::compensated::product_error(value, x, product);
        let sum = product + coefficient;
        let z = sum - product;
        let sum_error = (product - (sum - z)) + (coefficient - z);
        error = error * x + (product_error + sum_error);
        value = sum;
    }
    value + error
}

// ── Method 5: oklch-halley ──────────────────────────────────────────────────

// Bracketed Halley solve for the first RGB channel boundary along a
// constant-lightness, constant-hue chroma ray. Ported from color-js/apps#44.
#[inline(always)]
fn solve_halley_iteration<G: RgbGamut>(
    l_value: Float,
    q0: Float,
    q1: Float,
    q2: Float,
    seed: Float,
    mut lo: Float,
    mut hi: Float,
) -> Float {
    let rows = G::LMS_TO_RGB;
    let mut c = seed;
    if c.is_nan() {
        let l3 = l_value * l_value * l_value;
        let l2x3 = 3.0 * l_value * l_value;
        c = hi;
        for row in rows {
            let slope = l2x3 * (row[0] * q0 + row[1] * q1 + row[2] * q2);
            let crossing = if slope > 0.0 {
                (1.0 - l3) / slope
            } else if slope < 0.0 {
                -l3 / slope
            } else {
                Float::INFINITY
            };
            if crossing < c {
                c = crossing;
            }
        }
    }

    let mut best = c;
    let mut best_err = Float::INFINITY;

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

        let mut g = Float::NEG_INFINITY;
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
        if step.abs() < STEP_TOLERANCE || (SINGLE && c - step == c) {
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

#[inline(always)]
fn solve_halley<G: RgbGamut>(
    l: Float,
    h: Float,
    q0: Float,
    q1: Float,
    q2: Float,
    input_c: Float,
) -> Float {
    if in_blue_fold::<G>(h) {
        outer_fold_boundary::<G>(l, [q0, q1, q2], input_c)
    } else {
        solve_halley_iteration::<G>(l, q0, q1, q2, Float::NAN, 0.0, 0.5)
    }
}

pub(crate) struct OklchHalley<G>(PhantomData<G>);

impl<G: RgbGamut> OklchHalley<G> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }

    #[inline(always)]
    pub(crate) fn map(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    pub(crate) fn map_with_in_gamut_check(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[Float; 3], out: &mut [Float; 3], check_in_gamut: bool) {
        let (l, c, h) = (oklch[0], oklch[1], oklch[2]);
        if l <= 0.0 || l >= 1.0 || c <= 0.0 {
            let ll = if l <= 0.0 {
                0.0
            } else if l >= 1.0 {
                1.0
            } else {
                l
            };
            oklch_to_clipped_rgb::<G>(ll, 0.0, h, out);
            return;
        }
        if check_in_gamut && oklch_to_rgb_if_in_gamut::<G>(l, c, h, out) {
            return;
        }

        let rad = hue_radians(h);
        let (sin, cos) = rad.sin_cos();
        let q0 = KA0 * cos + KB0 * sin;
        let q1 = KA1 * cos + KB1 * sin;
        let q2 = KA2 * cos + KB2 * sin;
        let mapped_c = c.min(solve_halley::<G>(l, h, q0, q1, q2, c));
        lms_slopes_to_clipped_rgb::<G>(l, mapped_c, q0, q1, q2, out);
    }
}

// ── Method 6: oklch-ostrowski ───────────────────────────────────────────────

// Bracketed Ostrowski solve proposed in the color-js/apps#44 follow-up. This
// replaces Halley's second-derivative step with a Newton step followed by the
// fourth-order Ostrowski correction, while retaining the same bracket and seed.
#[inline(always)]
fn solve_ostrowski_iteration<G: RgbGamut>(
    l_value: Float,
    q0: Float,
    q1: Float,
    q2: Float,
    seed: Float,
    mut lo: Float,
    mut hi: Float,
) -> Float {
    let rows = G::LMS_TO_RGB;
    let mut c = seed;
    if c.is_nan() {
        let l3 = l_value * l_value * l_value;
        let l2x3 = 3.0 * l_value * l_value;
        c = hi;
        for row in rows {
            let d1 = l2x3 * (row[0] * q0 + row[1] * q1 + row[2] * q2);
            let crossing = if d1 > 0.0 {
                (1.0 - l3) / d1
            } else if d1 < 0.0 {
                -l3 / d1
            } else {
                Float::INFINITY
            };
            if crossing < c {
                c = crossing;
            }
        }
    }

    let mut best = c;
    let mut best_err = Float::INFINITY;

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

        let mut g = Float::NEG_INFINITY;
        let mut g1 = 0.0;
        for row in rows {
            let v = row[0] * l3 + row[1] * m3 + row[2] * s3;
            let d1 = 3.0 * (row[0] * q0 * l2 + row[1] * q1 * m2 + row[2] * q2 * s2);
            if v - 1.0 > g {
                g = v - 1.0;
                g1 = d1;
            }
            if -v > g {
                g = -v;
                g1 = -d1;
            }
        }

        if g > 0.0 {
            hi = c;
        } else {
            lo = c;
        }

        if g1 == 0.0 {
            c = (lo + hi) / 2.0;
            continue;
        }

        let mut step = g / g1;
        if step.abs() < best_err {
            best_err = step.abs();
            best = c;
        }

        if step.abs() < STEP_TOLERANCE || (SINGLE && c - step == c) {
            return c;
        }

        let mut next = c - step;
        let l = l_value + next * q0;
        let m = l_value + next * q1;
        let s = l_value + next * q2;
        let l2 = l * l;
        let m2 = m * m;
        let s2 = s * s;
        let l3 = l2 * l;
        let m3 = m2 * m;
        let s3 = s2 * s;

        let mut g_at_next = Float::NEG_INFINITY;
        for row in rows {
            let v = row[0] * l3 + row[1] * m3 + row[2] * s3;
            if v - 1.0 > g_at_next {
                g_at_next = v - 1.0;
            }
            if -v > g_at_next {
                g_at_next = -v;
            }
        }

        let twice_g_at_next = 2.0 * g_at_next;
        if twice_g_at_next != g {
            step = g / (g - twice_g_at_next) * (g_at_next / g1);

            if step.abs() < best_err {
                best_err = step.abs();
                best = next;
            }

            if step.abs() < STEP_TOLERANCE || (SINGLE && next - step == next) {
                return next;
            }

            next -= step;
        }

        c = if next > lo && next < hi {
            next
        } else {
            (lo + hi) / 2.0
        };
    }

    best
}

#[inline(always)]
fn solve_ostrowski<G: RgbGamut>(
    l: Float,
    h: Float,
    q0: Float,
    q1: Float,
    q2: Float,
    input_c: Float,
) -> Float {
    if in_blue_fold::<G>(h) {
        outer_fold_boundary::<G>(l, [q0, q1, q2], input_c)
    } else {
        solve_ostrowski_iteration::<G>(l, q0, q1, q2, Float::NAN, 0.0, 0.5)
    }
}

pub(crate) struct OklchOstrowski<G>(PhantomData<G>);

impl<G: RgbGamut> OklchOstrowski<G> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }

    #[inline(always)]
    pub(crate) fn map(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    pub(crate) fn map_with_in_gamut_check(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[Float; 3], out: &mut [Float; 3], check_in_gamut: bool) {
        let (l, c, h) = (oklch[0], oklch[1], oklch[2]);
        if l <= 0.0 || l >= 1.0 || c <= 0.0 {
            let ll = if l <= 0.0 {
                0.0
            } else if l >= 1.0 {
                1.0
            } else {
                l
            };
            oklch_to_clipped_rgb::<G>(ll, 0.0, h, out);
            return;
        }
        if check_in_gamut && oklch_to_rgb_if_in_gamut::<G>(l, c, h, out) {
            return;
        }

        let rad = hue_radians(h);
        let (sin, cos) = rad.sin_cos();
        let q0 = KA0 * cos + KB0 * sin;
        let q1 = KA1 * cos + KB1 * sin;
        let q2 = KA2 * cos + KB2 * sin;
        let mapped_c = c.min(solve_ostrowski::<G>(l, h, q0, q1, q2, c));
        lms_slopes_to_clipped_rgb::<G>(l, mapped_c, q0, q1, q2, out);
    }
}

// ── Method 8: raytrace ───────────────────────────────────────────────────────

pub(crate) struct Raytrace<G>(PhantomData<G>);

impl<G: RgbGamut> Raytrace<G> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }

    #[inline(always)]
    pub(crate) fn map(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    pub(crate) fn map_with_in_gamut_check(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[Float; 3], out: &mut [Float; 3], check_in_gamut: bool) {
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
            oklch_to_clipped_rgb::<G>(l, 0.0, h, out);
            return;
        }
        let hr = hue_radians(h);
        let (unit_b, unit_a) = hr.sin_cos();
        let (mut mr, mut mg, mut mb) =
            oklab_to_linear_rgb_components::<G>(l, c * unit_a, c * unit_b);
        if check_in_gamut
            && mr >= 0.0
            && mr <= 1.0
            && mg >= 0.0
            && mg <= 1.0
            && mb >= 0.0
            && mb <= 1.0
        {
            out[0] = encode::<G>(mr);
            out[1] = encode::<G>(mg);
            out[2] = encode::<G>(mb);
            return;
        }

        let anchor = l * l * l;
        // If L^3 underflows in the current precision, the anchor sits on
        // the cube corner, so there is no interior ray anchor.
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
                let corrected_c = linear_rgb_to_oklab_chroma::<G>(mr, mg, mb);
                (mr, mg, mb) = oklab_to_linear_rgb_components::<G>(
                    l,
                    corrected_c * unit_a,
                    corrected_c * unit_b,
                );
            }

            // Near convergence, a ray direction can consist of rounding noise.
            // Also compare with the last hit: near white the anchor can be
            // farther away, while reprojection of the hit changes only a few
            // ULPs. Casting that noise can select the opposite cube face.
            if i != 0
                && ((mr - ar).abs().max((mg - ag).abs()).max((mb - ab).abs())
                    <= (if SINGLE { 8.0 } else { 32.0 })
                        * Float::EPSILON
                        * ar.abs().max(ag.abs()).max(ab.abs())
                    || (mr - last_r)
                        .abs()
                        .max((mg - last_g).abs())
                        .max((mb - last_b).abs())
                        <= (if SINGLE { 8.0 } else { 32.0 })
                            * Float::EPSILON
                            * last_r.abs().max(last_g.abs()).max(last_b.abs()))
            {
                mr = last_r;
                mg = last_g;
                mb = last_b;
                break;
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

        out[0] = encode::<G>(mr);
        out[1] = encode::<G>(mg);
        out[2] = encode::<G>(mb);
    }
}

#[cfg(test)]
mod tests {
    include!("rgb_solver_tests.rs");
}
