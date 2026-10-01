import { boundarySamples } from "./boundary-samples.js";
import { matrixSamples } from "./matrix-samples.js";
import { createBottossonReference } from "./bottosson-reference.js";
export function bottossonProbes(id) {
 const hues = createBottossonReference(id).hues, samples=[];
 for(const h of hues) for(const delta of [-.1,-.01,-1e-6,-1e-10,-1e-12,0,1e-12,1e-10,1e-6,.01,.1])
  for(const offset of [-720,-360,0,360,720]) for(const l of [.01,.3,.49,.7,.99]) samples.push([l,.4,h+delta+offset]);
 for(const l of [-1,0,1e-12,.001,.5,1-2**-52,1,2]) for(const c of [-.01,0,1e-14,1e-12,1e-11,.00003,.02,.4]) for(const h of [-720,-95.95087,0,30.0123,245.067,264.04913,359.95,720]) samples.push([l,c,h]);
 return samples.concat(boundarySamples(id));
}
export const bottossonSamples = id => [...matrixSamples(),...bottossonProbes(id)];
