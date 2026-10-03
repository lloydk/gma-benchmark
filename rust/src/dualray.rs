// Dualray: guarded upper-first shortcut and upper-face retry.
// Target-specific basis and seeds; retains the benchmark intrinsic-in-gamut policy.
// Fold windows and every rejected guard use one exact first-exit search; in
// f32 fold windows red is evaluated around its minimum with a fitted depth.
// The balanced seed evaluation matches the JS port. The corrections reuse
// Horner intermediates and cancel common factors, changing last-bit rounding.
// Arithmetic stays in the containing module's precision.

use super::gamut::{DisplayP3, Rec2020, RgbGamut, Srgb};
use super::rgb_solvers::in_blue_fold;
use super::transfer::TransferFunction;
use super::{Float, PI, SINGLE};
use std::marker::PhantomData;

const TOLERANCE: Float = if SINGLE { 8.0 * Float::EPSILON } else { 1e-12 };
const HUE_FAST_LIMIT: Float = if SINGLE { 360.0 } else { 1e9 };

pub(crate) trait DualrayData: RgbGamut {
    const BASIS: [[Float; 9]; 3];
    const SECTORS: [[Float; 2]; 2];
    // Red's fold hue split into two Float constants (and the same less 360,
    // for negative hues), and its fitted dip depth near the fold.
    const FOLD: [Float; 2];
    const FOLD_NEG: [Float; 2];
    const FOLD_DEPTH: [Float; 6];
    const ROOT_LIMIT: Float = crate::dualray_config::config(Self::ID).root_limit as Float;
    fn seed(a: Float, b: Float, face: u8) -> Float;
}
macro_rules! fitted {
    ($gamut:ty, $module:ident, $file:literal) => {
        mod $module {
            use super::Float;
            include!($file);
        }
        impl DualrayData for $gamut {
            const BASIS: [[Float; 9]; 3] = narrow_rows($module::BASIS);
            const SECTORS: [[Float; 2]; 2] = $module::SECTORS;
            const FOLD: [Float; 2] = split($module::FOLD_HUE);
            const FOLD_NEG: [Float; 2] = split($module::FOLD_HUE - 360.0);
            const FOLD_DEPTH: [Float; 6] = narrow($module::FOLD_DEPTH);
            #[inline(always)]
            fn seed(a: Float, b: Float, face: u8) -> Float {
                $module::seed(a, b, face)
            }
        }
    };
}
fitted!(Srgb, srgb, "generated/dualray_srgb.rs");
fitted!(DisplayP3, p3, "generated/dualray_display_p3.rs");
fitted!(Rec2020, rec2020, "generated/dualray_rec2020.rs");

// A binary64 constant as a Float plus its rounding remainder.
const fn split(x: f64) -> [Float; 2] {
    let high = x as Float;
    [high, (x - high as f64) as Float]
}

// Binary64 data rounded to Float (shared with Dualray Fast).
pub(crate) const fn narrow<const N: usize>(input: [f64; N]) -> [Float; N] {
    let mut result = [0.0; N];
    let mut i = 0;
    while i < N {
        result[i] = input[i] as Float;
        i += 1;
    }
    result
}

pub(crate) const fn narrow_rows<const N: usize>(input: [[f64; N]; 3]) -> [[Float; N]; 3] {
    [narrow(input[0]), narrow(input[1]), narrow(input[2])]
}

// A channel cubic along the ray, divided by L³: ((d·u + b)·u + a)·u + r.
// Rows are normalized so r = 1, except red's local form near an f32 fold.
#[inline(always)]
pub(crate) fn cubic([d, b, a, r]: [Float; 4], u: Float) -> Float {
    ((d * u + b) * u + a) * u + r
}

// The three channel cubics in u = C/L at hue (cos, sin) = (a, b), from the
// target's basis. Shared with Dualray Fast.
#[inline(always)]
pub(crate) fn ray_rows<G: DualrayData>(a: Float, b: Float) -> [[Float; 4]; 3] {
    let a2 = a * a;
    let ab = a * b;
    let a3 = a2 * a;
    let a2b = a2 * b;
    G::BASIS.map(|k| {
        [
            k[0] * a3 + k[1] * a2b + k[2] * a + k[3] * b,
            k[4] + k[5] * a2 + k[6] * ab,
            k[7] * a + k[8] * b,
            1.0,
        ]
    })
}

// The cubic, its derivative and half its second derivative at u, sharing
// Horner intermediates.
#[inline(always)]
fn derivatives([d, b, a, r]: [Float; 4], u: Float) -> (Float, Float, Float) {
    let du = d * u;
    let q = du + b;
    let p = q * u + a;
    (p * u + r, (du + q) * u + p, (du + du) + q)
}

// Interior Bernstein controls bound the whole interval, not just its endpoint.
// Controls and guard are scaled by three, avoiding two divisions.
#[inline(always)]
fn interior_within([_, b, a, _]: [Float; 4], u: Float, guard: Float) -> bool {
    let limit = 3.0 * guard;
    let linear = a * u;
    let c1 = 3.0 + linear;
    let c2 = 3.0 + 2.0 * linear + b * u * u;
    c1 >= 0.0 && c1 <= limit && c2 >= 0.0 && c2 <= limit
}

// One Halley step toward the channel's zero.
#[inline(always)]
fn polish(x: Float, row: [Float; 4]) -> Float {
    let (f, f1, half_second) = derivatives(row, x);
    // Cancel the common factor of two in the Halley correction.
    let denominator = f1 * f1 - f * half_second;
    if denominator != 0.0 {
        x - (f * f1) / denominator
    } else {
        x
    }
}

// First exit along u in [0, limit] from 0 <= channel <= target. Each row is
// a cubic in u - origin[k] (an origin of zero is plain u). Every channel is
// monotone
// between consecutive stationary points, so the first infeasible breakpoint
// brackets the exit and feasibility is monotone inside it. A tangent touch
// stays feasible: it does not leave the gamut. Returns the last feasible u
// and, if it exits, the first infeasible one. Shared with Dualray Fast.
pub(crate) fn first_exit(
    rows: &[[Float; 4]; 3],
    origin: [Float; 3],
    target: Float,
    limit: Float,
) -> (Float, Option<Float>) {
    let feasible = |u: Float| {
        (0..3).all(|k| {
            let v = cubic(rows[k], u - origin[k]);
            v >= 0.0 && v <= target
        })
    };
    let mut points = [limit; 7];
    let mut n = 0;
    for (&[d, b, a, _], shift) in rows.iter().zip(origin) {
        // Roots of the derivative 3d·u² + 2b·u + a.
        let disc = b * b - 3.0 * d * a;
        let roots = if d != 0.0 && disc >= 0.0 {
            [
                (-b - disc.sqrt()) / (3.0 * d),
                (-b + disc.sqrt()) / (3.0 * d),
            ]
        } else if d == 0.0 && b != 0.0 {
            [-a / (2.0 * b), limit]
        } else {
            [limit; 2]
        };
        for u in roots.map(|root| root + shift) {
            if u > 0.0 && u < limit {
                points[n] = u;
                n += 1;
            }
        }
    }
    n += 1;
    points[..n].sort_by(|x, y| x.total_cmp(y));
    let mut lo = 0.0;
    for &end in &points[..n] {
        if !feasible(end) {
            let mut hi = end;
            loop {
                let mid = lo + (hi - lo) * 0.5;
                if mid <= lo || mid >= hi {
                    return (lo, Some(hi));
                }
                if feasible(mid) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
        }
        lo = end;
    }
    (limit, None)
}

// Two Householder steps toward one upper face from `u`. Returns the converged
// first upper exit in [0, limit] and the channel values there.
// - `near_white` (the upper-first seed): skip the second step once the
//   residual is within tolerance, and reject rather than retry.
// - Otherwise (the chord seed): always take both steps, because near bright
//   yellow the face channel is almost flat and a small f32 residual can leave
//   the root inaccurate. If another channel exceeds the target, it exits
//   first: retry on it once, never moving away from neutral.
// Shared with Dualray Fast.
#[inline(always)]
pub(crate) fn upper(
    rows: [[Float; 4]; 3],
    target: Float,
    mut u: Float,
    mut face: usize,
    limit: Float,
    near_white: bool,
) -> Option<(Float, usize, [Float; 3])> {
    let guard = target * (1.0 + TOLERANCE);
    for retry in [false, true] {
        let previous = u;
        let row = pick(rows, face);
        for step in 0..2 {
            let (value, f1, half_second) = derivatives(row, u);
            let f = value - target;
            if near_white && step == 1 && f.abs() <= target * TOLERANCE {
                break;
            }
            let f1_squared = f1 * f1;
            let product = f * half_second;
            // Cancel the common factor of six in the Householder correction.
            let denominator = f1 * (f1_squared - 2.0 * product) + f * f * row[0];
            if denominator == 0.0 {
                break;
            }
            u -= f * (f1_squared - product) / denominator;
        }
        let values = rows.map(|row| cubic(row, u));
        if !(u >= 0.0
            && u <= limit
            && (!retry || u <= previous)
            && values.iter().all(|&v| v >= -TOLERANCE)
            && (pick(values, face) - target).abs() <= target * TOLERANCE)
        {
            return None;
        }
        let brightest = brightest(values);
        if pick(values, brightest) <= guard {
            return Some((u, face, values));
        }
        if near_white {
            return None;
        }
        face = brightest;
    }
    None
}

// f32 blue-fold windows. Near the fold red's dip is nearly tangent: its
// depth, a sum of O(1) terms close to zero, is lost to f32 rounding, and with
// it the first exit. Around its local minimum the cubic is exactly
// depth + c2·δ² + d·δ³, and only the depth is ill-conditioned: it comes from
// a binary64 fit in the hue offset from the fold. The offset is exact
// (Sterbenz) against a fold constant on the same side of zero as the hue.
// Returns the rows and origins for `first_exit`.
#[inline(never)]
fn fold_local<G: DualrayData>(
    mut rows: [[Float; 4]; 3],
    hue: Float,
) -> ([[Float; 4]; 3], [Float; 3]) {
    let mut origin = [0.0; 3];
    let [d, b, a, _] = rows[0];
    let disc = (b * b - 3.0 * d * a).sqrt();
    let minimum = [(-b - disc) / (3.0 * d), (-b + disc) / (3.0 * d)]
        .into_iter()
        .find(|&u| u > 0.0 && 3.0 * d * u + b > 0.0);
    if let Some(u) = minimum {
        let fold = if hue < 0.0 { G::FOLD_NEG } else { G::FOLD };
        let offset = (hue - fold[0]) - fold[1];
        let [centre, inv_half, coef @ ..] = G::FOLD_DEPTH;
        let t = (offset - centre) * inv_half;
        let depth = offset * coef.iter().rev().fold(0.0, |sum, &c| sum * t + c);
        rows[0] = [d, 3.0 * d * u + b, 0.0, depth];
        origin[0] = u;
    }
    (rows, origin)
}

// The exact first exit bounded by the input (u <= limit), for blue-fold hues
// (where re-entry islands exist and fitted roots do not apply) and any
// rejected guard. An input before it keeps its conversion; at an exit the
// crossed channel is exactly on its face, as is an input exactly on the upper
// face (L³·target need not round to one). Shared with Dualray Fast.
#[inline(never)]
pub(crate) fn search<G: DualrayData>(
    rows: [[Float; 4]; 3],
    hue: Float,
    l3: Float,
    target: Float,
    limit: Float,
    encode: impl Fn(Float) -> Float,
) -> [Float; 3] {
    let (rows, origin) = if SINGLE && in_blue_fold::<G>(hue) {
        fold_local::<G>(rows, hue)
    } else {
        (rows, [0.0; 3])
    };
    let (u, beyond) = first_exit(&rows, origin, target, limit);
    let at = |u: Float| -> [Float; 3] { std::array::from_fn(|k| cubic(rows[k], u - origin[k])) };
    let values = at(u);
    let face = beyond.and_then(|hi| {
        let beyond = at(hi);
        (0..3)
            .find(|&k| beyond[k] < 0.0 || beyond[k] > target)
            .map(|k| (k, beyond[k] > target))
    });
    std::array::from_fn(|k| match face {
        Some((f, upper)) if f == k => {
            if upper {
                1.0
            } else {
                0.0
            }
        }
        _ if values[k] >= target => 1.0,
        _ => encode(l3 * values[k]),
    })
}

// Select by channel index without indexing memory: runtime-indexed arrays
// would leave the hot path's values on the stack.
#[inline(always)]
fn pick<T: Copy>([r, g, b]: [T; 3], k: usize) -> T {
    if k == 0 {
        r
    } else if k == 1 {
        g
    } else {
        b
    }
}

#[inline(always)]
fn brightest([r, g, b]: [Float; 3]) -> usize {
    if r > g {
        if r > b {
            0
        } else {
            2
        }
    } else if g > b {
        1
    } else {
        2
    }
}

pub(crate) struct Dualray<G: DualrayData>(PhantomData<G>);

impl<G: DualrayData> Dualray<G> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }

    // Intrinsic boundary comparisons are also the checked variant's policy.
    #[inline(always)]
    pub(crate) fn map_with_in_gamut_check(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map(oklch, out);
    }

    // An input before the first exit keeps its normalized conversion.
    #[inline(always)]
    fn interior(rows: [[Float; 4]; 3], l3: Float, input_u: Float) -> [Float; 3] {
        rows.map(|row| G::Transfer::encode_clamped(l3 * cubic(row, input_u)))
    }

    // Encoded channels at an exit, with the crossed channel exactly on its
    // face (0 or 1).
    #[inline(always)]
    fn on_face(l3: Float, values: [Float; 3], face: usize, bound: Float) -> [Float; 3] {
        std::array::from_fn(|k| {
            if k == face {
                bound
            } else {
                G::Transfer::encode_clamped(l3 * values[k])
            }
        })
    }

    // A validated exit at u: the input keeps its conversion if it lies before.
    #[inline(always)]
    fn exit(
        rows: [[Float; 4]; 3],
        l3: Float,
        input_u: Float,
        (u, values, face, bound): (Float, [Float; 3], usize, Float),
    ) -> [Float; 3] {
        if input_u < u {
            Self::interior(rows, l3, input_u)
        } else {
            Self::on_face(l3, values, face, bound)
        }
    }

    #[inline(always)]
    pub(crate) fn map(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        let [l, c, h] = *oklch;
        if l <= 0.0 || l >= 1.0 || c <= 0.0 {
            let neutral = if l <= 0.0 {
                0.0
            } else if l >= 1.0 {
                1.0
            } else {
                G::Transfer::encode_clamped(l * l * l)
            };
            *out = [neutral; 3];
            return;
        }
        let hue = if h > -HUE_FAST_LIMIT && h < HUE_FAST_LIMIT {
            h
        } else {
            h % 360.0
        };
        let radians = hue * (PI / 180.0);
        let (b, a) = radians.sin_cos();
        let l3 = l * l * l;
        let rows = ray_rows::<G>(a, b);
        let inv_l = 1.0 / l;
        let input_u = c * inv_l;
        let target = inv_l * inv_l * inv_l;
        let limit = input_u.min(G::ROOT_LIMIT);
        let fold = in_blue_fold::<G>(hue);
        let encode = G::Transfer::encode_clamped;
        if SINGLE && fold {
            *out = search::<G>(rows, hue, l3, target, limit, encode);
            return;
        }

        // Predict one upper face near white. The gate is a performance
        // heuristic; the solve's guards and containment decide acceptance.
        let delta = target - 1.0;
        let slopes = rows.map(|row| row[2]);
        let max_slope = slopes[0].max(slopes[1]).max(slopes[2]);
        if delta > 0.0 && delta < 0.15 * max_slope {
            let face = slopes.iter().position(|&s| s == max_slope).unwrap_or(2);
            let wb = pick(rows, face)[1];
            let seed =
                (2.0 * delta) / (max_slope + (max_slope * max_slope + 4.0 * wb * delta).sqrt());
            let guard = target * (1.0 + TOLERANCE);
            if let Some((u, face, values)) = upper(rows, target, seed, face, G::ROOT_LIMIT, true) {
                // No lower root bounds this solve: Bernstein controls certify
                // that no channel leaves [0, target] before the exit.
                if rows.iter().all(|&row| interior_within(row, u, guard)) {
                    *out = Self::exit(rows, l3, input_u, (u, values, face, 1.0));
                    return;
                }
            }
            // Rejections retain the lower-first path.
        }
        // The f32 lane already returned above for fold hues.
        if fold {
            *out = search::<G>(rows, hue, l3, target, limit, encode);
            return;
        }

        // The lower face: fitted seed and one Halley step on the sector's channel.
        let face = G::SECTORS
            .iter()
            .position(|&[x, y]| a * x + b * y > 1.0)
            .unwrap_or(2);
        let saturation = polish(G::seed(a, b, face as u8), pick(rows, face));
        let lower: [Float; 3] = std::array::from_fn(|k| {
            if k == face {
                0.0
            } else {
                cubic(rows[k], saturation)
            }
        });
        if !(saturation > 0.0 && saturation < G::ROOT_LIMIT)
            || lower.iter().any(|&v| v < -TOLERANCE)
        {
            *out = search::<G>(rows, hue, l3, target, limit, encode);
            return;
        }
        if !lower.iter().any(|&v| v > target) {
            // Below the cusp the lower face is the first exit.
            *out = Self::exit(rows, l3, input_u, (saturation, lower, face, 0.0));
            return;
        }
        // Above the cusp: the chord from the lower root to the brightest
        // lower channel's crossing, refined on that channel.
        let face = brightest(lower);
        let seed = (saturation * (target - 1.0)) / (pick(lower, face) - 1.0);
        *out = match upper(rows, target, seed, face, saturation, false) {
            Some((u, face, values)) => Self::exit(rows, l3, input_u, (u, values, face, 1.0)),
            None => search::<G>(rows, hue, l3, target, limit, encode),
        };
    }
}

#[cfg(test)]
mod tests {
    include!("dualray_tests.rs");
}
