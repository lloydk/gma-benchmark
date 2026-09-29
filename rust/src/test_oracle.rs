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
