import assert from "node:assert/strict";
import { test } from "node:test";
import { SRGB, REC2020 } from "../src/rgb-spaces.js";
import { createBottossonMappers, createBottossonLightness } from "../src/bottosson-lightness.js";
import { createEdgeSeekerMappers } from "../src/edge-seeker/index.js";
import { createMatrixMappers } from "../src/matrix-mappers.js";
import { makeEdgeSeekerFromTable, makeEdgeSeekerIndexedFromTable } from "../src/edge-seeker/makeEdgeSeeker.js";
import { validateBottossonMethods } from "./helpers/validate-bottosson-methods.js";
import { validateEdgeSeekerMethods } from "./helpers/validate-edge-seeker-methods.js";
import { validateMatrixMethods } from "./helpers/validate-matrix-methods.js";
import { createCanonicalReference, bindMapperMode } from "./helpers/canonical-reference.js";
import { createBoundaryReference } from "./helpers/matrix-reference.js";
import { neighbour, serializeProbes } from "./helpers/probe-utils.js";
import { inSteepInterval, steepIntervals } from "./helpers/edge-seeker-intervals.js";
import { checkCheckedBranch, assertBoundaryDisagreement } from "../scripts/mapping-parity.mjs";
const families=[
 [createBottossonMappers,validateBottossonMethods,"bottosson-lightness"],
 [createEdgeSeekerMappers,validateEdgeSeekerMethods,"edge-seeker"],
 [createMatrixMappers,validateMatrixMethods,"oklch-cubic"],
];
for(const [create,validate,name] of families) {
 test(`${name} validation calls the timed two-argument entry point`,()=>{
  const maps=create(SRGB),checked=bindMapperMode(maps,true),input=[[[.5,.01,30.123],[.1,.0533584430463057,300.07000000000266]]];
  for(const [key,map] of Object.entries(checked))checked[key]=function(...args){assert.equal(args.length,2);return map(...args);};
  validate(SRGB,input,true,checked);
  // Raw mapper supports a third argument: the former validator silently fixed this wiring error.
  assert.throws(()=>validate(SRGB,input,true,{...checked,[name]:maps[name]}));
 });
 test(`${name} validation rejects a relaxed membership predicate`,()=>{
  const maps=create(SRGB),checked=bindMapperMode(maps,true),canonical=createCanonicalReference(SRGB),ref=createBoundaryReference("srgb");
  let rejected=0;
  for(const l of [.3,.49,.9])for(const h of [30.123,100.037,150.037,264.04913]) {
   const input=[l,ref.boundaries(l,h).first*(1+1e-7),h],color=canonical(input);
   if(color.inside||!color.linear.every(v=>v>=-1e-7&&v<=1+1e-7))continue;
   const broken={...checked,[name]:(sample,out)=>{out.splice(0,3,...canonical(sample).encoded);return out;}};
   try {validate(SRGB,[[input]],true,broken);} catch(error){assert.ok(error instanceof assert.AssertionError);rejected++;}
  }
  assert.ok(rejected>0,"a widened production membership predicate must not pass validation");
 });
}
test("invalid runtime table knots fail before either lookup can divide by zero",()=>{
 const valid=[[.5,.2,0,0],[.6,.3,180,.1],[.5,.2,360,0]];
 for(const create of [makeEdgeSeekerFromTable,makeEdgeSeekerIndexedFromTable]) {
  assert.doesNotThrow(()=>create(valid));
  for(const mutate of [r=>r[1][2]=0,r=>r[1][2]=361,r=>r[1][3]=NaN,r=>r[1][3]=1,r=>r[1][0]=1,r=>r[1][1]=0,r=>r[2][1]=.3,r=>r[2][2]=359]) {
   const rows=valid.map(r=>[...r]);mutate(rows);assert.throws(()=>create(rows),RangeError);
  }
 }
});
test("Bottosson rejects custom descriptors reusing a fitted gamut id",()=>{
 assert.throws(()=>createBottossonLightness({...REC2020,id:SRGB.id}),RangeError);
 assert.throws(()=>createBottossonLightness({...SRGB}),RangeError);
});
test("wrapped probes retain adjacent floats and negative zero in transport",()=>{
 for(const offset of [-720,-360,0,360,720]) {
  const h=264.052+offset;
  assert.ok(neighbour(h,-1)<h&&neighbour(h,1)>h);
 }
 assert.equal(serializeProbes([[.5,.1,-0]]),"0.5,0.1,-0\n");
});
test("Edge Seeker parity slack follows table intervals only",()=>{
 for(const id of ["srgb","rec2020"]) {
  for(const [lo,hi] of steepIntervals(id))for(const offset of [-720,0,720])assert.ok(inSteepInterval(id,(lo+hi)/2+offset));
  for(const [lo,hi] of steepIntervals(id))assert.ok(!inSteepInterval(id,lo-.001)&&!inSteepInterval(id,hi+.001));
 }
 assert.deepEqual(steepIntervals("display-p3"),[]);
});
test("parity boundary classification cannot hide wrong branch output",()=>{
 const input=[.5,.1,30],a={inside:true,linear:[0,.2,.3],encoded:[0,.4,.5]},b={inside:false,linear:[-1e-16,.2,.3],encoded:[0,.4,.5]};
 assert.doesNotThrow(()=>assertBoundaryDisagreement(input,a,b,[0,.2,.3]));
 assert.throws(()=>assertBoundaryDisagreement(input,a,{...b,linear:[-1e-7,.2,.3]},[0,.2,.3]));
 assert.throws(()=>checkCheckedBranch(input,[0,0,0],[.1,.2,.3],a,"planted wrong pass-through"));
 assert.throws(()=>checkCheckedBranch(input,a.encoded,[.1,.2,.3],b,"planted wrong mapped output"));
});
test("factory-only imports allocate no P3 table or Bottosson cache",async()=>{
 const cbrt=Math.cbrt,ArrayType=globalThis.Float64Array;
 let roots=0,bytes=0;
 Math.cbrt=x=>{roots++;return cbrt(x);};
 globalThis.Float64Array=new Proxy(ArrayType,{construct(target,args){const value=new target(...args);bytes+=value.byteLength;return value;}});
 try {
  await import("../src/bottosson-factory.js?setup-test");
  await import("../src/edge-seeker/factory.js?setup-test");
  assert.equal(roots,0);assert.equal(bytes,0);
 } finally {Math.cbrt=cbrt;globalThis.Float64Array=ArrayType;}
});
