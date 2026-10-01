//! Line-oriented production f64 outputs for cross-language development checks.
#![allow(dead_code)]
use std::io::{self, BufRead};
type Float = f64;
#[path = "../src/clip.rs"]
mod clip;
#[path = "../src/color.rs"]
mod color;
#[path = "../src/css_minde.rs"]
mod css_minde;
#[path = "../src/gamut.rs"]
mod gamut;
#[path = "../src/rgb_spaces.rs"]
mod rgb_spaces;
#[path = "../src/transfer.rs"]
mod transfer;
fn hue_radians(h: Float) -> Float {
    h * std::f64::consts::PI / 180.0
}
fn map<G: gamut::RgbGamut>(input: [f64; 3]) {
    let mut clipped = [0.0; 3];
    let mut mapped = [0.0; 3];
    clip::Clip::<G>::new().map(&input, &mut clipped);
    css_minde::CssMinde::<G>::new().map(&input, &mut mapped);
    println!("[{clipped:?},{mapped:?}]");
}
fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let mut fields = line.split(',');
        let id = fields.next().unwrap();
        let input = std::array::from_fn(|_| fields.next().unwrap().parse().unwrap());
        assert!(fields.next().is_none(), "expected target,L,C,H");
        match id {
            "srgb" => map::<rgb_spaces::Srgb>(input),
            "display-p3" => map::<rgb_spaces::DisplayP3>(input),
            "rec2020" => map::<rgb_spaces::Rec2020>(input),
            _ => panic!("unsupported target: {id}"),
        }
    }
}
