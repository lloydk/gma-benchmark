import { makeLut } from "../src/edge-seeker/makeLut.js";
import { bindMapperMode } from "./helpers/canonical-reference.js";
import assert from "node:assert/strict";
import { test } from "node:test";
import { RGB_SPACES, SRGB, DISPLAY_P3, REC2020 } from "../src/rgb-spaces.js";
import { getRgbConversions } from "../src/rgb-convert.js";
import { edgeSeekerTables } from "./fixtures/edge-seeker.js";
import { createEdgeSeeker, createEdgeSeekerIndexed, createEdgeSeekerMappers, edgeSeeker, edgeSeekerIndexed } from "../src/edge-seeker/index.js";
import { makeEdgeSeeker, makeEdgeSeekerIndexed } from "../src/edge-seeker/makeEdgeSeeker.js";
import { createEdgeSeekerReference } from "./helpers/edge-seeker-reference.js";
import { edgeSeekerSamples, neighbour } from "./helpers/edge-seeker-samples.js";
import { validateEdgeSeekerMethods } from "./helpers/validate-edge-seeker-methods.js";
for(const space of Object.values(RGB_SPACES)) {
 test(`${space.id} Edge Seeker matches independent table/arc policy in both modes`,()=>{
  const maps=createEdgeSeekerMappers(space),samples=edgeSeekerSamples(space.id);
  for(const checked of [false,true])console.log(JSON.stringify(validateEdgeSeekerMethods(space,[samples],checked,bindMapperMode(maps,checked))));
 });
 test(`${space.id} both lookups resolve every knot, cusp and white neighbour`,()=>{
  const ref=createEdgeSeekerReference(space.id),rows=ref.rows;
  const convert=getRgbConversions(space).rgbToOklch;
  const binary=makeEdgeSeeker(convert),indexed=makeEdgeSeekerIndexed(convert);
  let count=0,maxError=0;
  for(const row of rows)for(const hue of [neighbour(row[2],-1),row[2],neighbour(row[2],1)])for(const offset of [-360,0,360]) {
   const h=hue+offset,[l]=ref.itemAt(h);
   for(const lightness of [0,1,1e-12,neighbour(l,-1),l,neighbour(l,1),neighbour(1,-1)]) {
    const a=binary(lightness,h),b=indexed(lightness,h),expected=ref.chroma(lightness,h);
    assert.equal(a,b,`${lightness} ${h}`);assert.ok(Number.isFinite(a)&&a>=0);
    const error=Math.abs(a-expected);assert.ok(error<=2e-12,`${lightness} ${h}: ${a} vs ${expected}`);
    maxError=Math.max(maxError,error);count++;
   }
  }
  console.log(JSON.stringify({gamut:space.id,lookup:{count,maxError}}));
 });
 test(`${space.id} table approximation quality stays within the Rust envelopes`,()=>{
  const ref=createEdgeSeekerReference(space.id),seek=makeEdgeSeeker(getRgbConversions(space).rgbToOklch);
  const limits={srgb:[.049,.049,.0034],"display-p3":[.026,.025,.004],rec2020:[.054,.054,.0049]}[space.id];
  let chroma=0,delta=0,clipping=0,count=0;
  const hues=Array.from({length:3600},(_,i)=>(i+.37)/10);
  for(const row of ref.rows)hues.push(row[2]);
  for(let i=1;i<ref.rows.length;i++)if(ref.rows[i][2]-ref.rows[i-1][2]<.001&&Math.abs(ref.rows[i][1]-ref.rows[i-1][1])>.001)
   for(let n=0;n<=200;n++)hues.push(ref.rows[i-1][2]+(ref.rows[i][2]-ref.rows[i-1][2])*n/200);
  for(const h of hues) {
   const [il]=ref.itemAt(h);
   for(const l of [.001,.1,.3,.49,.7,.9,.9999,il*.99,il+(1-il)*.5]) {
    const c=seek(l,h),first=ref.boundaries(l,h).first,angle=h*Math.PI/180;
    const raw=ref.linearRgb([l,c,h]),lab=ref.linearToLab(raw.map(v=>Math.max(0,Math.min(1,v))));
    const line=[l,c*Math.cos(angle),c*Math.sin(angle)],wanted=[l,first*Math.cos(angle),first*Math.sin(angle)];
    chroma=Math.max(chroma,Math.abs(c-first));delta=Math.max(delta,Math.hypot(...lab.map((v,i)=>v-wanted[i])));
    clipping=Math.max(clipping,Math.hypot(...lab.map((v,i)=>v-line[i])));count++;
   }
  }
  assert.ok([chroma,delta,clipping].every(Number.isFinite));
  assert.ok(chroma<=limits[0]&&delta<=limits[1]&&clipping<=limits[2],JSON.stringify({chroma,delta,clipping}));
  console.log(JSON.stringify({gamut:space.id,quality:{count,chroma,delta,clipping},limits}));
 });
}
test("Edge Seeker factories preserve aliases and canonical input coordinates",()=>{
 for(const space of Object.values(RGB_SPACES))for(const create of [createEdgeSeeker,createEdgeSeekerIndexed]) {
  const map=create(space),convert=getRgbConversions(space);
  for(const input of [[.5,.02,30.0123],[.001,0,150],[.5,-.01,245.1],[.5,.4,264.04913],[0,0,0],[1,0,0],[-.1,.4,0],[1.1,.4,30]])for(const checked of [false,true]) {
   const actual=map(input,[],checked),alias=[...input];assert.equal(map(alias,alias,checked),alias);assert.deepEqual(alias,actual);
   const canonical=[];if(checked&&convert.oklchToRgbIfInGamut(...input,canonical))assert.deepEqual(actual,canonical);
  }
 }
 assert.deepEqual(edgeSeeker([.5,.4,30],[]),createEdgeSeeker(DISPLAY_P3)([.5,.4,30],[]));
 assert.deepEqual(edgeSeekerIndexed([.5,.4,30],[]),createEdgeSeekerIndexed(DISPLAY_P3)([.5,.4,30],[]));
 for(const rows of Object.values(edgeSeekerTables))assert.ok(Object.isFrozen(rows)&&rows.every(Object.isFrozen));
});
test("Edge Seeker interleaved target lookups match independent colors",()=>{
 const spaces=[SRGB,DISPLAY_P3,REC2020],maps=spaces.map(createEdgeSeekerMappers);
 for(const order of [[0,1,2],[2,1,0],[1,0,2]])for(const i of order) {
  const space=spaces[i],ref=createEdgeSeekerReference(space.id);
  for(const h of [30.0123,150.037,245.067,264.04913])for(const map of Object.values(maps[i])) {
   const expected=ref.linearRgb([.49,Math.min(.4,ref.chroma(.49,h)),h]).map(v=>Math.max(0,Math.min(1,v)));
   const actual=map([.49,.4,h],[]).map(ref.decode);assert.ok(actual.every((v,j)=>Math.abs(v-expected[j])<=3e-12));
  }
 }
});
test("Edge Seeker validation rejects NaN, wrong target and wrong callbacks by name",()=>{
 const correct=createEdgeSeekerMappers(SRGB),samples=[[[.49,.4,245.067]]];
 for(const broken of [()=>[NaN,0,0],()=>[0,0,0],createEdgeSeeker(REC2020)])
  assert.throws(()=>validateEdgeSeekerMethods(SRGB,samples,false,{...correct,"edge-seeker":broken}));
 assert.doesNotThrow(()=>validateEdgeSeekerMethods(SRGB,samples,false,Object.fromEntries(Object.entries(correct).reverse())));
});

test("Edge Seeker samples once per descriptor and shares both lookup variants",()=>{
 for(const factories of [[createEdgeSeeker,createEdgeSeekerIndexed],[createEdgeSeekerIndexed,createEdgeSeeker]]) {
  const records=[SRGB,REC2020].map(original=>{
   let calls=0;
   // Deliberately reuse the same id: cache identity must follow the descriptor.
   const space={...original,id:"custom",transfer:{...original.transfer,decode(x){calls++;return original.transfer.decode(x);}}};
   return {space,original,count:()=>calls};
  });
  for(const record of records) {
   assert.equal(record.count(),0);
   const map=factories[0](record.space),count=record.count();assert.ok(count>0);
   const other=factories[1](record.space);
   factories[0](record.space);factories[1](record.space);
   assert.equal(record.count(),count,"subsequent factories must reuse sampling");
   const ref=createEdgeSeekerReference(record.original.id);
   for(const h of [30.0123,150.037,245.067,264.04913]) {
    const input=[.49,.4,h],a=map(input,[]),b=other(input,[]);
    assert.deepEqual(a,b);
    const expected=ref.linearRgb([.49,Math.min(.4,ref.chroma(.49,h)),h]).map(v=>Math.max(0,Math.min(1,v)));
    assert.ok(a.map(ref.decode).every((v,i)=>Math.abs(v-expected[i])<3e-12));
   }
   assert.equal(record.count(),count,"unchecked mapping must not sample the gamut");
  }
 }
});

test("runtime Edge Seeker knots agree with recorded shared-generator fixtures",()=>{
 for(const space of Object.values(RGB_SPACES)) {
  const actual=makeLut(getRgbConversions(space).rgbToOklch,400).map(({l,c,h,curvature})=>[l,c,h,curvature]),expected=edgeSeekerTables[space.id];
  assert.equal(actual.length,expected.length);
  let max=0;
  actual.forEach((row,i)=>row.forEach((v,j)=>{
   assert.ok(Number.isFinite(v));
   max=Math.max(max,Math.abs(v-expected[i][j]));
   assert.ok(Math.abs(v-expected[i][j])<=1e-12,`${space.id} knot ${i} column ${j}`);
  }));
  console.log(JSON.stringify({gamut:space.id,runtimeTableMax:max}));
 }
});
