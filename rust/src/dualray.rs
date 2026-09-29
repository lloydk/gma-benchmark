// Dualray: guarded upper-first shortcut and upper-face retry.
// Display-P3 solver matching
// src/dualray.js, including benchmark in-gamut handling.
// The balanced seed evaluation matches the JS port. The corrections reuse
// Horner intermediates and cancel common factors, changing last-bit rounding.
// Arithmetic uses the containing module's concrete precision, without explicit FMA.

use super::{clamped_gamma, Float, PI, SINGLE};

// Precision-specific residual guards and hue reduction.
const TOLERANCE: Float = if SINGLE { 8.0 * Float::EPSILON } else { 1e-12 };
const HUE_FAST_LIMIT: Float = if SINGLE { 360.0 } else { 1e9 };

const ROOT_LIMIT: Float = 4.0;
const RED1: Float = -1.772343927512981;
const RED2: Float = -0.8207587433674072;
const GREEN1: Float = 1.8031987175305495;
const GREEN2: Float = -1.1932813966558915;

const R_D3: Float = 0.0791058979743933;
const R_D2: Float = 0.5655398231442658;
const R_D1: Float = 0.11818565248860147;
const R_D0: Float = -0.24664826663200515;
const R_B0: Float = 1.0567091736627137;
const R_B2: Float = 0.34490732484069164;
const R_B1: Float = 1.6035484701795497;
const R_A1: Float = 4.39902903039207;
const R_A0: Float = 1.9561094754786617;
const G_D3: Float = -0.1513261575650534;
const G_D2: Float = -0.7888313324352596;
const G_D1: Float = 0.0807940995543469;
const G_D0: Float = 0.6827265304693584;
const G_B0: Float = -1.7357487868412962;
const G_B2: Float = 1.2945435832569667;
const G_B1: Float = -0.6857891074040978;
const G_A1: Float = -1.974959555170497;
const G_A0: Float = 0.08018985838156922;
const B_D3: Float = 0.685552592139929;
const B_D2: Float = 3.255498281543475;
const B_D1: Float = -0.687673578309416;
const B_D0: Float = -3.304652083761079;
const B_B0: Float = 7.666248909056808;
const B_B2: Float = -7.658638747824462;
const B_B1: Float = 1.0298244006069064;
const B_A1: Float = -0.28185879057092095;
const B_A0: Float = -5.863136490898882;

// Keep all 18 coefficients, grouping in t^4 and t^8 to shorten the serial
// dependency chain. The fitted polynomial and solver guards are unchanged.
#[inline(always)]
fn p3_seed(a: Float, b: Float, face: u8) -> Float {
    if face == 1 {
        let t = (b * 0.8339347326195725 - a * -0.5518630824133121 - 0.0) * 1.127863385513941;
        let t2 = t * t;
        let t4 = t2 * t2;
        let t8 = t4 * t4;
        let low = ((0.5055203051802346 + -0.08974151391142265 * t)
            + (0.003935796602735819 + -0.0031950200183823965 * t) * t2)
            + ((0.049812213498918206 + -0.014259466994389595 * t)
                + (-0.02608821946157984 + 0.008463197421688648 * t) * t2)
                * t4;
        let high = ((0.1286484638505835 + -0.05256623072311778 * t)
            + (-0.2965630803407685 + 0.12662941851675352 * t) * t2)
            + ((0.4318476522414585 + -0.18578934240509345 * t)
                + (-0.32435480435116104 + 0.13996762708411048 * t) * t2)
                * t4;
        return low + (high + (0.10458491835889717 + -0.04509353597445119 * t) * t8) * t8;
    }
    if face == 2 {
        let t = (b * 0.047079529957363205 - a * 0.9988911441488475 - 0.0) * 1.1747996078456162;
        let t2 = t * t;
        let t4 = t2 * t2;
        let t8 = t4 * t4;
        let low = ((0.23589963446491738 + -0.0038895135379108737 * t)
            + (0.08496374354387375 + -0.0027636516463751272 * t) * t2)
            + ((0.05350299282638939 + -0.00278232147399756 * t)
                + (-0.04400786078609473 + 0.006189289363385801 * t) * t2)
                * t4;
        let high = ((0.3630035541828265 + -0.03766350558486446 * t)
            + (-0.8898431315961659 + 0.09460387079467442 * t) * t2)
            + ((1.3164319733061864 + -0.13851221494888616 * t)
                + (-0.9972958875572052 + 0.10472689772454183 * t) * t2)
                * t4;
        return low + (high + (0.3252989657819146 + -0.03370793968835844 * t) * t8) * t8;
    }
    if a * -0.9400027760422874 - b * -0.3411667935670076 > 0.0 {
        let t = (b * -0.9518703435618234 - a * -0.3065009772374244 - 0.0) * 1.2655139197164653;
        let t2 = t * t;
        let t4 = t2 * t2;
        let t8 = t4 * t4;
        let low = ((0.2282479542436939 + -0.011978541363502394 * t)
            + (0.07677819272243186 + -0.007787115544658625 * t) * t2)
            + ((0.041419417874723675 + -0.005416858161601942 * t)
                + (-0.003312430161982946 + 0.00011000659405603977 * t) * t2)
                * t4;
        let high = ((0.13437401745475963 + -0.018667398971906446 * t)
            + (-0.3108563944891688 + 0.04236335317690669 * t) * t2)
            + ((0.47142535235365 + -0.06569767262273747 * t)
                + (-0.35860024295996273 + 0.05052013023156279 * t) * t2)
                * t4;
        return low + (high + (0.12054184701587978 + -0.017586285475974565 * t) * t8) * t8;
    }
    let t = ((a * -0.9954573465411122 - b * -0.09520856693243564)
        .max(0.0)
        .sqrt()
        - 0.29604590453444324)
        * 4.900165144269026;
    let t2 = t * t;
    let t4 = t2 * t2;
    let t8 = t4 * t4;
    let low = ((0.501535748298442 + -0.16297049180671477 * t)
        + (0.028564672193241766 + -0.0006869552267263243 * t) * t2)
        + ((-0.0007112199414283341 + 0.00013975671324474424 * t)
            + (0.000014732112336762131 + -0.000008637716697358479 * t) * t2)
            * t4;
    let high = ((0.0000011908160691594576 + 2.202000201931595e-7 * t)
        + (-7.327939657544548e-8 + 5.350645753626143e-9 * t) * t2)
        + ((3.5070115700364113e-9 + -7.488678319431428e-10 * t)
            + (6.265407945546839e-12 + 1.4208328341030413e-10 * t) * t2)
            * t4;
    return low + (high + (-5.35591324377391e-12 + -2.2838422511186864e-11 * t) * t8) * t8;
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
    let mut flo = constant;
    for interval in 0..3 {
        let mut hi = if interval == 0 {
            s0
        } else if interval == 1 {
            s1
        } else {
            limit
        };
        let fhi = ((d * hi + b) * hi + a) * hi + constant;
        if fhi == 0.0 {
            return hi;
        }
        if (flo < 0.0) != (fhi < 0.0) {
            for _ in 0..64 {
                let mid = lo + (hi - lo) * 0.5;
                if mid == lo || mid == hi {
                    break;
                }
                let f = ((d * mid + b) * mid + a) * mid + constant;
                if (f < 0.0) == (flo < 0.0) {
                    lo = mid;
                    flo = f;
                } else {
                    hi = mid;
                }
            }
            return lo + (hi - lo) * 0.5;
        }
        lo = hi;
        flo = fhi;
        if hi == limit {
            break;
        }
    }
    Float::INFINITY
}

pub(crate) struct Dualray;

impl Dualray {
    pub(crate) fn new() -> Self {
        Self
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
            *out = [clamped_gamma(l * l * l); 3];
            return;
        }
        let hue = if h > -HUE_FAST_LIMIT && h < HUE_FAST_LIMIT {
            h
        } else {
            h % 360.0
        };
        let radians = hue * (PI / 180.0);
        let a = radians.cos();
        let b = radians.sin();
        let l3 = l * l * l;
        let a2 = a * a;
        let ab = a * b;
        let a3 = a2 * a;
        let a2b = a2 * b;
        let rd = R_D3 * a3 + R_D2 * a2b + R_D1 * a + R_D0 * b;
        let rb = R_B0 + R_B2 * a2 + R_B1 * ab;
        let ra = R_A1 * a + R_A0 * b;
        let gd = G_D3 * a3 + G_D2 * a2b + G_D1 * a + G_D0 * b;
        let gb = G_B0 + G_B2 * a2 + G_B1 * ab;
        let ga = G_A1 * a + G_A0 * b;
        let bd = B_D3 * a3 + B_D2 * a2b + B_D1 * a + B_D0 * b;
        let bb = B_B0 + B_B2 * a2 + B_B1 * ab;
        let ba = B_A1 * a + B_A0 * b;
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
                && u < ROOT_LIMIT
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
                    out[0] = clamped_gamma(l3 * value(rd, rb, ra, input_u));
                    out[1] = clamped_gamma(l3 * value(gd, gb, ga, input_u));
                    out[2] = clamped_gamma(l3 * value(bd, bb, ba, input_u));
                    return;
                }
                out[0] = if upper_face == 0 {
                    1.0
                } else {
                    clamped_gamma(l3 * r)
                };
                out[1] = if upper_face == 1 {
                    1.0
                } else {
                    clamped_gamma(l3 * g)
                };
                out[2] = if upper_face == 2 {
                    1.0
                } else {
                    clamped_gamma(l3 * blue)
                };
                return;
            }
            // Rejections retain the lower-first path.
        }
        let mut face = if a * RED1 + b * RED2 > 1.0 {
            0
        } else if a * GREEN1 + b * GREEN2 > 1.0 {
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
        let mut saturation = polish(p3_seed(a, b, face), d, b2, a1);
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
        if !(saturation > 0.0 && saturation < ROOT_LIMIT)
            || lower_r < -TOLERANCE
            || lower_g < -TOLERANCE
            || lower_b < -TOLERANCE
        {
            let r = first_root(rd, rb, ra, 1.0, ROOT_LIMIT);
            let g = first_root(gd, gb, ga, 1.0, ROOT_LIMIT);
            let blue = first_root(bd, bb, ba, 1.0, ROOT_LIMIT);
            saturation = r.min(g).min(blue);
            face = if saturation == r {
                0
            } else if saturation == g {
                1
            } else {
                2
            };
            lower_r = value(rd, rb, ra, saturation);
            lower_g = value(gd, gb, ga, saturation);
            lower_b = value(bd, bb, ba, saturation);
        }
        if !(lower_r > target || lower_g > target || lower_b > target) {
            if input_u < saturation {
                out[0] = clamped_gamma(l3 * value(rd, rb, ra, input_u));
                out[1] = clamped_gamma(l3 * value(gd, gb, ga, input_u));
                out[2] = clamped_gamma(l3 * value(bd, bb, ba, input_u));
                return;
            }
            out[0] = if face == 0 {
                0.0
            } else {
                clamped_gamma(l3 * lower_r)
            };
            out[1] = if face == 1 {
                0.0
            } else {
                clamped_gamma(l3 * lower_g)
            };
            out[2] = if face == 2 {
                0.0
            } else {
                clamped_gamma(l3 * lower_b)
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
            out[0] = clamped_gamma(l3 * value(rd, rb, ra, input_u));
            out[1] = clamped_gamma(l3 * value(gd, gb, ga, input_u));
            out[2] = clamped_gamma(l3 * value(bd, bb, ba, input_u));
            return;
        }
        out[0] = if face == 0 {
            1.0
        } else {
            clamped_gamma(l3 * r)
        };
        out[1] = if face == 1 {
            1.0
        } else {
            clamped_gamma(l3 * g)
        };
        out[2] = if face == 2 {
            1.0
        } else {
            clamped_gamma(l3 * blue)
        };
    }
}

#[cfg(test)]
mod tests {
    include!("dualray_tests.rs");
}
