// Compiled in each concrete precision lane. Gamut tests inspect unclipped
// linear coordinates; clipping for MINDE and output encoding is explicit.
use super::Float;

#[inline(always)]
pub(super) fn clamp01(x: Float) -> Float {
    if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else {
        x
    }
}

pub(crate) trait TransferFunction {
    fn encode_clamped(x: Float) -> Float;
}

pub(crate) struct SrgbTransfer;
impl TransferFunction for SrgbTransfer {
    #[inline(always)]
    fn encode_clamped(x: Float) -> Float {
        let x = clamp01(x);
        if x <= 0.0031308 {
            x * 12.92
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    }
}

// CSS rec2020 is display-referred gamma 2.4 (BT.1886 with black=0, gain=1),
// as specified in CSS Color 4, 26 September 2026, section 10.8.
// This is distinct from the piecewise BT.2020 camera OETF.
pub(crate) struct Rec2020Transfer;
impl TransferFunction for Rec2020Transfer {
    #[inline(always)]
    fn encode_clamped(x: Float) -> Float {
        clamp01(x).powf(1.0 / 2.4)
    }
}
