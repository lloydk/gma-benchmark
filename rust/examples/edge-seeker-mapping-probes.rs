//! Production f64 outputs and native membership for cross-language checks.
#![allow(dead_code)]
include!("support/mapping-probes.rs");
#[path = "../src/edge_seeker.rs"]
mod edge_seeker;
mapping_probe_main!(edge_seeker, EdgeSeekerData, direct: EdgeSeeker => "edge-seeker", cached: EdgeSeekerIndexed => "edge-seeker-indexed");
