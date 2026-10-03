// Independent distributions fixed before timing the Dualray experiments.
// Chroma varies independently of lightness/hue; boundary locations come from
// the XYZ/stationary-interval oracle, never production fits or timed results.
import { createBoundaryReference } from '../tests/helpers/matrix-reference.js';

function random(seed) {
 let state=seed>>>0;
 return ()=>{
  state=(state+0x6d2b79f5)>>>0;
  let x=state;
  x=Math.imul(x^(x>>>15),x|1);
  x^=x+Math.imul(x^(x>>>7),x|61);
  return ((x^(x>>>14))>>>0)/4294967296;
 };
}

export function dualrayExperimentWorkloads(gamut, validation=false, count=8192,
 seed=validation?0x6d2b79f5:0x243f6a88) {
 const next=random(seed);
 const ref=createBoundaryReference(gamut);
 const workloads={volume:[],bright:[],boundary:[],interior:[]};
 for(let i=0;i<count;i++) {
  const l=1e-6+(1-2e-6)*next(),h=360*next(),c=.5*next();
  workloads.volume.push([l,c,h]);
  workloads.bright.push([1-2**(-1-15*next()),.5*next(),360*next()]);
  const first=ref.boundaries(l,h).first;
  workloads.boundary.push([l,first*(.5+1.5*next()),h]);
  workloads.interior.push([l,first*(.05+.85*next()),h]);
 }
 return workloads;
}
