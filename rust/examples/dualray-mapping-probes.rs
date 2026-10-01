//! Production f64 Dualray with intrinsic boundary membership in both modes.
#![allow(dead_code)]
include!("support/mapping-probes.rs");
#[path = "../src/dualray.rs"]
mod dualray;
#[path = "../src/dualray_config.rs"]
mod dualray_config;
mapping_probe_main!(dualray, DualrayData, mapper: Dualray => "dualray");
