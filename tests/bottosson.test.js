import { bindMapperMode, createCanonicalReference } from "./helpers/canonical-reference.js";
import assert from "node:assert/strict";
import { test } from "node:test";
import { RGB_SPACES, SRGB, DISPLAY_P3, REC2020 } from "../src/rgb-spaces.js";
import { createBottossonLightness, createBottossonLightnessCached, createBottossonMappers, bottossonLightness, bottossonLightnessCached } from "../src/bottosson-lightness.js";
import { bottossonFits } from "../src/generated/bottosson.js";
import { getRgbConversions } from "../src/rgb-convert.js";
import { createBottossonReference } from "./helpers/bottosson-reference.js";
import { bottossonSamples } from "./helpers/bottosson-samples.js";
import { validateBottossonMethods } from "./helpers/validate-bottosson-methods.js";
for(const space of Object.values(RGB_SPACES)) {
 test(`${space.id} Bottosson matches independent approximation policy in both modes`,()=>{
  const maps=createBottossonMappers(space),samples=bottossonSamples(space.id);
  for(const checked of [false,true]) console.log(JSON.stringify(validateBottossonMethods(space,[samples],checked,bindMapperMode(maps,checked))));
 });
 test(`${space.id} Bottosson fit quality is distinct from first-exit accuracy`,()=>{
  const ref=createBottossonReference(space.id);
  const limits={srgb:[.00041,.049,.001],"display-p3":[.029,.013,.00009],rec2020:[.00019,.054,.0018]}[space.id];
  const maxima={saturation:0,firstExit:0,clipping:0};
  const hues=Array.from({length:3600},(_,i)=>(i+.37)/10);
  // Dense blue-primary contacts supplement the broad, offset hue sweep.
  for(let i=-200;i<=200;i++)hues.push(ref.hues[2]+i*.0001);
  for(const h of hues) {
   if(ref.hues.some(v=>Math.abs(h-v)<1e-10))continue;
   const exact=ref.exactSaturation(h),cp=ref.cusp(h);
   maxima.saturation=Math.max(maxima.saturation,Math.abs(cp.saturation-exact));
   for(const l of [.001,.1,.3,.49,.7,.9,.9999,cp.l*.99,cp.l+(1-cp.l)*.5]) {
    const c=ref.policy(l,h),first=ref.boundaries(l,h).first;
    const raw=ref.linearRgb([l,c,h]),clipped=raw.map(v=>Math.max(0,Math.min(1,v)));
    const a=ref.linearToLab(clipped),angle=h*Math.PI/180;
    const wanted=[l,c*Math.cos(angle),c*Math.sin(angle)];
    maxima.firstExit=Math.max(maxima.firstExit,Math.abs(c-first));
    maxima.clipping=Math.max(maxima.clipping,Math.hypot(...a.map((v,j)=>v-wanted[j])));
   }
  }
  assert.ok(Object.values(maxima).every(Number.isFinite));
  assert.ok(maxima.saturation<=limits[0]&&maxima.firstExit<=limits[1]&&maxima.clipping<=limits[2],JSON.stringify(maxima));
  console.log(JSON.stringify({gamut:space.id,quality:maxima,hues:hues.length,limits}));
 });
}
test("Bottosson factories preserve aliases and canonical authored hues before caching",()=>{
 for(const space of Object.values(RGB_SPACES))for(const create of [createBottossonLightness,createBottossonLightnessCached]) {
  const map=create(space),conversion=getRgbConversions(space);
  for(const input of [[.5,.02,30.0123],[.001,0,150],[.5,1e-14,264.04913],[.5,-.01,245.1],[.5,.4,264.04913],[-.1,.4,0],[1.1,.4,30]])for(const checked of [false,true]){
   const expected=map(input,[],checked),alias=[...input];
   assert.equal(map(alias,alias,checked),alias);assert.deepEqual(alias,expected);
   const canonical=[];
   if(checked&&conversion.oklchToRgbIfInGamut(...input,canonical))assert.deepEqual(expected,canonical);
  }
  const input=[.5,.02,30.0123];assert.notDeepEqual(map(input,[],false),map(input,[],true));
 }
 assert.deepEqual(bottossonLightness([.5,.4,30],[]),createBottossonLightness(DISPLAY_P3)([.5,.4,30],[]));
 assert.deepEqual(bottossonLightnessCached([.5,.4,30.01],[]),createBottossonLightnessCached(DISPLAY_P3)([.5,.4,30.01],[]));
 assert.throws(()=>createBottossonLightness({id:"unknown"}),RangeError);
 assert.ok(Object.isFrozen(bottossonFits.srgb.fits[0]));
});
test("Bottosson target caches remain isolated against independent expected colors",()=>{
 const spaces=[SRGB,DISPLAY_P3,REC2020],maps=spaces.map(createBottossonLightnessCached);
 for(const order of [[0,1,2],[2,1,0],[1,0,2]])for(const i of order) {
  const space=spaces[i],ref=createBottossonReference(space.id);
  for(const h of [30.0123,150.037,245.067,264.04913]) {
   const hue=Math.round(h*10)/10,expected=ref.linearRgb([.49,ref.policy(.49,hue),hue]).map(v=>Math.max(0,Math.min(1,v)));
   const actual=maps[i]([.49,.4,h],[]).map(ref.decode);
   assert.ok(actual.every((v,j)=>Math.abs(v-expected[j])<2e-11),`${space.id} ${h}`);
  }
 }
});
test("Bottosson only preserves interior Rec.2020 workload colors when the precheck is enabled",()=>{
 const input=[.85,.4,148],canonical=createCanonicalReference(REC2020)(input);
 assert.ok(canonical.inside);
 for(const map of Object.values(createBottossonMappers(REC2020))) {
  const plain=map(input,[]),checked=map(input,[],true);
  assert.deepEqual(plain,map(input,[],false));
  assert.ok(Math.max(...plain.map((v,i)=>Math.abs(v-canonical.encoded[i])))>.1);
  assert.deepEqual(checked,canonical.encoded);
 }
});
test("Bottosson validation rejects wrong callbacks and NaN",()=>{
 const correct=createBottossonMappers(SRGB),input=[[[.49,.4,245.067]]];
 for(const broken of [()=>[NaN,0,0],()=>[0,0,0],createBottossonLightness(REC2020)]){
  assert.throws(()=>validateBottossonMethods(SRGB,input,false,{...correct,"bottosson-lightness":broken}));
 }
 assert.doesNotThrow(()=>validateBottossonMethods(SRGB,input,false,Object.fromEntries(Object.entries(correct).reverse())));
});
