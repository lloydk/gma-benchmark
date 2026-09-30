use crate::rgb_spaces::SpaceId;

// Algorithm configuration, shared by the native kernels and seed generator.
// Fits exclude the blue fold: first-exit isolation handles that interval.
pub(crate) struct Config {
    // Exported to the generator and tests; runtime membership uses in_blue_fold.
    #[allow(dead_code)]
    pub fold: Option<[f64; 2]>,
    pub root_limit: f64,
}

pub(crate) const fn config(id: SpaceId) -> Config {
    let fold = match crate::rgb_spaces::blue_fold_window(id) {
        Some([lo, hi]) => {
            // The compensated direction series is centred at 270 degrees.
            assert!(lo >= 244.0 && hi <= 296.0 && lo < hi);
            Some([lo as f64, hi as f64])
        }
        None => None,
    };
    Config {
        fold,
        root_limit: 4.0,
    }
}

// Evaluate the angle-domain checks even when no generator/example is built.
const _: () = {
    config(SpaceId::Srgb);
    config(SpaceId::DisplayP3);
    config(SpaceId::Rec2020);
};
