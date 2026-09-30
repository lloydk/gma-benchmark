// One ordered registry. Core entries are generic; extras remain P3-only.
// Keep historical P3 method order for controlled benchmark comparisons.
macro_rules! for_each_method {
    ($core:ident, $extra:ident) => {
        $core!("clip", clip, Clip, Clip);
        $core!("css-minde", css_minde, CssMinde, Minde);
        $core!("oklch-cubic (cached)", rgb_solvers, OklchCubic, Bucket);
        $core!(
            "oklch-cubic (no cache)",
            rgb_solvers,
            OklchCubicNoCache,
            Bucket
        );
        $core!(
            "oklch-cubic-direct",
            rgb_solvers,
            OklchCubicDirect,
            Boundary
        );
        $core!("oklch-halley", rgb_solvers, OklchHalley, Iterative);
        $core!("oklch-ostrowski", rgb_solvers, OklchOstrowski, Iterative);
        $extra!("dualray", Dualray, 0.0001);
        $core!(
            "bottosson-lightness",
            bottosson,
            BottossonLightness,
            Bottosson
        );
        $core!(
            "bottosson-lightness (cached)",
            bottosson,
            BottossonLightnessCached,
            BottossonBucket
        );
        $core!("edge-seeker", edge_seeker, EdgeSeeker, EdgeSeeker);
        $core!(
            "edge-seeker (indexed)",
            edge_seeker,
            EdgeSeekerIndexed,
            EdgeSeeker
        );
        $core!("raytrace", rgb_solvers, Raytrace, Raytrace);
    };
    // P3 test/snapshot compatibility adapter.
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
        for_each_method!(core, extra);
    };
}
