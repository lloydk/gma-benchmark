// Reproducible, serial, CPU-pinned measurements for PERFORMANCE.md.
// Validation and timing use separate processes. Resume only with identical sources.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, mkdirSync, existsSync, readdirSync, statSync, mkdtempSync, openSync, closeSync, rmSync } from 'node:fs';
import { resolve, dirname, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { cpus, platform, release, tmpdir } from 'node:os';
import { buildPerformanceWorkloads, methods, runtimeMethods } from './performance-workloads.mjs';
import { median, cpuList, DEFAULT_REPORT } from './performance-stats.mjs';
const root = fileURLToPath(new URL('../', import.meta.url));
process.chdir(root);
const { values } = parseArgs({ options: {
 output: { type: 'string', default: DEFAULT_REPORT },
 cpu: { type: 'string', default: '2,3' }, runs: { type: 'string', default: '3' },
 resume: { type: 'boolean', default: false }, 'prepare-only': { type: 'boolean', default: false },
 'validate-only': { type: 'boolean', default: false },
} });
const runs = Number(values.runs);
assert.ok(Number.isSafeInteger(runs) && runs >= 3);
const affinity = cpuList(values.cpu);
const output = resolve(values.output), inputDir = `${output}.inputs`;
mkdirSync(dirname(output), { recursive: true }); mkdirSync(inputDir, { recursive: true });
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const flags = '-C target-cpu=native';
// File-backed subprocess output also works in sandboxes where large pipes stall.
const io = mkdtempSync(resolve(tmpdir(), 'gma-performance-io-'));
process.on('exit', () => rmSync(io, { recursive: true, force: true }));
function command(args, options={}) {
 const out = openSync(`${io}/stdout`, 'w'), err = openSync(`${io}/stderr`, 'w');
 try {
  execFileSync(args[0], args.slice(1), { cwd: root, timeout: 180000, ...options, stdio: ['ignore', out, err] });
 } catch(error) {
  console.error(args.join(' '), readFileSync(`${io}/stderr`, 'utf8')); throw error;
 } finally { closeSync(out); closeSync(err); }
 return readFileSync(`${io}/stdout`, 'utf8');
}
const files = [];
function visit(dir) { for (const name of readdirSync(dir).sort()) {
 const path=resolve(dir,name); if (statSync(path).isDirectory()) visit(path); else files.push(path);
} }
visit('src'); visit('rust/src'); visit('rust/examples');
for (const file of ['benchmark-workloads.js','scripts/bench-rgb-kernels.mjs','scripts/performance-workloads.mjs','scripts/run-performance.mjs','scripts/performance-stats.mjs',
 'tests/helpers/matrix-reference.js','tests/helpers/css-minde-reference.js','tests/helpers/canonical-reference.js','rust/Cargo.toml','rust/Cargo.lock']) if(existsSync(file))files.push(resolve(file));
const sourceHashes = Object.fromEntries(files.sort().map(path=>[relative(root,path),hash(readFileSync(path))]));
const { workloads, p3RandomBelowCusp } = buildPerformanceWorkloads();
for (const work of workloads) writeFileSync(resolve(inputDir, `${work.id}.bin`), work.bytes);
const manifest = workloads.map(({bytes,...rest})=>rest);
let report;
if (values.resume && existsSync(output)) {
 report=JSON.parse(readFileSync(output,'utf8'));
 assert.deepEqual(report.sourceHashes,sourceHashes,'Sources changed since this run');
 assert.deepEqual(report.workloads,manifest,'Workloads changed since this run');
 assert.equal(report.runs,runs);assert.equal(report.environment.cpuAffinity,affinity);
 assert.deepEqual(report.runtimeMethods,runtimeMethods);
 assert.equal(report.environment.node,command(['node','--version']).trim());
 assert.equal(report.environment.bun,command(['bun','--version']).trim());
 assert.equal(report.environment.rust,command(['rustc','-Vv']).trim());
 assert.equal(report.environment.cpu,cpus()[0].model);
} else {
 assert.ok(!existsSync(output),'Output exists: use --resume or choose a new output');
 report={schema:'gma-performance-v1',created:new Date().toISOString(),sourceCommit:command(['git','rev-parse','HEAD']).trim(),
 sourceStatus:command(['git','status','--short']),sourceHashes,runs,methods,runtimeMethods,workloads:manifest,p3RandomBelowCusp,
 environment:{cpu:cpus()[0].model,cpuAffinity:affinity,platform:platform(),kernel:release(),
 node:command(['node','--version']).trim(),bun:command(['bun','--version']).trim(),rust:command(['rustc','-Vv']).trim(),
 glibc:command(['ldd','--version']).split('\n')[0],rustflags:flags,releaseProfile:'opt-level=3, lto=true, codegen-units=1, panic=abort',
 lscpu:command(['lscpu']),warmup:50,measured:25},validation:[],measurements:[]};
}
const save=()=>writeFileSync(output,JSON.stringify(report,null,2)+'\n');
save();
console.log(`Prepared ${workloads.length} workloads; P3 random below cusp ${(100*p3RandomBelowCusp).toFixed(2)}%.`);
if(values['prepare-only']) process.exit();
command(['cargo','build','--release','--manifest-path','rust/Cargo.toml','--example','performance'],{env:{...process.env,RUSTFLAGS:flags},stdio:['ignore','pipe','inherit']});
const binary='rust/target/release/examples/performance', binarySha256=hash(readFileSync(binary));
if(report.binarySha256)assert.equal(binarySha256,report.binarySha256,'Binary changed since this run');
report.binarySha256=binarySha256; save();
const cells=[];
for(const work of workloads)for(const runtime of Object.keys(runtimeMethods))for(const [method,label] of methods) {
 if(!runtimeMethods[runtime].includes(method))continue;
 cells.push({workload:work.id,runtime,method,label});
}
const key=c=>`${c.workload}/${c.runtime}/${c.method}`;
function invoke(cell,validate) {
 const work=workloads.find(w=>w.id===cell.workload), path=resolve(inputDir,`${work.id}.bin`);
 const args=cell.runtime.startsWith('rust')
 ?[binary,work.gamut,cell.runtime.slice(5),cell.label,work.checked?'checked':'plain',path,validate?'validate':'timing']
 :[cell.runtime,'scripts/bench-rgb-kernels.mjs','--gamut',work.gamut,'--method',cell.method,'--input',path,'--workload',work.name,
 ...(work.checked?['--in-gamut-check']:[]),...(validate?['--validate-only']:[])];
 const parsed=JSON.parse(command(['taskset','-c',affinity,...args]));
 let row;
 if(parsed.rows){assert.equal(parsed.binaryInputSha256,work.sha256); assert.equal(parsed.rows.length,1);row=parsed.rows[0];assert.equal(row.workload,work.name);delete row.workload;}
 else row=parsed;
 assert.equal(row.count,work.count);
 assert.ok(Number.isFinite(row.checksum));
 if(!validate){assert.equal(row.passes.length,25);assert.ok(row.passes.every(x=>Number.isFinite(x)&&x>0));}
 return row;
}
console.log(`Validating ${cells.length} runtime/method/workload cells in separate processes.`);
const validated=new Set(report.validation.map(key));
for(const cell of cells) {
 if(validated.has(key(cell)))continue;
 report.validation.push({...cell,...invoke(cell,true)});
 if(report.validation.length%20===0){save();console.log(`Validation ${report.validation.length}/${cells.length}`);}
}
save();
if(values['validate-only'])process.exit();
// Rotate and reverse the complete cell order between fresh-process rounds.
// Every timing process owns one target, precision, method and workload.
const done=new Set(report.measurements.map(c=>`${c.run}/${key(c)}`));
for(let run=0;run<runs;run++) {
 const offset=Math.floor(cells.length*run/runs), order=[...cells.slice(offset),...cells.slice(0,offset)];
 if(run%2)order.reverse();
 for(const cell of order) {
  if(done.has(`${run}/${key(cell)}`))continue;
  const row=invoke(cell,false),validation=report.validation.find(v=>key(v)===key(cell));
  const expected=validation.checksum*75;
  assert.ok(Math.abs(row.checksum-expected)<=1e-10*Math.max(1,Math.abs(expected)),`Timing checksum differs: ${key(cell)}`);
  report.measurements.push({...cell,run,...row});
  if(report.measurements.length%20===0){save();console.log(`Timing ${report.measurements.length}/${runs*cells.length} (round ${run+1}): ${key(cell)}`);}
 }
 save();
}
report.completed=new Date().toISOString();
report.summary=cells.map(cell=>{
 const processes=report.measurements.filter(m=>key(m)===key(cell)).map(m=>m.ns).sort((a,b)=>a-b);
 assert.equal(processes.length,runs);
 return {...cell,ns:median(processes),min:processes[0],max:processes.at(-1),processes};
});
assert.deepEqual(Object.fromEntries(Object.keys(sourceHashes).map(path=>[path,hash(readFileSync(path))])),sourceHashes,'Sources changed during measurement');
save();console.log(`Completed ${output}`);
