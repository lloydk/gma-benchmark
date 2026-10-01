//! Production f64 outputs and native membership for cross-language checks.
#![allow(dead_code)]
include!("support/mapping-probes.rs");
#[path = "../src/bottosson.rs"]
mod bottosson;
mapping_probe_main!(bottosson, BottossonData, direct: BottossonLightness => "bottosson-lightness", cached: BottossonLightnessCached => "bottosson-lightness-cached");
