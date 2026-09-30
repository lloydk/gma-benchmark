//! Independent f64 validation oracle. RGB matrices are derived from chromaticities, and
//! conversions take the uncomposed path through XYZ. No production profiles,
//! composed coefficients, transfer functions, or mapper helpers are used.
//! Sources: CSS Color 4 (26 September 2026), sections 10, 14.2.2 and 19.

use crate::rgb_spaces::SpaceId;

type Matrix = [[f64; 3]; 3];

fn multiply(m: Matrix, v: [f64; 3]) -> [f64; 3] {
    m.map(|row| row.into_iter().zip(v).map(|(a, b)| a * b).sum())
}

fn inverse(m: Matrix) -> Matrix {
    let [[a, b, c], [d, e, f], [g, h, i]] = m;
    let cofactors = [
        [e * i - f * h, c * h - b * i, b * f - c * e],
        [f * g - d * i, a * i - c * g, c * d - a * f],
        [d * h - e * g, b * g - a * h, a * e - b * d],
    ];
    let determinant = a * cofactors[0][0] + b * cofactors[1][0] + c * cofactors[2][0];
    cofactors.map(|row| row.map(|v| v / determinant))
}

pub(crate) struct Reference {
    rgb_to_xyz: Matrix,
    #[cfg(test)]
    xyz_to_rgb: Matrix,
    rec2020: bool,
}

impl Reference {
    pub fn new(id: SpaceId) -> Self {
        let primaries = match id {
            SpaceId::Srgb => [[0.64, 0.33], [0.30, 0.60], [0.15, 0.06]],
            SpaceId::DisplayP3 => [[0.68, 0.32], [0.265, 0.69], [0.15, 0.06]],
            SpaceId::Rec2020 => [[0.708, 0.292], [0.170, 0.797], [0.131, 0.046]],
        };
        let columns = primaries.map(|[x, y]| [x / y, 1.0, (1.0 - x - y) / y]);
        let matrix = std::array::from_fn(|row| std::array::from_fn(|col| columns[col][row]));
        let white = [0.3127 / 0.3290, 1.0, (1.0 - 0.3127 - 0.3290) / 0.3290];
        let scale = multiply(inverse(matrix), white);
        let rgb_to_xyz = matrix.map(|row| std::array::from_fn(|i| row[i] * scale[i]));
        Self {
            rgb_to_xyz,
            #[cfg(test)]
            xyz_to_rgb: inverse(rgb_to_xyz),
            rec2020: id == SpaceId::Rec2020,
        }
    }

    #[cfg(test)]
    pub fn encode(&self, x: f64) -> f64 {
        if self.rec2020 {
            x.abs().powf(1.0 / 2.4).copysign(x)
        } else if x.abs() <= 0.0031308 {
            12.92 * x
        } else {
            (1.055 * x.abs().powf(1.0 / 2.4) - 0.055).copysign(x)
        }
    }

    pub fn decode(&self, x: f64) -> f64 {
        if self.rec2020 {
            x.abs().powf(2.4).copysign(x)
        } else if x.abs() <= 0.04045 {
            x / 12.92
        } else {
            ((x.abs() + 0.055) / 1.055).powf(2.4).copysign(x)
        }
    }

    #[cfg(test)]
    pub fn linear_rgb(&self, lch: [f64; 3]) -> [f64; 3] {
        let lms = multiply(
            [
                [1.0, 0.3963377773761749, 0.2158037573099136],
                [1.0, -0.1055613458156586, -0.0638541728258133],
                [1.0, -0.0894841775298119, -1.2914855480194092],
            ],
            lab(lch),
        )
        .map(|x| x.powi(3));
        self.lms_to_rgb(lms)
    }

    #[cfg(test)]
    pub fn lms_to_rgb(&self, lms: [f64; 3]) -> [f64; 3] {
        let xyz = multiply(
            [
                [1.2268798758459243, -0.5578149944602171, 0.2813910456659647],
                [-0.0405757452148008, 1.1122868032803170, -0.0717110580655164],
                [-0.0763729366746601, -0.4214933324022432, 1.5869240198367816],
            ],
            lms,
        );
        multiply(self.xyz_to_rgb, xyz)
    }

    pub fn linear_to_lab(&self, rgb: [f64; 3]) -> [f64; 3] {
        let xyz = multiply(self.rgb_to_xyz, rgb);
        let lms = multiply(
            [
                [0.8190224379967030, 0.3619062600528904, -0.1288737815209879],
                [0.0329836539323885, 0.9292868615863434, 0.0361446663506424],
                [0.0481771893596242, 0.2642395317527308, 0.6335478284694309],
            ],
            xyz,
        )
        .map(f64::cbrt);
        multiply(
            [
                [0.2104542683093140, 0.7936177747023054, -0.0040720430116193],
                [1.9779985324311684, -2.4285922420485799, 0.4505937096174110],
                [0.0259040424655478, 0.7827717124575296, -0.8086757549230774],
            ],
            lms,
        )
    }

    pub fn encoded_to_lab(&self, rgb: [f64; 3]) -> [f64; 3] {
        self.linear_to_lab(rgb.map(|x| self.decode(x)))
    }

    #[cfg(test)]
    pub fn css_minde(&self, mut current: [f64; 3]) -> ([f64; 3], usize) {
        // Exit IDs: black, white, in-gamut, initial-clip, close-enough, interval.
        if current[0] <= 0.0 {
            return ([0.0; 3], 0);
        }
        if current[0] >= 1.0 {
            return ([1.0; 3], 1);
        }
        let to_rgb = |lch| self.linear_rgb(lch).map(|x| self.encode(x));
        let in_gamut = |rgb: [f64; 3]| rgb.iter().all(|x| (0.0..=1.0).contains(x));
        let clip = |rgb: [f64; 3]| rgb.map(|x| x.clamp(0.0, 1.0));
        let converted = to_rgb(current);
        if in_gamut(converted) {
            return (converted, 2);
        }
        let mut clipped = clip(converted);
        if distance(self.encoded_to_lab(clipped), lab(current)) < 0.02 {
            return (clipped, 3);
        }
        let (mut min, mut max, mut min_in_gamut) = (0.0, current[1], true);
        while max - min > 0.0001 {
            let chroma = (min + max) / 2.0;
            current[1] = chroma;
            let rgb = to_rgb(current);
            if min_in_gamut && in_gamut(rgb) {
                min = chroma;
                continue;
            }
            clipped = clip(rgb);
            let error = distance(self.encoded_to_lab(clipped), lab(current));
            if error < 0.02 {
                if 0.02 - error < 0.0001 {
                    return (clipped, 4);
                }
                min_in_gamut = false;
                min = chroma;
            } else {
                max = chroma;
            }
        }
        (clipped, 5)
    }
}

#[cfg(test)]
pub(crate) fn lab([l, c, h]: [f64; 3]) -> [f64; 3] {
    let angle = h * std::f64::consts::PI / 180.0;
    [l, c * angle.cos(), c * angle.sin()]
}

#[cfg(test)]
pub(crate) fn lch([l, a, b]: [f64; 3]) -> [f64; 3] {
    [l, a.hypot(b), b.atan2(a).to_degrees()]
}

pub(crate) fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1]).hypot(a[2] - b[2])
}
