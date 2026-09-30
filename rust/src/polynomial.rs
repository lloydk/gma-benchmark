// Scalar polynomial candidates, shared by the gamut-specialized solvers.
// Cardano subtracts large terms to recover roots close to zero. Use the
// linear estimate only when the nonlinear derivative is bounded throughout
// [0, 2*x]; that interval then contains a unique root. Near black the same
// small x need not be well conditioned, so the derivative bound is essential.
#[inline(always)]
fn conditioned_first_root(a: Float, b: Float, c: Float, d: Float, hi: Float) -> Option<Float> {
    if d == 0.0 || c == 0.0 {
        return None;
    }
    // The full derivative bound requires |b*d| <= |c|²/8. Reject
    // clearly failing cases before division; the looser 1/4 threshold leaves
    // rounding margin. Every accepted case still passes the full bound below.
    if (b * d).abs() > 0.25 * c * c {
        return None;
    }
    let mut x = -d / c;
    if !(x != 0.0 && x.is_finite())
        || (12.0 * a.abs() * x.abs() + 4.0 * b.abs()) * x.abs() > 0.5 * c.abs()
    {
        return None;
    }
    for _ in 0..6 {
        let next = x - (((a * x + b) * x + c) * x + d) / ((3.0 * a * x + 2.0 * b) * x + c);
        if next == x {
            break;
        }
        x = next;
    }
    if x > 0.0 {
        return Some(if x <= hi { x } else { Float::INFINITY });
    }
    // Deflate the accurately recovered negative root. Otherwise Cardano can
    // round it positive and incorrectly select it ahead of the real exit.
    let bb = b + a * x;
    let cc = c + bb * x;
    let mut best = Float::INFINITY;
    let mut accept = |r: Float| {
        if r > 0.0 && r <= hi {
            best = best.min(r);
        }
    };
    if a == 0.0 {
        accept(-cc / bb);
    } else {
        let disc = bb * bb - 4.0 * a * cc;
        if disc >= 0.0 {
            let q = -0.5 * (bb + disc.sqrt().copysign(bb));
            accept(q / a);
            accept(cc / q);
        }
    }
    Some(best)
}

#[inline(always)]
fn first_root_candidate(
    a: Float,
    mut b: Float,
    mut c: Float,
    mut d: Float,
    lo: Float,
    hi: Float,
) -> Float {
    if !SINGLE && lo == 0.0 {
        if let Some(root) = conditioned_first_root(a, b, c, d, hi) {
            return root;
        }
    }
    let mut r0 = Float::INFINITY;
    let mut r1 = Float::INFINITY;
    let mut r2 = Float::INFINITY;
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
    let mut best = Float::INFINITY;
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
fn first_root_cubic_direct_candidate(
    a: Float,
    mut b: Float,
    mut c: Float,
    mut d: Float,
    hi: Float,
) -> Float {
    // Native f32 already brackets and refines the candidate in checked_cardano.
    // Avoid solving twice; f64 needs this cancellation-resistant path.
    if !SINGLE {
        if let Some(root) = conditioned_first_root(a, b, c, d, hi) {
            return root;
        }
    }
    let (aa, bb, cc, dd) = (a, b, c, d);
    let mut r0 = Float::INFINITY;
    let mut r1 = Float::INFINITY;
    let mut r2 = Float::INFINITY;
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
        let q_term = q * q / 4.0;
        let p_term = p * p * p / 27.0;
        let disc = q_term + p_term;
        let disc_tolerance = Float::EPSILON * 32.0 * (q_term.abs() + p_term.abs());
        if disc > disc_tolerance {
            let s = disc.sqrt();
            r0 = (-q / 2.0 + s).cbrt() + (-q / 2.0 - s).cbrt() + off;
        } else if disc >= -disc_tolerance {
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

    let mut best = Float::INFINITY;
    if r0 >= 0.0 && r0 <= hi {
        best = r0;
    }
    if r1 >= 0.0 && r1 <= hi && r1 < best {
        best = r1;
    }
    if r2 >= 0.0 && r2 <= hi && r2 < best {
        best = r2;
    }
    // checked_cardano performs native-f32 refinement inside an isolated
    // stationary interval. A preliminary unbracketed Newton step is redundant.
    if SINGLE {
        return best;
    }
    for _ in 0..1 {
        if !best.is_finite() {
            break;
        }
        let derivative = (3.0 * aa * best + 2.0 * bb) * best + cc;
        let derivative_scale =
            (3.0 * aa * best).abs() * best.abs() + (2.0 * bb * best).abs() + cc.abs();
        if derivative.abs() <= 32.0 * Float::EPSILON * derivative_scale {
            break;
        }
        let residual = ((aa * best + bb) * best + cc) * best + dd;
        let next = best - residual / derivative;
        // Only poorly conditioned roots need the additional residual check.
        // Ordinary simple roots retain the single-evaluation Newton polish.
        if next < 0.0
            || next > hi
            || (derivative.abs() < Float::EPSILON.sqrt() * derivative_scale
                && (((aa * next + bb) * next + cc) * next + dd).abs() > residual.abs())
        {
            break;
        }
        best = next;
    }
    best
}

// p(0)=0 is an exit only when the first nonzero derivative points outside.
// For an inward departure, factor out x and find the next positive root.
#[inline(always)]
fn face_exit(a: Float, b: Float, c: Float, upper: bool, hi: Float) -> Float {
    let direction = if c != 0.0 {
        c
    } else if b != 0.0 {
        b
    } else {
        a
    };
    if (upper && direction > 0.0) || (!upper && direction < 0.0) {
        0.0
    } else {
        first_root(0.0, a, b, c, Float::MIN_POSITIVE, hi)
    }
}
