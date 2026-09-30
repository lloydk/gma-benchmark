// Compiled twice with a concrete Float alias: all arithmetic, caches and LUT
// entries use that precision. SINGLE branches are resolved at compile time.
mod lut {
    use super::Float;
    include!("lut.rs");
}
use lut::LUT;
include!("conditioning.rs");

mod dualray {
    include!("dualray.rs");
}
pub(crate) use dualray::Dualray;

pub(crate) mod css_minde {
    include!("css_minde.rs");
}
pub(crate) mod gamut {
    include!("gamut.rs");
}
mod transfer {
    include!("transfer.rs");
}
pub(crate) mod color {
    include!("color.rs");
}
pub(crate) mod clip {
    include!("clip.rs");
}
mod p3_compat {
    include!("p3_compat.rs");
}
mod p3_fits {
    include!("p3_fits.rs");
}
use color::{KA0, KA1, KA2, KB0, KB1, KB2};
use p3_compat::*;
use p3_fits::*;
use transfer::clamp01;
include!("polynomial.rs");
pub(crate) mod rgb_solvers {
    include!("rgb_solvers.rs");
}

#[cfg(test)]
pub(crate) mod rgb_tests {
    include!("rgb_tests.rs");
}

const PI: Float = std::f64::consts::PI as Float;

// ── Method 7: Bottosson constant lightness ──────────────────────────────────

#[inline(always)]
fn compute_max_saturation_p3(a: Float, b: Float) -> Float {
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
fn find_cusp_p3(a: Float, b: Float) -> [Float; 2] {
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
fn find_gamut_intersection_p3(
    a: Float,
    b: Float,
    l1: Float,
    c1: Float,
    l0: Float,
    cusp: [Float; 2],
) -> Float {
    let mut t: Float;

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
        let tr = if ur >= 0.0 { -r * ur } else { Float::MAX };

        let g = GL * l3 + GM * m3 + GS * s3 - 1.0;
        let g1 = GL * ldt + GM * mdt + GS * sdt;
        let g2 = GL * ldt2 + GM * mdt2 + GS * sdt2;
        let ug = g1 / (g1 * g1 - 0.5 * g * g2);
        let tg = if ug >= 0.0 { -g * ug } else { Float::MAX };

        let blue = BL * l3 + BM * m3 + BS * s3 - 1.0;
        let blue1 = BL * ldt + BM * mdt + BS * sdt;
        let blue2 = BL * ldt2 + BM * mdt2 + BS * sdt2;
        let ub = blue1 / (blue1 * blue1 - 0.5 * blue * blue2);
        let tb = if ub >= 0.0 { -blue * ub } else { Float::MAX };

        t += tr.min(tg.min(tb));
    }

    t
}

pub(crate) struct BottossonLightness;

impl BottossonLightness {
    pub(crate) fn new() -> Self {
        BottossonLightness
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
        let mut l = oklch[0];
        let c = oklch[1].max(0.0);
        let h = oklch[2];

        if c <= BOTTOSSON_EPSILON {
            l = clamp01(l);
            let gray = clamped_gamma(l * l * l);
            *out = [gray, gray, gray];
            return;
        }

        let hr = hue_radians(h);
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

// ── Method 7b: Bottosson constant lightness, cached ─────────────────────────
// The cusp and the LMS' hue slopes depend only on hue, so they are memoized in
// 0.1° buckets (same bucketed-hue semantics as oklch-cubic's hue structure).
// Each bucket stores [cusp_l, cusp_c, q0, q1, q2] where q_i is the LMS' slope
// for the hue direction: LMS'_i(L, C) = L + C * q_i. With those cached, the
// per-call path needs no trig at all.

// find_gamut_intersection_p3 with the hue expressed as LMS' slopes q0..q2 (the
// kl/km/ks it would otherwise recompute) and the cusp as scalars.
#[inline(always)]
fn find_gamut_intersection_q(
    q0: Float,
    q1: Float,
    q2: Float,
    l1: Float,
    c1: Float,
    l0: Float,
    cusp_l: Float,
    cusp_c: Float,
) -> Float {
    let mut t: Float;

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
        let tr = if ur >= 0.0 { -r * ur } else { Float::MAX };

        let g = GL * l3 + GM * m3 + GS * s3 - 1.0;
        let g1 = GL * ldt + GM * mdt + GS * sdt;
        let g2 = GL * ldt2 + GM * mdt2 + GS * sdt2;
        let ug = g1 / (g1 * g1 - 0.5 * g * g2);
        let tg = if ug >= 0.0 { -g * ug } else { Float::MAX };

        let blue = BL * l3 + BM * m3 + BS * s3 - 1.0;
        let blue1 = BL * ldt + BM * mdt + BS * sdt;
        let blue2 = BL * ldt2 + BM * mdt2 + BS * sdt2;
        let ub = blue1 / (blue1 * blue1 - 0.5 * blue * blue2);
        let tb = if ub >= 0.0 { -blue * ub } else { Float::MAX };

        t += tr.min(tg.min(tb));
    }

    t
}

// LMS'-slope form of the OKLab -> clipped P3 conversion: LMS'_i = L + C * q_i.
#[inline(always)]
fn lms_slopes_to_clipped_p3(
    l: Float,
    c: Float,
    q0: Float,
    q1: Float,
    q2: Float,
    out: &mut [Float; 3],
) {
    rgb_solvers::lms_slopes_to_clipped_rgb::<crate::rgb_spaces::DisplayP3>(l, c, q0, q1, q2, out);
}

#[inline(always)]
fn lms_slopes_to_p3_if_in_gamut(
    l: Float,
    c: Float,
    q0: Float,
    q1: Float,
    q2: Float,
    out: &mut [Float; 3],
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

pub(crate) struct BottossonLightnessCached {
    cache: Vec<[Float; 5]>, // 3601 buckets of 0.1°; slot 0 (cusp L) is never 0 once filled
}

impl BottossonLightnessCached {
    pub(crate) fn new() -> Self {
        BottossonLightnessCached {
            cache: vec![[0.0; 5]; 3601],
        }
    }

    #[inline(always)]
    fn cusp_data(&mut self, h: Float) -> [Float; 5] {
        let mut hh = h % 360.0;
        if hh < 0.0 {
            hh += 360.0;
        }
        let key = (hh * 10.0).round() as usize;
        if self.cache[key][0] != 0.0 {
            return self.cache[key];
        }
        let rad = key as Float / 10.0 * PI / 180.0;
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
    pub(crate) fn map(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    pub(crate) fn map_with_in_gamut_check(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[Float; 3], out: &mut [Float; 3], check_in_gamut: bool) {
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

// ── Method 9: edge-seeker ───────────────────────────────────────────────────
// Reduce chroma to a precomputed LUT of the gamut edge. The lookup evaluates the
// LUT at the exact normalized hue.

#[inline(always)]
fn lerp(a: Float, b: Float, t: Float) -> Float {
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

fn find_closest(hue: Float) -> (usize, usize) {
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

fn lerp_lut(start: &[Float; 4], end: &[Float; 4], hue: Float) -> [Float; 4] {
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

fn get_lut_item(h: Float) -> [Float; 4] {
    let (lo, hi) = find_closest(h);
    lerp_lut(&LUT[lo], &LUT[hi], h)
}

fn build_interval_index() -> Vec<usize> {
    let mut intervals = vec![0; HUE_INDEX_BUCKETS];
    let mut interval = 0;
    for (bucket, slot) in intervals.iter_mut().enumerate() {
        let hue = bucket as Float / HUE_INDEX_SCALE as Float;
        while interval + 1 < LUT.len() - 1 && LUT[interval + 1][2] <= hue {
            interval += 1;
        }
        *slot = interval;
    }
    intervals
}

fn get_lut_item_indexed(h: Float, interval_index: &[usize]) -> [Float; 4] {
    let bucket = ((h * HUE_INDEX_SCALE as Float) as usize).min(HUE_INDEX_BUCKETS - 1);
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
fn normalized_hue(h: Float) -> Float {
    if h < 0.0 {
        (h % 360.0) + 360.0
    } else {
        h % 360.0
    }
}

#[inline(always)]
fn intersection_with_arc(x: Float, curvature: Float) -> Float {
    if curvature == 0.0 {
        return x;
    }
    // Solve k*y² + (t-k)*y - x*(t+k*(1-x)) = 0, t = sqrt(2-k²).
    // The rationalized root avoids cancellation near k=0 and never switches
    // to the other circle intersection because of endpoint rounding.
    let t = (2.0 - curvature * curvature).sqrt();
    let b = t - curvature;
    let d = x * (t + curvature * (1.0 - x));
    let disc = (b * b + 4.0 * curvature * d).max(0.0);
    (2.0 * d / (b + disc.sqrt())).clamp(0.0, 1.0)
}

#[inline(always)]
fn max_chroma_from_item(l: Float, item: [Float; 4]) -> Float {
    let (il, ic, icv) = (item[0], item[1], item[3]);
    if l <= il {
        return (l / il) * ic;
    }
    let x = (1.0 - l) / (1.0 - il);
    ic * intersection_with_arc(x, icv)
}

#[inline(always)]
fn map_edge_seeker(oklch: &[Float; 3], max_chroma: Float, out: &mut [Float; 3]) {
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

pub(crate) struct EdgeSeeker;

impl EdgeSeeker {
    pub(crate) fn new() -> Self {
        EdgeSeeker
    }

    #[inline(always)]
    fn max_chroma(&mut self, l: Float, h: Float) -> Float {
        if l <= 0.0 || l >= 1.0 {
            return 0.0;
        }
        max_chroma_from_item(l, get_lut_item(normalized_hue(h)))
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
        if check_in_gamut && oklch_to_p3_if_in_gamut(oklch[0], oklch[1], oklch[2], out) {
            return;
        }
        let mc = self.max_chroma(oklch[0], oklch[2]);
        map_edge_seeker(oklch, mc, out);
    }
}

pub(crate) struct EdgeSeekerIndexed {
    interval_index: Vec<usize>,
}

impl EdgeSeekerIndexed {
    pub(crate) fn new() -> Self {
        EdgeSeekerIndexed {
            interval_index: build_interval_index(),
        }
    }

    #[inline(always)]
    fn max_chroma(&mut self, l: Float, h: Float) -> Float {
        if l <= 0.0 || l >= 1.0 {
            return 0.0;
        }
        max_chroma_from_item(
            l,
            get_lut_item_indexed(normalized_hue(h), &self.interval_index),
        )
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
        if check_in_gamut && oklch_to_p3_if_in_gamut(oklch[0], oklch[1], oklch[2], out) {
            return;
        }
        let mc = self.max_chroma(oklch[0], oklch[2]);
        map_edge_seeker(oklch, mc, out);
    }
}

#[cfg(test)]
mod edge_seeker_tests {
    include!("edge_seeker_tests.rs");
}
