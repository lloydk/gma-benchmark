// One ordered registry: every method supports all three targets.
// Keep historical P3 method order for controlled benchmark comparisons.
macro_rules! for_each_rgb_method {
    ($core:ident) => {
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
        $core!("dualray", dualray, Dualray, Dualray);
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
}

// P3 test/snapshot compatibility adapter.
#[cfg(test)]
macro_rules! for_each_method {
    ($visit:ident) => {
        macro_rules! core {
            ($name:literal, $module:ident, $method:ident, $policy:ident) => {
                $visit!($name, $method);
            };
        }
        for_each_rgb_method!(core);
    };
}
