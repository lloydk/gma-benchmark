import assert from "node:assert/strict";
import { test } from "node:test";
import { RGB_SPACES, SRGB, REC2020, DISPLAY_P3 } from "../src/rgb-spaces.js";
import { createDualray, firstRoot, mapFold } from "../src/dualray-factory.js";
import { dualray } from "../src/dualray.js";
import { dualraySamples } from "./helpers/dualray-samples.js";
import { validateDualrayMethod } from "./helpers/validate-dualray-method.js";
import { createBoundaryReference } from "./helpers/matrix-reference.js";
import { mapperOutput } from "./helpers/mapper-output.js";
import { interiorWithin } from "../src/dualray-interval.js";
import { bisectFoldExit } from "../src/matrix-solver-policy.js";
for(const space of Object.values(RGB_SPACES)) {
 test(`${space.id} Dualray matches independent first-exit geometry`,()=>{
  const samples=dualraySamples(space.id),map=createDualray(space);
  console.log(JSON.stringify(validateDualrayMethod(space,[samples],map)));
 });
 test(`${space.id} Dualray endpoints, exact gray, huge hues and aliasing`,()=>{
  const map=createDualray(space);
  for(const l of [-.1,0,.001,.5,.99,1,1.1])for(const c of [-.1,0,.001,.4])for(const h of [-Number.MAX_VALUE,-1e21,-1e9,-0,30.123,1e9,Number.MAX_VALUE]) {
   const input=[l,c,h],out=map(input,[]),alias=[...input];
   assert.ok(out.every(v=>Number.isFinite(v)&&v>=0&&v<=1));
   assert.equal(map(alias,alias),alias);assert.deepEqual(alias,out);
   if(Math.abs(h)>=1e9)assert.deepEqual(out,map([l,c,h%360],[]));
   if(l>0&&l<1&&c<=0)assert.deepEqual(out,Array(3).fill(space.transfer.encodeClamped(l*l*l)));
  }
 });
}
test("Dualray folded re-entry maps to the first exit and snaps the correct face",()=>{
 for(const [space,l,h] of [[SRGB,.3,264.053],[REC2020,.2,245.067]]) {
  const ref=createBoundaryReference(space.id),edges=ref.boundaries(l,h),map=createDualray(space);
  assert.ok(edges.outer>edges.first+.001);
  const expected=ref.linearRgb([l,edges.first,h]).map(v=>Math.max(0,Math.min(1,v)));
  const face=expected.findIndex(v=>Math.min(v,1-v)<1e-12);assert.ok(face>=0);
  for(const c of [edges.first*.5,(edges.first+edges.outer)/2,edges.outer-1e-7,.4]) {
   const out=mapperOutput(map,[l,c,h]);
   if(c>=edges.first){assert.equal(out[face],expected[face]<.5?0:1);assert.ok(out.every((v,i)=>Math.abs(ref.decode(v)-expected[i])<2e-11));}
   else {
    const interior=ref.linearRgb([l,c,h]);
    assert.ok(out.every((v,i)=>v>0&&v<1&&Math.abs(ref.decode(v)-interior[i])<2e-13));
   }
  }
 }
});

test("mapper validation rejects returned replacement arrays and unwritten buffers",()=>{
 const map=createDualray(SRGB),input=[.3,.2,264.053];
 for(const broken of [(input,out)=>map(input,[]),(input,out)=>out]) {
  assert.throws(()=>mapperOutput(broken,input));
  assert.throws(()=>validateDualrayMethod(SRGB,[[input]],broken));
 }
});

test("fold kernel snaps a lower face whose isolated root rounds inside",()=>{
 // This cubic's first exit evaluates to +2^-53 after root isolation. A
 // tolerance check or final clipping alone would accept the unsnapped value.
 const row=[3.1554404040798545,-5.84003952331841,-7.066192119382322];
 const root=firstRoot(...row,1,4);
 assert.ok(((row[0]*root+row[1])*root+row[2])*root+1>0);
 const out=[NaN,NaN,NaN];
 assert.equal(mapFold([row,[0,0,0],[0,0,0]],.5,2,.5,4,v=>v,out),out);
 assert.deepEqual(out,[0,.5,.5]);
});

test("fold fast path retains upper-face snapping at rounding-scale contacts",()=>{
 const map=createDualray(SRGB);
 for(const input of [[.49,.2876573921394428,-815.94797838363],[.49,.28765739213944286,-815.9479783836301]]) {
  assert.equal(mapperOutput(map,input)[2],1);
 }
});

test("Bernstein certification rejects lower and upper excursions with inside endpoints",()=>{
 // p(x)=1-6x+6x² has p(0)=p(1)=1, but p(.5)=-.5.
 assert.equal(interiorWithin(-6,6,1,2),false);
 // p(x)=1+6x-6x² has the same endpoints but p(.5)=2.5 > 2.
 assert.equal(interiorWithin(6,-6,1,2),false);
 assert.equal(interiorWithin(-6,6,.01,2),true);
 assert.equal(interiorWithin(0,0,1,1),true);
});

test("outer fold bisection handles an exactly-on-upper-face interval origin",()=>{
 // p(x)=(1+x)^3-1 starts exactly on the upper face and points outward.
 assert.ok(bisectFoldExit([1,3,3,0],0,.4,false)<1e-14);
 // An exactly-on-lower-face starting residual with the opposite orientation.
 assert.ok(bisectFoldExit([-1,-3,-3,0],0,.4,true)<1e-14);
});
test("Dualray target factories stay isolated and reject borrowed fit ids",()=>{
 const maps=Object.values(RGB_SPACES).map(space=>[space,createDualray(space)]);
 for(const order of [maps,[...maps].reverse(),maps])for(const [space,map] of order)validateDualrayMethod(space,[[[.49,.4,245.067],[.9,.4,104],[.5,.01,30.123]]],map);
 assert.deepEqual(dualray([.49,.4,30],[]),createDualray(DISPLAY_P3)([.49,.4,30],[]));
 assert.throws(()=>createDualray({...REC2020,id:'srgb'}),RangeError);
 assert.throws(()=>validateDualrayMethod(SRGB,[[[.49,.4,245.067]]],createDualray(REC2020)));
 assert.throws(()=>validateDualrayMethod(SRGB,[[[.49,.4,245.067]]],()=>[NaN,0,0]));
});

test("independent boundary reference treats an outward upper-face origin as first exit",()=>{
 const ref=createBoundaryReference('rec2020');
 assert.ok(ref.boundaries(1-Number.EPSILON/2,80).first<1e-12);
});

test("Dualray fallback ignores stationary touches and accepts outward endpoint crossings",()=>{
 // (1-x)^2 (1-x/4): touching zero at x=1 does not leave the gamut;
 // the first outward crossing is x=4. Negation exercises upper-face orientation.
 for(const sign of [1,-1]) {
  assert.equal(firstRoot(sign*-.25,sign*1.5,sign*-2.25,sign,1),Infinity);
  assert.equal(firstRoot(sign*-.25,sign*1.5,sign*-2.25,sign,4),4);
  assert.ok(Math.abs(firstRoot(sign*-.25,sign*1.5,sign*-2.25,sign,5)-4)<1e-13);
 }
});
