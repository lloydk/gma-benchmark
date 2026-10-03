// Bottosson's constant-lightness approximation with algorithm-owned target data.
#[cfg(test)]
use super::color::Oklch;
use super::color::{Oklab, KA0, KA1, KA2, KB0, KB1, KB2};
use super::compensated::{mul_add, product_error};
use super::gamut::{DisplayP3, Rec2020, RgbGamut, Srgb};
use super::rgb_solvers::{oklab_to_rgb_if_in_gamut, oklch_to_rgb_if_in_gamut};
use super::transfer::{clamp01, TransferFunction};
use super::SINGLE;
use super::{hue_bucket, hue_radians, Float, PI};
use std::marker::PhantomData;

const BOTTOSSON_EPSILON: Float = 1e-12;

pub(crate) trait BottossonData: RgbGamut {
    const PRIMARY_HUES: [f64; 3];
    const HUE_BOUNDS: [[[Float; 2]; 2]; 3] = split_primary_hues(Self::PRIMARY_HUES);
    const RED_SECTOR: [Float; 2];
    const GREEN_SECTOR: [Float; 2];
    const SATURATION_FITS: [[Float; 5]; 3];
}
macro_rules! fitted {
    ($gamut:ty, $module:ident, $file:literal) => {
        mod $module {
            use super::Float;
            include!($file);
        }
        impl BottossonData for $gamut {
            const PRIMARY_HUES: [f64; 3] = $module::PRIMARY_HUES;
            const RED_SECTOR: [Float; 2] = $module::RED_SECTOR;
            const GREEN_SECTOR: [Float; 2] = $module::GREEN_SECTOR;
            const SATURATION_FITS: [[Float; 5]; 3] = $module::SATURATION_FITS;
        }
    };
}
fitted!(Srgb, srgb, "generated/bottosson_srgb.rs");
fitted!(Rec2020, rec2020, "generated/bottosson_rec2020.rs");
fitted!(DisplayP3, p3, "generated/bottosson_display_p3.rs");

// Build native high/low thresholds at compile time. Comparing a negative
// hue to the negative threshold avoids losing bits by adding 360 in f32.
const fn split_primary_hues(hues: [f64; 3]) -> [[[Float; 2]; 2]; 3] {
    let mut out = [[[0.0; 2]; 2]; 3];
    let mut i = 0;
    while i < 3 {
        let mut sign = 0;
        while sign < 2 {
            let exact = hues[i] - 360.0 * sign as f64;
            let high = exact as Float;
            out[i][sign] = [high, (exact - high as f64) as Float];
            sign += 1;
        }
        i += 1;
    }
    out
}

#[inline(always)]
fn sector<G: BottossonData>(a: Float, b: Float, h: Float) -> usize {
    let red = a * G::RED_SECTOR[0] + b * G::RED_SECTOR[1];
    let green = a * G::GREEN_SECTOR[0] + b * G::GREEN_SECTOR[1];
    // Both half-planes can round to <=1 at their shared blue primary,
    // incorrectly selecting the third (blue) face. Resolve narrow contacts
    // using authored hue in either precision; f32 retains split thresholds.
    if (red - 1.0).abs() <= 16.0 * Float::EPSILON || (green - 1.0).abs() <= 16.0 * Float::EPSILON {
        let h = h % 360.0;
        let sign = usize::from(h < 0.0);
        let below = |i: usize| {
            let [high, low] = G::HUE_BOUNDS[i][sign];
            h < high || (h == high && low > 0.0)
        };
        if !below(1) && below(2) {
            return 0;
        }
        if below(0) || !below(2) {
            return 1;
        }
        return 2;
    }
    if red > 1.0 {
        0
    } else if green > 1.0 {
        1
    } else {
        2
    }
}

// Compensate the cancellation-prone channel residual using native scalars.
// product_error recovers each product's rounding residual; the low cube part also
// retains the rounding lost when forming 1 + saturation * slope.
#[inline(always)]
fn saturation_residual(sat: Float, q: [Float; 3], weights: [Float; 3]) -> Float {
    let terms = std::array::from_fn::<_, 3, _>(|i| {
        let x = mul_add(sat, q[i], 1.0);
        let dx = mul_add(sat, q[i], 1.0 - x);
        let x2 = x * x;
        let dx2 = product_error(x, x, x2) + 2.0 * x * dx;
        let x3 = x2 * x;
        let dx3 = product_error(x2, x, x3) + dx2 * x + x2 * dx;
        let product = weights[i] * x3;
        [
            product,
            product_error(weights[i], x3, product) + weights[i] * dx3,
        ]
    });
    let sum = terms[0][0] + terms[1][0];
    let recovered = sum - terms[0][0];
    let low = (terms[0][0] - (sum - recovered)) + (terms[1][0] - recovered);
    let total = sum + terms[2][0];
    let recovered = total - sum;
    let low = low + (sum - (total - recovered)) + (terms[2][0] - recovered);
    total + (low + terms[0][1] + terms[1][1] + terms[2][1])
}

// ── Method 7: Bottosson constant lightness ──────────────────────────────────

#[inline(always)]
fn compute_max_saturation<G: BottossonData, const FUSED: bool>(
    a: Float,
    b: Float,
    h: Float,
) -> Float {
    let channel = sector::<G>(a, b, h);
    let [k0, k1, k2, k3, k4] = G::SATURATION_FITS[channel];
    let [wl, wm, ws] = G::LMS_TO_RGB[channel];

    let a2 = a * a;
    let sat = if FUSED {
        mul_add(
            k4 * a,
            b,
            mul_add(k3 * a, a, mul_add(k2, b, mul_add(k1, a, k0))),
        )
    } else {
        k0 + k1 * a + k2 * b + k3 * a2 + k4 * a * b
    };
    let (kl, km, ks) = if FUSED {
        (
            mul_add(KA0, a, KB0 * b),
            mul_add(KA1, a, KB1 * b),
            mul_add(KA2, a, KB2 * b),
        )
    } else {
        (KA0 * a + KB0 * b, KA1 * a + KB1 * b, KA2 * a + KB2 * b)
    };
    let (l, m, s) = if FUSED {
        (
            mul_add(sat, kl, 1.0),
            mul_add(sat, km, 1.0),
            mul_add(sat, ks, 1.0),
        )
    } else {
        (1.0 + sat * kl, 1.0 + sat * km, 1.0 + sat * ks)
    };
    let l2 = l * l;
    let m2 = m * m;
    let s2 = s * s;
    let (f, f1, f2) = if FUSED {
        (
            saturation_residual(sat, [kl, km, ks], [wl, wm, ws]),
            3.0 * mul_add(wl * kl, l2, mul_add(wm * km, m2, ws * ks * s2)),
            6.0 * mul_add(wl * kl * kl, l, mul_add(wm * km * km, m, ws * ks * ks * s)),
        )
    } else {
        (
            wl * l2 * l + wm * m2 * m + ws * s2 * s,
            3.0 * (wl * kl * l2 + wm * km * m2 + ws * ks * s2),
            6.0 * (wl * kl * kl * l + wm * km * km * m + ws * ks * ks * s),
        )
    };

    sat - (f * f1) / (f1 * f1 - 0.5 * f * f2)
}

#[inline(always)]
fn find_cusp<G: BottossonData>(a: Float, b: Float, h: Float) -> [Float; 2] {
    // Near blue, the saturation residual amplifies the error from forming a
    // large f32 radian angle. Rotate a small angle around 270 degrees instead.
    // Keep the authored-hue canonical conversion on its original path.
    let (a, b, s_cusp) = if SINGLE {
        let h = if h <= -360.0 || h >= 360.0 {
            h % 360.0
        } else {
            h
        };
        let primary = G::HUE_BOUNDS[2][usize::from(h < 0.0)][0];
        if (h - primary).abs() < 1.0 {
            let delta = if h < 0.0 { h + 90.0 } else { h - 270.0 };
            let angle = delta * PI / 180.0;
            let (a, cos) = angle.sin_cos();
            let b = -cos;
            (a, b, compute_max_saturation::<G, true>(a, b, h))
        } else {
            (a, b, compute_max_saturation::<G, false>(a, b, h))
        }
    } else {
        (a, b, compute_max_saturation::<G, false>(a, b, h))
    };
    let l = 1.0 + s_cusp * (KA0 * a + KB0 * b);
    let m = 1.0 + s_cusp * (KA1 * a + KB1 * b);
    let s = 1.0 + s_cusp * (KA2 * a + KB2 * b);
    let l3 = l * l * l;
    let m3 = m * m * m;
    let s3 = s * s * s;
    let r = G::LMS_TO_RGB[0][0] * l3 + G::LMS_TO_RGB[0][1] * m3 + G::LMS_TO_RGB[0][2] * s3;
    let g = G::LMS_TO_RGB[1][0] * l3 + G::LMS_TO_RGB[1][1] * m3 + G::LMS_TO_RGB[1][2] * s3;
    let blue = G::LMS_TO_RGB[2][0] * l3 + G::LMS_TO_RGB[2][1] * m3 + G::LMS_TO_RGB[2][2] * s3;
    let l_cusp = (1.0 / r.max(g).max(blue)).cbrt();

    [l_cusp, l_cusp * s_cusp]
}

#[inline(always)]
fn find_gamut_intersection<G: BottossonData>(
    a: Float,
    b: Float,
    l1: Float,
    c1: Float,
    l0: Float,
    cusp: [Float; 2],
) -> Float {
    find_gamut_intersection_q::<G>(
        a * KA0 + b * KB0,
        a * KA1 + b * KB1,
        a * KA2 + b * KB2,
        l1,
        c1,
        l0,
        cusp[0],
        cusp[1],
    )
}

pub(crate) struct BottossonLightness<G>(PhantomData<G>);

impl<G: BottossonData> BottossonLightness<G> {
    pub(crate) fn new() -> Self {
        BottossonLightness(PhantomData)
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
            if check_in_gamut && oklch_to_rgb_if_in_gamut::<G>(l, oklch[1], h, out) {
                return;
            }
            l = clamp01(l);
            let gray = G::Transfer::encode_clamped(l * l * l);
            *out = [gray, gray, gray];
            return;
        }

        let hr = hue_radians(h);
        let (unit_b, unit_a) = hr.sin_cos();
        let lab_a = c * unit_a;
        let lab_b = c * unit_b;

        if check_in_gamut && oklab_to_rgb_if_in_gamut::<G>(l, lab_a, lab_b, out) {
            return;
        }

        let cusp = find_cusp::<G>(unit_a, unit_b, h);
        let l0 = clamp01(l);
        let t = find_gamut_intersection::<G>(unit_a, unit_b, l, c, l0, cusp);
        let mapped_l = l0 + t * (l - l0);
        let mapped_c = t * c;

        *out = Oklab {
            l: mapped_l,
            a: mapped_c * unit_a,
            b: mapped_c * unit_b,
        }
        .to_linear_rgb::<G>()
        .encode_clamped()
        .channels;
    }
}

// ── Method 7b: Bottosson constant lightness, cached ─────────────────────────
// The cusp and the LMS' hue slopes depend only on hue, so they are memoized in
// 0.1° buckets (same bucketed-hue semantics as oklch-cubic's hue structure).
// Each bucket stores [cusp_l, cusp_c, q0, q1, q2] where q_i is the LMS' slope
// for the hue direction: LMS'_i(L, C) = L + C * q_i. With those cached, the
// plain mapping path needs no trig after the cache is warm. Checked mapping
// still converts the authored hue before consulting this quantized cache.

// find_gamut_intersection with the hue expressed as LMS' slopes q0..q2 (the
// kl/km/ks it would otherwise recompute) and the cusp as scalars.
#[inline(always)]
fn find_gamut_intersection_q<G: BottossonData>(
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
        let l_value = l0 + t * (l1 - l0);
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

        let r =
            G::LMS_TO_RGB[0][0] * l3 + G::LMS_TO_RGB[0][1] * m3 + G::LMS_TO_RGB[0][2] * s3 - 1.0;
        let r1 = G::LMS_TO_RGB[0][0] * ldt + G::LMS_TO_RGB[0][1] * mdt + G::LMS_TO_RGB[0][2] * sdt;
        let r2 =
            G::LMS_TO_RGB[0][0] * ldt2 + G::LMS_TO_RGB[0][1] * mdt2 + G::LMS_TO_RGB[0][2] * sdt2;
        let ur = r1 / (r1 * r1 - 0.5 * r * r2);
        let tr = if ur >= 0.0 { -r * ur } else { Float::MAX };

        let g =
            G::LMS_TO_RGB[1][0] * l3 + G::LMS_TO_RGB[1][1] * m3 + G::LMS_TO_RGB[1][2] * s3 - 1.0;
        let g1 = G::LMS_TO_RGB[1][0] * ldt + G::LMS_TO_RGB[1][1] * mdt + G::LMS_TO_RGB[1][2] * sdt;
        let g2 =
            G::LMS_TO_RGB[1][0] * ldt2 + G::LMS_TO_RGB[1][1] * mdt2 + G::LMS_TO_RGB[1][2] * sdt2;
        let ug = g1 / (g1 * g1 - 0.5 * g * g2);
        let tg = if ug >= 0.0 { -g * ug } else { Float::MAX };

        let blue =
            G::LMS_TO_RGB[2][0] * l3 + G::LMS_TO_RGB[2][1] * m3 + G::LMS_TO_RGB[2][2] * s3 - 1.0;
        let blue1 =
            G::LMS_TO_RGB[2][0] * ldt + G::LMS_TO_RGB[2][1] * mdt + G::LMS_TO_RGB[2][2] * sdt;
        let blue2 =
            G::LMS_TO_RGB[2][0] * ldt2 + G::LMS_TO_RGB[2][1] * mdt2 + G::LMS_TO_RGB[2][2] * sdt2;
        let ub = blue1 / (blue1 * blue1 - 0.5 * blue * blue2);
        let tb = if ub >= 0.0 { -blue * ub } else { Float::MAX };

        t += tr.min(tg.min(tb));
    }

    t
}

pub(crate) struct BottossonLightnessCached<G> {
    gamut: PhantomData<G>,
    pub(super) cache: Vec<[Float; 5]>, // 3601 buckets of 0.1°; slot 0 (cusp L) is never 0 once filled
}

impl<G: BottossonData> BottossonLightnessCached<G> {
    pub(crate) fn new() -> Self {
        BottossonLightnessCached {
            gamut: PhantomData,
            cache: vec![[0.0; 5]; 3601],
        }
    }

    #[inline(always)]
    fn cusp_data(&mut self, h: Float) -> [Float; 5] {
        let key = hue_bucket(h);
        if self.cache[key][0] != 0.0 {
            return self.cache[key];
        }
        let rad = key as Float / 10.0 * PI / 180.0;
        let (unit_b, unit_a) = rad.sin_cos();
        let cusp = find_cusp::<G>(unit_a, unit_b, key as Float / 10.0);
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
            if check_in_gamut && oklch_to_rgb_if_in_gamut::<G>(l, oklch[1], h, out) {
                return;
            }
            l = clamp01(l);
            let gray = G::Transfer::encode_clamped(l * l * l);
            *out = [gray, gray, gray];
            return;
        }

        if check_in_gamut && oklch_to_rgb_if_in_gamut::<G>(l, c, h, out) {
            return;
        }
        let d = self.cusp_data(h);
        let (cusp_l, cusp_c, q0, q1, q2) = (d[0], d[1], d[2], d[3], d[4]);

        let l0 = clamp01(l);
        let t = find_gamut_intersection_q::<G>(q0, q1, q2, l, c, l0, cusp_l, cusp_c);
        let mapped_l = l0 + t * (l - l0);
        let mapped_c = t * c;

        super::rgb_solvers::lms_slopes_to_clipped_rgb::<G>(mapped_l, mapped_c, q0, q1, q2, out);
    }
}

#[cfg(test)]
mod tests {
    include!("bottosson_tests.rs");
}
