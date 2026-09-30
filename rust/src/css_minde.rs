// CSS Color 4 binary search with Local MINDE, specialized by target gamut.
// https://www.w3.org/TR/css-color-4/#binsearch
use super::color::Oklab;
use super::gamut::RgbGamut;
use super::{hue_radians, Float};
use std::marker::PhantomData;

const JND: Float = 0.02;
const EPSILON: Float = 0.0001;

// Each supported transfer is monotone and maps 0/1 to 0/1, so clipping may
// happen in linear RGB. All arithmetic uses the lane's native precision.
fn delta(one: Oklab, two: Oklab) -> Float {
    let dl = one.l - two.l;
    let da = one.a - two.a;
    let db = one.b - two.b;
    (dl * dl + da * da + db * db).sqrt()
}

pub(crate) struct CssMinde<G>(PhantomData<G>);

impl<G: RgbGamut> CssMinde<G> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
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
        let mut lab = Oklab {
            l,
            a: c * hue_a,
            b: c * hue_b,
        };
        let mut rgb = lab.to_linear_rgb::<G>();
        // Preserve the canonical conversion for in-gamut inputs. EPSILON is
        // not a gamut-membership tolerance.
        if !rgb.in_gamut() {
            rgb = rgb.clipped();
            if delta(rgb.to_oklab(), lab) >= JND {
                let (mut min, mut max, mut min_in_gamut) = (0.0, c, true);
                while max - min > EPSILON {
                    let chroma = (min + max) / 2.0;
                    lab.a = chroma * hue_a;
                    lab.b = chroma * hue_b;
                    let candidate = lab.to_linear_rgb::<G>();
                    if min_in_gamut && candidate.in_gamut() {
                        min = chroma;
                        continue;
                    }
                    rgb = candidate.clipped();
                    let e = delta(rgb.to_oklab(), lab);
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
        *out = rgb.encode_clamped().channels;
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
