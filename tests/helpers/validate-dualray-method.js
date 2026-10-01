import assert from "node:assert/strict";
import { createDualray } from "../../src/dualray-factory.js";
import { createBoundaryReference } from "./matrix-reference.js";
import { mapperOutput } from "./mapper-output.js";
// Independent XYZ geometry and first-exit policy, never the production seeds,
// sector choice, normalized basis, or membership comparison.
export function validateDualrayMethod(space,datasets,map=createDualray(space)) {
 const ref=createBoundaryReference(space.id),maxima={linear:0,delta:0,encoded:0};let count=0;
 for(const samples of datasets)for(const input of samples) {
  const [l,c,h]=input,hue=Math.abs(h)<1e9?h:h%360;
  const actual=mapperOutput(map,input);
  assert.ok(actual.every(v=>Number.isFinite(v)&&v>=0&&v<=1),`${space.id} Dualray ${input}: ${actual}`);
  let expected;
  if(l<=0||l>=1)expected=Array(3).fill(l<=0?0:1);
  else if(c<=0)expected=ref.linearRgb([l,0,hue]);
  else {
   const edges=ref.boundaries(l,hue);
   expected=ref.linearRgb([l,Math.min(c,edges.first),hue]).map(v=>Math.max(0,Math.min(1,v)));
   // Exclude ambiguous primary ties and rounding-scale near-neutral exits.
   // Away from those cases the independent oracle identifies the exact face.
   const exits=edges.exits.toSorted((a,b)=>a.chroma-b.chroma);
   if(c>edges.first+1e-9 && edges.first>1e-9 && exits.length &&
      (!exits[1] || exits[1].chroma-exits[0].chroma>1e-9)) {
    const {channel,face}=exits[0];
    assert.equal(actual[channel],face,`${space.id} Dualray ${input}: exact exit face ${channel}=${face}`);
   }
  }
  const lab=ref.lab(actual),wanted=ref.linearToLab(expected);
  const error={linear:Math.max(...actual.map((v,i)=>Math.abs(ref.decode(v)-expected[i]))),
   delta:Math.hypot(...lab.map((v,i)=>v-wanted[i])),encoded:Math.max(...actual.map((v,i)=>Math.abs(v-ref.encode(expected[i]))))};
  assert.ok(Object.values(error).every(Number.isFinite)&&error.linear<=2e-11&&error.delta<=2e-11&&error.encoded<=(space.id==='rec2020'?5e-5:1e-8),`${space.id} Dualray ${input}: ${JSON.stringify(error)}; ${actual} vs ${expected.map(ref.encode)}`);
  for(const key of Object.keys(error))if(error[key]>maxima[key]){maxima[key]=error[key];maxima[`${key}Input`]=input;}
  count++;
 }
 return {gamut:space.id,count,maxima};
}
