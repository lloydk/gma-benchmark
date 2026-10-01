import { createDualray } from "../src/dualray-factory.js";
import { dualraySamples } from "../tests/helpers/dualray-samples.js";
import { runMappingParity } from "./mapping-parity.mjs";
console.log(JSON.stringify(runMappingParity({
 example:"dualray-mapping-probes",createMappers:space=>({dualray:createDualray(space)}),
 samplesFor:dualraySamples,intrinsic:true,exact:!!process.versions.bun,
 limitsFor:()=>({linear:1e-13,delta:1e-13}),
}),null,2));
