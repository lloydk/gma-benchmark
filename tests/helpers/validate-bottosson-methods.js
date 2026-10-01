import assert from "node:assert/strict";
import { createBottossonMappers } from "../../src/bottosson-factory.js";
import { createCanonicalReference, bindMapperMode } from "./canonical-reference.js";
import { createBottossonReference } from "./bottosson-reference.js";
export function validateBottossonMethods(space,datasets,checked=false,registered=bindMapperMode(createBottossonMappers(space),checked),opposite) {
 const names=["bottosson-lightness","bottosson-lightness-cached"];
 assert.deepEqual(Object.keys(registered).sort(),names);
 const ref=createBottossonReference(space.id),conversion=createCanonicalReference(space);
 const maxima=names.map(method=>({method,linear:0,delta:0,encoded:0}));
 let count=0,canonical=0,contacts=0;
 for(const samples of datasets) for(const input of samples) {
  const [l,c,h]=input;
  const {inside,encoded:plain}=conversion(input);
  for(const [i,name] of names.entries()) {
   const actual=registered[name](input,[]);
   assert.ok(actual.every(v=>Number.isFinite(v)&&v>=0&&v<=1),`${space.id} ${name} ${input}: ${actual}`);
   if(checked&&inside) {assert.deepEqual(actual,plain,`canonical ${name} ${input}`);canonical++;continue;}
   let wrapped=h%360; if(wrapped<0)wrapped+=360;
   const hue=i===1?Math.round(wrapped*10)/10:h;
   let best;
   const channels=ref.adjacentFaces(hue);
   if(channels.length>1)contacts++;
   for(const channel of channels) {
    // Bottosson's fitted boundary can overshoot. Clipping is part of this
    // approximation policy; its geometric error is tested separately.
    const expected=c<=1e-12||l<=0||l>=1?Array(3).fill(Math.max(0,Math.min(1,l))**3):ref.linearRgb([l,ref.policy(l,hue,channel),hue]).map(v=>Math.max(0,Math.min(1,v)));
    const linear=Math.max(...actual.map((v,j)=>Math.abs(ref.decode(v)-expected[j])));
    const lab=ref.lab(actual),wanted=ref.linearToLab(expected);
    const delta=Math.hypot(...lab.map((v,j)=>v-wanted[j]));
    const encoded=Math.max(...actual.map((v,j)=>Math.abs(v-ref.encode(expected[j]))));
    if(!best||linear<best.linear)best={linear,delta,encoded};
   }
   assert.ok(Number.isFinite(best.linear)&&Number.isFinite(best.delta)&&best.linear<=2e-11&&best.delta<=2e-11,`${space.id} ${name} checked=${checked} ${input}: ${JSON.stringify(best)}`);
   const max=maxima[i];
   if(best.linear>max.linear){max.linear=best.linear;max.input=input;}
   max.delta=Math.max(max.delta,best.delta);max.encoded=Math.max(max.encoded,best.encoded);
   if(opposite&&!inside)assert.deepEqual(actual,opposite[name](input,[]));
  }
  count++;
 }
 return {gamut:space.id,checked,count,canonical,contacts,independentReference:maxima};
}
