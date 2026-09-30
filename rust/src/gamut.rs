use super::transfer::{Rec2020Transfer, SrgbTransfer, TransferFunction};
use super::Float;
use crate::rgb_spaces::{self, Matrix, RgbSpace};

// Restricted to the three D65 SDR profiles validated here. Fitted solver data
// belongs to the algorithm, not to this conversion/gamut contract.
pub(crate) trait RgbGamut: RgbSpace {
    const LMS_TO_RGB: [[Float; 3]; 3];
    const RGB_TO_LMS: [[Float; 3]; 3];
    type Transfer: TransferFunction;
}

pub(crate) use crate::rgb_spaces::{DisplayP3, Rec2020, Srgb};

// Composition is evaluated only in associated constants, in f64 before the
// final cast. No f64 arithmetic is introduced into the native f32 hot path.
const fn compose(a: Matrix, b: Matrix) -> [[Float; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    let mut i = 0;
    while i < 3 {
        let mut j = 0;
        while j < 3 {
            out[i][j] = (a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j]) as Float;
            j += 1;
        }
        i += 1;
    }
    out
}

impl RgbGamut for Srgb {
    const LMS_TO_RGB: [[Float; 3]; 3] =
        compose(rgb_spaces::SRGB.xyz_to_rgb, rgb_spaces::LMS_TO_XYZ);
    const RGB_TO_LMS: [[Float; 3]; 3] =
        compose(rgb_spaces::XYZ_TO_LMS, rgb_spaces::SRGB.rgb_to_xyz);
    type Transfer = SrgbTransfer;
}

impl RgbGamut for DisplayP3 {
    // Preserve the incumbent P3 coefficients exactly. Recomposition differs in
    // last bits; tests check these against the independent XYZ conversion.
    const LMS_TO_RGB: [[Float; 3]; 3] = [
        [3.127768971361874, -2.2571357625916395, 0.12936679122976516],
        [-1.0910090184377979, 2.413331710306922, -0.32232269186912466],
        [-0.02601080193857028, -0.508041331704167, 1.5340521336427373],
    ];
    const RGB_TO_LMS: [[Float; 3]; 3] = [
        [0.4813798527499543, 0.4621183710113182, 0.05650177623872754],
        [0.2288319418112447, 0.6532168193835677, 0.11795123880518772],
        [0.08394575232299314, 0.22416527097756647, 0.6918889766994405],
    ];
    type Transfer = SrgbTransfer;
}

impl RgbGamut for Rec2020 {
    const LMS_TO_RGB: [[Float; 3]; 3] =
        compose(rgb_spaces::REC2020.xyz_to_rgb, rgb_spaces::LMS_TO_XYZ);
    const RGB_TO_LMS: [[Float; 3]; 3] =
        compose(rgb_spaces::XYZ_TO_LMS, rgb_spaces::REC2020.rgb_to_xyz);
    type Transfer = Rec2020Transfer;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_p3_coefficients_agree_with_xyz_definitions() {
        // Keep historical output bits, while checking both directions against
        // the shared definitions. Recomposition can differ in the last bits.
        for (pinned, composed) in [
            (
                DisplayP3::LMS_TO_RGB,
                compose(rgb_spaces::DISPLAY_P3.xyz_to_rgb, rgb_spaces::LMS_TO_XYZ),
            ),
            (
                DisplayP3::RGB_TO_LMS,
                compose(rgb_spaces::XYZ_TO_LMS, rgb_spaces::DISPLAY_P3.rgb_to_xyz),
            ),
        ] {
            for (a, b) in pinned
                .into_iter()
                .flatten()
                .zip(composed.into_iter().flatten())
            {
                assert!(
                    (a - b).abs() <= 8.0 * Float::EPSILON * b.abs().max(1.0),
                    "{a} vs {b}"
                );
            }
        }
    }
}
