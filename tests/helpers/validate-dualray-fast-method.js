import assert from "node:assert/strict";
import { createDualrayFast } from "../../src/dualray-fast.js";
import { createCanonicalReference } from "./canonical-reference.js";
import { createBoundaryReference } from "./matrix-reference.js";
import { mapperOutput } from "./mapper-output.js";
// Independent XYZ first exits and canonical membership, never the production
// fits, basis or solver. In-gamut colors must keep their canonical conversion
// bit for bit; the rest must lie within the design budget, a deltaEOK of 1e-3
// from the constant-L/h first exit at the reduced hue.
export const DUALRAY_FAST_DELTA_BUDGET = 1e-3;
export function validateDualrayFastMethod(space,datasets,map=createDualrayFast(space)) {
 const ref=createBoundaryReference(space.id),canonical=createCanonicalReference(space),deltas=[];
 let count=0,inside=0,max=0,maxInput=null;
 for(const samples of datasets)for(const input of samples) {
  const [l,c,h]=input,actual=mapperOutput(map,input);
  assert.ok(actual.every(v=>v>=0&&v<=1),`${space.id} Dualray Fast ${input}: ${actual}`);
  count++;
  if(!(l>0&&l<1)) {
   assert.deepEqual(actual,Array(3).fill(l>=1?1:0),`${space.id} Dualray Fast endpoint ${input}`);
   continue;
  }
  const chroma=Math.max(0,c),membership=canonical([l,chroma,h]);
  if(membership.inside) {
   assert.deepEqual(actual,membership.encoded,`${space.id} Dualray Fast canonical ${input}`);
   inside++;
   continue;
  }
  let hue=h%360;
  if(hue<0)hue+=360;
  const first=ref.boundaries(l,hue).first;
  const expected=ref.linearRgb([l,Math.min(chroma,first),hue]).map(v=>Math.max(0,Math.min(1,v)));
  const lab=ref.lab(actual),wanted=ref.linearToLab(expected);
  const delta=Math.hypot(...lab.map((v,i)=>v-wanted[i]));
  assert.ok(delta<=DUALRAY_FAST_DELTA_BUDGET,`${space.id} Dualray Fast ${input}: deltaEOK ${delta}; ${actual} vs ${expected.map(ref.encode)}`);
  deltas.push(delta);
  if(delta>max){max=delta;maxInput=input;}
 }
 deltas.sort((a,b)=>a-b);
 return {gamut:space.id,count,inside,deltaMax:max,deltaP99:deltas.length?deltas[Math.floor(.99*(deltas.length-1))]:0,maxInput};
}
