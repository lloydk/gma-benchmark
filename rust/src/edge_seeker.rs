// Target-specific tables belong to the algorithm, separate from RgbGamut.
#[cfg(test)]
use super::color::Oklch;
use super::gamut::{DisplayP3, Rec2020, RgbGamut, Srgb};
use super::rgb_solvers::{oklch_to_clipped_rgb, oklch_to_rgb_if_in_gamut};
use super::{Float, SINGLE};
use std::marker::PhantomData;

pub(crate) trait EdgeSeekerData: RgbGamut {
    const LUT: &'static [[Float; 4]];
    const SHARP_INTERVALS: &'static [(usize, [Float; 4], [Float; 2])];
    const INTERVAL_INDEX: &'static [usize; HUE_INDEX_BUCKETS] = &build_interval_index::<Self>();
}
macro_rules! table {
    ($gamut:ty, $module:ident, $file:literal) => {
        mod $module {
            use super::Float;
            include!($file);
        }
        impl EdgeSeekerData for $gamut {
            const LUT: &'static [[Float; 4]] = &$module::LUT;
            const SHARP_INTERVALS: &'static [(usize, [Float; 4], [Float; 2])] =
                &$module::SHARP_INTERVALS;
        }
    };
}
table!(Srgb, srgb, "generated/edge_seeker_srgb.rs");
table!(DisplayP3, p3, "generated/edge_seeker_display_p3.rs");
table!(Rec2020, rec2020, "generated/edge_seeker_rec2020.rs");

// ── Method 9: edge-seeker ───────────────────────────────────────────────────
// Reduce chroma to a precomputed LUT of the gamut edge. The lookup evaluates the
// LUT at the exact normalized hue.

#[inline(always)]
fn lerp(a: Float, b: Float, t: Float) -> Float {
    if t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    a * (1.0 - t) + b * t
}

// LUT row = [l, c, h, curvature].
const HUE_INDEX_SCALE: usize = 10;
const HUE_INDEX_BUCKETS: usize = 360 * HUE_INDEX_SCALE;

fn find_closest<G: EdgeSeekerData>(hue: Float) -> (usize, usize) {
    let mut start: i64 = 0;
    let mut end: i64 = G::LUT.len() as i64 - 1;
    let mut mid = (start + end) / 2;
    while start <= end {
        let mh = G::LUT[mid as usize][2];
        if mh == hue {
            return (mid as usize, mid as usize);
        } else if mh < hue {
            start = mid + 1;
        } else {
            end = mid - 1;
        }
        mid = (start + end) / 2;
    }
    let last = G::LUT.len() as i64 - 1;
    (
        mid.clamp(0, last) as usize,
        (mid + 1).clamp(0, last) as usize,
    )
}

fn lerp_lut(start: &[Float; 4], end: &[Float; 4], hue: Float) -> [Float; 4] {
    if hue == start[2] {
        return *start;
    }
    if hue == end[2] {
        return *end;
    }
    let t = (hue - start[2]) / (end[2] - start[2]);
    [
        lerp(start[0], end[0], t),
        lerp(start[1], end[1], t),
        hue,
        lerp(start[3], end[3], t),
    ]
}

fn get_lut_item<G: EdgeSeekerData>(h: Float) -> [Float; 4] {
    let (lo, hi) = find_closest::<G>(h);
    lerp_lut(&G::LUT[lo], &G::LUT[hi], h)
}

const fn build_interval_index<G: EdgeSeekerData>() -> [usize; HUE_INDEX_BUCKETS] {
    let mut intervals = [0; HUE_INDEX_BUCKETS];
    let mut interval = 0;
    let mut bucket = 0;
    while bucket < HUE_INDEX_BUCKETS {
        let hue = bucket as Float / HUE_INDEX_SCALE as Float;
        while interval + 1 < G::LUT.len() - 1 && G::LUT[interval + 1][2] <= hue {
            interval += 1;
        }
        intervals[bucket] = interval;
        bucket += 1;
    }
    intervals
}

fn get_lut_item_indexed<G: EdgeSeekerData>(h: Float, interval_index: &[usize]) -> [Float; 4] {
    let bucket = ((h * HUE_INDEX_SCALE as Float) as usize).min(HUE_INDEX_BUCKETS - 1);
    let mut interval = interval_index[bucket];
    while interval > 0 && h < G::LUT[interval][2] {
        interval -= 1;
    }
    while interval + 1 < G::LUT.len() - 1 && h > G::LUT[interval + 1][2] {
        interval += 1;
    }
    lerp_lut(&G::LUT[interval], &G::LUT[interval + 1], h)
}

#[inline(always)]
fn normalized_hue(h: Float) -> Float {
    if h >= 0.0 && h < 360.0 {
        h
    } else if h < 0.0 {
        (h % 360.0) + 360.0
    } else {
        h % 360.0
    }
}

// A repaired blue fold can change C by 0.025 over only 0.0001 degrees.
// Binary32 hue knots alone do not resolve its interpolation accurately.
// Evaluate this interval and its neighbours with a split hue; all arithmetic
// remains native Float. P3 has no such intervals, so this compiles away.
#[inline(always)]
fn conditioned_item<G: EdgeSeekerData>(raw_hue: Float) -> Option<[Float; 4]> {
    if !SINGLE {
        return None;
    }
    let h = normalized_hue(raw_hue);
    for &(i, residuals, [lo, hi]) in G::SHARP_INTERVALS {
        // Keep the band safely inside the outer knots; ordinary lookup is
        // well-conditioned there. Only the middle interval has a steep slope.
        if h < lo || h > hi {
            continue;
        }
        // Error-free addition of a negative remainder to 360. A positive
        // remainder needs no addition and has no lost low part.
        let low = if raw_hue < 0.0 {
            (raw_hue % 360.0) - (h - 360.0)
        } else {
            0.0
        };
        let relative = |j: usize| (h - G::LUT[j][2]) + (low - residuals[j + 1 - i]);
        let j = if relative(i) < 0.0 {
            i - 1
        } else if relative(i + 1) > 0.0 {
            i + 1
        } else {
            i
        };
        let (a, b) = (G::LUT[j], G::LUT[j + 1]);
        let width = (b[2] - a[2]) + (residuals[j + 2 - i] - residuals[j + 1 - i]);
        let t = relative(j) / width;
        return Some([
            lerp(a[0], b[0], t),
            lerp(a[1], b[1], t),
            h,
            lerp(a[3], b[3], t),
        ]);
    }
    None
}

#[inline(always)]
pub(super) fn intersection_with_arc(x: Float, curvature: Float) -> Float {
    if curvature == 0.0 {
        return x;
    }
    // Solve k*y² + (t-k)*y - x*(t+k*(1-x)) = 0, t = sqrt(2-k²).
    // The rationalized root avoids cancellation near k=0 and never switches
    // to the other circle intersection because of endpoint rounding.
    let t = (2.0 - curvature * curvature).sqrt();
    let b = t - curvature;
    let d = x * (t + curvature * (1.0 - x));
    let disc = (b * b + 4.0 * curvature * d).max(0.0);
    (2.0 * d / (b + disc.sqrt())).clamp(0.0, 1.0)
}

#[inline(always)]
fn max_chroma_from_item(l: Float, item: [Float; 4]) -> Float {
    let (il, ic, icv) = (item[0], item[1], item[3]);
    if l <= il {
        return (l / il) * ic;
    }
    let x = (1.0 - l) / (1.0 - il);
    ic * intersection_with_arc(x, icv)
}

#[inline(always)]
fn map_edge_seeker<G: RgbGamut>(oklch: &[Float; 3], max_chroma: Float, out: &mut [Float; 3]) {
    let (l, c, h) = (oklch[0], oklch[1], oklch[2]);
    if l <= 0.0 {
        *out = [0.0, 0.0, 0.0];
        return;
    }
    if l >= 1.0 {
        *out = [1.0, 1.0, 1.0];
        return;
    }
    oklch_to_clipped_rgb::<G>(l, if c > max_chroma { max_chroma } else { c }, h, out);
}

pub(crate) struct EdgeSeeker<G>(PhantomData<G>);

impl<G: EdgeSeekerData> EdgeSeeker<G> {
    pub(crate) fn new() -> Self {
        EdgeSeeker(PhantomData)
    }

    #[inline(always)]
    fn max_chroma(&mut self, l: Float, h: Float) -> Float {
        if l <= 0.0 || l >= 1.0 {
            return 0.0;
        }
        let item = conditioned_item::<G>(h).unwrap_or_else(|| get_lut_item::<G>(normalized_hue(h)));
        max_chroma_from_item(l, item)
    }

    #[inline(always)]
    pub(crate) fn map(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    pub(crate) fn map_with_in_gamut_check(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[Float; 3], out: &mut [Float; 3], check_in_gamut: bool) {
        if check_in_gamut && oklch_to_rgb_if_in_gamut::<G>(oklch[0], oklch[1], oklch[2], out) {
            return;
        }
        let max_chroma = self.max_chroma(oklch[0], oklch[2]);
        map_edge_seeker::<G>(oklch, max_chroma, out);
    }
}

pub(crate) struct EdgeSeekerIndexed<G>(PhantomData<G>);

impl<G: EdgeSeekerData> EdgeSeekerIndexed<G> {
    pub(crate) fn new() -> Self {
        EdgeSeekerIndexed(PhantomData)
    }

    #[inline(always)]
    fn max_chroma(&mut self, l: Float, h: Float) -> Float {
        if l <= 0.0 || l >= 1.0 {
            return 0.0;
        }
        let item = conditioned_item::<G>(h)
            .unwrap_or_else(|| get_lut_item_indexed::<G>(normalized_hue(h), G::INTERVAL_INDEX));
        max_chroma_from_item(l, item)
    }

    #[inline(always)]
    pub(crate) fn map(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, false);
    }

    #[inline(always)]
    pub(crate) fn map_with_in_gamut_check(&mut self, oklch: &[Float; 3], out: &mut [Float; 3]) {
        self.map_impl(oklch, out, true);
    }

    #[inline(always)]
    fn map_impl(&mut self, oklch: &[Float; 3], out: &mut [Float; 3], check_in_gamut: bool) {
        // Keep the lookup in the mapper's optimization scope. Passing it as a
        // callback let LLVM outline the whole boundary solve on Apple ARM64.
        if check_in_gamut && oklch_to_rgb_if_in_gamut::<G>(oklch[0], oklch[1], oklch[2], out) {
            return;
        }
        let max_chroma = self.max_chroma(oklch[0], oklch[2]);
        map_edge_seeker::<G>(oklch, max_chroma, out);
    }
}

#[cfg(test)]
mod edge_seeker_tests {
    include!("edge_seeker_tests.rs");
}
