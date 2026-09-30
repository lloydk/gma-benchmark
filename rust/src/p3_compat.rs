// Compatibility helpers for the P3-only solvers. Retain their arithmetic and
// gamut-check semantics while clip/CSS MINDE use the generic typed kernel.
use super::color::{LinearRgb, Oklab, KA0, KA1, KA2, KB0, KB1, KB2};
use super::gamut::{DisplayP3, RgbGamut};
use super::transfer::{SrgbTransfer, TransferFunction};
use super::{hue_radians, Float};

pub(super) const RL: Float = DisplayP3::LMS_TO_RGB[0][0];
pub(super) const RM: Float = DisplayP3::LMS_TO_RGB[0][1];
pub(super) const RS: Float = DisplayP3::LMS_TO_RGB[0][2];
pub(super) const GL: Float = DisplayP3::LMS_TO_RGB[1][0];
pub(super) const GM: Float = DisplayP3::LMS_TO_RGB[1][1];
pub(super) const GS: Float = DisplayP3::LMS_TO_RGB[1][2];
pub(super) const BL: Float = DisplayP3::LMS_TO_RGB[2][0];
pub(super) const BM: Float = DisplayP3::LMS_TO_RGB[2][1];
pub(super) const BS: Float = DisplayP3::LMS_TO_RGB[2][2];

#[inline(always)]
pub(super) fn clamped_gamma(x: Float) -> Float {
    SrgbTransfer::encode_clamped(x)
}

#[inline(always)]
pub(super) fn oklab_to_clipped_p3(l: Float, a: Float, b: Float, out: &mut [Float; 3]) {
    let l_ = (l + KA0 * a + KB0 * b).powi(3);
    let m_ = (l + KA1 * a + KB1 * b).powi(3);
    let s_ = (l + KA2 * a + KB2 * b).powi(3);
    out[0] = clamped_gamma(RL * l_ + RM * m_ + RS * s_);
    out[1] = clamped_gamma(GL * l_ + GM * m_ + GS * s_);
    out[2] = clamped_gamma(BL * l_ + BM * m_ + BS * s_);
}

#[inline(always)]
pub(super) fn oklch_to_clipped_p3(l: Float, c: Float, h: Float, out: &mut [Float; 3]) {
    let hr = hue_radians(h);
    oklab_to_clipped_p3(l, c * hr.cos(), c * hr.sin(), out);
}

#[inline(always)]
pub(super) fn oklch_to_p3_if_in_gamut(l: Float, c: Float, h: Float, out: &mut [Float; 3]) -> bool {
    let hr = hue_radians(h);
    let a = c * hr.cos();
    let b = c * hr.sin();
    let l_ = (l + KA0 * a + KB0 * b).powi(3);
    let m_ = (l + KA1 * a + KB1 * b).powi(3);
    let s_ = (l + KA2 * a + KB2 * b).powi(3);
    let r = RL * l_ + RM * m_ + RS * s_;
    let g = GL * l_ + GM * m_ + GS * s_;
    let bl = BL * l_ + BM * m_ + BS * s_;
    if r < 0.0 || r > 1.0 || g < 0.0 || g > 1.0 || bl < 0.0 || bl > 1.0 {
        return false;
    }
    out[0] = clamped_gamma(r);
    out[1] = clamped_gamma(g);
    out[2] = clamped_gamma(bl);
    true
}

#[inline(always)]
pub(super) fn oklab_to_linear_p3_components(l: Float, a: Float, b: Float) -> (Float, Float, Float) {
    let [r, g, b] = Oklab { l, a, b }.to_linear_rgb::<DisplayP3>().channels;
    (r, g, b)
}

#[inline(always)]
pub(super) fn oklab_to_clipped_p3_fast(l: Float, a: Float, b: Float, out: &mut [Float; 3]) {
    let (r, g, bl) = oklab_to_linear_p3_components(l, a, b);
    out[0] = clamped_gamma(r);
    out[1] = clamped_gamma(g);
    out[2] = clamped_gamma(bl);
}

#[inline(always)]
pub(super) fn oklab_to_p3_if_in_gamut(l: Float, a: Float, b: Float, out: &mut [Float; 3]) -> bool {
    let (r, g, bl) = oklab_to_linear_p3_components(l, a, b);
    if r < 0.0 || r > 1.0 || g < 0.0 || g > 1.0 || bl < 0.0 || bl > 1.0 {
        return false;
    }
    out[0] = clamped_gamma(r);
    out[1] = clamped_gamma(g);
    out[2] = clamped_gamma(bl);
    true
}

#[inline(always)]
pub(super) fn linear_p3_to_oklab_chroma(r: Float, g: Float, b: Float) -> Float {
    let lab = LinearRgb::<DisplayP3>::new([r, g, b]).to_oklab();
    (lab.a * lab.a + lab.b * lab.b).sqrt()
}
