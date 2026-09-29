// CSS Color 4 binary search with Local MINDE, specialized to OKLCh → Display-P3.
// https://www.w3.org/TR/css-color-4/#binsearch
use super::{clamp01, clamped_gamma, hue_radians, oklab_to_linear_p3_components, Float};

const JND: Float = 0.02;
const EPSILON: Float = 0.0001;

// Linear clipping commutes with the P3 transfer function. All intermediates,
// including these cube roots and deltaEOK, use the lane's native precision.
fn delta_from_clipped(lab_l: Float, a: Float, b: Float, r: Float, g: Float, blue: Float) -> Float {
    let l = (0.4813798527499543 * r + 0.4621183710113182 * g + 0.05650177623872754 * blue).cbrt();
    let m = (0.2288319418112447 * r + 0.6532168193835677 * g + 0.11795123880518772 * blue).cbrt();
    let s = (0.08394575232299314 * r + 0.22416527097756647 * g + 0.6918889766994405 * blue).cbrt();
    let dl = 0.2104542683093140 * l + 0.7936177747023054 * m - 0.0040720430116193 * s - lab_l;
    let da = 1.9779985324311684 * l - 2.4285922420485799 * m + 0.4505937096174110 * s - a;
    let db = 0.0259040424655478 * l + 0.7827717124575296 * m - 0.8086757549230774 * s - b;
    (dl * dl + da * da + db * db).sqrt()
}

fn in_gamut(r: Float, g: Float, b: Float) -> bool {
    r >= 0.0 && r <= 1.0 && g >= 0.0 && g <= 1.0 && b >= 0.0 && b <= 1.0
}

pub(crate) struct CssMinde;

impl CssMinde {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn map(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        let [l, c, h] = *oklch;
        if l <= 0.0 || l >= 1.0 {
            *out = [if l <= 0.0 { 0.0 } else { 1.0 }; 3];
            return;
        }
        let c = c.max(0.0);
        let h = if c == 0.0 {
            0.0
        } else if h <= -1e9 || h >= 1e9 {
            h % 360.0
        } else {
            h
        };
        let angle = hue_radians(h);
        let (hue_a, hue_b) = (angle.cos(), angle.sin());
        let (mut a, mut b) = (c * hue_a, c * hue_b);
        let (mut r, mut g, mut blue) = oklab_to_linear_p3_components(l, a, b);
        // Preserve the canonical conversion for in-gamut inputs. EPSILON is
        // not a gamut-membership tolerance.
        if !in_gamut(r, g, blue) {
            (r, g, blue) = (clamp01(r), clamp01(g), clamp01(blue));
            if delta_from_clipped(l, a, b, r, g, blue) >= JND {
                let (mut min, mut max, mut min_in_gamut) = (0.0, c, true);
                while max - min > EPSILON {
                    let chroma = (min + max) / 2.0;
                    a = chroma * hue_a;
                    b = chroma * hue_b;
                    let (cr, cg, cb) = oklab_to_linear_p3_components(l, a, b);
                    if min_in_gamut && in_gamut(cr, cg, cb) {
                        min = chroma;
                        continue;
                    }
                    (r, g, blue) = (clamp01(cr), clamp01(cg), clamp01(cb));
                    let e = delta_from_clipped(l, a, b, r, g, blue);
                    if e < JND {
                        if JND - e < EPSILON {
                            break;
                        }
                        min_in_gamut = false;
                        min = chroma;
                    } else {
                        max = chroma;
                    }
                }
            }
        }
        // The spec returns the last clip even if interval convergence leaves
        // its deltaEOK slightly above the JND.
        *out = [clamped_gamma(r), clamped_gamma(g), clamped_gamma(blue)];
    }

    pub(crate) fn map_with_in_gamut_check(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        // The spec's in-gamut check is intrinsic in both benchmark modes.
        self.map(oklch, out);
    }
}

#[cfg(test)]
mod tests {
    include!("css_minde_tests.rs");
}
