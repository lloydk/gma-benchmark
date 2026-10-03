// Untimed Math-call counts on the exact report inputs. Wrappers delegate to
// the original functions; checksums verify that instrumentation preserves output.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { DISPLAY_P3 } from '../src/rgb-spaces.js';
import { DEFAULT_REPORT } from './performance-stats.mjs';

process.chdir(fileURLToPath(new URL('../',import.meta.url)));
const input = process.argv[2] ?? DEFAULT_REPORT;
const output = process.argv[3] ?? input.replace(/\.json$/, '-math.json');
const bytes = readFileSync(input), report = JSON.parse(bytes);
assert.equal(process.version,report.environment.node,'Use the measured Node version');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
for (const [file,sha] of Object.entries(report.sourceHashes).filter(([file])=>file.startsWith('src/'))) {
 assert.equal(hash(readFileSync(file)),sha,file);
}
// Instrument a temporary module in memory; production source and arithmetic
// stay unchanged. Exact replacement checks make source drift fail explicitly.
const fastUrl=new URL('../src/dualray-fast.js',import.meta.url);
let fastSource=readFileSync(fastUrl,'utf8');
for (const [before,after] of [
 ['createDualrayFast (space, { tables = false } = {})', 'createDualrayFast (space, { tables = false } = {}, counts)'],
 ['const { oklchToRgbIfInGamut } = getRgbConversions(space);',
  'const { oklchToRgbIfInGamut: canonicalCheck } = getRgbConversions(space); const oklchToRgbIfInGamut = (...args) => { counts.precheck++; return canonicalCheck(...args); };'],
 ['const exact = (L, C, hue, out) => {', 'const exact = (L, C, hue, out) => { counts.exact++;'],
 ['const lower = (sector, x, y, out) => {', 'const lower = (sector, x, y, out) => { counts.lower++;'],
 ['const brighter = g1 > g0;', 'counts.upper++; const brighter = g1 > g0;'],
]) {
 assert.equal(fastSource.split(before).length,2,`Update Fast instrumentation: ${before}`);
 fastSource=fastSource.replace(before,after);
}
fastSource=fastSource.replace(/from "(\.\/[^"\n]+)"/g,(_,path)=>`from "${new URL(path,fastUrl).href}"`);
const {createDualrayFast: instrumentFast}=await import(`data:text/javascript;base64,${Buffer.from(fastSource).toString('base64')}`);
const entries = [
 ['clip','clip','clip'], ['css-minde','css-minde','cssMinde'],
 ['oklch-cubic','oklch-cubic','oklchCubic'], ['oklch-cubic-no-cache','oklch-cubic-no-cache','oklchCubicNoCache'],
 ['oklch-cubic-direct','oklch-cubic-direct','oklchCubicDirect'], ['oklch-halley','oklch-halley','oklchHalley'],
 ['oklch-ostrowski','oklch-ostrowski','oklchOstrowski'], ['dualray','dualray','dualray'],
 ['dualray-fast','dualray-fast','createDualrayFast'],
 ['dualray-fast-tables','dualray-fast','createDualrayFastTables'],
 ['bottosson-lightness','bottosson-lightness','bottossonLightness'],
 ['bottosson-lightness-cached','bottosson-lightness','bottossonLightnessCached'],
 ['edge-seeker','edge-seeker/index','edgeSeeker'], ['edge-seeker-indexed','edge-seeker/index','edgeSeekerIndexed'],
 ['raytrace','raytrace','raytrace'],
];
const operations = ['cbrt','sqrt','sin','cos','acos'];
assert.deepEqual(entries.map(([id])=>id).sort(),[...report.runtimeMethods.node].sort(),'Update profiler method coverage');
const rows=[];
for (const work of report.workloads.filter(w=>w.gamut==='display-p3')) {
 const data=readFileSync(`${input}.inputs/${work.id}.bin`);
 assert.equal(hash(data),work.sha256);
 const samples=Array.from({length:work.count},(_,i)=>Array.from({length:3},(_,j)=>data.readDoubleLE(i*24+j*8)));
 for (const [method,path,name] of entries) {
  const module=await import(`../src/${path}.js`);
  const fast=method.startsWith('dualray-fast');
  const mapper=fast?module[name](DISPLAY_P3):module[name];
  const checked=work.checked && !fast && !['clip','css-minde','dualray'].includes(method);
  const out=[0,0,0];
  // Fill every hue bucket touched by this corpus before counting.
  for (const sample of samples) mapper(sample,out,checked);
  const counts=Object.fromEntries(operations.map(op=>[op,0]));
  const original=Object.fromEntries(operations.map(op=>[op,Math[op]]));
  let checksum=0;
  try {
   for (const op of operations) Math[op]=(...args)=>{counts[op]++;return original[op](...args);};
   for (const sample of samples) {
    mapper(sample,out,checked);
    checksum+=out[0]+out[1]+out[2];
   }
  } finally {for (const op of operations) Math[op]=original[op];}
  const validation=report.validation.find(v=>v.workload===work.id && v.runtime==='node' && v.method===method);
  assert.equal(checksum,validation.checksum,`${work.id}/${method}`);
  const row={workload:work.id,method,count:work.count,checksum,counts,
   perColor:Object.fromEntries(operations.map(op=>[op,counts[op]/work.count]))};
  if (fast) {
   const paths={lower:0,upper:0,exact:0,precheck:0};
   const instrumented=instrumentFast(DISPLAY_P3,{tables:method==='dualray-fast-tables'},paths), reference=[0,0,0];
   for (const sample of samples) {
    instrumented(sample,out);
    mapper(sample,reference);
    assert.ok(out.every((x,i)=>Object.is(x,reference[i])),`Fast instrumentation changed output at ${sample}`);
   }
   row.paths=paths;
   row.pathsPerColor=Object.fromEntries(Object.entries(paths).map(([key,count])=>[key,count/work.count]));
   assert.equal(counts.sin,paths.upper+paths.exact+paths.precheck,'Unattributed Fast sine calls');
   assert.equal(counts.cos,counts.sin,'Fast trig calls disagree');
  }
  rows.push(row);
 }
}
writeFileSync(output,JSON.stringify({schema:'gma-performance-math-v1',created:new Date().toISOString(),
 runtime:process.version,sourceCommit:report.sourceCommit,timingArtifactSha256:hash(bytes),
 profilerSha256:hash(readFileSync(fileURLToPath(import.meta.url))),
 scope:'Untimed Node/P3 Math calls after cache warmup plus instrumented Fast path entries. Fast instrumentation matches production output per channel. No power or hardware-branch counters.',rows},null,2)+'\n');
console.log(`Validated and counted ${rows.length} cells: ${resolve(output)}`);
