import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { RGB_SPACES } from "../src/rgb-spaces.js";
import { createReference } from "../tests/helpers/css-minde-reference.js";
import { createCanonicalReference } from "../tests/helpers/canonical-reference.js";
import { serializeProbes } from "../tests/helpers/probe-utils.js";
import { mapperOutput } from "../tests/helpers/mapper-output.js";
const root=fileURLToPath(new URL("../",import.meta.url));
const metric=(ref,a,b)=>({
 linear:Math.max(...a.map((v,i)=>Math.abs(ref.decode(v)-ref.decode(b[i])))),
 delta:Math.hypot(...ref.lab(a).map((v,i)=>v-ref.lab(b)[i])),
 encoded:Math.max(...a.map((v,i)=>Math.abs(v-b[i]))),
});
const empty=()=>({count:0,linear:0,delta:0,encoded:0});
function accumulate(max,error,input) {
 max.count++;
 for(const key of ["linear","delta","encoded"])if(error[key]>max[key]){max[key]=error[key];max[`${key}Input`]=input;}
}
// Membership disagreement is a separate discontinuous policy event, never a
// larger blanket numerical tolerance. Each side must still obey its own branch.
export function checkCheckedBranch(input,output,plain,membership,label) {
 assert.deepEqual(output,membership.inside?membership.encoded:plain,`${label} checked branch ${input}`);
}
export function assertBoundaryDisagreement(input,a,b,independent) {
 assert.notEqual(a.inside,b.inside);
 assert.ok(a.linear.every((v,i)=>Number.isFinite(v)&&Math.abs(v-b.linear[i])<=3e-14),`native membership disagreement too large: ${input}`);
 assert.ok(independent.every(v=>v>=-3e-14&&v<=1+3e-14)
  && independent.some(v=>Math.min(Math.abs(v),Math.abs(v-1))<=3e-14),`not a rounding-scale boundary: ${input}`);
}
// `precheck`: an intrinsic method whose canonical precheck is part of both
// modes. Each side must return its own canonical conversion when inside; a
// membership disagreement is a rounding-scale branch event, not a tolerance.
// `exact`: true, false, or a predicate on the method name.
export function runMappingParity({example,createMappers,samplesFor,limitsFor,intrinsic=false,precheck=false,exact=false}) {
 const report=[];
 for(const space of Object.values(RGB_SPACES)) {
  const samples=samplesFor(space.id),input=serializeProbes(samples);
  const lines=execFileSync("cargo",["run","--quiet","--release","--manifest-path","rust/Cargo.toml","--example",example,"--",space.id],{cwd:root,input,encoding:"utf8",maxBuffer:200*1024*1024}).trim().split("\n");
  assert.equal(lines.length,samples.length);
  const exactFor=typeof exact==="function"?exact:()=>exact;
  const ref=createReference(space.id),canonical=intrinsic&&!precheck?null:createCanonicalReference(space),maps=Object.entries(createMappers(space));
  const maxima=maps.map(([method])=>intrinsic?{method,intrinsic:empty(),...(precheck?{boundaryDisagreement:empty()}:{})}:{method,plain:empty(),checkedSameBranch:empty(),checkedBoundaryDisagreement:empty()});
  for(let i=0;i<samples.length;i++) {
   const rust=JSON.parse(lines[i]),sample=samples[i],jsMembership=canonical&&canonical(sample);
   assert.equal(rust.negativeZeroHue,Object.is(sample[2],-0));
   assert.deepEqual(Object.keys(rust.methods).sort(),maps.map(([name])=>name).sort());
   const differs=!!canonical&&jsMembership.inside!==rust.membership.inside;
   // Endpoint returns and clamped negative chroma do not use membership at
   // the authored coordinates, so their parity limits must remain strict.
   const precheckApplies=precheck&&sample[0]>0&&sample[0]<1&&sample[1]>=0;
   if(differs)assertBoundaryDisagreement(sample,jsMembership,rust.membership,ref.linearRgb(sample));
   const limits=limitsFor(space.id,sample);
   for(const [m,[name,map]] of maps.entries()) {
    const plain=mapperOutput(map,sample,...(intrinsic?[]:[false])),expected=rust.methods[name];
    const checked=intrinsic?plain:mapperOutput(map,sample,true);
    for(const output of [expected.plain,expected.checked])assert.ok(output.every(Number.isFinite));
    if(intrinsic) {
     assert.deepEqual(expected.checked,expected.plain,`Rust ${name} intrinsic modes`);
     if(precheckApplies) {
      if(jsMembership.inside)assert.deepEqual(plain,jsMembership.encoded,`JS ${name} canonical ${sample}`);
      if(rust.membership.inside)assert.deepEqual(expected.plain,rust.membership.encoded,`Rust ${name} canonical ${sample}`);
     }
    } else {
     checkCheckedBranch(sample,checked,plain,jsMembership,`JS ${name}`);
     checkCheckedBranch(sample,expected.checked,expected.plain,rust.membership,`Rust ${name}`);
    }
    for(const mode of intrinsic?["plain"]:["plain","checked"]) {
     const error=metric(ref,mode==="plain"?plain:checked,expected[mode]);
     if(exactFor(name))assert.deepEqual(mode==="plain"?plain:checked,expected[mode],`${space.id} ${name} exact parity ${sample}`);
     const category=intrinsic?(precheckApplies&&differs?"boundaryDisagreement":"intrinsic"):mode==="plain"?"plain":differs?"checkedBoundaryDisagreement":"checkedSameBranch";
     if(category!=="checkedBoundaryDisagreement"&&category!=="boundaryDisagreement")assert.ok(error.linear<=limits.linear&&error.delta<=limits.delta,`${space.id} ${name} ${mode} ${sample}: ${JSON.stringify(error)}`);
     accumulate(maxima[m][category],error,sample);
    }
   }
  }
  report.push({gamut:space.id,samples:samples.length,inputSha256:createHash("sha256").update(input).digest("hex"),maxima});
 }
 return {runtime:process.versions,rustF64:report};
}
