// Precision choices are compile-time constants; f64 retains its original path.
const STEP_TOLERANCE: Float = if SINGLE { Float::EPSILON } else { 1e-9 };

#[inline(always)]
fn hue_radians(h: Float) -> Float {
    let h = if SINGLE && (h <= -360.0 || h >= 360.0) {
        h % 360.0
    } else {
        h
    };
    h * PI / 180.0
}

#[inline(always)]
fn first_root(a: Float, b: Float, c: Float, d: Float, lo: Float, hi: Float) -> Float {
    if lo == 0.0 && d == 0.0 {
        return face_exit(a, b, c, true, hi);
    }
    let candidate = first_root_candidate(a, b, c, d, lo, hi);
    if SINGLE {
        checked_cardano([a, b, c, d], lo, hi, candidate)
    } else {
        candidate
    }
}
#[inline(always)]
fn first_root_no_cache(a: Float, b: Float, c: Float, d: Float, lo: Float, hi: Float) -> Float {
    first_root(a, b, c, d, lo, hi)
}
// Forced inlining regressed native f32 timing here. Let LLVM optimize the
// three-channel caller independently of this solver wrapper.
fn first_root_cubic_direct(
    a: Float,
    b: Float,
    c: Float,
    d: Float,
    hi: Float,
    upper: bool,
) -> Float {
    if d == 0.0 {
        return face_exit(a, b, c, upper, hi);
    }
    if SINGLE {
        // Evaluate the coefficient array before the candidate. Moving the
        // candidate into a preceding local delayed coefficient stores and
        // slowed validation in the native f32 build.
        checked_cardano(
            [a, b, c, d],
            0.0,
            hi,
            first_root_cubic_direct_candidate(a, b, c, d, hi),
        )
    } else {
        first_root_cubic_direct_candidate(a, b, c, d, hi)
    }
}

// Cardano's depressed cubic loses small roots when its large intermediate
// terms cancel. Validate and polish its candidate in the FIRST stationary
// interval containing a crossing; recover by bisection if it cannot be used.
// This also prevents a Newton correction from jumping to a later root.
fn checked_cardano(p: [Float; 4], lo: Float, hi: Float, candidate: Float) -> Float {
    let [a, b, c, d] = p;
    let eval = |x: Float| ((a * x + b) * x + c) * x + d;
    let bound = if a != 0.0 {
        1.0 + (b / a).abs().max((c / a).abs()).max((d / a).abs())
    } else if b != 0.0 {
        1.0 + (c / b).abs().max((d / b).abs())
    } else if c != 0.0 {
        1.0 + (d / c).abs()
    } else {
        return Float::INFINITY;
    };
    let hi = hi.min(bound);
    let mut points = [lo, hi, hi, hi];
    if a == 0.0 {
        let t = -c / (2.0 * b);
        if t > lo && t < hi {
            points[1] = t;
        }
    } else {
        let disc = b * b - 3.0 * a * c;
        if disc >= 0.0 {
            let q = -b - disc.sqrt().copysign(b);
            let r0 = q / (3.0 * a);
            let r1 = c / q;
            if r0 > lo && r0 < hi {
                points[1] = r0;
            }
            if r1 > lo && r1 < hi {
                points[2] = r1;
            }
        }
    }
    points.sort_by(Float::total_cmp);
    for pair in points.windows(2) {
        let (mut left, mut right) = (pair[0], pair[1]);
        let mut fl = eval(left);
        let fr = eval(right);
        if fr == 0.0 {
            return right;
        }
        if (fl < 0.0) == (fr < 0.0) {
            continue;
        }
        let mut x = candidate;
        if x >= left && x <= right {
            for _ in 0..3 {
                let f = eval(x);
                let derivative = (3.0 * a * x + 2.0 * b) * x + c;
                let next = x - f / derivative;
                if !(next >= left && next <= right) {
                    break;
                }
                x = next;
            }
            let scale = ((a.abs() * x.abs() + b.abs()) * x.abs() + c.abs()) * x.abs() + d.abs();
            if eval(x).abs() <= 2.0 * Float::EPSILON * scale {
                return x;
            }
        }
        for _ in 0..64 {
            let mid = left + (right - left) * 0.5;
            if mid == left || mid == right {
                break;
            }
            let f = eval(mid);
            if f == 0.0 {
                return mid;
            }
            if (f < 0.0) == (fl < 0.0) {
                left = mid;
                fl = f;
            } else {
                right = mid;
            }
        }
        return left + (right - left) * 0.5;
    }
    Float::INFINITY
}

// Flushing a small, nonzero axis to zero can ignore the nearest face near
// black/white. Direct division also avoids overflowing 1/d before multiplying
// by the distance to the face. Exactly parallel axes have no intersection.
#[inline(always)]
fn stable_exit_t(ar: Float, ag: Float, ab: Float, dr: Float, dg: Float, db: Float) -> Float {
    let axis = |a: Float, d: Float| {
        if d > 0.0 {
            (1.0 - a) / d
        } else if d < 0.0 {
            -a / d
        } else {
            Float::INFINITY
        }
    };
    axis(ar, dr).min(axis(ag, dg)).min(axis(ab, db))
}
