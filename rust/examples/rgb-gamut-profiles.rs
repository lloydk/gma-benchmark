//! Build-time bridge for generators of gamut-specific algorithm data.
#![allow(dead_code)]

type Float = f64;
#[path = "../src/color.rs"]
mod color;
#[path = "../src/dualray_config.rs"]
mod dualray_config;
#[path = "../src/gamut.rs"]
mod gamut;
#[path = "../src/rgb_spaces.rs"]
mod rgb_spaces;
#[path = "../src/transfer.rs"]
mod transfer;

fn hue_radians(h: Float) -> Float {
    h * std::f64::consts::PI / 180.0
}

fn profile<G: gamut::RgbGamut>() {
    use transfer::TransferFunction;
    let encoding = match G::ID {
        rgb_spaces::SpaceId::Srgb | rgb_spaces::SpaceId::DisplayP3 => "srgb",
        rgb_spaces::SpaceId::Rec2020 => "gamma-2.4",
    };
    let oklab_to_lms = [
        [1.0, color::KA0, color::KB0],
        [1.0, color::KA1, color::KB1],
        [1.0, color::KA2, color::KB2],
    ];
    let samples = [0.0, 0.001, 0.0031308, 0.01, 0.1, 0.5, 1.0];
    let config = dualray_config::config(G::ID);
    let blue_fold = rgb_spaces::blue_fold_window(G::ID).map_or("null".to_owned(), |[lo, hi]| {
        format!("[{},{}]", f64::from(lo), f64::from(hi))
    });
    let fold = config.fold.map_or("null".to_owned(), |v| format!("{v:?}"));
    let lab_samples = samples.map(|x| {
        let lab = color::LinearRgb::<G>::new([x, 0.25, 0.75]).to_oklab();
        [lab.l, lab.a, lab.b]
    });
    print!(
        "{{\"name\":{:?},\"encoding\":{encoding:?},\"rgbToXyz\":{rgb_to_xyz:?},\"xyzToRgb\":{xyz_to_rgb:?},\"rgbToLms\":{:?},\"lmsToRgb\":{:?},\"oklabToLms\":{oklab_to_lms:?},\"linearSamples\":{samples:?},\"encodedSamples\":{:?},\"labSamples\":{lab_samples:?},\"blueFoldWindow\":{blue_fold},\"dualray\":{{\"rootLimit\":{},\"foldWindow\":{fold}}}}}",
        G::DEFINITION.name,
        G::RGB_TO_LMS,
        G::LMS_TO_RGB,
        samples.map(G::Transfer::encode_clamped),
        config.root_limit,
        rgb_to_xyz = G::DEFINITION.rgb_to_xyz,
        xyz_to_rgb = G::DEFINITION.xyz_to_rgb,
    );
}

fn main() {
    print!("[");
    profile::<rgb_spaces::Srgb>();
    print!(",");
    profile::<rgb_spaces::DisplayP3>();
    print!(",");
    profile::<rgb_spaces::Rec2020>();
    println!("]");
}
