import { matrixSamples } from "./matrix-samples.js";
import { boundarySamples } from "./boundary-samples.js";
import { RGB_SPACES } from "../../src/rgb-spaces.js";
import { blueFoldWindow } from "../../src/matrix-solver-policy.js";
import { neighbour } from "./probe-utils.js";
export function dualrayProbes(id) {
 const samples=[],window=blueFoldWindow(RGB_SPACES[id]);
 if(window) {
  for(let i=0;i<=1200;i++)for(const l of [.01,.1,.3,.414,.49,.7,.9,.99])for(const c of [.001,.1,.2,.4])
   samples.push([l,c,window[0]-.01+(window[1]-window[0]+.02)*i/1200]);
  for(const h of window)for(const offset of [-720,-360,0,360,720])for(const angle of [neighbour(h+offset,-1),h+offset,neighbour(h+offset,1)])for(const l of [.01,.414,.98])samples.push([l,.4,angle]);
 }
 for(const l of [Number.MIN_VALUE,1e-300,1e-110,1e-12,.001,.5,.9999,neighbour(1,-1)])for(const c of [0,1e-12,.02,.4,.6])for(const h of [-1e21,-1e9,-720,-0,0,18.5,104,245.1,264.05,301.75,720,1e9,1e21])samples.push([l,c,h]);
 return samples;
}
export const dualraySamples=id=>[...matrixSamples(),...boundarySamples(id),...dualrayProbes(id)];
