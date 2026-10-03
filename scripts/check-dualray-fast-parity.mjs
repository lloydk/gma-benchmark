import { createDualrayFast, createDualrayFastTables } from "../src/dualray-fast.js";
import { dualraySamples } from "../tests/helpers/dualray-samples.js";
import { runMappingParity } from "./mapping-parity.mjs";
console.log(JSON.stringify(runMappingParity({
 example:"dualray-fast-mapping-probes",
 createMappers:space=>({"dualray fast":createDualrayFast(space),"dualray fast (tables)":createDualrayFastTables(space)}),
 // Rust's tables row takes its upper direction from a 22.5° table, JS from
 // Math.cos/Math.sin: last-bit differences, so only plain Fast is exact in Bun.
 samplesFor:dualraySamples,intrinsic:true,precheck:true,exact:name=>!!process.versions.bun&&name==="dualray fast",
 limitsFor:()=>({linear:1e-13,delta:1e-13}),
}),null,2));
