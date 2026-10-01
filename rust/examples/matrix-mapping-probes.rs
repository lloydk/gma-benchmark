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
const SINGLE: bool = false;
const PI: Float = std::f64::consts::PI;
use color::{KA0, KA1, KA2, KB0, KB1, KB2};
include!("../src/conditioning.rs");
include!("../src/polynomial.rs");
#[path = "../src/compensated.rs"]
mod compensated;
#[path = "../src/rgb_solvers.rs"]
mod rgb_solvers;

fn run<G: gamut::RgbGamut>() {
    use rgb_solvers::*;
    let mut cached = OklchCubic::<G>::new();
    let mut uncached = OklchCubicNoCache::<G>::new();
    let mut direct = OklchCubicDirect::<G>::new();
    let mut halley = OklchHalley::<G>::new();
    let mut ostrowski = OklchOstrowski::<G>::new();
    let mut raytrace = Raytrace::<G>::new();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let mut fields = line.split(',');
        let input = std::array::from_fn(|_| fields.next().unwrap().parse().unwrap());
        assert!(fields.next().is_none());
        macro_rules! emit {
            ($name:literal, $mapper:ident) => {{
                let mut plain = [0.0; 3];
                let mut checked = [0.0; 3];
                $mapper.map(&input, &mut plain);
                $mapper.map_with_in_gamut_check(&input, &mut checked);
                print!(
                    "\"{}\":{{\"plain\":{:?},\"checked\":{:?}}}",
                    $name, plain, checked
                );
            }};
        }
        print!("{{");
        emit!("oklch-cubic", cached);
        print!(",");
        emit!("oklch-cubic-no-cache", uncached);
        print!(",");
        emit!("oklch-cubic-direct", direct);
        print!(",");
        emit!("oklch-halley", halley);
        print!(",");
        emit!("oklch-ostrowski", ostrowski);
        print!(",");
        emit!("raytrace", raytrace);
        println!("}}");
    }
}
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("srgb") => run::<rgb_spaces::Srgb>(),
        Some("display-p3") => run::<rgb_spaces::DisplayP3>(),
        Some("rec2020") => run::<rgb_spaces::Rec2020>(),
        _ => panic!("expected target argument"),
    }
}
