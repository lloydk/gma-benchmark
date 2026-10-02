import { createDualrayFast } from "../src/dualray-fast.js";
import { dualraySamples } from "../tests/helpers/dualray-samples.js";
import { runMappingParity } from "./mapping-parity.mjs";
console.log(JSON.stringify(runMappingParity({
 example:"dualray-fast-mapping-probes",
 createMappers:space=>({"dualray fast":createDualrayFast(space)}),
 samplesFor:dualraySamples,intrinsic:true,precheck:true,exact:!!process.versions.bun,
 limitsFor:()=>({linear:1e-13,delta:1e-13}),
}),null,2));
