// Native branch decisions are reported separately at rounding-scale boundaries.
import { createBottossonMappers } from "../src/bottosson-factory.js";
import { bottossonSamples } from "../tests/helpers/bottosson-samples.js";
import { runMappingParity } from "./mapping-parity.mjs";
console.log(JSON.stringify(runMappingParity({
 example:"bottosson-mapping-probes",createMappers:createBottossonMappers,
 samplesFor:bottossonSamples,limitsFor:()=>({linear:2e-11,delta:2e-11}),
}),null,2));
