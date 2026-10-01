import { createEdgeSeekerMappers } from "../src/edge-seeker/factory.js";
import { edgeSeekerSamples } from "../tests/helpers/edge-seeker-samples.js";
import { inSteepInterval } from "../tests/helpers/edge-seeker-intervals.js";
import { runMappingParity } from "./mapping-parity.mjs";
console.log(JSON.stringify(runMappingParity({
 example:"edge-seeker-mapping-probes",createMappers:createEdgeSeekerMappers,
 samplesFor:edgeSeekerSamples,
 limitsFor:(id,input)=>inSteepInterval(id,input[2])?{linear:2e-10,delta:5e-11}:{linear:3e-12,delta:3e-12},
}),null,2));
