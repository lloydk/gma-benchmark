// Small coordinate values. Construction never clips or normalizes coordinates.
// RGB space and encoding are represented by types, without runtime tags.
use super::gamut::RgbGamut;
use super::transfer::{clamp01, TransferFunction};
use super::{hue_radians, Float};
use std::marker::PhantomData;

// Oklab -> LMS' a/b columns, shared by every destination gamut.
pub(super) const KA0: Float = 0.3963377773761749;
pub(super) const KB0: Float = 0.2158037573099136;
pub(super) const KA1: Float = -0.1055613458156586;
pub(super) const KB1: Float = -0.0638541728258133;
pub(super) const KA2: Float = -0.0894841775298119;
pub(super) const KB2: Float = -1.2914855480194092;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Oklch {
    pub l: Float,
    pub c: Float,
    pub h_degrees: Float,
}
impl From<[Float; 3]> for Oklch {
    fn from([l, c, h_degrees]: [Float; 3]) -> Self {
        Self { l, c, h_degrees }
    }
}
impl Oklch {
    #[inline(always)]
    pub fn to_oklab(self) -> Oklab {
        let h = hue_radians(self.h_degrees);
        // Use the same paired trig operation as the mappers. Separate calls
        // can be combined differently by the optimizer, changing output bits.
        let (sin, cos) = h.sin_cos();
        Oklab {
            l: self.l,
            a: self.c * cos,
            b: self.c * sin,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Oklab {
    pub l: Float,
    pub a: Float,
    pub b: Float,
}
impl Oklab {
    #[inline(always)]
    pub fn to_linear_rgb<G: RgbGamut>(self) -> LinearRgb<G> {
        let l = self.l + KA0 * self.a + KB0 * self.b;
        let m = self.l + KA1 * self.a + KB1 * self.b;
        let s = self.l + KA2 * self.a + KB2 * self.b;
        let (l3, m3, s3) = (l * l * l, m * m * m, s * s * s);
        let matrix = G::LMS_TO_RGB;
        LinearRgb::new([
            matrix[0][0] * l3 + matrix[0][1] * m3 + matrix[0][2] * s3,
            matrix[1][0] * l3 + matrix[1][1] * m3 + matrix[1][2] * s3,
            matrix[2][0] * l3 + matrix[2][1] * m3 + matrix[2][2] * s3,
        ])
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub(crate) struct LinearRgb<G> {
    pub channels: [Float; 3],
    gamut: PhantomData<G>,
}
impl<G: RgbGamut> LinearRgb<G> {
    pub fn new(channels: [Float; 3]) -> Self {
        Self {
            channels,
            gamut: PhantomData,
        }
    }

    #[inline(always)]
    pub fn in_gamut(self) -> bool {
        let [r, g, b] = self.channels;
        r >= 0.0 && r <= 1.0 && g >= 0.0 && g <= 1.0 && b >= 0.0 && b <= 1.0
    }

    #[inline(always)]
    pub fn clipped(self) -> Self {
        Self::new(self.channels.map(clamp01))
    }

    #[inline(always)]
    pub fn encode_clamped(self) -> EncodedRgb<G> {
        EncodedRgb {
            channels: self.channels.map(G::Transfer::encode_clamped),
            gamut: PhantomData,
        }
    }

    #[inline(always)]
    pub fn to_oklab(self) -> Oklab {
        let [r, g, b] = self.channels;
        let matrix = G::RGB_TO_LMS;
        let l = (matrix[0][0] * r + matrix[0][1] * g + matrix[0][2] * b).cbrt();
        let m = (matrix[1][0] * r + matrix[1][1] * g + matrix[1][2] * b).cbrt();
        let s = (matrix[2][0] * r + matrix[2][1] * g + matrix[2][2] * b).cbrt();
        Oklab {
            l: 0.2104542683093140 * l + 0.7936177747023054 * m - 0.0040720430116193 * s,
            a: 1.9779985324311684 * l - 2.4285922420485799 * m + 0.4505937096174110 * s,
            b: 0.0259040424655478 * l + 0.7827717124575296 * m - 0.8086757549230774 * s,
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub(crate) struct EncodedRgb<G> {
    pub channels: [Float; 3],
    gamut: PhantomData<G>,
}
