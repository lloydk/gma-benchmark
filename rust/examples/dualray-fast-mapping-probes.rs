//! Production f64 Dualray Fast; its canonical precheck is part of both modes.
#![allow(dead_code)]
include!("support/mapping-probes.rs");
#[path = "../src/dualray.rs"]
mod dualray;
#[path = "../src/dualray_config.rs"]
mod dualray_config;
#[path = "../src/dualray_fast.rs"]
mod dualray_fast;
mapping_probe_main!(dualray_fast, DualrayFastData, fast: DualrayFast => "dualray fast");
