use super::color::Oklch;
use super::gamut::RgbGamut;
use super::Float;
use std::marker::PhantomData;

pub(crate) struct Clip<G>(PhantomData<G>);
impl<G: RgbGamut> Clip<G> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }

    #[inline(always)]
    pub(crate) fn map(&mut self, input: &[Float; 3], out: &mut [Float; 3]) {
        *out = Oklch::from(*input)
            .to_oklab()
            .to_linear_rgb::<G>()
            .encode_clamped()
            .channels;
    }

    #[inline(always)]
    pub(crate) fn map_with_in_gamut_check(&mut self, input: &[Float; 3], out: &mut [Float; 3]) {
        self.map(input, out);
    }
}
