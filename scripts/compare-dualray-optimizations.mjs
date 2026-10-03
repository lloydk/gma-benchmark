// Compare isolated source trees. Candidates are selected by general changes
// to the computation, not fitted to these workloads. No thresholds are tuned.
// Example: node scripts/compare-dualray-optimizations.mjs --tree base=/tmp/base
//   --tree candidate=/tmp/candidate --output /tmp/comparison.json
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, mkdirSync, readdirSync, statSync, mkdtempSync, openSync, closeSync, rmSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { cpus, tmpdir } from 'node:os';
import { parseArgs } from 'node:util';
import { fileURLToPath } from 'node:url';
import { dualrayExperimentWorkloads } from './dualray-experiment-workloads.mjs';

const {values}=parseArgs({options:{
 tree:{type:'string',multiple:true}, output:{type:'string'},
 runtimes:{type:'string',default:'node,bun,rust-f64,rust-f32'},
 runs:{type:'string',default:'3'}, cpu:{type:'string',default:'2,3'},
 seed:{type:'string',default:'0x243f6a88'},
 'baseline-revision':{type:'string'},
}});
assert.ok(values.tree?.length>=2 && values.output);
const trees=values.tree.map(value=>{
 const at=value.indexOf('='); assert.ok(at>0);
 return {name:value.slice(0,at),path:resolve(value.slice(at+1))};
});
assert.equal(new Set(trees.map(t=>t.name)).size,trees.length);
const runtimes=values.runtimes.split(','),runs=Number(values.runs),seed=Number(values.seed);
assert.ok(runtimes.every(r=>['node','bun','rust-f64','rust-f32'].includes(r)));
assert.ok(Number.isSafeInteger(runs)&&runs>=3);
assert.ok(Number.isInteger(seed)&&seed>=0&&seed<=0xffffffff);
const output=resolve(values.output),inputDir=`${output}.inputs`;
mkdirSync(dirname(output),{recursive:true});mkdirSync(inputDir,{recursive:true});
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
// File-backed subprocess output also works with restricted subprocess pipes.
const io=mkdtempSync(resolve(tmpdir(),'dualray-comparison-'));
process.on('exit',()=>rmSync(io,{recursive:true,force:true}));
function command(args,cwd,env) {
 const out=openSync(`${io}/stdout`,'w'),err=openSync(`${io}/stderr`,'w');
 try {execFileSync(args[0],args.slice(1),{cwd,env,stdio:['ignore',out,err]});}
 catch(error){console.error(args.join(' '),readFileSync(`${io}/stderr`,'utf8'));throw error;}
 finally{closeSync(out);closeSync(err);}
 return readFileSync(`${io}/stdout`,'utf8');
}
function sources(tree) {
 const paths=[];
 function walk(dir) {for(const name of readdirSync(resolve(tree,dir)).sort()) {
  const relative=`${dir}/${name}`;
  if(statSync(resolve(tree,relative)).isDirectory())walk(relative);else paths.push(relative);
 }}
 for(const dir of ['src','rust/src','rust/examples','scripts/templates'])walk(dir);
 paths.push('rust/Cargo.toml','rust/Cargo.lock','scripts/bench-rgb-kernels.mjs');
 return Object.fromEntries(paths.sort().map(path=>[path,hash(readFileSync(resolve(tree,path)))]));
}
const origin=fileURLToPath(new URL('../',import.meta.url));
const report={schema:'dualray-optimization-comparison-v1',created:new Date().toISOString(),
 baselineRevision:values['baseline-revision']??null,
 runnerRevision:command(['git','rev-parse','HEAD'],origin).trim(),
 protocol:'Frozen independent distributions; unchanged tolerances, fit coefficients and near-white gate. Separate validation/timing seeds. Serial fresh processes, rotated/reversed order, three-channel checksums. No parameter search.',
 environment:{cpu:cpus()[0].model,affinity:values.cpu,node:command(['node','--version']).trim(),
 bun:command(['bun','--version']).trim(),rust:command(['rustc','-Vv']).trim(),rustflags:'-C target-cpu=native',
 warmup:50,measured:25,runs},
 timingSeed:seed,
 workloadSource:readFileSync(new URL('./dualray-experiment-workloads.mjs',import.meta.url),'utf8'),
 runnerSha256:hash(readFileSync(fileURLToPath(import.meta.url))),
 trees:trees.map(tree=>({...tree,sourceHashes:sources(tree.path)})),workloads:[],validation:[],measurements:[]};
// Embed changed source files so each candidate can be reconstructed from the
// baseline revision even after its temporary checkout has been removed.
for(const tree of report.trees.slice(1))tree.changedSources=Object.fromEntries(
 Object.entries(tree.sourceHashes).filter(([path,sha])=>sha!==report.trees[0].sourceHashes[path])
 .map(([path])=>[path,readFileSync(resolve(tree.path,path),'utf8')]));
for(const gamut of ['srgb','display-p3','rec2020']) {
 for(const [name,inputs] of Object.entries(dualrayExperimentWorkloads(gamut,false,8192,seed))) {
  const bytes=Buffer.alloc(inputs.length*24);
  inputs.forEach((row,i)=>row.forEach((v,j)=>bytes.writeDoubleLE(v,i*24+j*8)));
  const id=`${gamut}-${name}`,path=resolve(inputDir,`${id}.bin`);
  writeFileSync(path,bytes);
  report.workloads.push({id,gamut,name,count:inputs.length,sha256:hash(bytes),path});
 }
}
const save=()=>writeFileSync(output,JSON.stringify(report,null,2)+'\n');
save();
if(runtimes.some(r=>r.startsWith('rust')))for(const tree of report.trees) {
 console.log(`Building ${tree.name}`);
 command(['cargo','build','--release','--manifest-path','rust/Cargo.toml','--example','performance'],tree.path,
  {...process.env,RUSTFLAGS:report.environment.rustflags});
 tree.binarySha256=hash(readFileSync(resolve(tree.path,'rust/target/release/examples/performance')));
}
const cells=[];
for(const workload of report.workloads)for(const runtime of runtimes)for(const method of ['dualray','dualray-fast'])for(const tree of trees)
 cells.push({workload:workload.id,runtime,method,tree:tree.name});
function invoke(cell,validate) {
 const tree=trees.find(t=>t.name===cell.tree),work=report.workloads.find(w=>w.id===cell.workload);
 const args=cell.runtime.startsWith('rust')
 ?['rust/target/release/examples/performance',work.gamut,cell.runtime.slice(5),cell.method.replace('-',' '),'plain',work.path,validate?'validate':'timing']
 :[cell.runtime,'scripts/bench-rgb-kernels.mjs','--gamut',work.gamut,'--method',cell.method,'--input',work.path,'--workload',work.name,...(validate?['--validate-only']:[])];
 const parsed=JSON.parse(command(['taskset','-c',values.cpu,...args],tree.path));
 if(parsed.rows)assert.equal(parsed.binaryInputSha256,work.sha256);
 const row=parsed.rows?parsed.rows[0]:parsed;
 assert.equal(row.count,work.count);assert.ok(Number.isFinite(row.checksum));
 if(!validate)assert.equal(row.passes.length,25);
 const {workload:unused,...rest}=row;return rest;
}
const key=c=>`${c.workload}/${c.runtime}/${c.method}/${c.tree}`;
for(const cell of cells)report.validation.push({...cell,...invoke(cell,true)});
save();console.log(`Validated ${cells.length} cells`);
for(let run=0;run<runs;run++) {
 const offset=Math.floor(cells.length*run/runs),order=[...cells.slice(offset),...cells.slice(0,offset)];
 if(run%2)order.reverse();
 for(const cell of order) {
  const row=invoke(cell,false),check=report.validation.find(v=>key(v)===key(cell));
  assert.ok(Math.abs(row.checksum-check.checksum*75)<=1e-10*Math.max(1,Math.abs(check.checksum*75)),key(cell));
  report.measurements.push({...cell,run,...row});
  if(report.measurements.length%48===0){save();console.log(`Timing ${report.measurements.length}/${runs*cells.length}`);}
 }
 save();
}
report.summary=cells.map(cell=>{
 const times=report.measurements.filter(row=>key(row)===key(cell)).map(row=>row.ns).sort((a,b)=>a-b);
 return {...cell,ns:times[Math.floor(times.length/2)],min:times[0],max:times.at(-1),processes:times};
});
for(const tree of report.trees)assert.deepEqual(sources(tree.path),tree.sourceHashes,`Sources changed: ${tree.name}`);
report.completed=new Date().toISOString();save();console.log(`Completed ${output}`);
