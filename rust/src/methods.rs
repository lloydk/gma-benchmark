// Core methods and P3-only extras are each registered once. The explicit
// policies are validation concerns, not properties of a physical RGB space.
macro_rules! for_each_rgb_method {
    ($visit:ident) => {
        $visit!("clip", clip, Clip, Clip);
        $visit!("css-minde", css_minde, CssMinde, Minde);
    };
}
macro_rules! for_each_p3_extra {
    ($visit:ident) => {
        $visit!("oklch-cubic (cached)", OklchCubic, 0.002);
        $visit!("oklch-cubic (no cache)", OklchCubicNoCache, 0.002);
        $visit!("oklch-cubic-direct", OklchCubicDirect, 0.0001);
        $visit!("oklch-halley", OklchHalley, 0.0001);
        $visit!("oklch-ostrowski", OklchOstrowski, 0.0001);
        $visit!("dualray", Dualray, 0.0001);
        $visit!("bottosson-lightness", BottossonLightness, 0.001);
        $visit!(
            "bottosson-lightness (cached)",
            BottossonLightnessCached,
            0.002
        );
        $visit!("edge-seeker", EdgeSeeker, 0.0001);
        $visit!("edge-seeker (indexed)", EdgeSeekerIndexed, 0.0001);
        $visit!("raytrace", Raytrace, 0.0002);
    };
}
// Compatibility adapter for the P3-only tests and output snapshots.
#[cfg(test)]
macro_rules! for_each_method {
    ($visit:ident) => {
        macro_rules! core {
            ($name:literal, $module:ident, $method:ident, $policy:ident) => {
                $visit!($name, $method);
            };
        }
        macro_rules! extra {
            ($name:literal, $method:ident, $limit:literal) => {
                $visit!($name, $method);
            };
        }
        for_each_rgb_method!(core);
        for_each_p3_extra!(extra);
    };
}
