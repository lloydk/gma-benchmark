// One registry for both precision lanes, timing, and validation.
macro_rules! for_each_method {
    ($visit:ident) => {
        $visit!("clip", Clip);
        $visit!("oklch-cubic (cached)", OklchCubic);
        $visit!("oklch-cubic (no cache)", OklchCubicNoCache);
        $visit!("oklch-cubic-direct", OklchCubicDirect);
        $visit!("oklch-halley", OklchHalley);
        $visit!("oklch-ostrowski", OklchOstrowski);
        $visit!("bottosson-lightness", BottossonLightness);
        $visit!("bottosson-lightness (cached)", BottossonLightnessCached);
        $visit!("edge-seeker", EdgeSeeker);
        $visit!("edge-seeker (indexed)", EdgeSeekerIndexed);
        $visit!("raytrace", Raytrace);
    };
}
