// Dualray: guarded upper-first shortcut and upper-face retry.
// Target-specific basis and seeds; retains the benchmark intrinsic-in-gamut policy.
// Fold windows use first-exit isolation instead of trusting fitted roots.
// The balanced seed evaluation matches the JS port. The corrections reuse
// Horner intermediates and cancel common factors, changing last-bit rounding.
// Arithmetic stays in the containing module's precision. Compensated products
// use hardware FMA when enabled, with a native split-product fallback.

use super::gamut::{DisplayP3, Rec2020, RgbGamut, Srgb};
use super::rgb_solvers::in_blue_fold;
use super::transfer::TransferFunction;
use super::{Float, PI, SINGLE};
use std::marker::PhantomData;

// Precision-specific residual guards and hue reduction.
mod fold {
    include!("dualray_fold.rs");
}

const TOLERANCE: Float = if SINGLE { 8.0 * Float::EPSILON } else { 1e-12 };
const HUE_FAST_LIMIT: Float = if SINGLE { 360.0 } else { 1e9 };

pub(crate) trait DualrayData: RgbGamut {
    const BASIS: [[Float; 9]; 3];
    const SECTORS: [[Float; 2]; 2];
    const PRECISE_BASIS: [[fold::Wide; 9]; 3];
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
            const BASIS: [[Float; 9]; 3] = round_basis($module::BASIS);
            const SECTORS: [[Float; 2]; 2] = $module::SECTORS;
            const PRECISE_BASIS: [[fold::Wide; 9]; 3] = fold::split_basis($module::BASIS);
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

const fn round_basis(input: [[f64; 9]; 3]) -> [[Float; 9]; 3] {
    let mut result = [[0.0; 9]; 3];
    let mut i = 0;
    while i < 3 {
        let mut j = 0;
        while j < 9 {
            result[i][j] = input[i][j] as Float;
            j += 1;
        }
        i += 1;
    }
    result
}

#[inline(always)]
fn value(d: Float, b: Float, a: Float, x: Float) -> Float {
    ((d * x + b) * x + a) * x + 1.0
}

// Interior Bernstein controls bound the whole interval, not just its endpoint.
// Controls and guard are scaled by three, avoiding two divisions.
#[inline(always)]
fn interior_within(a: Float, b: Float, u: Float, guard: Float) -> bool {
    let limit = 3.0 * guard;
    let linear = a * u;
    let c1 = 3.0 + linear;
    let c2 = 3.0 + 2.0 * linear + b * u * u;
    c1 >= 0.0 && c1 <= limit && c2 >= 0.0 && c2 <= limit
}

#[inline(always)]
fn polish(x: Float, d: Float, b: Float, a: Float) -> Float {
    // Reuse Horner intermediates for f, f', and f'' / 2.
    let dx = d * x;
    let q = dx + b;
    let r = q * x + a;
    let f = r * x + 1.0;
    let half_second = (dx + dx) + q;
    let f1 = (dx + q) * x + r;
    // Cancel the common factor of two in the Halley correction.
    let denominator = f1 * f1 - f * half_second;
    if denominator != 0.0 {
        x - (f * f1) / denominator
    } else {
        x
    }
}

// Partition at derivative roots, then bisect the first sign-changing interval.
#[inline(never)]
fn first_root(d: Float, b: Float, a: Float, constant: Float, limit: Float) -> Float {
    let mut s0 = limit;
    let mut s1 = limit;
    if d == 0.0 {
        let s = -a / (2.0 * b);
        if s > 0.0 && s < limit {
            s0 = s;
        }
    } else {
        let discriminant = b * b - 3.0 * d * a;
        if discriminant >= 0.0 {
            let q = -b - (if b < 0.0 { -1.0 } else { 1.0 }) * discriminant.sqrt();
            let t0 = q / (3.0 * d);
            let t1 = if q == 0.0 { 0.0 } else { a / q };
            let low = t0.min(t1);
            let high = t0.max(t1);
            if low > 0.0 && low < limit {
                s0 = low;
            }
            if high > 0.0 && high < limit {
                if s0 == limit {
                    s0 = high;
                } else {
                    s1 = high;
                }
            }
        }
    }
    let mut lo = 0.0;
    let negative = constant < 0.0;
    for interval in 0..3 {
        let mut hi = if interval == 0 {
            s0
        } else if interval == 1 {
            s1
        } else {
            limit
        };
        let fhi = ((d * hi + b) * hi + a) * hi + constant;
        // A stationary touch does not leave the gamut. At an exact endpoint
        // root, accept only an outward crossing; keep the original inside sign.
        if fhi == 0.0 && hi == limit {
            let slope = (3.0 * d * hi + 2.0 * b) * hi + a;
            if if negative { slope > 0.0 } else { slope < 0.0 } {
                return hi;
            }
        }
        if if negative { fhi > 0.0 } else { fhi < 0.0 } {
            for _ in 0..64 {
                let mid = lo + (hi - lo) * 0.5;
                if mid == lo || mid == hi {
                    break;
                }
                let f = ((d * mid + b) * mid + a) * mid + constant;
                if if negative { f <= 0.0 } else { f >= 0.0 } {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return lo + (hi - lo) * 0.5;
        }
        lo = hi;
        if hi == limit {
            break;
        }
    }
    Float::INFINITY
}

#[inline(always)]
fn lower_exit(rows: [[Float; 3]; 3], limit: Float) -> (Float, u8) {
    let mut root = Float::INFINITY;
    let mut face = 0;
    for (i, [d, b, a]) in rows.into_iter().enumerate() {
        let candidate = first_root(d, b, a, 1.0, root.min(limit));
        if candidate < root {
            root = candidate;
            face = i as u8;
        }
    }
    (root, face)
}

pub(crate) struct Dualray<G: DualrayData>(PhantomData<G>);

// f64 fold isolation is bounded by the authored input, including upper faces:
// an inside endpoint alone cannot rule out an earlier exit and re-entry.
#[inline(never)]
fn map_fold<G: DualrayData>(
    rows: [[Float; 3]; 3],
    l3: Float,
    target: Float,
    input_u: Float,
    out: &mut [Float; 3],
) {
    let mut u = input_u.min(G::ROOT_LIMIT);
    let mut face = None;
    for (i, [d, b, a]) in rows.into_iter().enumerate() {
        let root = first_root(d, b, a, 1.0, u);
        if root <= u {
            u = root;
            face = Some((i, 0.0));
        }
    }
    if target.is_finite() {
        for (i, [d, b, a]) in rows.into_iter().enumerate() {
            let root = first_root(-d, -b, -a, target - 1.0, u);
            if root <= u {
                u = root;
                face = Some((i, 1.0));
            }
        }
    }
    *out = rows.map(|[d, b, a]| G::Transfer::encode_clamped(l3 * value(d, b, a, u)));
    if let Some((channel, value)) = face {
        out[channel] = value;
    }
}

impl<G: DualrayData> Dualray<G> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }

    // Intrinsic boundary comparisons are also the checked variant's policy.
    #[inline(always)]
    pub(crate) fn map_with_in_gamut_check(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map(oklch, out);
    }

    #[inline(always)]
    pub(crate) fn map(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        let [l, c, h] = *oklch;
        if l <= 0.0 {
            *out = [0.0; 3];
            return;
        }
        if l >= 1.0 {
            *out = [1.0; 3];
            return;
        }
        if c <= 0.0 {
            *out = [G::Transfer::encode_clamped(l * l * l); 3];
            return;
        }
        let hue = if h > -HUE_FAST_LIMIT && h < HUE_FAST_LIMIT {
            h
        } else {
            h % 360.0
        };
        if SINGLE && in_blue_fold::<G>(hue) {
            fold::map::<G>(l, c, hue, out);
            return;
        }
        let radians = hue * (PI / 180.0);
        let a = radians.cos();
        let b = radians.sin();
        let l3 = l * l * l;
        let a2 = a * a;
        let ab = a * b;
        let a3 = a2 * a;
        let a2b = a2 * b;
        let rd =
            G::BASIS[0][0] * a3 + G::BASIS[0][1] * a2b + G::BASIS[0][2] * a + G::BASIS[0][3] * b;
        let rb = G::BASIS[0][4] + G::BASIS[0][5] * a2 + G::BASIS[0][6] * ab;
        let ra = G::BASIS[0][7] * a + G::BASIS[0][8] * b;
        let gd =
            G::BASIS[1][0] * a3 + G::BASIS[1][1] * a2b + G::BASIS[1][2] * a + G::BASIS[1][3] * b;
        let gb = G::BASIS[1][4] + G::BASIS[1][5] * a2 + G::BASIS[1][6] * ab;
        let ga = G::BASIS[1][7] * a + G::BASIS[1][8] * b;
        let bd =
            G::BASIS[2][0] * a3 + G::BASIS[2][1] * a2b + G::BASIS[2][2] * a + G::BASIS[2][3] * b;
        let bb = G::BASIS[2][4] + G::BASIS[2][5] * a2 + G::BASIS[2][6] * ab;
        let ba = G::BASIS[2][7] * a + G::BASIS[2][8] * b;
        let inv_l = 1.0 / l;
        let input_u = c * inv_l;
        let target = inv_l * inv_l * inv_l;

        // Predict one upper face near white. The gate is a performance heuristic;
        // containment, first-interval, and residual guards decide acceptance.
        let delta = target - 1.0;
        let max_slope = ra.max(ga).max(ba);
        if delta > 0.0 && delta < 0.15 * max_slope {
            let upper_face = if ra == max_slope {
                0
            } else if ga == max_slope {
                1
            } else {
                2
            };
            let wd = if upper_face == 0 {
                rd
            } else if upper_face == 1 {
                gd
            } else {
                bd
            };
            let wb = if upper_face == 0 {
                rb
            } else if upper_face == 1 {
                gb
            } else {
                bb
            };
            let wa = max_slope;
            let mut u = (2.0 * delta) / (wa + (wa * wa + 4.0 * wb * delta).sqrt());
            for step in 0..2 {
                // Reuse Horner intermediates for f, f', and f'' / 2.
                let du = wd * u;
                let q = du + wb;
                let r = q * u + wa;
                let f = (r * u + 1.0) - target;
                // Use the same precision-specific tolerance as the final guard.
                if step == 1 && f.abs() <= target * TOLERANCE {
                    break;
                }
                let half_second = (du + du) + q;
                let f1 = (du + q) * u + r;
                let f1_squared = f1 * f1;
                let product = f * half_second;
                // Cancel the common factor of six in the Householder correction.
                let denominator = f1 * (f1_squared - 2.0 * product) + f * f * wd;
                if denominator == 0.0 {
                    break;
                }
                u -= f * (f1_squared - product) / denominator;
            }
            let r = value(rd, rb, ra, u);
            let g = value(gd, gb, ga, u);
            let blue = value(bd, bb, ba, u);
            let guard = target * (1.0 + TOLERANCE);
            let selected = if upper_face == 0 {
                r
            } else if upper_face == 1 {
                g
            } else {
                blue
            };
            if u >= 0.0
                && u < G::ROOT_LIMIT
                && r >= -TOLERANCE
                && g >= -TOLERANCE
                && blue >= -TOLERANCE
                && r <= guard
                && g <= guard
                && blue <= guard
                && (selected - target).abs() <= target * TOLERANCE
                && interior_within(ra, rb, u, guard)
                && interior_within(ga, gb, u, guard)
                && interior_within(ba, bb, u, guard)
            {
                // u is the validated first exit; classify the input against it.
                if input_u < u {
                    out[0] = G::Transfer::encode_clamped(l3 * value(rd, rb, ra, input_u));
                    out[1] = G::Transfer::encode_clamped(l3 * value(gd, gb, ga, input_u));
                    out[2] = G::Transfer::encode_clamped(l3 * value(bd, bb, ba, input_u));
                    return;
                }
                out[0] = if upper_face == 0 {
                    1.0
                } else {
                    G::Transfer::encode_clamped(l3 * r)
                };
                out[1] = if upper_face == 1 {
                    1.0
                } else {
                    G::Transfer::encode_clamped(l3 * g)
                };
                out[2] = if upper_face == 2 {
                    1.0
                } else {
                    G::Transfer::encode_clamped(l3 * blue)
                };
                return;
            }
            // Rejections retain the lower-first path.
        }
        // The f32 lane already returned above for fold hues. Compute this
        // once for f64 and reuse it when selecting the lower-bound solve.
        let fold = !SINGLE && in_blue_fold::<G>(hue);
        if fold
            && [
                value(rd, rb, ra, input_u),
                value(gd, gb, ga, input_u),
                value(bd, bb, ba, input_u),
            ]
            .into_iter()
            .all(|v| v >= 0.0 && v <= target)
        {
            // An inside endpoint may lie beyond a first exit/re-entry. Bound
            // isolation by the input; do not treat it as proof of pass-through.
            map_fold::<G>(
                [[rd, rb, ra], [gd, gb, ga], [bd, bb, ba]],
                l3,
                target,
                input_u,
                out,
            );
            return;
        }
        let mut face = if a * G::SECTORS[0][0] + b * G::SECTORS[0][1] > 1.0 {
            0
        } else if a * G::SECTORS[1][0] + b * G::SECTORS[1][1] > 1.0 {
            1
        } else {
            2
        };
        let d = if face == 0 {
            rd
        } else if face == 1 {
            gd
        } else {
            bd
        };
        let b2 = if face == 0 {
            rb
        } else if face == 1 {
            gb
        } else {
            bb
        };
        let a1 = if face == 0 {
            ra
        } else if face == 1 {
            ga
        } else {
            ba
        };
        let mut saturation = if fold {
            // Outside endpoints retain the fast upper-face refinement below.
            // Bounding every upper face with bisection costs more here.
            let (root, exit_face) =
                lower_exit([[rd, rb, ra], [gd, gb, ga], [bd, bb, ba]], G::ROOT_LIMIT);
            face = exit_face;
            root
        } else {
            polish(G::seed(a, b, face), d, b2, a1)
        };
        let mut lower_r = if face == 0 {
            0.0
        } else {
            value(rd, rb, ra, saturation)
        };
        let mut lower_g = if face == 1 {
            0.0
        } else {
            value(gd, gb, ga, saturation)
        };
        let mut lower_b = if face == 2 {
            0.0
        } else {
            value(bd, bb, ba, saturation)
        };
        if !(saturation > 0.0 && saturation < G::ROOT_LIMIT)
            || lower_r < -TOLERANCE
            || lower_g < -TOLERANCE
            || lower_b < -TOLERANCE
        {
            (saturation, face) =
                lower_exit([[rd, rb, ra], [gd, gb, ga], [bd, bb, ba]], G::ROOT_LIMIT);
            lower_r = value(rd, rb, ra, saturation);
            lower_g = value(gd, gb, ga, saturation);
            lower_b = value(bd, bb, ba, saturation);
        }
        if !(lower_r > target || lower_g > target || lower_b > target) {
            if input_u < saturation {
                out[0] = G::Transfer::encode_clamped(l3 * value(rd, rb, ra, input_u));
                out[1] = G::Transfer::encode_clamped(l3 * value(gd, gb, ga, input_u));
                out[2] = G::Transfer::encode_clamped(l3 * value(bd, bb, ba, input_u));
                return;
            }
            out[0] = if face == 0 {
                0.0
            } else {
                G::Transfer::encode_clamped(l3 * lower_r)
            };
            out[1] = if face == 1 {
                0.0
            } else {
                G::Transfer::encode_clamped(l3 * lower_g)
            };
            out[2] = if face == 2 {
                0.0
            } else {
                G::Transfer::encode_clamped(l3 * lower_b)
            };
            return;
        }
        face = if lower_r > lower_g {
            if lower_r > lower_b {
                0
            } else {
                2
            }
        } else if lower_g > lower_b {
            1
        } else {
            2
        };
        let wd = if face == 0 {
            rd
        } else if face == 1 {
            gd
        } else {
            bd
        };
        let wb = if face == 0 {
            rb
        } else if face == 1 {
            gb
        } else {
            bb
        };
        let wa = if face == 0 {
            ra
        } else if face == 1 {
            ga
        } else {
            ba
        };
        let max_lower = if face == 0 {
            lower_r
        } else if face == 1 {
            lower_g
        } else {
            lower_b
        };
        let mut u = (saturation * (target - 1.0)) / (max_lower - 1.0);
        for _ in 0..2 {
            // Reuse Horner intermediates for f, f', and f'' / 2.
            let du = wd * u;
            let q = du + wb;
            let r = q * u + wa;
            let f = (r * u + 1.0) - target;
            let half_second = (du + du) + q;
            let f1 = (du + q) * u + r;
            let f1_squared = f1 * f1;
            let product = f * half_second;
            // Cancel the common factor of six in the Householder correction.
            let denominator = f1 * (f1_squared - 2.0 * product) + f * f * wd;
            if denominator == 0.0 {
                break;
            }
            u -= f * (f1_squared - product) / denominator;
        }
        let mut r = value(rd, rb, ra, u);
        let mut g = value(gd, gb, ga, u);
        let mut blue = value(bd, bb, ba, u);
        let guard = target * (1.0 + TOLERANCE);
        if !(u >= 0.0
            && u <= saturation
            && r >= -TOLERANCE
            && g >= -TOLERANCE
            && blue >= -TOLERANCE
            && r <= guard
            && g <= guard
            && blue <= guard
            && ((if face == 0 {
                r
            } else if face == 1 {
                g
            } else {
                blue
            }) - target)
                .abs()
                <= target * TOLERANCE)
        {
            // Retry a converged prediction only when another upper face rejects it.
            let predicted_value = if face == 0 {
                r
            } else if face == 1 {
                g
            } else {
                blue
            };
            if u >= 0.0
                && u <= saturation
                && r >= -TOLERANCE
                && g >= -TOLERANCE
                && blue >= -TOLERANCE
                && (predicted_value - target).abs() <= target * TOLERANCE
                && (r > guard || g > guard || blue > guard)
            {
                face = if r > g {
                    if r > blue {
                        0
                    } else {
                        2
                    }
                } else if g > blue {
                    1
                } else {
                    2
                };
                let retry_d = if face == 0 {
                    rd
                } else if face == 1 {
                    gd
                } else {
                    bd
                };
                let retry_b = if face == 0 {
                    rb
                } else if face == 1 {
                    gb
                } else {
                    bb
                };
                let retry_a = if face == 0 {
                    ra
                } else if face == 1 {
                    ga
                } else {
                    ba
                };
                let previous_u = u;
                for _ in 0..2 {
                    let f = value(retry_d, retry_b, retry_a, u) - target;
                    let f1 = (3.0 * retry_d * u + 2.0 * retry_b) * u + retry_a;
                    let half_second = 3.0 * retry_d * u + retry_b;
                    let f1_squared = f1 * f1;
                    let product = f * half_second;
                    let denominator = f1 * (f1_squared - 2.0 * product) + f * f * retry_d;
                    if denominator == 0.0 {
                        break;
                    }
                    u -= f * (f1_squared - product) / denominator;
                }
                r = value(rd, rb, ra, u);
                g = value(gd, gb, ga, u);
                blue = value(bd, bb, ba, u);
                // Never accept a retry that moves away from neutral.
                if !(u >= 0.0 && u <= previous_u) {
                    u = Float::NAN;
                }
            }
            let selected_value = if face == 0 {
                r
            } else if face == 1 {
                g
            } else {
                blue
            };
            if !(u >= 0.0
                && u <= saturation
                && r >= -TOLERANCE
                && g >= -TOLERANCE
                && blue >= -TOLERANCE
                && r <= guard
                && g <= guard
                && blue <= guard
                && (selected_value - target).abs() <= target * TOLERANCE)
            {
                let ru = first_root(rd, rb, ra, 1.0 - target, saturation);
                let gu = first_root(gd, gb, ga, 1.0 - target, saturation);
                let bu = first_root(bd, bb, ba, 1.0 - target, saturation);
                u = ru.min(gu).min(bu);
                face = if u == ru {
                    0
                } else if u == gu {
                    1
                } else {
                    2
                };
                r = value(rd, rb, ra, u);
                g = value(gd, gb, ga, u);
                blue = value(bd, bb, ba, u);
            }
        }
        if input_u < u {
            out[0] = G::Transfer::encode_clamped(l3 * value(rd, rb, ra, input_u));
            out[1] = G::Transfer::encode_clamped(l3 * value(gd, gb, ga, input_u));
            out[2] = G::Transfer::encode_clamped(l3 * value(bd, bb, ba, input_u));
            return;
        }
        out[0] = if face == 0 {
            1.0
        } else {
            G::Transfer::encode_clamped(l3 * r)
        };
        out[1] = if face == 1 {
            1.0
        } else {
            G::Transfer::encode_clamped(l3 * g)
        };
        out[2] = if face == 2 {
            1.0
        } else {
            G::Transfer::encode_clamped(l3 * blue)
        };
    }
}

#[cfg(test)]
mod tests {
    include!("dualray_tests.rs");
}
