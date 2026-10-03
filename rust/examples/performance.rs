//! One production method/target/precision per process, with externally supplied
//! little-endian f64 input triples. Used by scripts/run-performance.mjs.
#![allow(dead_code, unused_imports, unused_macros)]
#[macro_use]
#[path = "../src/methods.rs"]
mod methods;
#[path = "../src/dualray_config.rs"]
mod dualray_config;
#[path = "../src/rgb_spaces.rs"]
mod rgb_spaces;

mod float64 {
    type Float = f64;
    const SINGLE: bool = false;
    include!("../src/algorithms.rs");
    include!("support/performance-lane.rs");
}
mod float32 {
    type Float = f32;
    const SINGLE: bool = true;
    include!("../src/algorithms.rs");
    include!("support/performance-lane.rs");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    assert_eq!(
        args.len(),
        6,
        "performance GAMUT f64|f32 METHOD plain|checked INPUT timing|validate"
    );
    assert!(matches!(args[3].as_str(), "plain" | "checked"));
    assert!(matches!(args[5].as_str(), "timing" | "validate"));
    let bytes = std::fs::read(&args[4]).expect("input file");
    assert!(!bytes.is_empty() && bytes.len() % 24 == 0);
    let input: Vec<[f64; 3]> = bytes
        .chunks_exact(24)
        .map(|row| {
            std::array::from_fn(|i| f64::from_le_bytes(row[i * 8..i * 8 + 8].try_into().unwrap()))
        })
        .collect();
    assert!(input.iter().flatten().all(|x| x.is_finite()));
    macro_rules! run {
        ($lane:ident, $gamut:ident) => {
            $lane::run::<rgb_spaces::$gamut>(
                &args[2],
                args[3] == "checked",
                &input,
                args[5] == "validate",
            )
        };
    }
    match (args[0].as_str(), args[1].as_str()) {
        ("srgb", "f64") => run!(float64, Srgb),
        ("display-p3", "f64") => run!(float64, DisplayP3),
        ("rec2020", "f64") => run!(float64, Rec2020),
        ("srgb", "f32") => run!(float32, Srgb),
        ("display-p3", "f32") => run!(float32, DisplayP3),
        ("rec2020", "f32") => run!(float32, Rec2020),
        _ => panic!("unsupported gamut/precision"),
    }
}
