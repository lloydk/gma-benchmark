// Independent f64 physical-chroma oracle. It partitions each channel cubic
// at its stationary points, then bisects the first crossing of either face.
// It never calls the production Cardano, Halley, or Ostrowski solvers.
const A: [[f64; 2]; 3] = [
    [0.3963377773761749, 0.2158037573099136],
    [-0.1055613458156586, -0.0638541728258133],
    [-0.0894841775298119, -1.2914855480194092],
];
const M: [[f64; 3]; 3] = [
    [3.127768971361874, -2.2571357625916395, 0.12936679122976516],
    [-1.0910090184377979, 2.413331710306922, -0.32232269186912466],
    [-0.02601080193857028, -0.508041331704167, 1.5340521336427373],
];

fn first_crossing(p: [f64; 4], limit: f64) -> f64 {
    let [a, b, c, d] = p;
    let eval = |x| ((a * x + b) * x + c) * x + d;
    let mut points = vec![0.0, limit];
    if a == 0.0 {
        let x = -c / (2.0 * b);
        if x > 0.0 && x < limit {
            points.push(x);
        }
    } else {
        let discriminant = b * b - 3.0 * a * c;
        if discriminant >= 0.0 {
            let q = -b - discriminant.sqrt().copysign(b);
            for x in [q / (3.0 * a), c / q] {
                if x > 0.0 && x < limit {
                    points.push(x);
                }
            }
        }
    }
    points.sort_by(f64::total_cmp);
    for pair in points.windows(2) {
        let (mut lo, mut hi) = (pair[0], pair[1]);
        let sign = eval(lo).is_sign_negative();
        if eval(hi) == 0.0 {
            return hi;
        }
        if eval(hi).is_sign_negative() == sign {
            continue;
        }
        for _ in 0..64 {
            let mid = lo + (hi - lo) * 0.5;
            if mid == lo || mid == hi {
                break;
            }
            if eval(mid).is_sign_negative() == sign {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        return lo + (hi - lo) * 0.5;
    }
    f64::INFINITY
}

fn direction(h: f64) -> [f64; 3] {
    let h = h * std::f64::consts::PI / 180.0;
    A.map(|[a, b]| a * h.cos() + b * h.sin())
}

pub(crate) fn boundary(l: f64, h: f64) -> f64 {
    let q = direction(h);
    let mut chroma: f64 = 0.5;
    for row in M {
        let a = (0..3).map(|i| row[i] * q[i].powi(3)).sum();
        let b = 3.0 * l * (0..3).map(|i| row[i] * q[i].powi(2)).sum::<f64>();
        let c = 3.0 * l * l * (0..3).map(|i| row[i] * q[i]).sum::<f64>();
        let d = l.powi(3) * row.iter().sum::<f64>();
        chroma = chroma.min(first_crossing([a, b, c, d], chroma));
        chroma = chroma.min(first_crossing([a, b, c, d - 1.0], chroma));
    }
    chroma
}

pub(crate) fn encoded(l: f64, c: f64, h: f64) -> [f64; 3] {
    let lms = direction(h).map(|q| (l + c * q).powi(3));
    M.map(|row| {
        let linear = (0..3).map(|i| row[i] * lms[i]).sum::<f64>().clamp(0.0, 1.0);
        if linear <= 0.0031308 {
            linear * 12.92
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        }
    })
}

// Multi-gamut oracle. The matrix is derived through the independent XYZ
// reference, never read from a production gamut or solver coefficient array.
pub(crate) struct BoundaryOracle {
    pub reference: crate::rgb_reference::Reference,
    rows: [[f64; 3]; 3],
    id: crate::rgb_spaces::SpaceId,
}
impl BoundaryOracle {
    pub fn new(id: crate::rgb_spaces::SpaceId) -> Self {
        let reference = crate::rgb_reference::Reference::new(id);
        let columns = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
            .map(|basis| reference.lms_to_rgb(basis));
        let rows = std::array::from_fn(|i| columns.map(|col| col[i]));
        Self {
            reference,
            rows,
            id,
        }
    }

    // Selected lower face, independent of Bottosson's fits and Halley step.
    pub fn face_saturation(&self, h: f64, channel: usize) -> f64 {
        let q = direction(h);
        let row = self.rows[channel];
        first_exit(
            [
                (0..3).map(|i| row[i] * q[i].powi(3)).sum(),
                3.0 * (0..3).map(|i| row[i] * q[i].powi(2)).sum::<f64>(),
                3.0 * (0..3).map(|i| row[i] * q[i]).sum::<f64>(),
                row.iter().sum(),
            ],
            f64::INFINITY,
        )
    }

    pub fn boundary(&self, l: f64, h: f64) -> f64 {
        if l <= 0.0 || l >= 1.0 {
            return 0.0;
        }
        let q = direction(h);
        // Solve in t=C/L, so near-black channel coefficients do not underflow.
        let polynomials = self.rows.map(|row| {
            [
                (0..3).map(|i| row[i] * q[i].powi(3)).sum::<f64>(),
                3.0 * (0..3).map(|i| row[i] * q[i].powi(2)).sum::<f64>(),
                3.0 * (0..3).map(|i| row[i] * q[i]).sum::<f64>(),
                row.iter().sum::<f64>(),
            ]
        });
        let mut limit = f64::INFINITY;
        for polynomial in polynomials {
            limit = limit.min(first_exit(polynomial, f64::INFINITY));
        }
        assert!(limit.is_finite(), "no lower exit at {l}, {h}");
        let white = 1.0 / l.powi(3);
        if white.is_finite() {
            for [a, b, c, d] in polynomials {
                limit = limit.min(first_exit([-a, -b, -c, white - d], limit));
            }
        }
        l * limit
    }

    // Geometric reference: no production iteration or stopping criterion.
    pub fn iterative_boundary(&self, l: f64, h: f64, _ostrowski: bool) -> f64 {
        if self.in_fold(h) {
            self.fold_boundaries(l, h, 2e-14, false)
                .into_iter()
                .fold(self.boundary(l, h), f64::max)
        } else {
            self.boundary(l, h)
        }
    }

    pub fn iterative_chroma(&self, l: f64, c: f64, h: f64) -> f64 {
        if !self.in_fold(h) {
            return c.min(self.boundary(l, h));
        }
        if self
            .reference
            .linear_rgb([l, c, h])
            .iter()
            .all(|v| *v >= 0.0 && *v <= 1.0)
        {
            return c;
        }
        self.fold_boundaries(l, h, 2e-14, false)
            .into_iter()
            .filter(|&exit| exit <= c)
            .fold(0.0, f64::max)
    }

    pub fn in_fold(&self, h: f64) -> bool {
        // Share only the authored policy window; the root geometry above is
        // independent of the production solvers.
        crate::float64::rgb_solvers::in_blue_fold_id(self.id, h)
    }

    // All feasible outward face intersections, only for the fold window.
    // Native rounding can decide whether a nearly touching outer island exists.
    // Keep that branch ambiguity separate from error within either branch.
    pub fn fold_boundaries(
        &self,
        l: f64,
        h: f64,
        linear_slack: f64,
        only_ambiguous: bool,
    ) -> Vec<f64> {
        if !self.in_fold(h) || l <= 0.0 || l >= 1.0 {
            return vec![];
        }
        let q = direction(h);
        let mut result = Vec::new();
        let mut ambiguous = false;
        for row in self.rows {
            let a = (0..3).map(|i| row[i] * q[i].powi(3)).sum::<f64>();
            let b = 3.0 * l * (0..3).map(|i| row[i] * q[i].powi(2)).sum::<f64>();
            let c = 3.0 * l * l * (0..3).map(|i| row[i] * q[i]).sum::<f64>();
            for face in [0.0, 1.0] {
                let d = l.powi(3) * row.iter().sum::<f64>() - face;
                let eval = |x| ((a * x + b) * x + c) * x + d;
                let mut knots = vec![0.0, 0.5];
                let disc = b * b - 3.0 * a * c;
                if a != 0.0 && disc >= 0.0 {
                    for x in [
                        (-b - disc.sqrt()) / (3.0 * a),
                        (-b + disc.sqrt()) / (3.0 * a),
                    ] {
                        if x > 0.0 && x < 0.5 {
                            knots.push(x);
                        }
                    }
                } else if a == 0.0 && b != 0.0 {
                    let x = -c / (2.0 * b);
                    if x > 0.0 && x < 0.5 {
                        knots.push(x);
                    }
                }
                // A stationary contact can create/remove the gap, while a
                // corner contact can create/remove the outer island. Away
                // from these cases the geometric outer branch is required.
                for &x in &knots {
                    if x > 0.0 && x < 0.5 && eval(x).abs() <= linear_slack {
                        let rgb = self.reference.linear_rgb([l, x, h]);
                        ambiguous |= rgb
                            .iter()
                            .all(|v| *v >= -linear_slack && *v <= 1.0 + linear_slack);
                    }
                }
                knots.sort_by(f64::total_cmp);
                for interval in knots.windows(2) {
                    let (mut lo, mut hi) = (interval[0], interval[1]);
                    let sign = eval(lo).is_sign_negative();
                    if sign == eval(hi).is_sign_negative() {
                        continue;
                    }
                    for _ in 0..100 {
                        let mid = (lo + hi) / 2.0;
                        if mid == lo || mid == hi {
                            break;
                        }
                        if eval(mid).is_sign_negative() == sign {
                            lo = mid;
                        } else {
                            hi = mid;
                        }
                    }
                    let x = (lo + hi) / 2.0;
                    let derivative = (3.0 * a * x + 2.0 * b) * x + c;
                    if (face == 0.0 && derivative >= 0.0) || (face == 1.0 && derivative <= 0.0) {
                        continue;
                    }
                    let rgb = self.reference.linear_rgb([l, x, h]);
                    if rgb
                        .iter()
                        .all(|v| *v >= -linear_slack && *v <= 1.0 + linear_slack)
                    {
                        ambiguous |= rgb
                            .iter()
                            .filter(|v| {
                                v.abs() <= linear_slack || (**v - 1.0).abs() <= linear_slack
                            })
                            .count()
                            >= 2;
                        result.push(x);
                    }
                }
            }
        }
        if only_ambiguous && !ambiguous {
            vec![]
        } else {
            result
        }
    }
}

// Earliest exit from p(t)>=0. Tangencies that stay inside are skipped. Unlike
// global membership bisection, stationary intervals retain early exit/re-entry.
fn first_exit(p: [f64; 4], limit: f64) -> f64 {
    let [a, b, c, d] = p;
    if d < 0.0 {
        return 0.0;
    }
    let bound = if a != 0.0 {
        1.0 + (b / a).abs().max((c / a).abs()).max((d / a).abs())
    } else if b != 0.0 {
        1.0 + (c / b).abs().max((d / b).abs())
    } else if c != 0.0 {
        1.0 + (d / c).abs()
    } else {
        return f64::INFINITY;
    };
    let hi = limit.min(bound);
    let mut points = vec![0.0, hi];
    if a == 0.0 {
        if b != 0.0 {
            let t = -c / (2.0 * b);
            if t > 0.0 && t < hi {
                points.push(t);
            }
        }
    } else {
        let discriminant = b * b - 3.0 * a * c;
        if discriminant >= 0.0 {
            let numerator = -b - discriminant.sqrt().copysign(b);
            for t in [numerator / (3.0 * a), c / numerator] {
                if t > 0.0 && t < hi {
                    points.push(t);
                }
            }
        }
    }
    points.sort_by(f64::total_cmp);
    let eval = |t| ((a * t + b) * t + c) * t + d;
    for pair in points.windows(2) {
        let (mut lo, mut hi) = (pair[0], pair[1]);
        if eval(hi) >= 0.0 {
            continue;
        }
        if eval(lo) <= 0.0 {
            return lo;
        }
        for _ in 0..100 {
            let mid = lo + (hi - lo) * 0.5;
            if mid == lo || mid == hi {
                break;
            }
            if eval(mid) < 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        return lo + (hi - lo) * 0.5;
    }
    f64::INFINITY
}

#[test]
fn multi_gamut_oracle_handles_reentry_tangency_and_degeneracy() {
    assert!((first_exit([-1.0, 1.5, -0.6875, 0.09375], 1.0) - 0.25).abs() < 1e-14);
    assert!((first_exit([-1.0, 1.25, -0.4375, 0.046875], 1.0) - 0.75).abs() < 1e-14);
    assert!((first_exit([0.0, 1.0, -1.0, 0.1875], 1.0) - 0.25).abs() < 1e-14);
    assert_eq!(first_exit([0.0, 0.0, -2.0, 1.0], 1.0), 0.5);
    assert!(first_exit([0.0, 0.0, 0.0, 1.0], 1.0).is_infinite());
    assert!(first_exit([0.0, 0.0, 1.0, 0.0], 1.0).is_infinite());
    assert_eq!(first_exit([0.0, 0.0, -1.0, 0.0], 1.0), 0.0);
}

#[test]
fn multi_gamut_oracle_exits_all_six_faces() {
    use crate::rgb_spaces::SpaceId;
    for id in [SpaceId::Srgb, SpaceId::DisplayP3, SpaceId::Rec2020] {
        let oracle = BoundaryOracle::new(id);
        let mut faces = [false; 6];
        let mut max_c = 0.0f64;
        for l in [0.001, 0.01, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99, 0.9999] {
            for hi in 0..1440 {
                let h = hi as f64 / 4.0;
                let c = oracle.boundary(l, h);
                max_c = max_c.max(c);
                let inside = oracle.reference.linear_rgb([l, c * (1.0 - 1e-7), h]);
                let outside = oracle.reference.linear_rgb([l, c * (1.0 + 1e-7), h]);
                assert!(
                    inside.iter().all(|v| *v >= -2e-14 && *v <= 1.0 + 2e-14),
                    "{id:?}: {l} {c} {h} {inside:?}"
                );
                for i in 0..3 {
                    if outside[i] < 0.0 {
                        faces[2 * i] = true;
                    }
                    if outside[i] > 1.0 {
                        faces[2 * i + 1] = true;
                    }
                }
                assert!(
                    outside.iter().any(|v| *v < 0.0 || *v > 1.0),
                    "{id:?}: {l} {c} {h} {outside:?}"
                );
            }
        }
        assert!(faces.iter().all(|v| *v), "{id:?} {faces:?}");
        eprintln!("{id:?} oracle maximum sampled chroma {max_c}");
    }
}

// Independent four-cast Raytrace policy, using the uncomposed XYZ reference.
// It deliberately has no first-exit expectation: this mapper follows successive
// RGB box intersections and can choose a different point on a folded ray.
pub(crate) fn raytrace(
    reference: &crate::rgb_reference::Reference,
    input: [f64; 3],
    single: bool,
) -> [f64; 3] {
    let [l, c, h] = input;
    if l <= 0.0 {
        return [0.0; 3];
    }
    if l >= 1.0 {
        return [1.0; 3];
    }
    let gray = l * l * l;
    if c <= 0.0 {
        return [gray; 3];
    }
    let mut target = reference.linear_rgb(input);
    let mut anchor = [gray; 3];
    let mut last = target;
    let low = if single {
        8.0 * f64::from(f32::EPSILON)
    } else {
        1e-12
    };
    for i in 0..4 {
        if i != 0 {
            let lab = reference.linear_to_lab(target);
            target = reference.linear_rgb([l, lab[1].hypot(lab[2]), h]);
        }
        let direction = std::array::from_fn::<_, 3, _>(|j| target[j] - anchor[j]);
        let convergence = (if single { 8.0 } else { 32.0 })
            * if single {
                f64::from(f32::EPSILON)
            } else {
                f64::EPSILON
            };
        if i != 0
            && (direction.iter().map(|v| v.abs()).fold(0.0, f64::max)
                <= convergence * anchor.iter().map(|v| v.abs()).fold(0.0, f64::max)
                || (0..3)
                    .map(|j| (target[j] - last[j]).abs())
                    .fold(0.0, f64::max)
                    <= convergence * last.iter().map(|v| v.abs()).fold(0.0, f64::max))
        {
            break;
        }
        let mut distance = f64::INFINITY;
        for j in 0..3 {
            let d = direction[j];
            if d == 0.0 {
                continue;
            }
            let face = if d > 0.0 { 1.0 } else { 0.0 };
            distance = distance.min((face - anchor[j]) / d);
        }
        if !distance.is_finite() {
            break;
        }
        last = std::array::from_fn(|j| anchor[j] + direction[j] * distance);
        if i != 0 && target.iter().all(|v| *v > low && *v < 1.0 - low) {
            anchor = target;
        }
        target = last;
    }
    last.map(|v| v.clamp(0.0, 1.0))
}
