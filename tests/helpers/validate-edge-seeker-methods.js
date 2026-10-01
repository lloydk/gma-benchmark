import assert from "node:assert/strict";
import { createEdgeSeekerMappers } from "../../src/edge-seeker/factory.js";
import { createCanonicalReference, bindMapperMode } from "./canonical-reference.js";
import { createEdgeSeekerReference } from "./edge-seeker-reference.js";
export function validateEdgeSeekerMethods(space,datasets,checked=false,registered=bindMapperMode(createEdgeSeekerMappers(space),checked),opposite) {
 const names=["edge-seeker","edge-seeker-indexed"];
 assert.deepEqual(Object.keys(registered).sort(),names);
 const ref=createEdgeSeekerReference(space.id),conversion=createCanonicalReference(space);
 const maxima=names.map(method=>({method,linear:0,delta:0,encoded:0}));
 let count=0,canonical=0;
 for(const samples of datasets)for(const input of samples) {
  const [l,c,h]=input;
  const {inside,encoded:converted}=conversion(input);
  const expected=checked&&inside?converted.map(ref.decode):l<=0?[0,0,0]:l>=1?[1,1,1]:ref.linearRgb([l,Math.min(c,ref.chroma(l,h)),h]).map(v=>Math.max(0,Math.min(1,v)));
  const wanted=ref.linearToLab(expected),outputs=[];
  for(const [i,name] of names.entries()) {
   const actual=registered[name](input,[]);outputs.push(actual);
   assert.ok(actual.every(v=>Number.isFinite(v)&&v>=0&&v<=1),`${space.id} ${name} ${input}: ${actual}`);
   if(checked&&inside){assert.deepEqual(actual,converted,`canonical ${name} ${input}`);canonical++;}
   const linear=Math.max(...actual.map((v,j)=>Math.abs(ref.decode(v)-expected[j])));
   const lab=ref.lab(actual),delta=Math.hypot(...lab.map((v,j)=>v-wanted[j]));
   assert.ok(Number.isFinite(linear)&&Number.isFinite(delta)&&linear<=3e-12&&delta<=3e-12,`${space.id} ${name} checked=${checked} ${input}: linear ${linear}, delta ${delta}`);
   const max=maxima[i];if(linear>max.linear){max.linear=linear;max.input=input;}
   max.delta=Math.max(max.delta,delta);max.encoded=Math.max(max.encoded,...actual.map((v,j)=>Math.abs(v-ref.encode(expected[j]))));
   if(opposite&&!inside)assert.deepEqual(actual,opposite[name](input,[]));
  }
  assert.deepEqual(...outputs);count++;
 }
 return {gamut:space.id,checked,count,canonical,independentReference:maxima};
}
