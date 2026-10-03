import assert from 'node:assert/strict';
import { test } from 'node:test';
import { renderPerformance } from '../scripts/render-performance.mjs';
import { methods, runtimeMethods } from '../scripts/performance-workloads.mjs';
import { median, cpuList, DEFAULT_REPORT } from '../scripts/performance-stats.mjs';
import { readFileSync } from 'node:fs';
import { renderPerformanceAnalysis } from '../scripts/performance-analysis.mjs';

// Synthetic numbers intentionally contradict the initial report's rankings.
function fixture({ extraMethod = false, extraWorkload = false } = {}) {
 const report = {
  schema: 'gma-performance-v1', completed: '2026-10-02', sourceCommit: 'fixture', runs: 4,
  methods: structuredClone(methods), runtimeMethods: structuredClone(runtimeMethods),
  workloads: [], validation: [], measurements: [], summary: [], p3RandomBelowCusp: .75,
  environment: { cpu: 'fixture', cpuAffinity: '2,3', platform: 'linux', kernel: 'fixture',
   glibc: 'fixture', node: 'fixture', bun: 'fixture', rust: 'fixture', rustflags: 'fixture',
   releaseProfile: 'fixture', warmup: 50, measured: 25 },
 };
 for (const gamut of ['display-p3','srgb','rec2020']) for (const name of ['grid','random']) {
  report.workloads.push({id:`${gamut}-${name}`,gamut,name,count:10,inside:0,checked:false});
 }
 for (const name of ['below-cusp','above-cusp','random-checked','mixed-checked','inside-checked']) {
  report.workloads.push({id:`display-p3-${name}`,gamut:'display-p3',name,count:10,
   inside:name==='inside-checked'?10:name==='mixed-checked'?5:0,checked:name.endsWith('checked')});
 }
 if (extraMethod) {
  report.methods.push(['fixture-mapper','fixture mapper']);
  Object.values(report.runtimeMethods).forEach(ids=>ids.push('fixture-mapper'));
 }
 if (extraWorkload) report.workloads.push({id:'display-p3-extra',gamut:'display-p3',name:'extra',count:20,inside:0});
 for (const work of report.workloads) for (const [runtime,ids] of Object.entries(report.runtimeMethods)) for (const method of ids) {
  const base = method==='dualray-fast'?200:method==='dualray-fast-poly'?300:method==='fixture-mapper'?1:100;
  const ns = base * (runtime==='node'||runtime==='rust-f32'?2:1);
  const cell = {workload:work.id,runtime,method};
  report.validation.push({...cell,count:work.count,checksum:1});
  const processes = [ns-ns/4,ns-ns/8,ns+ns/8,ns+ns/4];
  // Even process counts need the average of the central pair, not the upper one.
  report.summary.push({...cell,ns,min:processes[0],max:processes[3],processes});
  processes.forEach((ns,run)=>report.measurements.push({...cell,run,ns,passes:Array(25).fill(ns)}));
 }
 return report;
}

const render = report => renderPerformance(report,{artifact:'fixture.json',includeAnalysis:false});

test('CPU lists and even/odd medians use one shared contract', () => {
 assert.equal(cpuList('2'),'2');
 assert.equal(cpuList('3,2,2'),'2,3');
 for (const invalid of ['', '2,', '-1', '2 3', '2;3']) assert.throws(()=>cpuList(invalid));
 assert.equal(median([9,1,3]),3);
 assert.equal(median([9,1,5,3]),4);
});

test('tables retain reversed measurements and text columns align left', () => {
 const text = render(fixture());
 assert.match(text,/\| dualray fast \| 400\.0 \| 200\.0 \| 200\.0 \| 400\.0 \|/);
 assert.match(text,/All workloads contain 10 colors/);
 assert.match(text,/\| Workload \| Target \| Canonical in gamut \(f64\) \| Entry mode \|\n\| --- \| --- \| ---: \| --- \|/);
 assert.doesNotMatch(text,/NaN|undefined|sandbox|EPERM/);
});

test('additional methods and workloads change coverage without a fixed cell count', () => {
 const text = render(fixture({extraMethod:true,extraWorkload:true}));
 assert.match(text,/16 methods in Node, 16 methods in Bun, 17 methods in Rust f64, 17 methods in Rust f32/);
 assert.match(text,/Workloads contain 10–20 colors/);
 assert.match(text,/792 measured cells/);
 assert.match(text,/\| fixture mapper \| 2\.0 \| 1\.0 \| 1\.0 \| 2\.0 \|/);
});

// The runner, profiler and renderer share this default artifact.
const measured = JSON.parse(readFileSync(new URL(`../${DEFAULT_REPORT}`,import.meta.url)));
const profile = JSON.parse(readFileSync(new URL(`../${DEFAULT_REPORT.replace(/\.json$/,'-math.json')}`,import.meta.url)));

test('checked narrative leads the report and resolves all measurement placeholders', () => {
 const text=renderPerformance(measured,{artifact:'timings.json',mathProfile:profile,mathArtifact:'math.json'});
 assert.equal((text.match(/^### \d\./gm)??[]).length,9);
 assert.ok(text.indexOf('### 9.')<text.indexOf('| Method |'));
 assert.ok(text.indexOf('## Appendix A')<text.indexOf('### sRGB'));
 assert.match(text,/checked_cardano/);
 assert.match(text,/\*\*32\.8 cbrt calls\/color\*\*/);
 assert.doesNotMatch(text,/\{\{|NaN|undefined/);
});

test('changed findings or mismatched counter evidence require editorial review', () => {
 const reversed=structuredClone(measured);
 reversed.summary.find(x=>x.workload==='display-p3-random' && x.runtime==='node' && x.method==='dualray-fast').ns=1000;
 assert.throws(()=>renderPerformanceAnalysis(reversed,profile),/editorial review: Fast no longer beats Dualray/);
 const slowTables=structuredClone(measured);
 slowTables.summary.find(x=>x.workload==='display-p3-above-cusp' && x.runtime==='rust-f32' && x.method==='dualray-fast-tables').ns=1000;
 assert.throws(()=>renderPerformanceAnalysis(slowTables,profile),/editorial review: tables row no longer speeds up the upper solve/);
 const wrong=structuredClone(profile);
 wrong.rows.find(x=>x.workload==='display-p3-below-cusp' && x.method==='dualray-fast').checksum++;
 assert.throws(()=>renderPerformanceAnalysis(measured,wrong),/profile inputs\/output changed/);
 const newer=structuredClone(measured); newer.environment.node='v99.0.0';
 assert.throws(()=>renderPerformanceAnalysis(newer,profile),/editorial review/);
 const changedPaths=structuredClone(profile);
 const above=changedPaths.rows.find(x=>x.workload==='display-p3-above-cusp' && x.method==='dualray-fast');
 above.paths.exact=1; above.pathsPerColor.exact=1/above.count;
 assert.throws(()=>renderPerformanceAnalysis(measured,changedPaths),/above-cusp branch attribution changed/);
});

test('branch attribution, runtime guidance and cusp evidence appear beside the analysis', () => {
 const text=renderPerformance(measured,{artifact:'timings.json',mathProfile:profile,mathArtifact:'math.json'});
 const above=profile.rows.find(x=>x.workload==='display-p3-above-cusp' && x.method==='dualray-fast');
 assert.equal(above.paths.upper,above.count);
 assert.equal(above.paths.exact,0);
 assert.equal(above.counts.sin,above.paths.upper+above.paths.precheck);
 assert.match(text,/none enters exact\s+recovery/);
 assert.match(text,/21\.7% first run the canonical precheck/);
 assert.match(text,/\| Above \| 0\.000 \| 1\.000 \| 0\.000 \| 0\.217 \|/);
 assert.match(text,/\| edge-seeker \| Above \| 0\.000 \| 2\.000 \|/);
 assert.match(text,/## Comparing runtimes/);
 assert.match(text,/## Choosing a method/);
 assert.match(text,/`dualray fast \(tables\)` in Node, Bun and Rust f32 and `dualray fast \(poly encode\)` in Rust f64/);
 assert.match(text,/31\.0 ns in f32 versus 47\.9 in f64/);
 assert.match(text,/Node is slower than Rust f64 here/);
});

test('gamut comparison uses identical inputs and checks target-specific conclusions', () => {
 const text=renderPerformanceAnalysis(measured,profile);
 assert.match(text,/## Comparing gamuts/);
 assert.match(text,/\| Rust f32 \| 1\.00× \| 0\.99× \| 1\.00× \| 1\.03× \|/);
 assert.match(text,/140\.8 ns for sRGB, 113\.2 for P3 and\s+148\.0 for Rec\.2020/);
 assert.match(text,/Rec\.2020 contains 126 interior colors\s+\(0\.4%\)/);
 const mismatched=structuredClone(measured);
 mismatched.workloads.find(x=>x.id==='srgb-random').sha256='different-inputs';
 assert.throws(()=>renderPerformanceAnalysis(mismatched,profile),/gamut random inputs are no longer identical/);
 const reversed=structuredClone(measured);
 reversed.summary.find(x=>x.workload==='rec2020-random' && x.runtime==='bun' && x.method==='oklch-halley').ns/=2;
 assert.throws(()=>renderPerformanceAnalysis(reversed,profile),/Bun iterative Rec\.2020 penalty changed/);
 const newWinner=structuredClone(measured);
 newWinner.summary.find(x=>x.workload==='rec2020-random' && x.runtime==='node' && x.method==='css-minde').ns=1;
 assert.throws(()=>renderPerformanceAnalysis(newWinner,profile),/random gamut winner changed/);
});

test('incomplete, duplicated and incorrectly aggregated measurements are rejected', () => {
 const missing=fixture(); missing.summary.pop(); assert.throws(()=>render(missing),/Incomplete cells/);
 const duplicate=fixture(); duplicate.measurements[1]=duplicate.measurements[0]; assert.throws(()=>render(duplicate),/Duplicate timing process/);
 const wrong=fixture(); wrong.summary[0].ns=wrong.summary[0].processes[2]; assert.throws(()=>render(wrong),/Incorrect aggregate median/);
});
