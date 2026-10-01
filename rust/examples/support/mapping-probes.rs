use std::io::{self, BufRead};
type Float = f64;
#[path = "../../src/clip.rs"]
mod clip;
#[path = "../../src/color.rs"]
mod color;
#[path = "../../src/css_minde.rs"]
mod css_minde;
#[path = "../../src/gamut.rs"]
mod gamut;
#[path = "../../src/rgb_spaces.rs"]
mod rgb_spaces;
#[path = "../../src/transfer.rs"]
mod transfer;
const SINGLE: bool = false;
const PI: Float = std::f64::consts::PI;
use color::{KA0, KA1, KA2, KB0, KB1, KB2};
include!("../../src/conditioning.rs");
include!("../../src/polynomial.rs");
#[path = "../../src/compensated.rs"]
mod compensated;
#[path = "../../src/rgb_solvers.rs"]
mod rgb_solvers;

// Shared transport and native membership metadata for JS parity checks.
macro_rules! mapping_probe_main {
    ($module:ident, $bound:ident, $( $mapper:ident : $kind:ident => $name:literal ),+ $(,)?) => {
        fn run<G: $module::$bound>() {
            $(let mut $mapper = $module::$kind::<G>::new();)+
            for line in io::stdin().lock().lines() {
                let line = line.unwrap();
                let mut fields = line.split(',');
                let input = std::array::from_fn(|_| fields.next().unwrap().parse().unwrap());
                assert!(fields.next().is_none());
                let rgb = color::Oklch::from(input).to_oklab().to_linear_rgb::<G>();
                print!("{{\"membership\":{{\"inside\":{},\"linear\":{:?},\"encoded\":{:?}}},\"negativeZeroHue\":{},\"methods\":{{",
                    rgb.in_gamut(), rgb.channels, rgb.encode_clamped().channels,
                    input[2] == 0.0 && input[2].is_sign_negative());
                let methods = [$({
                    let mut plain = [0.0; 3];
                    let mut checked = [0.0; 3];
                    $mapper.map(&input, &mut plain);
                    $mapper.map_with_in_gamut_check(&input, &mut checked);
                    ($name, plain, checked)
                }),+];
                for (i, (name, plain, checked)) in methods.into_iter().enumerate() {
                    if i > 0 { print!(","); }
                    print!("\"{}\":{{\"plain\":{:?},\"checked\":{:?}}}", name, plain, checked);
                }
                println!("}}}}");
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
    };
}
