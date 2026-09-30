// Native two-component arithmetic for the f32 fold only. A nearly tangent
// channel can amplify ordinary coefficient/trig rounding into a large root
// displacement. Constants split at compile time; runtime arithmetic stays Float.
use super::{DualrayData, Float, TransferFunction};

#[derive(Clone, Copy)]
pub(crate) struct Wide(Float, Float);
impl Wide {
    pub const fn constant(x: f64) -> Self {
        let hi = x as Float;
        Self(hi, (x - hi as f64) as Float)
    }
    fn scalar(x: Float) -> Self {
        Self(x, 0.0)
    }
    fn add(self, rhs: Self) -> Self {
        let sum = self.0 + rhs.0;
        let b = sum - self.0;
        let low = (self.0 - (sum - b)) + (rhs.0 - b) + self.1 + rhs.1;
        let high = sum + low;
        Self(high, low - (high - sum))
    }
    fn neg(self) -> Self {
        Self(-self.0, -self.1)
    }
    fn mul(self, rhs: Self) -> Self {
        let product = self.0 * rhs.0;
        let residual = super::super::compensated::product_error(self.0, rhs.0, product);
        let low = residual + self.0 * rhs.1 + self.1 * rhs.0;
        let high = product + low;
        Self(high, low - (high - product))
    }
    fn value(self) -> Float {
        self.0 + self.1
    }
}
pub const fn split_basis(input: [[f64; 9]; 3]) -> [[Wide; 9]; 3] {
    let mut result = [[Wide(0.0, 0.0); 9]; 3];
    let mut i = 0;
    while i < 3 {
        let mut j = 0;
        while j < 9 {
            result[i][j] = Wide::constant(input[i][j]);
            j += 1;
        }
        i += 1;
    }
    result
}

fn direction(h: Float) -> (Wide, Wide) {
    // Both supported fold windows are close to 270 degrees. Keep authored
    // negative hue small too, avoiding an inexact addition of 360 in f32.
    const DEGREES_TO_RADIANS: Wide = Wide::constant(std::f64::consts::PI / 180.0);
    // The f32 caller has already reduced h into (-360, 360).
    debug_assert!(h > -360.0 && h < 360.0);
    let x = Wide::scalar(if h < 0.0 { h + 90.0 } else { h - 270.0 }).mul(DEGREES_TO_RADIANS);
    let x2 = x.mul(x);
    // Taylor remainders on |x| <= 26 degrees are below 1e-17.
    const SIN: [Wide; 7] = [
        Wide::constant(1.0),
        Wide::constant(-1.0 / 6.0),
        Wide::constant(1.0 / 120.0),
        Wide::constant(-1.0 / 5040.0),
        Wide::constant(1.0 / 362880.0),
        Wide::constant(-1.0 / 39916800.0),
        Wide::constant(1.0 / 6227020800.0),
    ];
    const COS: [Wide; 8] = [
        Wide::constant(1.0),
        Wide::constant(-1.0 / 2.0),
        Wide::constant(1.0 / 24.0),
        Wide::constant(-1.0 / 720.0),
        Wide::constant(1.0 / 40320.0),
        Wide::constant(-1.0 / 3628800.0),
        Wide::constant(1.0 / 479001600.0),
        Wide::constant(-1.0 / 87178291200.0),
    ];
    let mut sin = SIN[6];
    for i in (0..6).rev() {
        sin = sin.mul(x2).add(SIN[i]);
    }
    let mut cos = COS[7];
    for i in (0..7).rev() {
        cos = cos.mul(x2).add(COS[i]);
    }
    (x.mul(sin), cos.neg())
}
fn polynomials<G: DualrayData>(h: Float) -> [[Wide; 4]; 3] {
    let (a, b) = direction(h);
    let a2 = a.mul(a);
    let a3 = a2.mul(a);
    let a2b = a2.mul(b);
    let ab = a.mul(b);
    G::PRECISE_BASIS.map(|k| {
        [
            k[0].mul(a3)
                .add(k[1].mul(a2b))
                .add(k[2].mul(a))
                .add(k[3].mul(b)),
            k[4].add(k[5].mul(a2)).add(k[6].mul(ab)),
            k[7].mul(a).add(k[8].mul(b)),
            Wide::scalar(1.0),
        ]
    })
}
fn eval(p: [Wide; 4], x: Float) -> Wide {
    let x = Wide::scalar(x);
    p[0].mul(x).add(p[1]).mul(x).add(p[2]).mul(x).add(p[3])
}
fn first_exit(p: [Wide; 4], limit: Float) -> Float {
    let [d, b, a, _] = p;
    let mut points = [limit; 3];
    let disc = b.mul(b).add(d.mul(a).mul(Wide::scalar(-3.0))).value();
    if d.0 == 0.0 {
        let x = -a.value() / (2.0 * b.value());
        if x > 0.0 && x < limit {
            points[0] = x;
        }
    } else if disc >= 0.0 {
        let q = -b.value() - disc.sqrt().copysign(b.value());
        let x = q / (3.0 * d.value());
        let y = a.value() / q;
        let lo = x.min(y);
        let hi = x.max(y);
        let mut i = 0;
        for t in [lo, hi] {
            if t > 0.0 && t < limit {
                points[i] = t;
                i += 1;
            }
        }
    }
    let mut lo = 0.0;
    for mut hi in points {
        let fhi = eval(p, hi).value();
        if fhi == 0.0 && hi == limit && slope(p, hi) < 0.0 {
            return hi;
        }
        if fhi < 0.0 {
            // Newton stays within the first monotone sign-changing bracket.
            // Check an adjacent float on stagnation, rather than discarding a
            // converged root just because the other endpoint is still distant.
            let mut x = lo + (hi - lo) * 0.5;
            for _ in 0..8 {
                let f = eval(p, x).value();
                if f == 0.0 {
                    return x;
                }
                if f < 0.0 {
                    hi = x;
                } else {
                    lo = x;
                }
                let next = x - f / slope(p, x);
                if next == x {
                    let adjacent = if f < 0.0 { x.next_down() } else { x.next_up() };
                    let other = eval(p, adjacent).value();
                    if adjacent >= lo
                        && adjacent <= hi
                        && ((f < 0.0 && other >= 0.0) || (f > 0.0 && other <= 0.0))
                    {
                        return if f < 0.0 { adjacent } else { x };
                    }
                }
                x = if next > lo && next < hi {
                    next
                } else {
                    lo + (hi - lo) * 0.5
                };
                if x == lo || x == hi {
                    return lo;
                }
            }
            // Retain bounded bisection for near-tangent or rejected steps.
            for _ in 0..64 {
                let mid = lo + (hi - lo) * 0.5;
                if mid == lo || mid == hi {
                    break;
                }
                if eval(p, mid).value() < 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return lo;
        }
        lo = hi;
        if hi == limit {
            break;
        }
    }
    Float::INFINITY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_product_retains_the_rounding_residual() {
        let epsilon = Float::EPSILON;
        let residual = Wide::scalar(1.0 + epsilon)
            .mul(Wide::scalar(1.0 - epsilon))
            .add(Wide::scalar(-1.0));
        assert_eq!(residual.value(), -epsilon * epsilon);
        if super::super::SINGLE {
            for a in [-3.71, -0.173, 0.0317, 0.414, 0.937, 1.391, 2.81, 9.27] {
                for b in [-2.731, -0.017, 0.073, 0.499, 0.83, 1.773, 3.57] {
                    let a = a as Float;
                    let b = b as Float;
                    let product = Wide::scalar(a).mul(Wide::scalar(b));
                    assert_eq!(
                        f64::from(product.0) + f64::from(product.1),
                        f64::from(a) * f64::from(b)
                    );
                }
            }
        }
    }

    #[test]
    fn bracketed_steps_keep_the_first_exit_and_ignore_tangent_touches() {
        let p = [0.0, 1.0, -0.75, 0.125].map(Wide::scalar);
        assert!((first_exit(p, 1.0) - 0.25).abs() <= Float::EPSILON);
        assert_eq!(first_exit(p, 0.2), Float::INFINITY);
        assert_eq!(first_exit(p, 0.25), 0.25);
        let tangent = [0.0, 1.0, -1.0, 0.25].map(Wide::scalar);
        assert_eq!(first_exit(tangent, 1.0), Float::INFINITY);
    }
}

fn slope(p: [Wide; 4], x: Float) -> Float {
    let x = Wide::scalar(x);
    p[0].mul(Wide::scalar(3.0))
        .mul(x)
        .add(p[1].mul(Wide::scalar(2.0)))
        .mul(x)
        .add(p[2])
        .value()
}

#[inline(never)]
pub(super) fn map<G: DualrayData>(l: Float, c: Float, h: Float, out: &mut [Float; 3]) {
    let p = polynomials::<G>(h);
    let input_u = c / l;
    let mut u = G::ROOT_LIMIT.min(input_u);
    let mut face = None;
    for (i, row) in p.into_iter().enumerate() {
        let root = first_exit(row, u);
        if root <= u {
            u = root;
            face = Some((i, 0.0));
        }
    }
    let inv = 1.0 / l;
    let target = inv * inv * inv;
    if target.is_finite() {
        // Upper faces start inside and cross downward after negating p.
        for (i, row) in p.into_iter().enumerate() {
            let upper = [
                row[0].neg(),
                row[1].neg(),
                row[2].neg(),
                Wide::scalar(target - 1.0),
            ];
            let root = first_exit(upper, u);
            if root <= u {
                u = root;
                face = Some((i, 1.0));
            }
        }
    }
    let l3 = l * l * l;
    *out = p.map(|row| G::Transfer::encode_clamped(l3 * eval(row, u).value()));
    // Only snap a face we actually crossed; interior inputs retain conversion.
    if let Some((channel, value)) = face {
        out[channel] = value;
    }
}
