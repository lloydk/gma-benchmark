// Render the report from captured measurements; never runs timed code.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { median } from './performance-stats.mjs';
import { renderPerformanceAnalysis } from './performance-analysis.mjs';
import { createHash } from 'node:crypto';

export function renderPerformance(report, { artifact, mathProfile, mathArtifact, includeAnalysis = true, validationNote = 'Independent regression-suite results were not supplied with this artifact; run the existing tests separately before interpreting performance changes.' } = {}) {
assert.equal(report.schema, 'gma-performance-v1');
assert.ok(report.completed && report.summary?.length, 'Incomplete report');
const runtimes = Object.keys(report.runtimeMethods);
assert.equal(new Set(report.workloads.map(w=>w.id)).size, report.workloads.length, 'Duplicate workload');
const methodIds = new Set(report.methods.map(([id]) => id));
assert.equal(methodIds.size, report.methods.length, 'Duplicate method');
const key = c => `${c.workload}/${c.runtime}/${c.method}`;
const expected = new Set();
for (const w of report.workloads) for (const runtime of runtimes) {
 const ids = report.runtimeMethods[runtime];
 assert.equal(new Set(ids).size, ids.length, 'Duplicate supported method');
 for (const method of ids) {
  assert.ok(methodIds.has(method), `Unregistered method: ${method}`);
  expected.add(key({workload:w.id,runtime,method}));
 }
}
for (const rows of [report.summary, report.validation]) {
 assert.equal(rows.length, expected.size, 'Incomplete cells');
 assert.deepEqual(new Set(rows.map(key)), expected, 'Missing or unexpected cells');
}
assert.equal(report.measurements.length, expected.size * report.runs);
const processes = new Map();
for (const m of report.measurements) {
 assert.ok(expected.has(key(m)) && Number.isInteger(m.run) && m.run >= 0 && m.run < report.runs);
 const id = `${m.run}/${key(m)}`;
 assert.ok(!processes.has(id), 'Duplicate timing process');
 assert.equal(m.ns, median(m.passes), 'Incorrect process median');
 processes.set(id,m.ns);
}
for (const row of report.summary) {
 const times = Array.from({length:report.runs}, (_,run)=>processes.get(`${run}/${key(row)}`)).sort((a,b)=>a-b);
 assert.deepEqual(row.processes,times);
 assert.equal(row.ns,median(times), 'Incorrect aggregate median');
 assert.equal(row.min,times[0]); assert.equal(row.max,times.at(-1));
}
const titles = { node: 'Node', bun: 'Bun', 'rust-f64': 'Rust f64', 'rust-f32': 'Rust f32' };
const n=x=>x.toFixed(1), pct=x=>`${(100*x).toFixed(1)}%`;
const ns=(workload,runtime,method)=>report.summary.find(x=>x.workload===workload && x.runtime===runtime && x.method===method)?.ns;
const time=(workload,runtime,method)=>{const x=ns(workload,runtime,method);return x===undefined?'—':n(x);};
const label=id=>report.methods.find(([method])=>method===id)[1];
const table=(headers,rows,textColumns=[0])=>[
 '| '+headers.join(' | ')+' |',
 '| '+headers.map((_,i)=>textColumns.includes(i)?'---':'---:').join(' | ')+' |',
 ...rows.map(row=>'| '+row.join(' | ')+' |'),'',
].join('\n');
const tableFor=work=>table(['Method',...runtimes.map(r=>titles[r])],report.methods.map(([id,name])=>[name,...runtimes.map(r=>time(work,r,id))]));
const counts=runtimes.map(r=>`${report.runtimeMethods[r].length} methods in ${titles[r]}`).join(', ');
const sizes=[...new Set(report.workloads.map(w=>w.count))];
const sizeNote=sizes.length===1?`All workloads contain ${sizes[0].toLocaleString('en-US')} colors.`:`Workloads contain ${Math.min(...sizes).toLocaleString('en-US')}–${Math.max(...sizes).toLocaleString('en-US')} colors; individual counts are listed below.`;
const lines=[];
const add=s=>lines.push(s.trim(),'');
add(`# Performance analysis

The largest speed differences here come from avoiding work: fitting a lower face instead of solving it, caching hue structure, reusing RGB values, or returning before a perceptual search. Which shortcut applies depends on the input as much as the method.

Measured ${report.completed.slice(0,10)} on ${report.environment.cpu}, using Node ${report.environment.node}, Bun ${report.environment.bun} and ${report.environment.rust.split('\n')[0]}. Coverage: **${counts}**, across ${new Set(report.workloads.map(w=>w.gamut)).size} gamuts. All times are **nanoseconds per color**, including encoded RGB output.`);
if (includeAnalysis) add(renderPerformanceAnalysis(report,mathProfile));
add(`## How to read these numbers

- Use the shuffled P3 workload as the main comparison, then check the cusp-side and in-gamut results for your input distribution. The integer-hue grid favors repeated lookup and branch patterns; the mixed/interior sets are controlled diagnostics. Full sRGB, Rec.2020 and diagnostic tables are in the appendices.
- Compare mapping policies as well as speed. Clipping and CSS MINDE can change lightness/hue; cached Bottosson uses 0.1° hue buckets; Fast targets an empirical ΔEOK budget of 1e-3 maximum / 1e-4 p99. Dualray retains first-exit boundary semantics, while canonical-checking paths preserve the authored conversion. [Method contracts](README.md#methods) describe the differences.
- Times are medians of ${report.runs} fresh-process medians after warmup, with all three RGB channels consumed. Setup and cold-cache costs are excluded. These are fixed-target OKLCh kernels; public color objects, input-space conversion, alpha and browser rendering would add different work.
- The explanations combine measured timings, untimed operation counts and source inspection. The cube-root library and branch-prediction explanations are supported mechanisms, not isolated shares of CPU time; f32 power latency remains an inference. Historical microbenchmarks and counter experiments are labeled in [PERFORMANCE-NOTES.md](PERFORMANCE-NOTES.md).
- Close rankings can change with JIT state, binary layout and scheduling. CPU affinity applies to worker threads too. The recorded process ranges below make the remaining variation visible; small median differences are not guarantees.

[Raw timing data](${artifact})${mathArtifact?`, [current Math-call counts](${mathArtifact})`:''} accompany the report. These tables replace the historical timings that predated complete output consumption.`);
add('## Display-P3: the main comparison');
add('Shuffled fractional hue/lightness, C=0.4, all inputs out of gamut. The findings above explain the differences in this table.');
add(tableFor('display-p3-random'));
add(`## Where the next experiments would pay

For Fast, measure whether sharing trig between a failed canonical precheck and the upper solve removes useful work in both JS and Rust. Any reuse must preserve authored-hue conversion and in-gamut output bits. CSS MINDE provides the clearest current evidence for testing a faster native cube root; Raytrace needs a separate dependency-chain profile because the same throughput calculation does not explain its measured gap. For uncached f32 cubic, profile candidate validation before changing arithmetic. On mostly interior input, measure conversion and membership handling first.

Memory-focused experiments have different goals: a u16 Rust Edge Seeker index could reduce its extra payload from 28 to 7 KiB; a seven-scalar cubic cache would exchange coefficient reconstruction for less storage. Earlier prototypes and the cube-expression/cache-layout history are preserved in [the notes](PERFORMANCE-NOTES.md). Each experiment needs its own output/accuracy checks and end-to-end timing.`);
add('## Measurement and reproduction');
add(`Jobs ran serially with logical CPU affinity \`${report.environment.cpuAffinity}\`, one method/target/runtime per process, under ${report.environment.platform} ${report.environment.kernel} and ${report.environment.glibc}. Rust used \`${report.environment.rustflags}\`, ${report.environment.releaseProfile}. f32 samples were rounded before timing; output channels were individually widened for the checksum. CPU frequency was not fixed.

Each process ran ${report.environment.warmup} complete warmup passes and ${report.environment.measured} timed passes; the cell order rotated and reversed between rounds. ${sizeNote} Rust used input and checksum optimization barriers. Factories, table/index construction and initial cache filling preceded the measured passes.`);
const spreads=report.summary.map(r=>({...r,spread:(r.max-r.min)/r.ns})).sort((a,b)=>b.spread-a.spread);
add(`There are **${report.summary.length} measured cells** and **${report.measurements.length} fresh timing processes**. The median process range, (maximum−minimum)/median, is **${pct(median(spreads.map(x=>x.spread)))}**; **${spreads.filter(x=>x.spread>.1).length} cells** span more than 10%. The ranges are repeatability diagnostics, not confidence intervals.`);
const bunFast=spreads.filter(x=>x.runtime==='bun' && x.method==='dualray-fast');
if(bunFast.length) add(`Bun Fast has **${bunFast.filter(x=>x.spread>.05).length}/${bunFast.length} cells** with a process range above 5%. The earlier single-CPU run showed multiple timing modes, motivating the current affinity setting; the [affinity history](PERFORMANCE-NOTES.md#affinity-experiment) records that investigation.`);
add(`Every cell was validated in a separate process before timing. All outputs had to be finite and in range, and timed checksums had to agree with the validation sum over ${report.environment.warmup+report.environment.measured} passes. ${validationNote}`);
if(mathArtifact) add(`The [Math-call profiler](scripts/profile-performance-math.mjs) replayed each P3 workload after warming caches, delegated every counted call to the original Math function, and required exact agreement with the Node validation checksum. A separate pass instruments Fast's precheck, lower, upper and exact-search entries in an in-memory source copy, checking every output channel against production. Source replacement markers must match exactly. Gamma powers and hardware branch misses were not counted. The artifact is bound to the timing artifact's hash.`);
add(`The measured algorithm baseline is \`${report.sourceCommit}\`. The artifact records input, source and Rust-binary hashes. Resume verifies source hashes, binary hash, runtime versions, CPU model and affinity.

\`\`\`sh
# Generate fresh measurements on available logical CPUs.
bun scripts/run-performance.mjs --output reports/performance-new.json --cpu ${report.environment.cpuAffinity} --runs ${report.runs}

# Resume an interrupted run with the same code, inputs and environment.
bun scripts/run-performance.mjs --output reports/performance-new.json --cpu ${report.environment.cpuAffinity} --runs ${report.runs} --resume

# Collect untimed operation counts, then regenerate the article and tables.
node scripts/profile-performance-math.mjs reports/performance-new.json
bun scripts/render-performance.mjs reports/performance-new.json
\`\`\`

The [analysis template](scripts/templates/performance-analysis.md) is hand-written; the renderer fills its numbers from the artifacts. [Editorial checks](scripts/performance-analysis.mjs) stop regeneration if a central comparison changes, so the explanation can be revised with the data. [PERFORMANCE-NOTES.md](PERFORMANCE-NOTES.md) retains history rather than current ranking claims.

Use \`--prepare-only\` or \`--validate-only\`, then \`--resume\`, for staged measurement. The ignored \`<output>.inputs/\` directory holds common little-endian f64 triples regenerated by [the workload builder](scripts/performance-workloads.mjs). The [JS runner](scripts/bench-rgb-kernels.mjs) and [Rust runner](rust/examples/performance.rs) call production methods; [orchestration](scripts/run-performance.mjs) keeps timing and validation separate.`);
add('## Appendix A: standard workloads');
for(const gamut of new Set(report.workloads.map(w=>w.gamut))){
 const title={'display-p3':'Display-P3',srgb:'sRGB',rec2020:'Rec.2020'}[gamut]??gamut;
 for(const [name,description] of [['grid','ordered integer hues'],['random','shuffled fractional hues']]) {
  if(gamut==='display-p3' && name==='random') continue;
  if(!report.workloads.some(w=>w.id===`${gamut}-${name}`)) continue;
  add(`### ${title}: ${description}`);add(tableFor(`${gamut}-${name}`));
 }
}
add('## Appendix B: cusp-side and membership workloads');
for(const [name,title] of [['below-cusp','Below the P3 cusp'],['above-cusp','Above the P3 cusp'],['random-checked','P3 random, checked entry'],['mixed-checked','P3 50% in gamut, checked entry'],['inside-checked','P3 100% in gamut, checked entry']]) {
 add(`### ${title}`);add(tableFor(`display-p3-${name}`));
}
add('## Appendix C: inputs, operation counts and process ranges');
add(table(['Workload','Target','Canonical in gamut (f64)','Entry mode'],report.workloads.map(w=>[w.name,w.gamut,`${w.inside}/${w.count} (${pct(w.inside/w.count)})`,w.checked?'checked':'plain']),[0,1,3]));
add(`The grid descends through integer hues at C=0.4; random uses independently stratified and shuffled fractional hue/lightness at the same chroma. Rec.2020 contains a small in-gamut subset, listed above. The artifact also records native f32 membership counts.

The P3 cusp-side inputs preserve random hue/order and redistribute lightness with a 0.01 margin either side of a cusp derived from the independent XYZ/stationary-interval reference. The mixed workload alternates C=0.4 with C=0.01·min(L,1−L); the interior workload uses the latter throughout.

Checked entry adds canonical conversion/membership where supported. CSS MINDE, Dualray and Fast retain intrinsic handling in both modes. Dualray uses its normalized-cubic first-exit policy; unchecked Bottosson deliberately projects even interior input to its fitted boundary. Narrow blue-fold paths have dedicated correctness tests, but the aggregate workloads do not characterize their worst-case latency.`);
if(mathProfile) {
 add('### Current P3 random Math calls per color');
 add(table(['Method','cbrt','sqrt','sin','cos','acos'],mathProfile.rows.filter(x=>x.workload==='display-p3-random').map(x=>[label(x.method),...['cbrt','sqrt','sin','cos','acos'].map(op=>x.perColor[op].toFixed(2))])));
 add('### Cusp-side operation and path counts');
 add('Untimed Node/P3 calls per color on the same below/above-cusp inputs as the timing tables.');
 const cuspRows=mathProfile.rows.filter(x=>['display-p3-below-cusp','display-p3-above-cusp'].includes(x.workload) && ['oklch-cubic','dualray-fast','edge-seeker'].includes(x.method));
 const side=x=>x.workload==='display-p3-below-cusp'?'Below':'Above';
 add(table(['Method','Cusp side','cbrt','sqrt','sin','cos','acos'],cuspRows.map(x=>[label(x.method),side(x),...['cbrt','sqrt','sin','cos','acos'].map(op=>x.perColor[op].toFixed(3))]),[0,1]));
 add(table(['Dualray Fast cusp side','Lower entries','Upper entries','Exact-search entries','Canonical prechecks'],cuspRows.filter(x=>x.method==='dualray-fast').map(x=>[side(x),...['lower','upper','exact','precheck'].map(path=>x.pathsPerColor[path].toFixed(3))])));
 add('Path entries overlap: a canonical precheck can precede upper solving, and rejected upper work can precede exact recovery. On this above-cusp corpus, the extra trig comes entirely from prechecks; exact-search entries are zero.');
}
add('### Largest process ranges');
add(table(['Workload','Runtime','Method','Median','Min–max','Range/median'],spreads.slice(0,8).map(r=>[r.workload,titles[r.runtime],label(r.method),n(r.ns),`${n(r.min)}–${n(r.max)}`,pct(r.spread)]),[0,1,2]));
return lines.join('\n');
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
 const path = process.argv[2] ?? 'reports/performance-2026-10-02.json';
 const report = JSON.parse(readFileSync(path, 'utf8'));
 const validationPath = path.replace(/\.json$/, '-validation.json');
 let validationNote;
 if (existsSync(validationPath)) {
  const evidence = JSON.parse(readFileSync(validationPath, 'utf8'));
  assert.equal(evidence.sourceCommit, report.sourceCommit);
  for (const [file,hash] of Object.entries(evidence.algorithmSourceHashes)) assert.equal(report.sourceHashes[file],hash);
  assert.ok(evidence.checks.every(c=>c.exitCode===0));
  validationNote = `The regression suites and report-harness checks passed: ${evidence.checks.map(c=>`**${c.passed} ${c.name} tests**`).join(', ')}. [Commands and complete validation logs](${relative(process.cwd(),resolve(validationPath))}) accompany the timing artifact.`;
 }
 const mathPath=path.replace(/\.json$/, '-math.json');
 const mathProfile=JSON.parse(readFileSync(mathPath,'utf8'));
 assert.equal(mathProfile.timingArtifactSha256,createHash('sha256').update(readFileSync(path)).digest('hex'),'Math profile is from a different timing artifact');
 writeFileSync('PERFORMANCE.md',renderPerformance(report,{artifact:relative(process.cwd(),resolve(path)),validationNote,mathProfile,mathArtifact:relative(process.cwd(),resolve(mathPath))}));
 console.log(`Rendered PERFORMANCE.md from ${path}`);
}
